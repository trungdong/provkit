//! Attribute values.

use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

use crate::namespaces::Scope;
use crate::well_known::{
    PROV_QUALIFIED_NAME, XSD_BOOLEAN, XSD_BYTE, XSD_DATETIME, XSD_DECIMAL, XSD_DOUBLE, XSD_FLOAT,
    XSD_INT, XSD_INTEGER, XSD_LONG, XSD_NEGATIVE_INTEGER, XSD_NON_NEGATIVE_INTEGER,
    XSD_NON_POSITIVE_INTEGER, XSD_POSITIVE_INTEGER, XSD_QNAME, XSD_SHORT, XSD_STRING,
    XSD_UNSIGNED_BYTE, XSD_UNSIGNED_INT, XSD_UNSIGNED_LONG, XSD_UNSIGNED_SHORT,
};
use crate::{DateTime, Error, QName, Result};

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

/// A typed literal kept as written: its text and its datatype.
///
/// The fields are private, so each value has one representation. Build one
/// through [`Value::typed`] or a `From` conversion; a `TypedLiteral` never
/// holds an `xsd:string`, a well-formed `xsd:dateTime` or a qualified-name
/// datatype.
///
/// ```compile_fail,E0451
/// use provkit::{TypedLiteral, well_known::XSD_INT};
///
/// let literal = TypedLiteral { lexical: "1".into(), datatype: XSD_INT.clone() };
/// ```
///
/// ```compile_fail,E0616
/// use provkit::{Value, well_known::XSD_STRING};
///
/// let mut value = Value::from(5i32);
/// if let Value::Typed(literal) = &mut value {
///     literal.datatype = XSD_STRING.clone();
/// }
/// ```
#[derive(Clone, Debug)]
pub struct TypedLiteral {
    lexical: String,
    datatype: QName,
}

impl TypedLiteral {
    /// The text as written.
    #[must_use]
    pub fn lexical(&self) -> &str {
        &self.lexical
    }

    /// The datatype.
    #[must_use]
    pub fn datatype(&self) -> &QName {
        &self.datatype
    }
}

impl PartialEq for TypedLiteral {
    fn eq(&self, other: &Self) -> bool {
        self.datatype == other.datatype
            && (self.lexical == other.lexical
                || Key::of(&self.lexical, &self.datatype)
                    == Key::of(&other.lexical, &other.datatype))
    }
}

impl Eq for TypedLiteral {}

impl Hash for TypedLiteral {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.datatype.hash(state);
        Key::of(&self.lexical, &self.datatype).hash(state);
    }
}

/// The value of a non-formal attribute.
///
/// Values compare and hash by what they denote. Under one datatype, two
/// typed literals are equal when their values are equal, whatever their
/// spelling, so `"5"`, `" 5 "`, `"+5"` and `"005"` are one `xsd:int`. This
/// holds for the XSD integer types, `xsd:decimal`, `xsd:double`,
/// `xsd:float` and `xsd:boolean`. A literal of any other datatype, or one
/// whose text is outside its datatype's lexical space (`"abc"` or `"300"`
/// as `xsd:byte`), compares by its text. Values of different datatypes are
/// never equal.
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
    /// Any other typed literal, kept as written. See [`TypedLiteral`].
    Typed(TypedLiteral),
}

/// The XSD integer datatypes with their inclusive bounds as decimal digits.
/// `None` is unbounded on that side. Their literals compare by integer value.
static INTEGER_TYPES: [(&LazyLock<QName>, Option<&str>, Option<&str>); 13] = [
    (&XSD_INT, Some("-2147483648"), Some("2147483647")),
    (
        &XSD_LONG,
        Some("-9223372036854775808"),
        Some("9223372036854775807"),
    ),
    (&XSD_INTEGER, None, None),
    (&XSD_SHORT, Some("-32768"), Some("32767")),
    (&XSD_BYTE, Some("-128"), Some("127")),
    (&XSD_NON_NEGATIVE_INTEGER, Some("0"), None),
    (&XSD_POSITIVE_INTEGER, Some("1"), None),
    (&XSD_NON_POSITIVE_INTEGER, None, Some("0")),
    (&XSD_NEGATIVE_INTEGER, None, Some("-1")),
    (&XSD_UNSIGNED_LONG, Some("0"), Some("18446744073709551615")),
    (&XSD_UNSIGNED_INT, Some("0"), Some("4294967295")),
    (&XSD_UNSIGNED_SHORT, Some("0"), Some("65535")),
    (&XSD_UNSIGNED_BYTE, Some("0"), Some("255")),
];

/// The reason [`Value::typed`] gives for a qualified-name datatype.
const QNAME_REASON: &str =
    "a qualified-name value must be resolved against a namespace table; use Value::QName";

impl Value {
    /// A literal with a datatype. `xsd:string` folds to [`Value::Str`], a
    /// well-formed `xsd:dateTime` is parsed, and anything else is kept as
    /// written, whether or not its text is well-formed for the datatype. The
    /// space, tab, CR and LF around a dateTime are trimmed before it is
    /// parsed, and the parsed value drops them.
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
            Ok(match DateTime::parse(trim_xsd_space(&lexical)) {
                Ok(parsed) => Value::DateTime(parsed),
                Err(_) => Value::literal(lexical, datatype),
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
            Ok(Value::literal(lexical, datatype))
        }
    }

    /// A typed literal as given; no caller passes a datatype another variant owns.
    fn literal(lexical: String, datatype: QName) -> Value {
        Value::Typed(TypedLiteral { lexical, datatype })
    }

    /// An integer written as digits, with an optional sign: `xsd:int`,
    /// `xsd:long` or `xsd:integer` by range, keeping the text. The range
    /// follows the digits alone, so leading zeros do not change the type and
    /// a number beyond `i64` is an `xsd:integer`.
    // The readers and the PROV-N writer call this, all feature-gated.
    #[cfg_attr(not(any(feature = "json", feature = "provn")), allow(dead_code))]
    pub(crate) fn integer(lexical: &str) -> Value {
        let datatype = match lexical.parse::<i64>() {
            Ok(n) if i32::try_from(n).is_ok() => XSD_INT.clone(),
            Ok(_) => XSD_LONG.clone(),
            Err(_) => XSD_INTEGER.clone(),
        };
        Value::literal(lexical.to_owned(), datatype)
    }

    /// The rule both readers use for `"lexical" %% datatype` and
    /// `{"$": lexical, "type": datatype}`. Under a qualified-name datatype
    /// the text resolves in `names` to a [`Value::QName`]. Every other
    /// datatype goes through [`Value::typed`].
    ///
    /// # Errors
    ///
    /// As [`Namespaces::resolve`](crate::Namespaces::resolve) for a
    /// qualified-name datatype.
    // Only the readers call this, and both are feature-gated.
    #[cfg_attr(not(any(feature = "json", feature = "provn")), allow(dead_code))]
    pub(crate) fn typed_in(names: Scope<'_>, lexical: &str, datatype: QName) -> Result<Self> {
        if is_qname_datatype(&datatype) {
            names.resolve(lexical).map(Value::QName)
        } else {
            Value::typed(lexical, datatype)
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
            Value::Typed(t) => Some(t.datatype.clone()),
        }
    }
}

/// `lexical` without the space, tab, CR and LF that XSD collapses around a
/// number, a boolean or a dateTime.
pub(crate) fn trim_xsd_space(lexical: &str) -> &str {
    lexical.trim_matches([' ', '\t', '\r', '\n'])
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
            (Value::Typed(a), Value::Typed(b)) => a == b,
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
            Value::Typed(t) => t.hash(state),
        }
    }
}

/// What a typed literal compares and hashes by, beside its datatype.
#[derive(Debug, PartialEq, Eq, Hash)]
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
        let text = trim_xsd_space(lexical);
        let key = if let Some(&(_, min, max)) =
            INTEGER_TYPES.iter().find(|(t, _, _)| ***t == *datatype)
        {
            Key::integer(text, min, max)
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

    /// `[+-]?[0-9]+` within `min` and `max`. Text outside the range is not
    /// in the datatype's lexical space, so it has no integer key.
    fn integer(text: &'a str, min: Option<&str>, max: Option<&str>) -> Option<Self> {
        let (negative, digits) = split_sign(text);
        if digits.is_empty() || !all_digits(digits) {
            return None;
        }
        let digits = digits.trim_start_matches('0');
        let negative = negative && !digits.is_empty();
        let below = min.is_some_and(|min| compare_integers((negative, digits), parts(min)).is_lt());
        let above = max.is_some_and(|max| compare_integers((negative, digits), parts(max)).is_gt());
        (!below && !above).then_some(Key::Integer(negative, digits))
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

/// The sign and the digits without leading zeros of a bound from
/// `INTEGER_TYPES`. Zero has no digits and is never negative.
fn parts(bound: &str) -> (bool, &str) {
    let (negative, digits) = split_sign(bound);
    let digits = digits.trim_start_matches('0');
    (negative && !digits.is_empty(), digits)
}

/// Order two integers given as a sign and digits without leading zeros.
/// More digits is the larger magnitude; equal lengths compare as text.
fn compare_integers(a: (bool, &str), b: (bool, &str)) -> std::cmp::Ordering {
    let magnitude = |x: &str, y: &str| x.len().cmp(&y.len()).then_with(|| x.cmp(y));
    match (a.0, b.0) {
        (false, false) => magnitude(a.1, b.1),
        (true, true) => magnitude(b.1, a.1),
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
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
                    Value::literal(v.to_string(), $dt.clone())
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
                    Value::literal(lexical, $dt.clone())
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
mod tests;
