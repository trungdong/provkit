//! Prefix tables and name resolution.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use indexmap::IndexMap;

use crate::well_known::{PROV, XSD};
use crate::{Error, QName, Result, pn};

/// The reserved `prov` prefix.
pub(crate) const PROV_PREFIX: &str = "prov";

/// The prefixes PROV-N predeclares (section 3.7.4), with their IRIs.
static RESERVED: LazyLock<[(&str, Arc<str>); 2]> =
    LazyLock::new(|| [(PROV_PREFIX, Arc::from(PROV)), ("xsd", Arc::from(XSD))]);

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
    /// [`Error::InvalidPrefix`] if `prefix` is not a `PN_PREFIX`: it must
    /// start with a letter, continue with letters, digits, `-`, `_`, `.` or
    /// another `PN_CHARS` character such as U+00B7, and not end in `.`. `_`,
    /// which introduces blank nodes, is never a prefix.
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

    /// Declared prefixes and their IRIs, in declaration order. The reserved
    /// `prov` and `xsd` are not listed.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.prefixes.iter().map(|(p, iri)| (&**p, &**iri))
    }

    /// Whether `prefix` at `iri` restates a predeclared binding.
    pub(crate) fn is_predeclared(prefix: &str, iri: &str) -> bool {
        reserved(prefix) == Some(iri)
    }

    /// The IRI bound to `prefix`, including the reserved `prov` and `xsd`.
    pub fn lookup(&self, prefix: &str) -> Option<&str> {
        self.binding(prefix).map(|iri| &**iri)
    }

    /// As [`lookup`](Self::lookup), with the stored allocation.
    fn binding(&self, prefix: &str) -> Option<&Arc<str>> {
        reserved_shared(prefix).or_else(|| self.prefixes.get(prefix))
    }

    /// Resolve `prefix:local` or a bare `local` to a name.
    ///
    /// # Errors
    ///
    /// - [`Error::UndeclaredPrefix`] if the prefix has no binding.
    /// - [`Error::NoDefaultNamespace`] if the name is bare and no default
    ///   namespace is set.
    /// - [`Error::InvalidQName`] if the text is empty, starts with `:`,
    ///   uses the blank-node prefix `_`, or has a local part PROV-N cannot
    ///   write (see [`QName::new`]).
    pub fn resolve(&self, text: &str) -> Result<QName> {
        Scope::of(self).resolve(text)
    }

    /// The prefix bound to `namespace`, if any (`prov` and `xsd` included).
    /// When several prefixes share the IRI, the first declared answers. The
    /// default namespace has no prefix and never answers.
    pub fn prefix_for(&self, namespace: &str) -> Option<&str> {
        self.prefixes_for(namespace).next()
    }

    /// Every prefix bound to `namespace`, in declaration order (`prov` and
    /// `xsd` included).
    fn prefixes_for<'a>(&'a self, namespace: &str) -> impl Iterator<Item = &'a str> {
        let bound = self
            .by_iri
            .get(namespace)
            .into_iter()
            .flatten()
            .map(|p| &**p);
        reserved_prefix_for(namespace).into_iter().chain(bound)
    }

    /// The prefix for `namespace`, minting `ns1`, `ns2`, ... when none exists.
    pub fn ensure_prefix(&mut self, namespace: &str) -> String {
        WriteScope::of(self).ensure_prefix(namespace)
    }

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
}

/// A table read through to a parent table, without merging the two. A
/// bundle's table in front of its document's is the case it serves: the
/// bundle's bindings and default win, and anything the bundle leaves out
/// falls back to the document.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Scope<'a> {
    own: &'a Namespaces,
    parent: Option<&'a Namespaces>,
}

impl<'a> Scope<'a> {
    /// One table with nothing behind it.
    pub(crate) fn of(own: &'a Namespaces) -> Self {
        Scope { own, parent: None }
    }

    /// `own` in front of `parent`.
    pub(crate) fn within(own: &'a Namespaces, parent: &'a Namespaces) -> Self {
        Scope {
            own,
            parent: Some(parent),
        }
    }

    fn tables(self) -> impl Iterator<Item = &'a Namespaces> {
        std::iter::once(self.own).chain(self.parent)
    }

    /// Resolve `prefix:local` or a bare `local` to a name.
    ///
    /// # Errors
    ///
    /// As [`Namespaces::resolve`].
    pub(crate) fn resolve(self, text: &str) -> Result<QName> {
        match text.split_once(':') {
            Some(("", _)) => Err(Error::InvalidQName(text.into())),
            Some((prefix, local)) => self.resolve_parts(prefix, local),
            None => self.resolve_parts("", text),
        }
    }

    /// Resolve a name already split into `prefix` and `local`; an empty
    /// prefix means a bare name. A colon in `local` stays in the local part.
    ///
    /// # Errors
    ///
    /// As [`Namespaces::resolve`].
    pub(crate) fn resolve_parts(self, prefix: &str, local: &str) -> Result<QName> {
        if prefix.is_empty() {
            if local.is_empty() {
                return Err(Error::InvalidQName(String::new()));
            }
            let ns = self
                .tables()
                .find_map(|table| table.default.clone())
                .ok_or_else(|| Error::NoDefaultNamespace(local.into()))?;
            return QName::in_namespace(ns, local);
        }
        // `_:` marks a blank node; readers handle it before resolving.
        if prefix == "_" {
            return Err(Error::InvalidQName(format!("{prefix}:{local}")));
        }
        let ns = self
            .tables()
            .find_map(|table| table.binding(prefix).cloned())
            .ok_or_else(|| Error::UndeclaredPrefix(prefix.into()))?;
        // `in_namespace` fails only on the local part. The error names the
        // whole text.
        QName::in_namespace(ns, local).map_err(|_| Error::InvalidQName(format!("{prefix}:{local}")))
    }
}

/// The writers' side of a scope: names render against `own`, then against
/// `parent`, and a minted prefix is added to `own`.
#[derive(Debug)]
pub(crate) struct WriteScope<'a> {
    own: &'a mut Namespaces,
    parent: Option<&'a Namespaces>,
}

impl<'a> WriteScope<'a> {
    /// One table with nothing behind it.
    pub(crate) fn of(own: &'a mut Namespaces) -> Self {
        WriteScope { own, parent: None }
    }

    /// `own` in front of `parent`.
    pub(crate) fn within(own: &'a mut Namespaces, parent: &'a Namespaces) -> Self {
        WriteScope {
            own,
            parent: Some(parent),
        }
    }

    /// The prefix that writes `namespace`: one of `own`'s, or the first of
    /// the parent's that `own` does not bind to something else.
    fn prefix_for(&self, namespace: &str) -> Option<&str> {
        if let Some(prefix) = self.own.prefix_for(namespace) {
            return Some(prefix);
        }
        self.parent?
            .prefixes_for(namespace)
            .find(|prefix| self.own.lookup(prefix).is_none())
    }

    fn default_namespace(&self) -> Option<&str> {
        self.own
            .default_namespace()
            .or_else(|| self.parent?.default_namespace())
    }

    /// The prefix that writes `name`, or `None` when it is written bare.
    ///
    /// A declared prefix wins, then the default namespace for a bare name,
    /// then a freshly minted `nsN` prefix, which is added to `own`. The local
    /// part is always `name.local()`, so a writer can escape it on its own.
    pub(crate) fn render_parts(&mut self, name: &QName) -> Option<String> {
        if let Some(prefix) = self.prefix_for(name.namespace()) {
            return Some(prefix.to_owned());
        }
        // A colon in the local part would read back as a prefix separator,
        // and an empty local part would vanish.
        let bare = !name.local().is_empty()
            && !name.local().contains(':')
            && self.default_namespace() == Some(name.namespace());
        if bare {
            return None;
        }
        Some(self.mint_prefix(name.namespace(), |_| true))
    }

    /// The prefix for `namespace`, minting `ns1`, `ns2`, ... when none exists.
    pub(crate) fn ensure_prefix(&mut self, namespace: &str) -> String {
        if let Some(prefix) = self.prefix_for(namespace) {
            return prefix.to_owned();
        }
        self.mint_prefix(namespace, |_| true)
    }

    /// Bind the first `nsN` that neither table uses and that `accept` takes
    /// to `namespace`, in `own`, even when `namespace` already has a prefix.
    pub(crate) fn mint_prefix(&mut self, namespace: &str, accept: impl Fn(&str) -> bool) -> String {
        let mut n = 1;
        loop {
            let candidate = format!("ns{n}");
            if !self.own.prefixes.contains_key(candidate.as_str())
                && self.parent.is_none_or(|p| p.lookup(&candidate).is_none())
                && accept(&candidate)
            {
                self.own.bind(&candidate, Arc::from(namespace));
                return candidate;
            }
            n += 1;
        }
    }
}

/// The shared IRI of a reserved prefix.
fn reserved_shared(prefix: &str) -> Option<&'static Arc<str>> {
    RESERVED
        .iter()
        .find_map(|(p, iri)| (*p == prefix).then_some(iri))
}

/// The IRI of a reserved prefix, if `prefix` is one of the two.
fn reserved(prefix: &str) -> Option<&'static str> {
    reserved_shared(prefix).map(|iri| &**iri)
}

/// The reserved prefix bound to `namespace`, if it is one of the two.
fn reserved_prefix_for(namespace: &str) -> Option<&'static str> {
    RESERVED
        .iter()
        .find_map(|(p, iri)| (**iri == *namespace).then_some(*p))
}

#[cfg(test)]
mod tests;
