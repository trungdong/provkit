//! The event interface both readers drive.

use std::collections::HashSet;

use crate::{Bundle, Document, Error, Namespaces, QName, Record, Result};

/// Receives a document as a sequence of events: `start_document`, then
/// records and bundles, then `end_document`. Bundles do not nest.
///
/// Every method returns an error if the sink rejects the event.
pub(crate) trait Sink {
    /// The document begins, with its top-level namespace table.
    fn start_document(&mut self, namespaces: Namespaces) -> Result<()>;

    /// A record, at the top level or inside the open bundle.
    fn record(&mut self, record: Record) -> Result<()>;

    /// A bundle begins.
    fn start_bundle(&mut self, id: QName, namespaces: Namespaces) -> Result<()>;

    /// The open bundle ends.
    fn end_bundle(&mut self) -> Result<()>;

    /// The document ends.
    fn end_document(&mut self) -> Result<()>;
}

/// Where the builder is in the event sequence. `InBundle` holds the open
/// bundle.
#[derive(Debug)]
enum State {
    Start,
    InDocument,
    InBundle(Bundle),
    Done,
}

/// Builds a [`Document`] from [`Sink`] events. The only way a reader makes a
/// document.
///
/// The readers send events in order, so the builder checks the order in
/// debug builds only. A repeated bundle identifier is
/// [`Error::DuplicateBundle`].
#[derive(Debug)]
pub(crate) struct DocumentBuilder {
    document: Document,
    /// Identifiers of the bundles started so far.
    bundle_ids: HashSet<QName>,
    state: State,
}

impl DocumentBuilder {
    /// A builder waiting for `start_document`.
    pub(crate) fn new() -> Self {
        DocumentBuilder {
            document: Document::new(),
            bundle_ids: HashSet::new(),
            state: State::Start,
        }
    }

    /// The finished document, once `end_document` has been received.
    pub(crate) fn finish(self) -> Document {
        debug_assert!(matches!(self.state, State::Done), "document not finished");
        self.document
    }
}

impl Sink for DocumentBuilder {
    fn start_document(&mut self, namespaces: Namespaces) -> Result<()> {
        debug_assert!(
            matches!(self.state, State::Start),
            "document already started"
        );
        self.document.namespaces = namespaces;
        self.state = State::InDocument;
        Ok(())
    }

    fn record(&mut self, record: Record) -> Result<()> {
        match &mut self.state {
            State::InBundle(bundle) => bundle.records.push(record),
            State::InDocument => self.document.records.push(record),
            State::Start | State::Done => debug_assert!(false, "record outside a document"),
        }
        Ok(())
    }

    fn start_bundle(&mut self, id: QName, namespaces: Namespaces) -> Result<()> {
        debug_assert!(
            matches!(self.state, State::InDocument),
            "bundle outside the document body"
        );
        if !self.bundle_ids.insert(id.clone()) {
            return Err(Error::DuplicateBundle(id));
        }
        self.state = State::InBundle(Bundle {
            id,
            namespaces,
            records: Vec::new(),
        });
        Ok(())
    }

    fn end_bundle(&mut self) -> Result<()> {
        match std::mem::replace(&mut self.state, State::InDocument) {
            State::InBundle(bundle) => self.document.bundles.push(bundle),
            other => {
                debug_assert!(false, "endBundle without an open bundle");
                self.state = other;
            }
        }
        Ok(())
    }

    fn end_document(&mut self) -> Result<()> {
        debug_assert!(
            matches!(self.state, State::InDocument),
            "endDocument with an open bundle or no document"
        );
        self.state = State::Done;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{entity, ex};

    #[test]
    fn builds_a_document_with_a_bundle() {
        let mut ns = Namespaces::new();
        ns.declare("ex", "http://example.org/").unwrap();
        let mut b = DocumentBuilder::new();
        b.start_document(ns.clone()).unwrap();
        b.record(entity("e1")).unwrap();
        b.start_bundle(ex("b"), Namespaces::new()).unwrap();
        b.record(entity("e2")).unwrap();
        b.end_bundle().unwrap();
        b.record(entity("e3")).unwrap();
        b.end_document().unwrap();
        let d = b.finish();
        assert_eq!(d.namespaces, ns);
        assert_eq!(d.records, [entity("e1"), entity("e3")]);
        assert_eq!(d.bundles.len(), 1);
        assert_eq!(d.bundles[0].id, ex("b"));
        assert_eq!(d.bundles[0].records, [entity("e2")]);
    }

    #[test]
    fn duplicate_bundle() {
        let mut b = DocumentBuilder::new();
        b.start_document(Namespaces::new()).unwrap();
        b.start_bundle(ex("b"), Namespaces::new()).unwrap();
        b.end_bundle().unwrap();
        assert!(matches!(
            b.start_bundle(ex("b"), Namespaces::new()),
            Err(Error::DuplicateBundle(id)) if id == ex("b")
        ));
    }

    #[test]
    fn many_bundles_check_duplicates_by_lookup() {
        let mut b = DocumentBuilder::new();
        b.start_document(Namespaces::new()).unwrap();
        for i in 0..20_000 {
            b.start_bundle(ex(&format!("b{i}")), Namespaces::new())
                .unwrap();
            b.end_bundle().unwrap();
        }
        assert!(matches!(
            b.start_bundle(ex("b7"), Namespaces::new()),
            Err(Error::DuplicateBundle(_))
        ));
        b.end_document().unwrap();
        let d = b.finish();
        assert_eq!(d.bundles.len(), 20_000);
        assert_eq!(d.bundles[0].id, ex("b0"));
        assert_eq!(d.bundles[19_999].id, ex("b19999"));
    }
}
