//! Attribute values.

use std::hash::{Hash, Hasher};

use crate::well_known::{
    PROV_QUALIFIED_NAME, XSD_BOOLEAN, XSD_BYTE, XSD_DATETIME, XSD_DECIMAL, XSD_DOUBLE, XSD_FLOAT,
    XSD_INT, XSD_INTEGER, XSD_LONG, XSD_NEGATIVE_INTEGER, XSD_NON_NEGATIVE_INTEGER,
    XSD_NON_POSITIVE_INTEGER, XSD_POSITIVE_INTEGER, XSD_QNAME, XSD_SHORT, XSD_STRING,
    XSD_UNSIGNED_BYTE, XSD_UNSIGNED_INT, XSD_UNSIGNED_LONG, XSD_UNSIGNED_SHORT,
};
use crate::{DateTime, Error, Namespaces, QName, Result, pn};

/// A string with a language tag.
///
/// Tags compare case-insensitively, as BCP 47 requires.
#[derive(Clone, Debug)]
pub struct LangStr {
    value: String,
    lang: String,
}

impl LangStr {
    /// Build a tagged string. The tag has the shape of the grammar's
    /// `LANGTAG` without its `@`: letters, then any number of `-` and
    /// letters or digits (`en`, `en-GB`, `zh-Hant-TW`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLanguageTag`] for any other tag.
    pub fn new(value: impl Into<String>, lang: impl Into<String>) -> Result<Self> {
        let lang = lang.into();
        let mut subtags = lang.split('-');
        let primary = subtags.next().unwrap_or_default();
        let well_formed = !primary.is_empty()
            && primary.bytes().all(|b| b.is_ascii_alphabetic())
            && subtags.all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric()));
        if !well_formed {
            return Err(Error::InvalidLanguageTag(lang));
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
///
/// Values compare and hash by what they denote. Under one datatype, two
/// typed literals are equal when their values are equal, whatever their
/// spelling, so `"5"`, `" 5 "`, `"+5"` and `"005"` are one `xsd:int`. This
/// holds for the XSD integer types, `xsd:decimal`, `xsd:double`,
/// `xsd:float` and `xsd:boolean`. A literal of any other datatype, or one
/// whose text is ill-formed for its datatype, compares by its text. Values
/// of different datatypes are never equal.
///
/// ```
/// use provkit::{Value, well_known::{XSD_DOUBLE, XSD_INT, XSD_LONG}};
///
/// let int = |text: &str| Value::typed(text, XSD_INT.clone()).unwrap();
/// assert_eq!(int("005"), int(" 5 "));
/// assert_ne!(int("abc"), int("abd"));
/// assert_ne!(int("5"), Value::typed("5", XSD_LONG.clone()).unwrap());
///
/// let double = |text: &str| Value::typed(text, XSD_DOUBLE.clone()).unwrap();
/// assert_eq!(double("1"), double("1.0E0"));
/// ```
#[derive(Clone, Debug)]
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
    /// Build it through [`Value::typed`] or a `From` conversion. The variant
    /// is `#[non_exhaustive]`, so code outside this crate cannot build it
    /// directly, and each value has one representation: a `Typed` never
    /// holds an `xsd:string`, a well-formed `xsd:dateTime` or a well-formed
    /// qualified name. Patterns outside the crate need `..`.
    ///
    /// ```compile_fail,E0639
    /// use provkit::{Value, well_known::XSD_INT};
    ///
    /// let v = Value::Typed { lexical: "1".into(), datatype: XSD_INT.clone() };
    /// ```
    #[non_exhaustive]
    Typed {
        /// The text as written.
        lexical: String,
        /// The datatype.
        datatype: QName,
    },
}

/// The reason [`Value::typed`] gives for a qualified-name datatype.
const QNAME_REASON: &str =
    "a qualified-name value must be resolved against a namespace table; use Value::QName";

impl Value {
    /// A literal with a datatype. `xsd:string` folds to [`Value::Str`], a
    /// well-formed `xsd:dateTime` is parsed, and anything else is kept as
    /// written, whether or not its text is well-formed for the datatype.
    ///
    /// ```
    /// use provkit::{Value, well_known::{XSD_DATETIME, XSD_INT, XSD_QNAME}};
    ///
    /// assert!(Value::typed("1", XSD_INT.clone()).is_ok());
    /// assert!(Value::typed("one", XSD_INT.clone()).is_ok());
    /// assert!(Value::typed("2012-13-01T00:00:00", XSD_DATETIME.clone()).is_ok());
    /// assert!(Value::typed("ex:e", XSD_QNAME.clone()).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLiteral`] if `datatype` is `xsd:QName` or
    /// `prov:QUALIFIED_NAME`. Resolving such a value needs namespaces, so
    /// build a [`Value::QName`] instead.
    pub fn typed(lexical: impl Into<String>, datatype: QName) -> Result<Self> {
        let lexical = lexical.into();
        if datatype == *XSD_STRING {
            Ok(Value::Str(lexical))
        } else if datatype == *XSD_DATETIME {
            Ok(match DateTime::parse(&lexical) {
                Ok(parsed) => Value::DateTime(parsed),
                Err(_) => Value::Typed { lexical, datatype },
            })
        } else if is_qname_datatype(&datatype) {
            let shown = if datatype == *XSD_QNAME {
                "xsd:QName"
            } else {
                "prov:QUALIFIED_NAME"
            };
            Err(Error::InvalidLiteral {
                datatype: shown.into(),
                lexical,
                reason: QNAME_REASON.into(),
            })
        } else {
            Ok(Value::Typed { lexical, datatype })
        }
    }

    /// The rule both readers use for `"lexical" %% datatype` and
    /// `{"$": lexical, "type": datatype}`. Under a qualified-name datatype,
    /// text that spells a qualified name resolves in `names` to a
    /// [`Value::QName`], and any other text is kept as written. Every other
    /// datatype goes through [`Value::typed`].
    // Only the readers call this, and both are feature-gated.
    #[cfg_attr(not(any(feature = "json", feature = "provn")), allow(dead_code))]
    pub(crate) fn typed_in(names: &Namespaces, lexical: &str, datatype: QName) -> Result<Self> {
        if !is_qname_datatype(&datatype) {
            Value::typed(lexical, datatype)
        } else if pn::is_qualified_name(lexical) {
            Ok(Value::QName(names.resolve(lexical)?))
        } else {
            Ok(Value::Typed {
                lexical: lexical.to_owned(),
                datatype,
            })
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

    /// False for a `Typed` that another variant owns: an `xsd:string`, a
    /// well-formed `xsd:dateTime` or a well-formed qualified name. The
    /// writers assert this in debug builds.
    // Only the writers call this, and both are feature-gated.
    #[cfg_attr(not(any(feature = "json", feature = "provn")), allow(dead_code))]
    pub(crate) fn has_one_representation(&self) -> bool {
        let Value::Typed { lexical, datatype } = self else {
            return true;
        };
        let folded = if *datatype == *XSD_STRING {
            true
        } else if *datatype == *XSD_DATETIME {
            DateTime::parse(lexical).is_ok()
        } else {
            is_qname_datatype(datatype) && pn::is_qualified_name(lexical)
        };
        !folded
    }
}

fn is_qname_datatype(datatype: &QName) -> bool {
    *datatype == *XSD_QNAME || *datatype == *PROV_QUALIFIED_NAME
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::LangStr(a), Value::LangStr(b)) => a == b,
            (Value::QName(a), Value::QName(b)) => a == b,
            (Value::DateTime(a), Value::DateTime(b)) => a == b,
            (
                Value::Typed {
                    lexical: a,
                    datatype: s,
                },
                Value::Typed {
                    lexical: b,
                    datatype: t,
                },
            ) => s == t && (a == b || Key::of(a, s) == Key::of(b, t)),
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Value::Str(s) => s.hash(state),
            Value::LangStr(l) => l.hash(state),
            Value::QName(q) => q.hash(state),
            Value::DateTime(d) => d.hash(state),
            Value::Typed { lexical, datatype } => {
                datatype.hash(state);
                Key::of(lexical, datatype).hash(state);
            }
        }
    }
}

/// What a typed literal compares and hashes by, beside its datatype.
#[derive(PartialEq, Eq, Hash)]
enum Key<'a> {
    /// An integer's sign, true when negative, and its digits without leading
    /// zeros. Zero has no digits and is never negative.
    Integer(bool, &'a str),
    /// A decimal's sign, its integer digits without leading zeros and its
    /// fraction digits without trailing zeros.
    Decimal(bool, &'a str, &'a str),
    /// The bits of a double or float, with one NaN and with -0 as +0.
    Float(u64),
    /// A boolean.
    Boolean(bool),
    /// The text itself, for any other datatype and for every ill-formed
    /// lexical form.
    Text(&'a str),
}

impl<'a> Key<'a> {
    /// The key of `lexical` under `datatype`. XSD trims the space, tab, CR
    /// and LF around a number or boolean before reading it.
    fn of(lexical: &'a str, datatype: &QName) -> Self {
        let text = lexical.trim_matches([' ', '\t', '\r', '\n']);
        let integers: [&QName; 13] = [
            &XSD_INT,
            &XSD_LONG,
            &XSD_INTEGER,
            &XSD_SHORT,
            &XSD_BYTE,
            &XSD_NON_NEGATIVE_INTEGER,
            &XSD_POSITIVE_INTEGER,
            &XSD_NON_POSITIVE_INTEGER,
            &XSD_NEGATIVE_INTEGER,
            &XSD_UNSIGNED_LONG,
            &XSD_UNSIGNED_INT,
            &XSD_UNSIGNED_SHORT,
            &XSD_UNSIGNED_BYTE,
        ];
        let key = if integers.contains(&datatype) {
            Key::integer(text)
        } else if *datatype == *XSD_DECIMAL {
            Key::decimal(text)
        } else if *datatype == *XSD_DOUBLE {
            Key::float(text, |number| number.parse().ok())
        } else if *datatype == *XSD_FLOAT {
            Key::float(text, |number| number.parse::<f32>().ok().map(f64::from))
        } else if *datatype == *XSD_BOOLEAN {
            Key::boolean(text)
        } else {
            None
        };
        key.unwrap_or(Key::Text(lexical))
    }

    /// `[+-]?[0-9]+`
    fn integer(text: &'a str) -> Option<Self> {
        let (negative, digits) = split_sign(text);
        if digits.is_empty() || !all_digits(digits) {
            return None;
        }
        let digits = digits.trim_start_matches('0');
        Some(Key::Integer(negative && !digits.is_empty(), digits))
    }

    /// `[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)`
    fn decimal(text: &'a str) -> Option<Self> {
        let (negative, number) = split_sign(text);
        let (whole, fraction) = split_decimal(number)?;
        let whole = whole.trim_start_matches('0');
        let fraction = fraction.trim_end_matches('0');
        let zero = whole.is_empty() && fraction.is_empty();
        Some(Key::Decimal(negative && !zero, whole, fraction))
    }

    /// `[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)([eE][+-]?[0-9]+)?`, or `INF`,
    /// `+INF`, `-INF`, `NaN`. `parse` reads a number that matches the
    /// pattern, which is tested first because Rust's parsers accept more
    /// than XSD does (`inf`, `nan`, `infinity`).
    fn float(text: &str, parse: impl Fn(&str) -> Option<f64>) -> Option<Self> {
        let value = match text {
            "INF" | "+INF" => f64::INFINITY,
            "-INF" => f64::NEG_INFINITY,
            "NaN" => f64::NAN,
            number if is_float_number(number) => parse(number)?,
            _ => return None,
        };
        let bits = if value.is_nan() {
            f64::NAN.to_bits()
        } else if value == 0.0 {
            0.0f64.to_bits()
        } else {
            value.to_bits()
        };
        Some(Key::Float(bits))
    }

    /// `true`, `false`, `1`, `0`
    fn boolean(text: &str) -> Option<Self> {
        match text {
            "true" | "1" => Some(Key::Boolean(true)),
            "false" | "0" => Some(Key::Boolean(false)),
            _ => None,
        }
    }
}

/// Whether `text` starts with `-`, and `text` without its `+` or `-`.
fn split_sign(text: &str) -> (bool, &str) {
    match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    }
}

fn all_digits(text: &str) -> bool {
    text.bytes().all(|b| b.is_ascii_digit())
}

/// The integer and fraction digits of `[0-9]+(\.[0-9]*)?|\.[0-9]+`.
fn split_decimal(number: &str) -> Option<(&str, &str)> {
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let has_digit = !whole.is_empty() || !fraction.is_empty();
    (has_digit && all_digits(whole) && all_digits(fraction)).then_some((whole, fraction))
}

/// `[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)([eE][+-]?[0-9]+)?`
fn is_float_number(text: &str) -> bool {
    let (_, number) = split_sign(text);
    let (mantissa, exponent) = match number.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, Some(split_sign(exponent).1)),
        None => (number, None),
    };
    split_decimal(mantissa).is_some()
        && exponent.is_none_or(|digits| !digits.is_empty() && all_digits(digits))
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
    use crate::test_util::hash;

    fn typed(lexical: &str, datatype: &QName) -> Value {
        Value::typed(lexical, datatype.clone()).unwrap()
    }

    fn integer_types() -> [&'static QName; 13] {
        [
            &XSD_INTEGER,
            &XSD_LONG,
            &XSD_INT,
            &XSD_SHORT,
            &XSD_BYTE,
            &XSD_NON_NEGATIVE_INTEGER,
            &XSD_POSITIVE_INTEGER,
            &XSD_NON_POSITIVE_INTEGER,
            &XSD_NEGATIVE_INTEGER,
            &XSD_UNSIGNED_LONG,
            &XSD_UNSIGNED_INT,
            &XSD_UNSIGNED_SHORT,
            &XSD_UNSIGNED_BYTE,
        ]
    }

    /// Every form in `forms` is one value of `datatype`: equal and hashing alike.
    fn assert_one_value(datatype: &QName, forms: &[&str]) {
        let first = typed(forms[0], datatype);
        for form in forms {
            let other = typed(form, datatype);
            assert_eq!(first, other, "{:?} and {form:?} as {datatype}", forms[0]);
            assert_eq!(
                hash(&first),
                hash(&other),
                "{:?} and {form:?} as {datatype}",
                forms[0]
            );
        }
    }

    /// No two forms in `forms` are one value of `datatype`.
    fn assert_distinct_values(datatype: &QName, forms: &[&str]) {
        for (i, a) in forms.iter().enumerate() {
            for b in &forms[i + 1..] {
                assert_ne!(
                    typed(a, datatype),
                    typed(b, datatype),
                    "{a:?} and {b:?} as {datatype}"
                );
            }
        }
    }

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
        assert_eq!(
            Value::typed("1", XSD_INT.clone()).unwrap(),
            Value::typed("1", XSD_INT.clone()).unwrap()
        );
    }

    #[test]
    fn ill_formed_literals_are_kept() {
        for (lexical, datatype) in [
            ("abc", &*XSD_INT),
            ("maybe", &*XSD_BOOLEAN),
            ("2012-13-01T00:00:00", &*XSD_DATETIME),
            ("2012-01-01", &*XSD_DATETIME),
        ] {
            let value = Value::typed(lexical, datatype.clone()).unwrap();
            assert!(
                matches!(&value, Value::Typed { lexical: l, .. } if l == lexical),
                "{lexical}"
            );
            assert_eq!(value.datatype().as_ref(), Some(datatype));
        }
    }

    #[test]
    fn integers_compare_by_value_under_one_datatype() {
        let long = "1234567890123456789012345678901234567890";
        for datatype in integer_types() {
            assert_one_value(datatype, &[" 5 ", "5", "+5", "005", "\t5\r\n", "+0005"]);
            assert_one_value(datatype, &["0", "-0", "+0", "000", "-000"]);
            assert_one_value(datatype, &["-5", "-05", " -5"]);
            // Forty digits compare exactly, and no range is checked.
            assert_one_value(datatype, &[long, &format!("+000{long}")]);
            assert_distinct_values(
                datatype,
                &[
                    "5",
                    "-5",
                    "6",
                    "50",
                    "0",
                    long,
                    &format!("{}1", &long[..39]),
                    &format!("-{long}"),
                ],
            );
        }
    }

    #[test]
    fn decimals_compare_by_value() {
        let dt = &*XSD_DECIMAL;
        assert_one_value(dt, &["1.50", "1.5", "+1.5", "01.5", " 1.500 "]);
        assert_one_value(dt, &["5", "5.0", "5.", "+05.00"]);
        assert_one_value(dt, &["0", "-0.0", "+0", "0.", ".0", "-.000", "000"]);
        assert_one_value(dt, &[".5", "0.5", "+.50", "00.500"]);
        assert_one_value(dt, &["-1.5", "-01.50"]);
        let long = "1234567890123456789012345678901234567890";
        assert_one_value(
            dt,
            &[&format!("{long}.{long}"), &format!("0{long}.{long}0")],
        );
        assert_distinct_values(
            dt,
            &[
                "1.5",
                "-1.5",
                "1.05",
                "15",
                "0.15",
                "0",
                &format!("{long}.{long}"),
                &format!("{long}.{long}1"),
            ],
        );
    }

    #[test]
    fn doubles_and_floats_compare_by_value() {
        for dt in [&*XSD_DOUBLE, &*XSD_FLOAT] {
            assert_one_value(
                dt,
                &[
                    "1", "1.0", "1e0", "1E0", "+1", "1.", "1.e0", "10e-1", ".1e1", " 1 ", "0.01E+2",
                ],
            );
            assert_one_value(dt, &["NaN", "NaN", " NaN\n"]);
            assert_one_value(dt, &["0", "-0", "0.0", "-0.0", "+0", "-.0e5", "0E0"]);
            // An exponent too large for the type rounds to infinity.
            assert_one_value(dt, &["INF", "+INF", "1e999", " INF "]);
            assert_one_value(dt, &["-INF", "-1e999"]);
            assert_one_value(dt, &[".5", "0.5", "5e-1", "5.E-1"]);
            assert_distinct_values(dt, &["1", "2", "-1", "0", "NaN", "INF", "-INF", "1.5"]);
        }
    }

    #[test]
    fn floats_compare_in_single_precision() {
        // Both round to one `f32` and to two `f64`s.
        let forms = ["0.1", "0.100000001"];
        assert_one_value(&XSD_FLOAT, &forms);
        assert_distinct_values(&XSD_DOUBLE, &forms);
        assert_one_value(&XSD_FLOAT, &["INF", "1e39"]);
        assert_distinct_values(&XSD_DOUBLE, &["INF", "1e39"]);
    }

    #[test]
    fn spellings_outside_the_xsd_pattern_are_not_numbers() {
        // Rust's float parser reads all of these; XSD reads none.
        let rust_only = [
            "inf", "+inf", "-inf", "infinity", "Infinity", "INFINITY", "nan", "NAN",
        ];
        for dt in [&*XSD_DOUBLE, &*XSD_FLOAT] {
            assert_distinct_values(
                dt,
                &["INF", "inf", "+inf", "infinity", "Infinity", "INFINITY"],
            );
            assert_distinct_values(dt, &["-INF", "-inf", "-infinity"]);
            assert_distinct_values(dt, &["NaN", "nan", "NAN", "+NaN", "-NaN"]);
            assert_distinct_values(
                dt,
                &["1", "1e", "1e+", "+", ".", "e1", "", " ", "1 0", "0x1"],
            );
            for form in rust_only {
                assert_eq!(typed(form, dt), typed(form, dt));
                assert_ne!(typed(form, dt), typed(&format!(" {form}"), dt), "{form:?}");
            }
        }
        for dt in integer_types() {
            assert_distinct_values(dt, &["5", "5.0", "5e0", "+", "-", "", "5 5", "٥", "0x5"]);
            // Only space, tab, CR and LF are trimmed.
            assert_distinct_values(dt, &["5", "\u{A0}5", "5\u{2003}", "\u{C}5", "5\u{B}"]);
        }
        assert_distinct_values(&XSD_DECIMAL, &["1", "1e0", ".", "+.", "1.0.0", "", "1,0"]);
    }

    #[test]
    fn booleans_compare_by_value() {
        let dt = &*XSD_BOOLEAN;
        assert_one_value(dt, &["true", "1", " true ", "\t1"]);
        assert_one_value(dt, &["false", "0", "0\n"]);
        assert_distinct_values(dt, &["true", "false", "TRUE", "True", "yes", "01", "maybe"]);
    }

    #[test]
    fn ill_formed_text_compares_untrimmed() {
        assert_one_value(&XSD_INT, &["abc", "abc"]);
        assert_distinct_values(&XSD_INT, &["abc", "abd", " abc", "abc "]);
        assert_distinct_values(
            &XSD_DATETIME,
            &["2012-13-01T00:00:00", "2012-13-01T00:00:00 "],
        );
        // Other datatypes have no value space here, so text is compared as written.
        let any_uri = QName::new(crate::well_known::XSD, "anyURI");
        assert_distinct_values(&any_uri, &["5", " 5 ", "05"]);
    }

    #[test]
    fn values_compare_within_one_datatype_only() {
        assert_ne!(typed("5", &XSD_INT), typed("5", &XSD_LONG));
        assert_ne!(typed("5", &XSD_INT), typed("5", &XSD_INTEGER));
        assert_ne!(typed("abc", &XSD_INT), typed("abc", &XSD_DOUBLE));
        assert_ne!(typed("1", &XSD_DOUBLE), typed("1", &XSD_FLOAT));
        assert_ne!(typed("1", &XSD_DECIMAL), typed("1", &XSD_DOUBLE));
        assert_ne!(typed("1", &XSD_BOOLEAN), typed("1", &XSD_INT));
        assert_ne!(typed("5", &XSD_INT), Value::from("5"));
        assert_ne!(
            typed("nope", &XSD_DATETIME),
            typed("2011-11-16T16:05:33Z", &XSD_DATETIME)
        );
        // The datatype is compared as a name: by its IRI, however it is split.
        let split = QName::new("http://www.w3.org/2001/", "XMLSchema#int");
        assert_one_value(&split, &["5", " 005 "]);
        assert_eq!(typed("+5", &split), typed("5", &XSD_INT));
        assert_eq!(hash(&typed("+5", &split)), hash(&typed("5", &XSD_INT)));
    }

    #[test]
    fn each_value_keeps_its_text() {
        for (lexical, datatype) in [
            (" 5 ", &*XSD_INT),
            ("005", &*XSD_INT),
            ("1.50", &*XSD_DECIMAL),
            ("1E0", &*XSD_DOUBLE),
            ("1", &*XSD_BOOLEAN),
        ] {
            assert!(
                matches!(typed(lexical, datatype), Value::Typed { lexical: l, .. } if l == lexical)
            );
        }
    }

    #[test]
    fn typed_rejects_qualified_name_datatypes() {
        for (datatype, spelling) in [
            (XSD_QNAME.clone(), "xsd:QName"),
            (PROV_QUALIFIED_NAME.clone(), "prov:QUALIFIED_NAME"),
        ] {
            for text in ["ex:foo", "not a name"] {
                match Value::typed(text, datatype.clone()) {
                    Err(Error::InvalidLiteral {
                        datatype: shown,
                        lexical,
                        reason,
                    }) => {
                        assert_eq!(shown, spelling);
                        assert_eq!(lexical, text);
                        assert_eq!(
                            reason,
                            "a qualified-name value must be resolved against a namespace table; \
                             use Value::QName"
                        );
                    }
                    other => panic!("expected InvalidLiteral, got {other:?}"),
                }
            }
        }
    }

    fn example_names() -> Namespaces {
        let mut ns = Namespaces::new();
        ns.declare("ex", "http://example.org/").unwrap();
        ns
    }

    #[test]
    fn typed_in_resolves_qualified_names_and_delegates_the_rest() {
        let ns = example_names();
        let foo = QName::new("http://example.org/", "foo");
        for datatype in [XSD_QNAME.clone(), PROV_QUALIFIED_NAME.clone()] {
            assert_eq!(
                Value::typed_in(&ns, "ex:foo", datatype).unwrap(),
                Value::QName(foo.clone())
            );
        }
        assert_eq!(
            Value::typed_in(&ns, "x", XSD_STRING.clone()).unwrap(),
            Value::Str("x".into())
        );
        assert_eq!(
            Value::typed_in(&ns, "1", XSD_INT.clone()).unwrap(),
            Value::from(1i32)
        );
        assert!(matches!(
            Value::typed_in(&ns, "nope", XSD_DATETIME.clone()).unwrap(),
            Value::Typed { lexical, .. } if lexical == "nope"
        ));
    }

    #[test]
    fn typed_in_keeps_text_that_is_not_a_qualified_name() {
        let mut with_default = example_names();
        with_default.set_default("http://default.org/");
        for ns in [example_names(), with_default] {
            for datatype in [XSD_QNAME.clone(), PROV_QUALIFIED_NAME.clone()] {
                for text in [
                    "not a name",
                    "",
                    ":foo",
                    "_:foo",
                    "1x:foo",
                    "ex:a b",
                    "ex:a\\.b",
                    "ex:a%",
                    "ex:<a>",
                    "ex:\u{301}a",
                ] {
                    let value = Value::typed_in(&ns, text, datatype.clone()).unwrap();
                    assert!(
                        matches!(&value, Value::Typed { lexical, datatype: d }
                            if lexical == text && *d == datatype),
                        "{text:?}: {value:?}"
                    );
                    assert!(value.has_one_representation());
                }
            }
        }
    }

    #[test]
    fn a_qualified_name_value_must_resolve() {
        let ns = example_names();
        for datatype in [XSD_QNAME.clone(), PROV_QUALIFIED_NAME.clone()] {
            assert!(matches!(
                Value::typed_in(&ns, "zz:foo", datatype.clone()),
                Err(Error::UndeclaredPrefix(p)) if p == "zz"
            ));
            assert!(matches!(
                Value::typed_in(&ns, "foo", datatype),
                Err(Error::NoDefaultNamespace(l)) if l == "foo"
            ));
        }
        // The text spells the name as the model holds it, without escapes.
        let name = |local: &str| Value::QName(QName::new("http://example.org/", local));
        for local in ["a=b", "a:b", "a.b.", "-a", "a(1)", "a%2Fb", "12", ""] {
            assert_eq!(
                Value::typed_in(&ns, &format!("ex:{local}"), XSD_QNAME.clone()).unwrap(),
                name(local),
                "{local:?}"
            );
        }
    }

    #[test]
    fn every_constructor_keeps_one_representation() {
        let ns = example_names();
        let values = [
            Value::from("s"),
            Value::from(1i32),
            Value::from(f64::NAN),
            Value::typed("x", XSD_STRING.clone()).unwrap(),
            Value::typed("2011-11-16T16:05:33Z", XSD_DATETIME.clone()).unwrap(),
            Value::typed("nope", XSD_DATETIME.clone()).unwrap(),
            Value::typed("x", QName::new("http://example.org/", "dt")).unwrap(),
            Value::typed_in(&ns, "ex:foo", XSD_QNAME.clone()).unwrap(),
            Value::typed_in(&ns, "not a name", XSD_QNAME.clone()).unwrap(),
        ];
        assert!(values.iter().all(Value::has_one_representation));
        // The shapes no constructor builds.
        let typed = |lexical: &str, datatype: &QName| Value::Typed {
            lexical: lexical.to_owned(),
            datatype: datatype.clone(),
        };
        for folded in [
            typed("x", &XSD_STRING),
            typed("2011-11-16T16:05:33Z", &XSD_DATETIME),
            typed("ex:foo", &XSD_QNAME),
            typed("foo", &PROV_QUALIFIED_NAME),
        ] {
            assert!(!folded.has_one_representation(), "{folded:?}");
        }
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
        assert_eq!(a.value(), "hi");
        assert_eq!(b.lang(), "EN-gb");
    }

    #[test]
    fn lang_tags_follow_the_langtag_shape() {
        for tag in [
            "en",
            "a",
            "EN",
            "en-GB",
            "zh-Hant-TW",
            "en-1",
            "x-1-2",
            "abcdefghi",
            "en-GB-x-averylongprivatesubtag01",
        ] {
            let tagged = LangStr::new("hi", tag).unwrap();
            assert_eq!(tagged.lang(), tag);
        }
        for tag in [
            "", "en_US", "en-", "-en", "en--GB", "en-GB-", "1a", "1", "é", "en-é", "en GB", " en",
            "@en", "-",
        ] {
            assert!(
                matches!(LangStr::new("hi", tag), Err(Error::InvalidLanguageTag(t)) if t == tag),
                "{tag:?}"
            );
        }
    }

    #[test]
    fn native_conversions_are_exact() {
        assert_eq!(
            Value::from(1i32),
            Value::typed("1", XSD_INT.clone()).unwrap()
        );
        assert_eq!(
            Value::from(1i64),
            Value::typed("1", XSD_LONG.clone()).unwrap()
        );
        assert_eq!(
            Value::from(1.5f64),
            Value::typed("1.5", XSD_DOUBLE.clone()).unwrap()
        );
        assert_eq!(
            Value::from(1.5f32),
            Value::typed("1.5", XSD_FLOAT.clone()).unwrap()
        );
        assert_eq!(
            Value::from(true),
            Value::typed("true", XSD_BOOLEAN.clone()).unwrap()
        );
        assert_eq!(Value::from("s"), Value::Str("s".into()));
    }

    #[test]
    fn native_floats_equal_their_xsd_spellings() {
        // `to_string` writes a float without an exponent, and the key reads it back.
        for v in [
            0.0,
            -0.0,
            1.0,
            1e300,
            1e-300,
            f64::MAX,
            f64::MIN_POSITIVE,
            0.1,
        ] {
            assert_eq!(Value::from(v), typed(&format!("{v:e}"), &XSD_DOUBLE), "{v}");
        }
        for v in [0.0f32, 1.0, 1e30, f32::MAX, f32::MIN_POSITIVE, 0.1] {
            assert_eq!(Value::from(v), typed(&format!("{v:e}"), &XSD_FLOAT), "{v}");
        }
        assert_eq!(Value::from(f64::INFINITY), typed("+INF", &XSD_DOUBLE));
        assert_eq!(Value::from(f64::NAN), Value::from(-f64::NAN));
        assert_eq!(hash(&Value::from(f64::NAN)), hash(&Value::from(-f64::NAN)));
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
