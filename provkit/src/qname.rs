//! Qualified names.

use std::fmt;
use std::sync::Arc;

/// A PROV identifier: a namespace IRI and a local part.
///
/// The prefix a document used to spell the name is not stored; two names are
/// equal when their namespace and local part are equal.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QName {
    namespace: Arc<str>,
    local: Box<str>,
}

impl QName {
    /// Build a name from a namespace IRI and a local part.
    pub fn new(namespace: impl Into<Arc<str>>, local: impl Into<Box<str>>) -> Self {
        QName {
            namespace: namespace.into(),
            local: local.into(),
        }
    }

    /// The namespace IRI.
    pub fn namespace(&self) -> &Arc<str> {
        &self.namespace
    }

    /// The local part.
    pub fn local(&self) -> &str {
        &self.local
    }

    /// The full IRI, namespace followed by local part.
    pub fn iri(&self) -> String {
        format!("{}{}", self.namespace, self.local)
    }
}

impl fmt::Debug for QName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>{}", self.namespace, self.local)
    }
}

/// Writes the full IRI, so error messages show IRIs rather than prefixed names.
impl fmt::Display for QName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.namespace, self.local)
    }
}

/// Well-known names in the `prov:` and `xsd:` namespaces.
pub mod well_known {
    use super::QName;
    use crate::namespaces::{PROV, XSD};
    use std::sync::LazyLock;

    macro_rules! names {
        ($($name:ident = $prefix:literal $ns:ident : $local:literal;)*) => {
            $(
                #[doc = concat!("`", $prefix, ":", $local, "`")]
                pub static $name: LazyLock<QName> = LazyLock::new(|| QName::new($ns, $local));
            )*
        };
    }

    names! {
        PROV_TYPE = "prov" PROV:"type";
        PROV_LABEL = "prov" PROV:"label";
        PROV_LOCATION = "prov" PROV:"location";
        PROV_ROLE = "prov" PROV:"role";
        PROV_VALUE = "prov" PROV:"value";
        PROV_INTERNATIONALIZED_STRING = "prov" PROV:"InternationalizedString";
        PROV_QUALIFIED_NAME = "prov" PROV:"QUALIFIED_NAME";
        XSD_STRING = "xsd" XSD:"string";
        XSD_QNAME = "xsd" XSD:"QName";
        XSD_DATETIME = "xsd" XSD:"dateTime";
        XSD_INT = "xsd" XSD:"int";
        XSD_LONG = "xsd" XSD:"long";
        XSD_INTEGER = "xsd" XSD:"integer";
        XSD_DOUBLE = "xsd" XSD:"double";
        XSD_FLOAT = "xsd" XSD:"float";
        XSD_BOOLEAN = "xsd" XSD:"boolean";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Namespaces;
    use crate::test_util::hash;

    #[test]
    fn equality_ignores_spelling() {
        let mut ns = Namespaces::new();
        ns.declare("a", "http://example.org/").unwrap();
        ns.declare("b", "http://example.org/").unwrap();
        let a = ns.resolve("a:x").unwrap();
        let b = ns.resolve("b:x").unwrap();
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
        assert_eq!(a, QName::new(Arc::from("http://example.org/"), "x"));
    }

    #[test]
    fn iri_joins_namespace_and_local() {
        let a = QName::new("http://example.org/", "e1");
        assert_eq!(a.iri(), "http://example.org/e1");
        assert_eq!(a.to_string(), "http://example.org/e1");
        assert_ne!(a, QName::new("http://example.org/", "e2"));
        assert_ne!(a, QName::new("http://other.org/", "e1"));
    }

    #[test]
    fn well_known_names_use_full_iris() {
        assert_eq!(
            well_known::PROV_TYPE.iri(),
            "http://www.w3.org/ns/prov#type"
        );
        assert_eq!(
            well_known::XSD_DATETIME.iri(),
            "http://www.w3.org/2001/XMLSchema#dateTime"
        );
    }
}
