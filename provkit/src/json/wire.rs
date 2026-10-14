//! The PROV-JSON shape, as serde sees it. Private; the model never sees these types.

use indexmap::IndexMap;
use indexmap::map::Entry;
use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;

use super::{char_column, without_position};
use crate::Kind;
use crate::error::clip;

/// The white space JSON allows before a value.
pub(super) const JSON_SPACE: [char; 4] = [' ', '\t', '\r', '\n'];

/// The `prefix` key that holds the default namespace.
pub(super) const DEFAULT: &str = "default";

/// A record's attributes, key order kept.
pub(super) type JsonRecord = IndexMap<String, JsonValue>;
/// Records of one kind, keyed by identifier.
pub(super) type JsonSection = IndexMap<String, OneOrMany<JsonRecord>>;

pub(super) struct JsonDocument {
    pub prefix: IndexMap<String, String>,
    pub sections: IndexMap<Kind, JsonSection>,
    /// Every bundle under its key, in order, up to the first key given
    /// twice.
    pub bundle: Vec<(String, JsonBundle)>,
    /// The first bundle key given twice, whose value was skipped unread.
    pub repeated_bundle: Option<String>,
}

pub(super) struct JsonBundle {
    pub prefix: IndexMap<String, String>,
    pub sections: IndexMap<Kind, JsonSection>,
}

/// The message for a key given twice in one object.
fn duplicate(key: &str) -> String {
    format!("duplicate field `{}`", clip(key))
}

/// Reads the keys of a document or bundle object. `prefix` is the
/// namespace table, `bundle` (documents only) the bundles, and every other
/// key a section. A repeated key is an error.
///
/// A key that is refused has its value skipped first, so text that is not
/// JSON is reported as that. The refusal then sits at the end of the value.
struct ContainerVisitor {
    expecting: &'static str,
    bundles: bool,
}

/// A document or bundle as read, before it is split into the two types.
type Container = (
    IndexMap<String, String>,
    IndexMap<Kind, JsonSection>,
    Bundles,
);

impl<'de> Visitor<'de> for ContainerVisitor {
    type Value = Container;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.expecting)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Container, A::Error> {
        let mut prefix = None;
        let mut sections = IndexMap::new();
        let mut bundle = None;
        while let Some(key) = map.next_key::<String>()? {
            let refused = match (key.as_str(), Kind::from_name(&key)) {
                ("prefix", _) if prefix.is_none() => {
                    prefix = Some(map.next_value::<Unique<String>>()?.0);
                    continue;
                }
                ("bundle", _) if self.bundles && bundle.is_none() => {
                    bundle = Some(map.next_value::<Bundles>()?);
                    continue;
                }
                ("prefix", _) => de::Error::duplicate_field("prefix"),
                ("bundle", _) if self.bundles => de::Error::duplicate_field("bundle"),
                ("bundle", _) => de::Error::custom("a bundle cannot hold a bundle"),
                (_, Some(kind)) if !sections.contains_key(&kind) => {
                    sections.insert(kind, map.next_value::<Unique<_>>()?.0);
                    continue;
                }
                (_, Some(kind)) => de::Error::duplicate_field(kind.name()),
                (name, None) => de::Error::custom(format!("unknown record kind `{}`", clip(name))),
            };
            map.next_value::<IgnoredAny>()?;
            return Err(refused);
        }
        Ok((
            prefix.unwrap_or_default(),
            sections,
            bundle.unwrap_or_default(),
        ))
    }
}

impl<'de> Deserialize<'de> for JsonDocument {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (prefix, sections, bundle) = d.deserialize_map(ContainerVisitor {
            expecting: "a PROV-JSON document, which must be a JSON object",
            bundles: true,
        })?;
        Ok(JsonDocument {
            prefix,
            sections,
            bundle: bundle.list,
            repeated_bundle: bundle.repeated,
        })
    }
}

impl<'de> Deserialize<'de> for JsonBundle {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (prefix, sections, _) = d.deserialize_map(ContainerVisitor {
            expecting: "a bundle, which must be a JSON object",
            bundles: false,
        })?;
        Ok(JsonBundle { prefix, sections })
    }
}

/// `prefix` first when there is one, then the sections, then `bundle`
/// when there are any.
fn serialize_container<S: Serializer>(
    s: S,
    prefix: &IndexMap<String, String>,
    sections: &IndexMap<Kind, JsonSection>,
    bundle: Option<&[(String, JsonBundle)]>,
) -> Result<S::Ok, S::Error> {
    let mut m = s.serialize_map(None)?;
    if !prefix.is_empty() {
        m.serialize_entry("prefix", prefix)?;
    }
    for (kind, section) in sections {
        m.serialize_entry(kind.name(), section)?;
    }
    if let Some(bundle) = bundle.filter(|b| !b.is_empty()) {
        m.serialize_entry("bundle", &BundlesOut(bundle))?;
    }
    m.end()
}

/// The bundles, written as one JSON object.
struct BundlesOut<'a>(&'a [(String, JsonBundle)]);

impl Serialize for BundlesOut<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.0.iter().map(|(key, bundle)| (key, bundle)))
    }
}

/// An object read into a map that keeps key order and refuses a key given
/// twice.
struct Unique<V>(IndexMap<String, V>);

impl<'de, V: Deserialize<'de>> Deserialize<'de> for Unique<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visit<V>(PhantomData<V>);
        impl<'de, V: Deserialize<'de>> Visitor<'de> for Visit<V> {
            type Value = Unique<V>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique<V>, A::Error> {
                let mut out = IndexMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    match out.entry(key) {
                        Entry::Vacant(entry) => {
                            entry.insert(map.next_value()?);
                        }
                        Entry::Occupied(entry) => {
                            map.next_value::<IgnoredAny>()?;
                            return Err(de::Error::custom(duplicate(entry.key())));
                        }
                    }
                }
                Ok(Unique(out))
            }
        }
        d.deserialize_map(Visit(PhantomData))
    }
}

/// The `bundle` object of a document, every entry kept in order up to the
/// first key given twice. That key's value and the rest of the object are
/// skipped, so a fault inside them does not stand in front of the repeat,
/// which the reader reports in its place among the bundle keys.
#[derive(Default)]
struct Bundles {
    list: Vec<(String, JsonBundle)>,
    repeated: Option<String>,
}

impl<'de> Deserialize<'de> for Bundles {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Bundles;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Bundles, A::Error> {
                let mut list = Vec::new();
                let mut seen = HashSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !seen.insert(key.clone()) {
                        // The parser refuses an object left half read.
                        map.next_value::<IgnoredAny>()?;
                        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                        return Ok(Bundles {
                            list,
                            repeated: Some(key),
                        });
                    }
                    list.push((key, map.next_value()?));
                }
                Ok(Bundles {
                    list,
                    repeated: None,
                })
            }
        }
        d.deserialize_map(V)
    }
}

impl Serialize for JsonDocument {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_container(s, &self.prefix, &self.sections, Some(&self.bundle[..]))
    }
}

impl Serialize for JsonBundle {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_container(s, &self.prefix, &self.sections, None)
    }
}

/// PROV-JSON lists several records under one identifier as an array.
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

/// An object is one record and an array is many. Dispatching on the JSON
/// shape, rather than trying each variant, keeps the inner error.
impl<'de> Deserialize<'de> for OneOrMany<JsonRecord> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = OneOrMany<JsonRecord>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a record object or an array of record objects")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Body.visit_map(map).map(OneOrMany::One)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut records = Vec::new();
                while let Some(record) = seq.next_element_seed(Body)? {
                    records.push(record);
                }
                Ok(OneOrMany::Many(records))
            }
        }
        d.deserialize_any(V)
    }
}

/// Reads a record object into its attributes.
///
/// Each value is captured as raw JSON text first. A number is kept as that
/// text, because the visitor interface hands it over as `i64`, `u64` or
/// `f64` (or, when a dependant enables the `arbitrary_precision` feature of
/// `serde_json`, as a private map), each of which loses the spelling.
///
/// The capture borrows the value's text from the input without recursing
/// into it. An array's elements are captured the same way, and one that is
/// itself an array is refused before it is parsed, so the depth of the input
/// never becomes depth of recursion.
///
/// A value that cannot be read is [`JsonValue::Invalid`], not an error: an
/// error raised here would be placed where the outer parser stands, past
/// the whole value, and would not name its record. The reader reports the
/// first invalid value of a record, so the first one in the text is the
/// first in the map. A key given twice, in a record that holds no invalid
/// value yet, takes the place of the key's value as an invalid value, which
/// is then the only one in the map. In a record that already holds one, a
/// repeat changes nothing, because the earlier error is reported.
struct Body;

impl<'de> DeserializeSeed<'de> for Body {
    type Value = JsonRecord;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<JsonRecord, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Body {
    type Value = JsonRecord;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a map")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsonRecord, A::Error> {
        let mut body = JsonRecord::new();
        let mut invalid = false;
        while let Some(key) = map.next_key::<String>()? {
            let text = map.next_value::<&RawValue>()?.get();
            match body.entry(key) {
                Entry::Vacant(entry) => {
                    let value = from_raw(text);
                    invalid |= matches!(value, JsonValue::Invalid(_));
                    entry.insert(value);
                }
                Entry::Occupied(_) if invalid => {}
                Entry::Occupied(mut entry) => {
                    let message = duplicate(entry.key());
                    entry.insert(JsonValue::Invalid(InvalidValue::after(text, message)));
                    invalid = true;
                }
            }
        }
        Ok(body)
    }
}

impl<T> OneOrMany<T> {
    /// The records as a slice, one or many.
    pub fn as_slice(&self) -> &[T] {
        match self {
            OneOrMany::One(t) => std::slice::from_ref(t),
            OneOrMany::Many(v) => v,
        }
    }
}

/// A JSON attribute value, one of the schema's `attributeValues` forms.
pub(super) enum JsonValue {
    Str(String),
    /// A JSON number, as its token text.
    Number(String),
    Bool(bool),
    Typed {
        value: String,
        datatype: Option<String>,
        lang: Option<String>,
    },
    /// An array of values. An element is never itself an array.
    Many(Vec<JsonValue>),
    /// A value the wire layer could not read. The reader reports it, with
    /// its position in the document and the record it belongs to.
    Invalid(InvalidValue),
}

/// Why a value could not be read, and where.
pub(super) struct InvalidValue {
    /// What was wrong, without a position.
    pub message: String,
    /// Where it was found.
    pub at: Place,
}

/// Where in the input an invalid value was found.
pub(super) enum Place {
    /// An error raised while parsing a fragment of the input: the address
    /// of the fragment's first byte, to find it in the input, and the line
    /// and column, in characters, of the error inside the fragment.
    Inside {
        start: usize,
        line: usize,
        column: usize,
    },
    /// An error raised once a value has been read: the address of the byte
    /// after the value.
    After(usize),
}

impl InvalidValue {
    /// The error `e` raised while parsing `fragment`, with its column
    /// counted in characters of the fragment's line.
    fn from_error(fragment: &str, e: &serde_json::Error) -> Self {
        InvalidValue {
            message: without_position(e),
            at: Place::Inside {
                start: fragment.as_ptr().addr(),
                line: e.line(),
                column: char_column(fragment, e.line(), e.column()),
            },
        }
    }

    /// An error at the first byte of `fragment`.
    fn at_start(fragment: &str, message: impl Into<String>) -> Self {
        InvalidValue {
            message: message.into(),
            at: Place::Inside {
                start: fragment.as_ptr().addr(),
                line: 1,
                column: 1,
            },
        }
    }

    /// An error raised once `fragment` has been read.
    fn after(fragment: &str, message: impl Into<String>) -> Self {
        InvalidValue {
            message: message.into(),
            at: Place::After(fragment.as_ptr().addr() + fragment.len()),
        }
    }
}

impl JsonValue {
    /// An array is several values; anything else is one.
    pub(super) fn items(&self) -> &[JsonValue] {
        match self {
            JsonValue::Many(items) => items,
            other => std::slice::from_ref(other),
        }
    }
}

/// A value from its JSON text. The text is one complete JSON value, which
/// the outer parser has skipped over. That checks everything but the
/// surrogates of a string's `\u` escapes, so a string can still fail to
/// decode here. It is an invalid value like any other.
fn from_raw(text: &str) -> JsonValue {
    let text = text.trim_start_matches(JSON_SPACE);
    if text.starts_with(['-', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
        return JsonValue::Number(text.to_owned());
    }
    if let Some(content) = unescaped(text) {
        return JsonValue::Str(content.to_owned());
    }
    serde_json::Deserializer::from_str(text)
        .deserialize_any(ValueVisitor)
        .unwrap_or_else(|e| JsonValue::Invalid(InvalidValue::from_error(text, &e)))
}

/// Builds a [`JsonValue`] from any JSON value but a number, which
/// [`from_raw`] takes before the visitor runs.
struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = JsonValue;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a string, number, boolean, {\"$\": ...} object or array")
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<JsonValue, E> {
        Ok(JsonValue::Str(v.to_owned()))
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<JsonValue, E> {
        Ok(JsonValue::Bool(v))
    }
    fn visit_unit<E: de::Error>(self) -> Result<JsonValue, E> {
        Err(E::custom("null is not an attribute value"))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsonValue, A::Error> {
        // The array is read to its end even after an invalid element,
        // because the parser refuses an array left half read. The first
        // invalid element then stands for the whole array.
        let mut items = Vec::new();
        let mut invalid = None;
        while let Some(raw) = seq.next_element::<&RawValue>()? {
            let text = raw.get().trim_start_matches(JSON_SPACE);
            let item = if text.starts_with('[') {
                JsonValue::Invalid(InvalidValue::at_start(
                    text,
                    "nested arrays are not allowed in an attribute value",
                ))
            } else {
                from_raw(text)
            };
            match item {
                JsonValue::Invalid(found) if invalid.is_none() => invalid = Some(found),
                item => items.push(item),
            }
        }
        Ok(invalid.map_or(JsonValue::Many(items), JsonValue::Invalid))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsonValue, A::Error> {
        // Each member is `Some` once read, holding `None` when it is
        // invalid. The first invalid member in the text is kept apart and
        // reported ahead of any later fault.
        let (mut value, mut datatype, mut lang) = (None, None, None);
        let mut invalid = None;
        while let Some(key) = map.next_key::<String>()? {
            if !matches!(key.as_str(), "$" | "type" | "lang") {
                // An unknown key is refused at the key, before its value is
                // read. An invalid member read before it comes first in the
                // text, so that one is reported instead; the rest of the
                // object is read first, because the parser refuses an
                // object left half read.
                return match invalid {
                    Some(invalid) => {
                        map.next_value::<IgnoredAny>()?;
                        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                        Ok(JsonValue::Invalid(invalid))
                    }
                    None => Err(de::Error::custom(format!(
                        "unknown key `{}` in a value object, expected `$`, \
                         `type` or `lang`",
                        clip(&key)
                    ))),
                };
            }
            let member: &mut Option<Option<String>> = match key.as_str() {
                "$" => &mut value,
                "type" => &mut datatype,
                _ => &mut lang,
            };
            let text = map.next_value::<&RawValue>()?.get();
            if member.is_none() {
                *member = Some(match member_string(&key, text) {
                    Ok(content) => Some(content),
                    Err(found) => {
                        invalid = invalid.or(Some(found));
                        None
                    }
                });
                continue;
            }
            // The member is given twice. The same rule: an invalid member
            // read before it is reported first.
            return match invalid {
                Some(invalid) => {
                    while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                    Ok(JsonValue::Invalid(invalid))
                }
                None => Err(de::Error::custom(duplicate(&key))),
            };
        }
        if let Some(invalid) = invalid {
            return Ok(JsonValue::Invalid(invalid));
        }
        Ok(JsonValue::Typed {
            value: value
                .flatten()
                .ok_or_else(|| de::Error::custom("a value object needs a `$` key"))?,
            datatype: datatype.flatten(),
            lang: lang.flatten(),
        })
    }
}

/// The string under `key` of a value object, from its JSON text. Text that
/// is not a string is refused by its first byte, without being parsed: how
/// a number is parsed depends on the features of `serde_json`.
fn member_string(key: &str, text: &str) -> Result<String, InvalidValue> {
    let text = text.trim_start_matches(JSON_SPACE);
    if !text.starts_with('"') {
        return Err(InvalidValue::at_start(
            text,
            format!("expected a string as the `{key}` of a value object"),
        ));
    }
    match unescaped(text) {
        Some(content) => Ok(content.to_owned()),
        None => serde_json::from_str(text).map_err(|e| InvalidValue::from_error(text, &e)),
    }
}

/// The content of a JSON string token with no `\`, which needs no parsing.
/// The token is valid JSON, so it holds no control character, and without
/// an escape its content is the text between the quotes.
fn unescaped(token: &str) -> Option<&str> {
    let content = token.strip_prefix('"')?.strip_suffix('"')?;
    (!content.contains('\\')).then_some(content)
}

impl Serialize for JsonValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            JsonValue::Str(v) => s.serialize_str(v),
            JsonValue::Number(text) => RawValue::from_string(text.clone())
                .map_err(serde::ser::Error::custom)?
                .serialize(s),
            JsonValue::Bool(v) => s.serialize_bool(*v),
            JsonValue::Typed {
                value,
                datatype,
                lang,
            } => {
                let mut m = s.serialize_map(None)?;
                m.serialize_entry("$", value)?;
                if let Some(t) = datatype {
                    m.serialize_entry("type", t)?;
                }
                if let Some(l) = lang {
                    m.serialize_entry("lang", l)?;
                }
                m.end()
            }
            JsonValue::Invalid(invalid) => Err(serde::ser::Error::custom(&invalid.message)),
            JsonValue::Many(items) => {
                let mut q = s.serialize_seq(Some(items.len()))?;
                for item in items {
                    q.serialize_element(item)?;
                }
                q.end()
            }
        }
    }
}
