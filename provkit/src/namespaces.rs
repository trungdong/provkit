//! Prefix tables and name resolution.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use indexmap::IndexMap;

use crate::well_known::{PROV, XSD};
use crate::{Error, QName, Result, pn};

static PROV_IRI: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from(PROV));
static XSD_IRI: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from(XSD));

/// A default namespace plus an ordered prefix-to-IRI table.
///
/// `prov` and `xsd` are always present and cannot be moved. The table is a
/// presentation detail: it decides how names are written and never takes part
/// in equality.
#[derive(Clone, Debug, Default)]
pub struct Namespaces {
    default: Option<Arc<str>>,
    prefixes: IndexMap<Box<str>, Arc<str>>,
    /// For each IRI, the prefixes bound to it in declaration order, so the
    /// first one answers `prefix_for`. Kept in step with `prefixes`.
    by_iri: HashMap<Arc<str>, Vec<Box<str>>>,
}

/// Compares the default namespace and the bindings, ignoring their order.
impl PartialEq for Namespaces {
    fn eq(&self, other: &Self) -> bool {
        self.default == other.default && self.prefixes == other.prefixes
    }
}

impl Eq for Namespaces {}

impl Namespaces {
    /// An empty table (with the predeclared `prov` and `xsd`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default namespace, used for bare names.
    pub fn set_default(&mut self, iri: &str) {
        self.default = Some(Arc::from(iri));
    }

    /// The default namespace, if any.
    pub fn default_namespace(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// Bind `prefix` to `iri`, replacing an earlier binding of the same prefix.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidPrefix`] if `prefix` is not a `PN_PREFIX`: empty,
    /// containing `:` or whitespace, not starting with a letter, or ending
    /// in `.`. `_`, which introduces blank nodes, is never a prefix.
    /// [`Error::ReservedPrefix`] if `prefix` is `prov` or `xsd` and `iri` is
    /// not its fixed IRI.
    pub fn declare(&mut self, prefix: &str, iri: &str) -> Result<()> {
        if !pn::is_pn_prefix(prefix) {
            return Err(Error::InvalidPrefix(prefix.into()));
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
        self.bind(prefix, Arc::from(iri));
        Ok(())
    }

    /// Bind `prefix` to `iri`, replacing an earlier binding of `prefix` and
    /// keeping its place in the declaration order.
    fn bind(&mut self, prefix: &str, iri: Arc<str>) {
        self.unindex(prefix);
        let (position, _) = self.prefixes.insert_full(prefix.into(), iri.clone());
        let prefixes = &self.prefixes;
        let bound = self.by_iri.entry(iri).or_default();
        let at = bound.partition_point(|p| prefixes.get_index_of(&**p) < Some(position));
        bound.insert(at, prefix.into());
    }

    /// Drop `prefix` from the reverse map, if it is bound.
    fn unindex(&mut self, prefix: &str) {
        let Some(iri) = self.prefixes.get(prefix) else {
            return;
        };
        if let Some(bound) = self.by_iri.get_mut(&**iri) {
            bound.retain(|p| &**p != prefix);
            if bound.is_empty() {
                self.by_iri.remove(&**iri);
            }
        }
    }

    /// Add the bindings of `other` that this table lacks. A prefix or default
    /// namespace already bound here keeps its binding, so merging cannot fail.
    pub(crate) fn absorb(&mut self, other: &Namespaces) {
        if self.default.is_none() {
            self.default.clone_from(&other.default);
        }
        for (prefix, iri) in &other.prefixes {
            if !self.prefixes.contains_key(prefix) {
                self.bind(prefix, iri.clone());
            }
        }
    }

    /// Remove the binding of `prefix`, keeping the order of the rest.
    // Only the JSON writer calls this, and it is feature-gated.
    #[cfg_attr(not(feature = "json"), allow(dead_code))]
    pub(crate) fn forget(&mut self, prefix: &str) {
        self.unindex(prefix);
        self.prefixes.shift_remove(prefix);
    }

    /// Declared prefixes and their IRIs, in declaration order. The reserved
    /// `prov` and `xsd` are not listed.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.prefixes.iter().map(|(p, iri)| (&**p, &**iri))
    }

    /// The IRI bound to `prefix`, including the reserved `prov` and `xsd`.
    pub fn lookup(&self, prefix: &str) -> Option<&str> {
        match reserved(prefix) {
            Some(iri) => Some(iri),
            None => self.prefixes.get(prefix).map(|iri| &**iri),
        }
    }

    /// As [`lookup`](Self::lookup), sharing the stored allocation.
    fn lookup_shared(&self, prefix: &str) -> Option<Arc<str>> {
        match prefix {
            "prov" => Some(PROV_IRI.clone()),
            "xsd" => Some(XSD_IRI.clone()),
            _ => self.prefixes.get(prefix).cloned(),
        }
    }

    /// Resolve `prefix:local` or a bare `local` to a name.
    ///
    /// # Errors
    ///
    /// - [`Error::UndeclaredPrefix`] if the prefix has no binding.
    /// - [`Error::NoDefaultNamespace`] if the name is bare and no default
    ///   namespace is set.
    /// - [`Error::InvalidQName`] if the text is empty, starts with `:` or
    ///   uses the blank-node prefix `_`.
    pub fn resolve(&self, text: &str) -> Result<QName> {
        match text.split_once(':') {
            // `_:` marks a blank node; readers handle it before resolving.
            Some(("" | "_", _)) => Err(Error::InvalidQName(text.into())),
            Some((prefix, local)) => self.resolve_parts(prefix, local),
            None => self.resolve_parts("", text),
        }
    }

    /// Resolve a name already split into `prefix` and `local`; an empty
    /// prefix means a bare name. A colon in `local` stays in the local part.
    ///
    /// # Errors
    ///
    /// As [`resolve`](Self::resolve).
    // Only the PROV-N reader calls this, and it is feature-gated.
    #[cfg_attr(not(feature = "provn"), allow(dead_code))]
    pub(crate) fn resolve_parts(&self, prefix: &str, local: &str) -> Result<QName> {
        if prefix.is_empty() {
            if local.is_empty() {
                return Err(Error::InvalidQName(String::new()));
            }
            let ns = self
                .default
                .clone()
                .ok_or_else(|| Error::NoDefaultNamespace(local.into()))?;
            return Ok(QName::in_namespace(ns, local));
        }
        if prefix == "_" {
            return Err(Error::InvalidQName(format!("{prefix}:{local}")));
        }
        let ns = self
            .lookup_shared(prefix)
            .ok_or_else(|| Error::UndeclaredPrefix(prefix.into()))?;
        Ok(QName::in_namespace(ns, local))
    }

    /// The prefix bound to `namespace`, if any (`prov` and `xsd` included).
    /// When several prefixes share the IRI, the first declared answers. The
    /// default namespace has no prefix and never answers.
    pub fn prefix_for(&self, namespace: &str) -> Option<&str> {
        if namespace == PROV {
            return Some("prov");
        }
        if namespace == XSD {
            return Some("xsd");
        }
        self.by_iri
            .get(namespace)
            .and_then(|bound| bound.first())
            .map(|p| &**p)
    }

    /// The prefix for `namespace`, minting `ns1`, `ns2`, ... when none exists.
    pub fn ensure_prefix(&mut self, namespace: &str) -> String {
        if let Some(p) = self.prefix_for(namespace) {
            return p.to_owned();
        }
        self.mint_prefix(namespace, |_| true)
    }

    /// Bind the first `nsN` that this table does not use and that `accept`
    /// takes to `namespace`, even when `namespace` already has a prefix.
    pub(crate) fn mint_prefix(&mut self, namespace: &str, accept: impl Fn(&str) -> bool) -> String {
        let mut n = 1;
        loop {
            let candidate = format!("ns{n}");
            if !self.prefixes.contains_key(candidate.as_str()) && accept(&candidate) {
                self.bind(&candidate, Arc::from(namespace));
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
    /// let merged = bundle.with_fallback(&document);
    /// assert_eq!(merged.lookup("ex"), Some("http://bundle.org/"));
    /// assert_eq!(merged.lookup("o"), Some("http://other.org/"));
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
    // Only the JSON writer calls this, and it is feature-gated.
    #[cfg_attr(not(feature = "json"), allow(dead_code))]
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
                .is_some_and(|d| d == name.namespace());
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
        assert!(matches!(ns.resolve("e1"), Err(Error::NoDefaultNamespace(l)) if l == "e1"));
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
            Err(Error::InvalidPrefix(p)) if p == "_"
        ));
        assert_eq!(ns.iter().count(), 0);
        // With no binding, `_:x` is still rejected rather than reported
        // as an undeclared prefix.
        assert!(matches!(ns.resolve("_:x"), Err(Error::InvalidQName(_))));
        let doc = Namespaces::new();
        let mut bundle = Namespaces::new();
        bundle.set_default("http://default.org/");
        let merged = bundle.with_fallback(&doc);
        assert!(matches!(merged.resolve("_:x"), Err(Error::InvalidQName(_))));
    }

    #[test]
    fn prefixes_must_be_pn_prefixes() {
        let mut ns = Namespaces::new();
        for bad in [
            "", "a:b", "a b", " a", "a\t", "1a", "-a", ".a", "a.", "é:", "_", "_a",
        ] {
            assert!(
                matches!(ns.declare(bad, "http://x/"), Err(Error::InvalidPrefix(p)) if p == bad),
                "{bad:?}"
            );
        }
        for good in ["a", "a1", "a.b", "a-b", "a_b", "é", "default", "ns1"] {
            ns.declare(good, "http://x/").unwrap();
        }
        assert_eq!(ns.iter().count(), 8);
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
    fn a_bundle_table_falls_back_to_the_document() {
        let mut doc = Namespaces::new();
        doc.declare("ex", "http://example.org/").unwrap();
        let mut bundle = Namespaces::new();
        bundle.declare("b", "http://bundle.org/").unwrap();
        let merged = bundle.with_fallback(&doc);
        assert_eq!(
            merged.resolve("ex:e").unwrap(),
            QName::new("http://example.org/", "e")
        );
        assert_eq!(
            merged.resolve("b:e").unwrap(),
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
        let merged = bundle.with_fallback(&doc);
        assert_eq!(
            merged.resolve_parts("", "a:b").unwrap(),
            QName::new("http://default.org/", "a:b")
        );
        assert_eq!(
            merged.resolve_parts("a", "b").unwrap(),
            QName::new("http://bundle.org/", "b")
        );
        assert!(
            matches!(merged.resolve_parts("zz", "b"), Err(Error::UndeclaredPrefix(p)) if p == "zz")
        );
        assert!(matches!(
            merged.resolve_parts("_", "b"),
            Err(Error::InvalidQName(_))
        ));
        let bare = Namespaces::new();
        assert!(matches!(
            bare.resolve_parts("", "b"),
            Err(Error::NoDefaultNamespace(l)) if l == "b"
        ));
    }

    #[test]
    fn a_merged_table_prefers_the_bundle_binding() {
        let mut doc = Namespaces::new();
        doc.declare("ex", "http://doc.org/").unwrap();
        doc.set_default("http://doc-default.org/");
        let mut bundle = Namespaces::new();
        bundle.declare("ex", "http://bundle.org/").unwrap();
        let merged = bundle.with_fallback(&doc);
        assert_eq!(
            merged.resolve("ex:e").unwrap(),
            QName::new("http://bundle.org/", "e")
        );
        assert_eq!(
            merged.resolve("e").unwrap(),
            QName::new("http://doc-default.org/", "e")
        );
        let mut own_default = Namespaces::new();
        own_default.set_default("http://bundle-default.org/");
        assert_eq!(
            own_default.with_fallback(&doc).resolve("e").unwrap(),
            QName::new("http://bundle-default.org/", "e")
        );
    }

    #[test]
    fn the_first_declared_prefix_answers_for_a_shared_iri() {
        let mut ns = Namespaces::new();
        ns.declare("a", "http://x.org/").unwrap();
        ns.declare("b", "http://x.org/").unwrap();
        assert_eq!(ns.prefix_for("http://x.org/"), Some("a"));
        // Redeclaring keeps a prefix's place, so `a` still leads.
        ns.declare("b", "http://y.org/").unwrap();
        ns.declare("b", "http://x.org/").unwrap();
        assert_eq!(ns.prefix_for("http://x.org/"), Some("a"));
        assert_eq!(ns.prefix_for("http://y.org/"), None);
    }

    #[test]
    fn a_redeclared_prefix_moves_to_its_new_iri() {
        let mut ns = Namespaces::new();
        ns.declare("a", "http://x.org/").unwrap();
        ns.declare("b", "http://x.org/").unwrap();
        ns.declare("a", "http://y.org/").unwrap();
        assert_eq!(ns.prefix_for("http://x.org/"), Some("b"));
        assert_eq!(ns.prefix_for("http://y.org/"), Some("a"));
        // `a` keeps its earlier position, so it leads a later shared IRI.
        ns.declare("b", "http://y.org/").unwrap();
        assert_eq!(ns.prefix_for("http://y.org/"), Some("a"));
        assert_eq!(ns.prefix_for("http://x.org/"), None);
    }

    #[test]
    fn a_forgotten_prefix_stops_answering() {
        let mut ns = Namespaces::new();
        ns.declare("a", "http://x.org/").unwrap();
        ns.declare("b", "http://x.org/").unwrap();
        ns.forget("a");
        assert_eq!(ns.prefix_for("http://x.org/"), Some("b"));
        ns.forget("b");
        assert_eq!(ns.prefix_for("http://x.org/"), None);
        ns.forget("b");
    }

    #[test]
    fn the_default_namespace_has_no_prefix() {
        let mut ns = Namespaces::new();
        ns.set_default("http://x.org/");
        assert_eq!(ns.prefix_for("http://x.org/"), None);
    }

    #[test]
    fn a_merged_table_answers_prefix_for() {
        let mut doc = Namespaces::new();
        doc.declare("d", "http://x.org/").unwrap();
        doc.declare("e", "http://z.org/").unwrap();
        let mut bundle = Namespaces::new();
        bundle.declare("b", "http://x.org/").unwrap();
        let merged = bundle.with_fallback(&doc);
        assert_eq!(merged.prefix_for("http://x.org/"), Some("b"));
        assert_eq!(merged.prefix_for("http://z.org/"), Some("e"));
    }

    #[test]
    fn minted_prefixes_answer_prefix_for() {
        let mut ns = Namespaces::new();
        assert_eq!(ns.ensure_prefix("http://x.org/"), "ns1");
        assert_eq!(ns.prefix_for("http://x.org/"), Some("ns1"));
    }

    #[test]
    fn equality_ignores_declaration_order() {
        let mut a = Namespaces::new();
        a.declare("p", "http://x.org/").unwrap();
        a.declare("q", "http://x.org/").unwrap();
        let mut b = Namespaces::new();
        b.declare("q", "http://x.org/").unwrap();
        b.declare("p", "http://x.org/").unwrap();
        assert_eq!(a, b);
    }
}
