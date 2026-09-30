//! Attribute values.

use std::hash::{Hash, Hasher};

use crate::qname::well_known::{
    XSD_BOOLEAN, XSD_DATETIME, XSD_DOUBLE, XSD_FLOAT, XSD_INT, XSD_LONG, XSD_QNAME, XSD_STRING,
};
use crate::{DateTime, Error, QName, Result};

/// A string with a BCP 47 language tag.
///
/// Tags compare case-insensitively, as BCP 47 requires.
#[derive(Clone, Debug)]
pub struct LangStr {
    value: String,
    lang: String,
}

impl LangStr {
    /// Build a tagged string. The tag must be well-formed (`en`, `en-GB`, `zh-Hant-TW`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLiteral`] if the tag is empty or malformed.
    pub fn new(value: impl Into<String>, lang: impl Into<String>) -> Result<Self> {
        let lang = lang.into();
        let well_formed = !lang.is_empty()
            && lang.split('-').all(|part| {
                (1..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
            })
            && lang
                .split('-')
                .next()
                .is_some_and(|p| p.bytes().all(|b| b.is_ascii_alphabetic()));
        if !well_formed {
            return Err(Error::InvalidLiteral {
                datatype: "language tag".into(),
                lexical: lang,
                reason: "not a well-formed BCP 47 tag".into(),
            });
        }
        Ok(LangStr {
            value: value.into(),
            lang,
        })
    }

    /// The text.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The tag as written.
    #[must_use]
    pub fn lang(&self) -> &str {
        &self.lang
    }
}

impl PartialEq for LangStr {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.lang.eq_ignore_ascii_case(&other.lang)
    }
}

impl Eq for LangStr {}

impl Hash for LangStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
        for b in self.lang.bytes() {
            b.to_ascii_lowercase().hash(state);
        }
    }
}

/// The value of a non-formal attribute.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    /// A plain string (`xsd:string`).
    Str(String),
    /// A language-tagged string.
    LangStr(LangStr),
    /// A qualified name (`xsd:QName`).
    QName(QName),
    /// An `xsd:dateTime`.
    DateTime(DateTime),
    /// Any other typed literal, kept as written.
    ///
    /// Build it through [`Value::typed`], which folds `xsd:string` to
    /// [`Value::Str`] and parses `xsd:dateTime` to [`Value::DateTime`], as
    /// the readers do. A hand-built `Typed` with either datatype is not equal
    /// to the value a reader produces. `xsd:QName` does not fold here either,
    /// because resolving the lexical form needs the document's namespaces, so
    /// a `Typed` with `xsd:QName` never equals a read [`Value::QName`].
    Typed {
        /// The text as written.
        lexical: String,
        /// The datatype.
        datatype: QName,
    },
}

impl Value {
    /// A literal with a datatype. `xsd:string` folds to [`Value::Str`],
    /// `xsd:dateTime` is parsed, and anything else is kept verbatim.
    ///
    /// `xsd:QName` stays [`Value::Typed`], because resolving `ex:name` needs
    /// namespaces. Build a [`Value::QName`] directly instead; the readers
    /// resolve `xsd:QName` literals to that variant.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLiteral`] if an `xsd:dateTime` does not parse.
    pub fn typed(lexical: impl Into<String>, datatype: QName) -> Result<Self> {
        let lexical = lexical.into();
        if datatype == *XSD_STRING {
            Ok(Value::Str(lexical))
        } else if datatype == *XSD_DATETIME {
            Ok(Value::DateTime(DateTime::parse(&lexical)?))
        } else {
            Ok(Value::Typed { lexical, datatype })
        }
    }

    /// The datatype this value asserts (`xsd:string` for `Str`, none for a
    /// language-tagged string).
    #[must_use]
    pub fn datatype(&self) -> Option<QName> {
        match self {
            Value::Str(_) => Some(XSD_STRING.clone()),
            Value::LangStr(_) => None,
            Value::QName(_) => Some(XSD_QNAME.clone()),
            Value::DateTime(_) => Some(XSD_DATETIME.clone()),
            Value::Typed { datatype, .. } => Some(datatype.clone()),
        }
    }
}

macro_rules! from_native {
    ($($t:ty => $dt:ident),* $(,)?) => {
        $(
            impl From<$t> for Value {
                fn from(v: $t) -> Self {
                    Value::Typed { lexical: v.to_string(), datatype: $dt.clone() }
                }
            }
        )*
    };
}

// Floats use the XSD spellings of the non-finite values.
macro_rules! from_float {
    ($($t:ty => $dt:ident),* $(,)?) => {
        $(
            impl From<$t> for Value {
                fn from(v: $t) -> Self {
                    let lexical = if v.is_nan() {
                        "NaN".to_owned()
                    } else if v.is_infinite() {
                        if v > 0.0 { "INF" } else { "-INF" }.to_owned()
                    } else {
                        v.to_string()
                    };
                    Value::Typed { lexical, datatype: $dt.clone() }
                }
            }
        )*
    };
}

from_native! { i32 => XSD_INT, i64 => XSD_LONG, bool => XSD_BOOLEAN }
from_float! { f32 => XSD_FLOAT, f64 => XSD_DOUBLE }

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<QName> for Value {
    fn from(q: QName) -> Self {
        Value::QName(q)
    }
}

impl From<DateTime> for Value {
    fn from(d: DateTime) -> Self {
        Value::DateTime(d)
    }
}

impl From<LangStr> for Value {
    fn from(l: LangStr) -> Self {
        Value::LangStr(l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qname::well_known::{
        XSD_BOOLEAN, XSD_DATETIME, XSD_DOUBLE, XSD_FLOAT, XSD_INT, XSD_LONG, XSD_STRING,
    };
    use crate::test_util::hash;

    #[test]
    fn typed_folds_string_and_parses_datetime() {
        assert_eq!(
            Value::typed("x", XSD_STRING.clone()).unwrap(),
            Value::Str("x".into())
        );
        assert!(matches!(
            Value::typed("2011-11-16T16:05:33Z", XSD_DATETIME.clone()).unwrap(),
            Value::DateTime(_)
        ));
        assert!(matches!(
            Value::typed("nope", XSD_DATETIME.clone()),
            Err(Error::InvalidLiteral { .. })
        ));
        assert_eq!(
            Value::typed("1", XSD_INT.clone()).unwrap(),
            Value::Typed {
                lexical: "1".into(),
                datatype: XSD_INT.clone()
            }
        );
    }

    #[test]
    fn datatype_reports_the_asserted_type() {
        assert_eq!(Value::from("s").datatype(), Some(XSD_STRING.clone()));
        assert_eq!(Value::from(1i32).datatype(), Some(XSD_INT.clone()));
        let tagged = Value::from(LangStr::new("hi", "en").unwrap());
        assert_eq!(tagged.datatype(), None);
    }

    #[test]
    fn lang_tags_compare_case_insensitively() {
        let a = LangStr::new("hi", "en-GB").unwrap();
        let b = LangStr::new("hi", "EN-gb").unwrap();
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
        assert_ne!(a, LangStr::new("hi", "en-US").unwrap());
        assert_ne!(a, LangStr::new("bye", "en-GB").unwrap());
        assert!(matches!(
            LangStr::new("hi", "en_US"),
            Err(Error::InvalidLiteral { .. })
        ));
        assert!(LangStr::new("hi", "").is_err());
        assert_eq!(a.value(), "hi");
        assert_eq!(b.lang(), "EN-gb");
    }

    #[test]
    fn native_conversions_are_exact() {
        assert_eq!(
            Value::from(1i32),
            Value::Typed {
                lexical: "1".into(),
                datatype: XSD_INT.clone()
            }
        );
        assert_eq!(
            Value::from(1i64),
            Value::Typed {
                lexical: "1".into(),
                datatype: XSD_LONG.clone()
            }
        );
        assert_eq!(
            Value::from(1.5f64),
            Value::Typed {
                lexical: "1.5".into(),
                datatype: XSD_DOUBLE.clone()
            }
        );
        assert_eq!(
            Value::from(1.5f32),
            Value::Typed {
                lexical: "1.5".into(),
                datatype: XSD_FLOAT.clone()
            }
        );
        assert_eq!(
            Value::from(true),
            Value::Typed {
                lexical: "true".into(),
                datatype: XSD_BOOLEAN.clone()
            }
        );
        assert_eq!(Value::from("s"), Value::Str("s".into()));
    }

    #[test]
    fn non_finite_floats_use_xsd_spellings() {
        let lexical = |v: Value| match v {
            Value::Typed { lexical, .. } => lexical,
            other => panic!("expected Typed, got {other:?}"),
        };
        assert_eq!(lexical(Value::from(f64::NAN)), "NaN");
        assert_eq!(lexical(Value::from(f64::INFINITY)), "INF");
        assert_eq!(lexical(Value::from(f64::NEG_INFINITY)), "-INF");
        assert_eq!(lexical(Value::from(f32::NAN)), "NaN");
        assert_eq!(lexical(Value::from(f32::INFINITY)), "INF");
        assert_eq!(lexical(Value::from(f32::NEG_INFINITY)), "-INF");
    }

    #[test]
    fn same_lexical_different_datatype_differs() {
        assert_ne!(Value::from(1i32), Value::from(1i64));
    }

    #[test]
    fn string_conversions_give_str() {
        assert_eq!(Value::from(String::from("s")), Value::Str("s".into()));
        assert_eq!(Value::from(String::new()), Value::from(""));
    }
}
