//! Prefix tables and name resolution.

use std::sync::Arc;

use indexmap::IndexMap;

use crate::{Error, QName, Result};

/// The PROV namespace IRI.
pub const PROV: &str = "http://www.w3.org/ns/prov#";
/// The XML Schema datatypes namespace IRI.
pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// A default namespace plus an ordered prefix-to-IRI table.
///
/// `prov` and `xsd` are always present and cannot be moved. The table is a
/// presentation detail: it decides how names are written and never takes part
/// in equality.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Namespaces {
    default: Option<Arc<str>>,
    prefixes: IndexMap<Box<str>, Arc<str>>,
}

impl Namespaces {
    /// An empty table (with the predeclared `prov` and `xsd`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default namespace, used for bare names.
    pub fn set_default(&mut self, iri: impl Into<Arc<str>>) {
        self.default = Some(iri.into());
    }

    /// The default namespace, if any.
    pub fn default_namespace(&self) -> Option<&Arc<str>> {
        self.default.as_ref()
    }

    /// Bind `prefix` to `iri`, replacing an earlier binding of the same prefix.
    pub fn declare(&mut self, prefix: &str, iri: &str) -> Result<()> {
        // `_` introduces blank nodes and is never a prefix.
        if prefix == "_" {
            return Err(Error::InvalidQName(prefix.into()));
        }
        if let Some(expected) = reserved(prefix) {
            if expected == iri {
                return Ok(());
            }
            return Err(Error::ReservedPrefix {
                prefix: prefix.into(),
                expected: expected.into(),
                found: iri.into(),
            });
        }
        self.prefixes.insert(prefix.into(), Arc::from(iri));
        Ok(())
    }

    /// Add the bindings of `other` that this table lacks. A prefix or default
    /// namespace already bound here keeps its binding, so merging cannot fail.
    pub(crate) fn absorb(&mut self, other: &Namespaces) {
        if self.default.is_none() {
            self.default.clone_from(&other.default);
        }
        for (prefix, iri) in &other.prefixes {
            self.prefixes
                .entry(prefix.clone())
                .or_insert_with(|| iri.clone());
        }
    }

    /// Declared prefixes in declaration order, without `prov` and `xsd`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<str>)> {
        self.prefixes.iter().map(|(p, iri)| (&**p, iri))
    }

    /// The IRI bound to `prefix`, including the predeclared ones.
    pub fn lookup(&self, prefix: &str) -> Option<Arc<str>> {
        match reserved(prefix) {
            Some(iri) => Some(Arc::from(iri)),
            None => self.prefixes.get(prefix).cloned(),
        }
    }

    /// Resolve `prefix:local` or a bare `local` to a name.
    pub fn resolve(&self, text: &str) -> Result<QName> {
        resolve_with(text, |prefix| self.lookup(prefix), self.default.clone())
    }

    /// The prefix bound to `namespace`, if any (`prov` and `xsd` included).
    pub fn prefix_for(&self, namespace: &str) -> Option<&str> {
        if namespace == PROV {
            return Some("prov");
        }
        if namespace == XSD {
            return Some("xsd");
        }
        self.prefixes
            .iter()
            .find(|(_, iri)| &***iri == namespace)
            .map(|(p, _)| &**p)
    }

    /// The prefix for `namespace`, minting `ns1`, `ns2`, ... when none exists.
    pub fn ensure_prefix(&mut self, namespace: &str) -> String {
        if let Some(p) = self.prefix_for(namespace) {
            return p.to_owned();
        }
        let mut n = 1;
        loop {
            let candidate = format!("ns{n}");
            if !self.prefixes.contains_key(candidate.as_str()) {
                self.prefixes
                    .insert(candidate.clone().into(), Arc::from(namespace));
                return candidate;
            }
            n += 1;
        }
    }
}

impl Namespaces {
    /// This table with `parent`'s bindings behind it.
    ///
    /// The result holds this table's prefixes and default namespace, then
    /// every binding of `parent` that this table has not already bound. A
    /// bundle's table over its document's is the typical case.
    ///
    /// ```
    /// use provkit::Namespaces;
    ///
    /// let mut document = Namespaces::new();
    /// document.declare("ex", "http://document.org/").unwrap();
    /// document.declare("o", "http://other.org/").unwrap();
    /// let mut bundle = Namespaces::new();
    /// bundle.declare("ex", "http://bundle.org/").unwrap();
    ///
    /// let scope = bundle.with_fallback(&document);
    /// assert_eq!(&*scope.lookup("ex").unwrap(), "http://bundle.org/");
    /// assert_eq!(&*scope.lookup("o").unwrap(), "http://other.org/");
    /// ```
    #[must_use]
    pub fn with_fallback(&self, parent: &Namespaces) -> Namespaces {
        let mut table = self.clone();
        table.absorb(parent);
        table
    }

    /// The text that writes `name` against this table.
    ///
    /// A declared prefix wins, then the default namespace for a bare name,
    /// then a freshly minted `nsN` prefix, which is added to this table.
    /// Callers that must not change a document pass a clone.
    // Only the writers call this, and both are feature-gated.
    #[cfg_attr(not(any(feature = "json", feature = "provn")), allow(dead_code))]
    pub(crate) fn render(&mut self, name: &QName) -> String {
        match self.render_parts(name) {
            Some(prefix) => format!("{prefix}:{}", name.local()),
            None => name.local().to_owned(),
        }
    }

    /// As [`render`](Self::render), split into the prefix and the local part.
    /// `None` means the name is written bare. The local part is always
    /// `name.local()`, so a writer can escape it on its own.
    // Only the writers call this, and both are feature-gated.
    #[cfg_attr(not(any(feature = "json", feature = "provn")), allow(dead_code))]
    pub(crate) fn render_parts(&mut self, name: &QName) -> Option<String> {
        if let Some(prefix) = self.prefix_for(name.namespace()) {
            return Some(prefix.to_owned());
        }
        // A colon in the local part would read back as a prefix separator,
        // and an empty local part would vanish.
        let bare = !name.local().is_empty()
            && !name.local().contains(':')
            && self
                .default
                .as_deref()
                .is_some_and(|d| d == &**name.namespace());
        if bare {
            return None;
        }
        Some(self.ensure_prefix(name.namespace()))
    }
}

fn reserved(prefix: &str) -> Option<&'static str> {
    match prefix {
        "prov" => Some(PROV),
        "xsd" => Some(XSD),
        _ => None,
    }
}

/// Shared resolution: split at the first colon, look the prefix up, or fall
/// back to the default namespace for a bare name.
fn resolve_with(
    text: &str,
    lookup: impl Fn(&str) -> Option<Arc<str>>,
    default: Option<Arc<str>>,
) -> Result<QName> {
    if let Some((prefix, local)) = text.split_once(':') {
        // `_:` marks a blank node; readers handle it before resolving.
        if prefix.is_empty() || prefix == "_" {
            return Err(Error::InvalidQName(text.into()));
        }
        let ns = lookup(prefix).ok_or_else(|| Error::UndeclaredPrefix(prefix.into()))?;
        Ok(QName::new(ns, local))
    } else {
        if text.is_empty() {
            return Err(Error::InvalidQName(text.into()));
        }
        let ns = default.ok_or_else(|| Error::UndeclaredPrefix(String::new()))?;
        Ok(QName::new(ns, text))
    }
}

/// Resolution inside a bundle: the bundle's table first, the document's second.
pub struct Scope<'a> {
    document: &'a Namespaces,
    bundle: Option<&'a Namespaces>,
}

impl<'a> Scope<'a> {
    /// A scope over `document`, and over `bundle` when inside one.
    pub fn new(document: &'a Namespaces, bundle: Option<&'a Namespaces>) -> Self {
        Scope { document, bundle }
    }

    /// Resolve as [`Namespaces::resolve`], with the fallback.
    pub fn resolve(&self, text: &str) -> Result<QName> {
        resolve_with(text, |prefix| self.lookup(prefix), self.default_namespace())
    }

    /// Resolve a name already split into `prefix` and `local`; an empty
    /// prefix means a bare name. A colon in `local` stays in the local part.
    // Only the PROV-N reader calls this, and it is feature-gated.
    #[cfg_attr(not(feature = "provn"), allow(dead_code))]
    pub(crate) fn resolve_parts(&self, prefix: &str, local: &str) -> Result<QName> {
        if prefix.is_empty() {
            if local.is_empty() {
                return Err(Error::InvalidQName(String::new()));
            }
            let ns = self
                .default_namespace()
                .ok_or_else(|| Error::UndeclaredPrefix(String::new()))?;
            return Ok(QName::new(ns, local));
        }
        if prefix == "_" {
            return Err(Error::InvalidQName(format!("{prefix}:{local}")));
        }
        let ns = self
            .lookup(prefix)
            .ok_or_else(|| Error::UndeclaredPrefix(prefix.into()))?;
        Ok(QName::new(ns, local))
    }

    fn lookup(&self, prefix: &str) -> Option<Arc<str>> {
        self.bundle
            .and_then(|b| b.lookup(prefix))
            .or_else(|| self.document.lookup(prefix))
    }

    fn default_namespace(&self) -> Option<Arc<str>> {
        self.bundle
            .and_then(|b| b.default_namespace().cloned())
            .or_else(|| self.document.default_namespace().cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_prefixed_bare_and_predeclared() {
        let mut ns = Namespaces::new();
        ns.declare("ex", "http://example.org/").unwrap();
        ns.set_default("http://default.org/");
        assert_eq!(
            ns.resolve("ex:e1").unwrap(),
            QName::new("http://example.org/", "e1")
        );
        assert_eq!(
            ns.resolve("e1").unwrap(),
            QName::new("http://default.org/", "e1")
        );
        assert_eq!(ns.resolve("prov:type").unwrap(), QName::new(PROV, "type"));
        assert_eq!(ns.resolve("xsd:int").unwrap(), QName::new(XSD, "int"));
    }

    #[test]
    fn undeclared_prefix_and_bare_without_default_fail() {
        let ns = Namespaces::new();
        assert!(matches!(ns.resolve("ex:e1"), Err(Error::UndeclaredPrefix(p)) if p == "ex"));
        assert!(matches!(ns.resolve("e1"), Err(Error::UndeclaredPrefix(p)) if p.is_empty()));
    }

    #[test]
    fn malformed_names_are_invalid() {
        let mut ns = Namespaces::new();
        ns.set_default("http://default.org/");
        assert!(matches!(ns.resolve(":e1"), Err(Error::InvalidQName(_))));
        assert!(matches!(ns.resolve(""), Err(Error::InvalidQName(_))));
    }

    #[test]
    fn underscore_is_never_a_prefix() {
        let mut ns = Namespaces::new();
        assert!(matches!(
            ns.declare("_", "http://example.org/"),
            Err(Error::InvalidQName(_))
        ));
        assert_eq!(ns.iter().count(), 0);
        // With no binding, `_:x` is still rejected rather than reported
        // as an undeclared prefix.
        assert!(matches!(ns.resolve("_:x"), Err(Error::InvalidQName(_))));
        let doc = Namespaces::new();
        let mut bundle = Namespaces::new();
        bundle.set_default("http://default.org/");
        let scope = Scope::new(&doc, Some(&bundle));
        assert!(matches!(scope.resolve("_:x"), Err(Error::InvalidQName(_))));
    }

    #[test]
    fn reserved_prefixes_cannot_move() {
        let mut ns = Namespaces::new();
        assert!(ns.declare("prov", PROV).is_ok());
        assert!(matches!(
            ns.declare("prov", "http://other/"),
            Err(Error::ReservedPrefix { .. })
        ));
        assert!(matches!(
            ns.declare("xsd", "http://other/"),
            Err(Error::ReservedPrefix { .. })
        ));
        assert_eq!(ns.iter().count(), 0);
    }

    #[test]
    fn ensure_prefix_reuses_then_mints() {
        let mut ns = Namespaces::new();
        ns.declare("ex", "http://example.org/").unwrap();
        ns.declare("ns1", "http://taken/").unwrap();
        assert_eq!(ns.ensure_prefix("http://example.org/"), "ex");
        assert_eq!(ns.ensure_prefix("http://new/"), "ns2");
        assert_eq!(ns.ensure_prefix("http://new/"), "ns2");
        assert_eq!(ns.prefix_for(PROV), Some("prov"));
    }

    #[test]
    fn render_prefers_prefix_then_default_then_mints() {
        let mut ns = Namespaces::new();
        ns.declare("ex", "http://example.org/").unwrap();
        ns.set_default("http://default.org/");
        assert_eq!(ns.render(&QName::new("http://example.org/", "a")), "ex:a");
        assert_eq!(ns.render(&QName::new("http://default.org/", "b")), "b");
        assert_eq!(ns.render(&QName::new(PROV, "type")), "prov:type");
        assert_eq!(ns.render(&QName::new("http://new/", "c")), "ns1:c");
        assert_eq!(ns.iter().count(), 2);
        // A name in the default namespace never mints a prefix.
        let mut plain = Namespaces::new();
        plain.set_default("http://default.org/");
        assert_eq!(plain.render(&QName::new("http://default.org/", "b")), "b");
        assert_eq!(plain.iter().count(), 0);
        // A colon in the local part cannot be written bare.
        assert_eq!(
            plain.render(&QName::new("http://default.org/", "x:y")),
            "ns1:x:y"
        );
        assert_eq!(plain.render(&QName::new("http://default.org/", "")), "ns1:");
    }

    #[test]
    fn render_parts_split_the_prefix_from_the_local_part() {
        let mut ns = Namespaces::new();
        ns.declare("ex", "http://example.org/").unwrap();
        ns.set_default("http://default.org/");
        let names = [
            QName::new("http://example.org/", "a"),
            QName::new("http://default.org/", "b"),
            QName::new("http://default.org/", "x:y"),
            QName::new("http://new/", "c"),
        ];
        for name in &names {
            let rendered = ns.render(name);
            let text = match ns.render_parts(name) {
                Some(prefix) => format!("{prefix}:{}", name.local()),
                None => name.local().to_owned(),
            };
            assert_eq!(rendered, text);
        }
        let mut fresh = Namespaces::new();
        fresh.set_default("http://default.org/");
        assert_eq!(fresh.render_parts(&names[1]), None);
    }

    #[test]
    fn scope_falls_back_to_document() {
        let mut doc = Namespaces::new();
        doc.declare("ex", "http://example.org/").unwrap();
        let mut bundle = Namespaces::new();
        bundle.declare("b", "http://bundle.org/").unwrap();
        let scope = Scope::new(&doc, Some(&bundle));
        assert_eq!(
            scope.resolve("ex:e").unwrap(),
            QName::new("http://example.org/", "e")
        );
        assert_eq!(
            scope.resolve("b:e").unwrap(),
            QName::new("http://bundle.org/", "e")
        );
    }

    #[test]
    fn resolve_parts_keeps_a_colon_in_the_local_part() {
        let mut doc = Namespaces::new();
        doc.set_default("http://default.org/");
        doc.declare("a", "http://a.org/").unwrap();
        let mut bundle = Namespaces::new();
        bundle.declare("a", "http://bundle.org/").unwrap();
        let scope = Scope::new(&doc, Some(&bundle));
        assert_eq!(
            scope.resolve_parts("", "a:b").unwrap(),
            QName::new("http://default.org/", "a:b")
        );
        assert_eq!(
            scope.resolve_parts("a", "b").unwrap(),
            QName::new("http://bundle.org/", "b")
        );
        assert!(
            matches!(scope.resolve_parts("zz", "b"), Err(Error::UndeclaredPrefix(p)) if p == "zz")
        );
        assert!(matches!(
            scope.resolve_parts("_", "b"),
            Err(Error::InvalidQName(_))
        ));
        let bare = Namespaces::new();
        assert!(matches!(
            Scope::new(&bare, None).resolve_parts("", "b"),
            Err(Error::UndeclaredPrefix(p)) if p.is_empty()
        ));
    }

    #[test]
    fn scope_prefers_bundle_binding() {
        let mut doc = Namespaces::new();
        doc.declare("ex", "http://doc.org/").unwrap();
        doc.set_default("http://doc-default.org/");
        let mut bundle = Namespaces::new();
        bundle.declare("ex", "http://bundle.org/").unwrap();
        let scope = Scope::new(&doc, Some(&bundle));
        assert_eq!(
            scope.resolve("ex:e").unwrap(),
            QName::new("http://bundle.org/", "e")
        );
        assert_eq!(
            scope.resolve("e").unwrap(),
            QName::new("http://doc-default.org/", "e")
        );
    }
}
