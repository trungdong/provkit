//! The event interface every reader drives.

use std::collections::HashSet;

use crate::{Bundle, Document, Error, Namespaces, QName, Record, Result};

/// Receives a document as a sequence of events.
///
/// Readers drive a `Sink`. [`DocumentBuilder`] turns the events into a
/// [`Document`], and a writer can implement `Sink` to stream output without
/// holding the document.
///
/// [`DocumentBuilder`] returns [`Error::Structure`] for an event out of
/// order and [`Error::DuplicateBundle`] for a repeated bundle identifier.
pub trait Sink {
    /// The document begins, with its top-level namespace table.
    ///
    /// # Errors
    ///
    /// Returns an error if the sink rejects the event.
    fn start_document(&mut self, namespaces: Namespaces) -> Result<()>;

    /// A record, at the top level or inside the open bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if the sink rejects the event.
    fn record(&mut self, record: Record) -> Result<()>;

    /// A bundle begins.
    ///
    /// # Errors
    ///
    /// Returns an error if the sink rejects the event.
    fn start_bundle(&mut self, id: QName, namespaces: Namespaces) -> Result<()>;

    /// The open bundle ends.
    ///
    /// # Errors
    ///
    /// Returns an error if the sink rejects the event.
    fn end_bundle(&mut self) -> Result<()>;

    /// The document ends.
    ///
    /// # Errors
    ///
    /// Returns an error if the sink rejects the event.
    fn end_document(&mut self) -> Result<()>;
}

/// Where the builder is in the event sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    InDocument,
    InBundle,
    Done,
}

/// Builds a [`Document`] from [`Sink`] events. The only way a reader makes a
/// document.
#[derive(Debug)]
pub struct DocumentBuilder {
    document: Document,
    open: Option<Bundle>,
    /// Identifiers of the bundles started so far.
    bundle_ids: HashSet<QName>,
    state: State,
}

impl Default for DocumentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentBuilder {
    /// A builder waiting for `start_document`.
    #[must_use]
    pub fn new() -> Self {
        DocumentBuilder {
            document: Document::new(),
            open: None,
            bundle_ids: HashSet::new(),
            state: State::Idle,
        }
    }

    /// The finished document.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Structure`] unless `end_document` has been received.
    pub fn finish(self) -> Result<Document> {
        if self.state != State::Done {
            return Err(bad_state("document not finished"));
        }
        Ok(self.document)
    }

    fn expect(&self, state: State, what: &str) -> Result<()> {
        if self.state == state {
            Ok(())
        } else {
            Err(bad_state(what))
        }
    }
}

fn bad_state(what: &str) -> Error {
    Error::Structure {
        format: "document",
        message: what.to_owned(),
    }
}

impl Sink for DocumentBuilder {
    fn start_document(&mut self, namespaces: Namespaces) -> Result<()> {
        self.expect(State::Idle, "document already started")?;
        self.document.namespaces = namespaces;
        self.state = State::InDocument;
        Ok(())
    }

    fn record(&mut self, record: Record) -> Result<()> {
        match (self.state, self.open.as_mut()) {
            (State::InDocument, _) => self.document.records.push(record),
            (State::InBundle, Some(bundle)) => bundle.records.push(record),
            _ => return Err(bad_state("record outside a document")),
        }
        Ok(())
    }

    fn start_bundle(&mut self, id: QName, namespaces: Namespaces) -> Result<()> {
        self.expect(
            State::InDocument,
            "bundle outside the document body (bundles do not nest)",
        )?;
        if !self.bundle_ids.insert(id.clone()) {
            return Err(Error::DuplicateBundle(id));
        }
        self.open = Some(Bundle {
            id,
            namespaces,
            records: Vec::new(),
        });
        self.state = State::InBundle;
        Ok(())
    }

    fn end_bundle(&mut self) -> Result<()> {
        self.expect(State::InBundle, "endBundle without an open bundle")?;
        if let Some(bundle) = self.open.take() {
            self.document.bundles.push(bundle);
        }
        self.state = State::InDocument;
        Ok(())
    }

    fn end_document(&mut self) -> Result<()> {
        self.expect(
            State::InDocument,
            "endDocument with an open bundle or no document",
        )?;
        self.state = State::Done;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{entity, q};

    fn is_structure<T>(r: &Result<T>) -> bool {
        matches!(r, Err(Error::Structure { .. }))
    }

    #[test]
    fn builds_a_document_with_a_bundle() {
        let mut ns = Namespaces::new();
        ns.declare("ex", "http://example.org/").unwrap();
        let mut b = DocumentBuilder::new();
        b.start_document(ns.clone()).unwrap();
        b.record(entity("e1")).unwrap();
        b.start_bundle(q("b"), Namespaces::new()).unwrap();
        b.record(entity("e2")).unwrap();
        b.end_bundle().unwrap();
        b.record(entity("e3")).unwrap();
        b.end_document().unwrap();
        let d = b.finish().unwrap();
        assert_eq!(d.namespaces, ns);
        assert_eq!(d.records, [entity("e1"), entity("e3")]);
        assert_eq!(d.bundles.len(), 1);
        assert_eq!(d.bundles[0].id, q("b"));
        assert_eq!(d.bundles[0].records, [entity("e2")]);
    }

    #[test]
    fn state_errors() {
        let mut b = DocumentBuilder::new();
        assert!(is_structure(&b.record(entity("e1"))));
        b.start_document(Namespaces::new()).unwrap();
        assert!(is_structure(&b.start_document(Namespaces::new())));
        assert!(is_structure(&b.end_bundle()));
        b.start_bundle(q("b"), Namespaces::new()).unwrap();
        assert!(is_structure(&b.start_bundle(q("c"), Namespaces::new())));
        assert!(is_structure(&b.end_document()));
        assert!(is_structure(&b.finish()));
    }

    #[test]
    fn finish_before_end_document_fails() {
        let mut b = DocumentBuilder::new();
        b.start_document(Namespaces::new()).unwrap();
        assert!(is_structure(&b.finish()));
    }

    #[test]
    fn events_after_end_document_fail() {
        let mut b = DocumentBuilder::new();
        b.start_document(Namespaces::new()).unwrap();
        b.end_document().unwrap();
        assert!(is_structure(&b.record(entity("e1"))));
        assert!(is_structure(&b.start_bundle(q("b"), Namespaces::new())));
    }

    #[test]
    fn duplicate_bundle() {
        let mut b = DocumentBuilder::new();
        b.start_document(Namespaces::new()).unwrap();
        b.start_bundle(q("b"), Namespaces::new()).unwrap();
        b.end_bundle().unwrap();
        assert!(matches!(
            b.start_bundle(q("b"), Namespaces::new()),
            Err(Error::DuplicateBundle(id)) if id == q("b")
        ));
    }

    #[test]
    fn many_bundles_check_duplicates_by_lookup() {
        let mut b = DocumentBuilder::new();
        b.start_document(Namespaces::new()).unwrap();
        for i in 0..20_000 {
            b.start_bundle(q(&format!("b{i}")), Namespaces::new())
                .unwrap();
            b.end_bundle().unwrap();
        }
        assert!(matches!(
            b.start_bundle(q("b7"), Namespaces::new()),
            Err(Error::DuplicateBundle(_))
        ));
        b.end_document().unwrap();
        let d = b.finish().unwrap();
        assert_eq!(d.bundles.len(), 20_000);
        assert_eq!(d.bundles[0].id, q("b0"));
        assert_eq!(d.bundles[19_999].id, q("b19999"));
    }
}
