//! Qualified names.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::{Error, Result, pn};

/// A PROV identifier: a namespace IRI and a local part.
///
/// A name denotes the IRI its two parts join to. Equality, ordering and
/// hashing use that IRI, so `http://example.org/` with `foo/bar` equals
/// `http://example.org/foo/` with `bar`. The split is kept for writing, and
/// the prefix a document used to spell the name is not stored.
///
/// ```
/// use provkit::QName;
///
/// let a = QName::new("http://example.org/", "foo/bar")?;
/// let b = QName::new("http://example.org/foo/", "bar")?;
/// assert_eq!(a, b);
/// # Ok::<(), provkit::Error>(())
/// ```
#[derive(Clone)]
pub struct QName {
    namespace: Arc<str>,
    local: Box<str>,
}

impl QName {
    /// A name in `namespace` with local part `local`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidQName`] if PROV-N cannot write `local`, even with an
    /// escape: a space, a control character, one of `{ } < > " | ^ \`
    /// or a backtick, a `%` not followed by two hex digits, or a first
    /// character that may only continue a name (`·`, `‿`, `⁀` or a
    /// combining mark). An empty local part is valid.
    pub fn new(namespace: &str, local: &str) -> Result<Self> {
        Self::in_namespace(Arc::from(namespace), local)
    }

    /// A name that shares `namespace` with the other names built from the
    /// same table.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub(crate) fn in_namespace(namespace: Arc<str>, local: &str) -> Result<Self> {
        Self::check_local(local)?;
        Ok(QName {
            namespace,
            local: local.into(),
        })
    }

    /// Every character of `local` has a place in a `PN_LOCAL`, bare or
    /// after `\`.
    fn check_local(local: &str) -> Result<()> {
        let writable = local
            .char_indices()
            .all(|(at, c)| pn::local_char(local, at, c).is_some());
        if writable {
            Ok(())
        } else {
            Err(Error::InvalidQName(local.to_owned()))
        }
    }

    /// The namespace IRI.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The local part.
    pub fn local(&self) -> &str {
        &self.local
    }

    /// The full IRI.
    pub fn iri(&self) -> String {
        format!("{}{}", self.namespace, self.local)
    }

    /// The bytes of the full IRI, without allocating.
    fn iri_bytes(&self) -> impl Iterator<Item = u8> + '_ {
        self.namespace.bytes().chain(self.local.bytes())
    }

    /// Whether this name's IRI is `namespace` followed by `local`, however
    /// either side splits it.
    pub(crate) fn is(&self, namespace: &str, local: &str) -> bool {
        self.namespace.len() + self.local.len() == namespace.len() + local.len()
            && self.iri_bytes().eq(namespace.bytes().chain(local.bytes()))
    }
}

impl PartialEq for QName {
    fn eq(&self, other: &Self) -> bool {
        self.is(&other.namespace, &other.local)
    }
}

impl Eq for QName {}

impl Ord for QName {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iri_bytes().cmp(other.iri_bytes())
    }
}

impl PartialOrd for QName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Hashes the IRI's bytes through a fixed buffer, so the same IRI feeds the
/// hasher the same `write` calls however it is split, then a terminator as
/// `str` does.
impl Hash for QName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut buf = [0u8; 64];
        let mut len = 0;
        for b in self.iri_bytes() {
            buf[len] = b;
            len += 1;
            if len == buf.len() {
                state.write(&buf);
                len = 0;
            }
        }
        state.write(&buf[..len]);
        state.write_u8(0xff);
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
    use std::sync::LazyLock;

    /// The PROV namespace IRI.
    pub const PROV: &str = "http://www.w3.org/ns/prov#";
    /// The XML Schema datatypes namespace IRI.
    pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

    macro_rules! names {
        ($($name:ident = $prefix:literal $ns:ident : $local:literal;)*) => {
            $(
                #[doc = concat!("`", $prefix, ":", $local, "`")]
                pub static $name: LazyLock<QName> = LazyLock::new(|| {
                    QName::new($ns, $local).expect("a well-known local part")
                });
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
        XSD_SHORT = "xsd" XSD:"short";
        XSD_BYTE = "xsd" XSD:"byte";
        XSD_NON_NEGATIVE_INTEGER = "xsd" XSD:"nonNegativeInteger";
        XSD_POSITIVE_INTEGER = "xsd" XSD:"positiveInteger";
        XSD_NON_POSITIVE_INTEGER = "xsd" XSD:"nonPositiveInteger";
        XSD_NEGATIVE_INTEGER = "xsd" XSD:"negativeInteger";
        XSD_UNSIGNED_LONG = "xsd" XSD:"unsignedLong";
        XSD_UNSIGNED_INT = "xsd" XSD:"unsignedInt";
        XSD_UNSIGNED_SHORT = "xsd" XSD:"unsignedShort";
        XSD_UNSIGNED_BYTE = "xsd" XSD:"unsignedByte";
        XSD_DECIMAL = "xsd" XSD:"decimal";
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
        assert_eq!(a, QName::new("http://example.org/", "x").unwrap());
    }

    #[test]
    fn identity_is_the_iri() {
        let split_late = QName::new("http://example.org/", "foo/bar").unwrap();
        let split_early = QName::new("http://example.org/foo/", "bar").unwrap();
        assert_eq!(split_late, split_early);
        assert_eq!(hash(&split_late), hash(&split_early));
        assert_eq!(split_late.cmp(&split_early), Ordering::Equal);
        assert_eq!(format!("{split_late:?}"), "<http://example.org/>foo/bar");
        assert_eq!(format!("{split_early:?}"), "<http://example.org/foo/>bar");

        // Past the hash buffer, where the split falls in a later block.
        let long = "x".repeat(150);
        let long_ns = QName::new(&format!("http://example.org/{long}"), "tail").unwrap();
        let long_local = QName::new("http://example.org/", &format!("{long}tail")).unwrap();
        assert_eq!(long_ns, long_local);
        assert_eq!(hash(&long_ns), hash(&long_local));

        let other = QName::new("http://example.org/", "foo/baz").unwrap();
        assert_ne!(split_late, other);
        assert!(split_late < other && other > split_early);
        assert!(QName::new("http://a/", "z").unwrap() < QName::new("http://b/", "a").unwrap());
        assert!(QName::new("http://a/", "b").unwrap() < QName::new("http://a/b", "c").unwrap());
        assert!(split_late.is("http://example.org/f", "oo/bar"));
        assert!(!split_late.is("http://example.org/", "foo/ba"));
    }

    #[test]
    fn a_local_part_provn_cannot_write_is_invalid() {
        let new = |local: &str| QName::new("http://example.org/", local);
        for local in [
            "a b", "a\tb", "a\u{7}b", "a{b", "a}b", "a<b", "a>b", "a\"b", "a|b", "a^b", "a\\b",
            "a`b", "a%", "a%4", "a%zz", "\u{301}a", "\u{B7}a", "\u{2028}",
        ] {
            assert!(
                matches!(new(local), Err(Error::InvalidQName(l)) if l == local),
                "{local:?}"
            );
        }
        // Escaped characters anywhere, `%XX`, an empty local part and, after
        // the first character, the rest of `PN_CHARS`.
        for local in [
            "",
            "a=b'c(d)e,f:g-h[i]j.k;l",
            "-a",
            ".a",
            "a.",
            "a%2Fb",
            "12",
            "a\u{301}\u{B7}",
            "a-",
            "@a",
            "_a~b!c$d&e*f+g/h?i#j@k",
            "é",
        ] {
            assert!(new(local).is_ok(), "{local:?}");
        }
    }

    #[test]
    fn iri_joins_namespace_and_local() {
        let a = QName::new("http://example.org/", "e1").unwrap();
        assert_eq!(a.iri(), "http://example.org/e1");
        assert_eq!(a.to_string(), "http://example.org/e1");
        assert_ne!(a, QName::new("http://example.org/", "e2").unwrap());
        assert_ne!(a, QName::new("http://other.org/", "e1").unwrap());
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
