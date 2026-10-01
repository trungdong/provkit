//! The PROV-JSON shape, as serde sees it. Private; the model never sees these types.

use indexmap::IndexMap;
use serde::de::value::{MapAccessDeserializer, SeqAccessDeserializer};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use std::fmt;
use std::marker::PhantomData;

use crate::Kind;

/// A record's attributes, key order kept.
pub(super) type JsonRecord = IndexMap<String, JsonValue>;
/// Records of one kind, keyed by identifier.
pub(super) type JsonSection = IndexMap<String, OneOrMany<JsonRecord>>;

pub(super) struct JsonDocument {
    pub prefix: IndexMap<String, String>,
    pub sections: IndexMap<Kind, JsonSection>,
    pub bundle: IndexMap<String, JsonBundle>,
}

pub(super) struct JsonBundle {
    pub prefix: IndexMap<String, String>,
    pub sections: IndexMap<Kind, JsonSection>,
}

/// Reads the keys of a document or bundle object. `prefix` is the
/// namespace table, `bundle` (documents only) the bundles, and every other
/// key a section. A repeated `prefix` or `bundle` is an error; a repeated
/// section keeps its last value.
struct ContainerVisitor {
    expecting: &'static str,
    bundles: bool,
}

/// A document or bundle as read, before it is split into the two types.
type Container = (
    IndexMap<String, String>,
    IndexMap<Kind, JsonSection>,
    IndexMap<String, JsonBundle>,
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
            match key.as_str() {
                "prefix" if prefix.is_some() => return Err(de::Error::duplicate_field("prefix")),
                "prefix" => prefix = Some(map.next_value()?),
                "bundle" if self.bundles && bundle.is_some() => {
                    return Err(de::Error::duplicate_field("bundle"));
                }
                "bundle" if self.bundles => bundle = Some(map.next_value()?),
                "bundle" => return Err(de::Error::custom("a bundle cannot hold a bundle")),
                name => {
                    let kind = Kind::from_name(name).ok_or_else(|| {
                        de::Error::custom(format!("unknown record kind `{name}`"))
                    })?;
                    sections.insert(kind, map.next_value()?);
                }
            }
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
            bundle,
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
    bundle: Option<&IndexMap<String, JsonBundle>>,
) -> Result<S::Ok, S::Error> {
    let mut m = s.serialize_map(None)?;
    if !prefix.is_empty() {
        m.serialize_entry("prefix", prefix)?;
    }
    for (kind, section) in sections {
        m.serialize_entry(kind.name(), section)?;
    }
    if let Some(bundle) = bundle.filter(|b| !b.is_empty()) {
        m.serialize_entry("bundle", bundle)?;
    }
    m.end()
}

impl Serialize for JsonDocument {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_container(s, &self.prefix, &self.sections, Some(&self.bundle))
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
impl<'de, T: Deserialize<'de>> Deserialize<'de> for OneOrMany<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for V<T> {
            type Value = OneOrMany<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a record object or an array of record objects")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(OneOrMany::One)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Vec::deserialize(SeqAccessDeserializer::new(seq)).map(OneOrMany::Many)
            }
        }
        d.deserialize_any(V(PhantomData))
    }
}

/// A string under a named key of a value object, with an error that names
/// the key.
struct StrAt(&'static str);

impl<'de> DeserializeSeed<'de> for StrAt {
    type Value = String;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<String, D::Error> {
        struct V(&'static str);
        impl Visitor<'_> for V {
            type Value = String;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a string as the `{}` of a value object", self.0)
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
                Ok(v.to_owned())
            }
        }
        d.deserialize_str(V(self.0))
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
#[derive(Debug, Clone, PartialEq)]
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

/// The value is captured as raw JSON text first. A number is kept as that
/// text, because the visitor interface hands it over as `i64`, `u64` or
/// `f64` (or, when a dependant enables the `arbitrary_precision` feature of
/// `serde_json`, as a private map), each of which loses the spelling.
///
/// The raw-value capture runs under `serde_json`'s recursion limit, so deeply
/// nested input is refused before [`from_raw`] sees it.
impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        from_raw(<&RawValue>::deserialize(d)?.get(), Nesting::Allowed)
    }
}

/// Whether the value being read may be an array.
#[derive(Clone, Copy, PartialEq)]
enum Nesting {
    Allowed,
    Inside,
}

/// An element of an array of values, which cannot be an array itself.
struct Element(JsonValue);

impl<'de> Deserialize<'de> for Element {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        from_raw(<&RawValue>::deserialize(d)?.get(), Nesting::Inside).map(Element)
    }
}

/// A value from its JSON text. The text is one complete JSON value: the
/// outer parser has already checked its syntax.
fn from_raw<E: de::Error>(text: &str, nesting: Nesting) -> Result<JsonValue, E> {
    let text = text.trim_start_matches([' ', '\t', '\r', '\n']);
    match text.as_bytes().first() {
        Some(b'-' | b'0'..=b'9') => Ok(JsonValue::Number(text.to_owned())),
        Some(b'[') if nesting == Nesting::Inside => Err(E::custom(
            "nested arrays are not allowed in an attribute value",
        )),
        _ => {
            let mut fragment = serde_json::Deserializer::from_str(text);
            fragment
                .deserialize_any(ValueVisitor)
                .map_err(|e| E::custom(without_position(&e)))
        }
    }
}

/// The message of an error raised while parsing a fragment. Its line and
/// column count from the fragment's start, so they are dropped; the outer
/// parser adds the position in the document.
fn without_position(e: &serde_json::Error) -> String {
    let message = e.to_string();
    match message.rfind(" at line ") {
        Some(end) => message[..end].to_owned(),
        None => message,
    }
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
    fn visit_string<E: de::Error>(self, v: String) -> Result<JsonValue, E> {
        Ok(JsonValue::Str(v))
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<JsonValue, E> {
        Ok(JsonValue::Bool(v))
    }
    fn visit_unit<E: de::Error>(self) -> Result<JsonValue, E> {
        Err(E::custom("null is not an attribute value"))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsonValue, A::Error> {
        let mut items = Vec::new();
        while let Some(Element(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(JsonValue::Many(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsonValue, A::Error> {
        let (mut value, mut datatype, mut lang) = (None, None, None);
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "$" => value = Some(map.next_value_seed(StrAt("$"))?),
                "type" => datatype = Some(map.next_value_seed(StrAt("type"))?),
                "lang" => lang = Some(map.next_value_seed(StrAt("lang"))?),
                other => {
                    return Err(de::Error::custom(format!(
                        "unknown key `{other}` in a value object, expected `$`, \
                         `type` or `lang`"
                    )));
                }
            }
        }
        let value = value.ok_or_else(|| de::Error::custom("a value object needs a `$` key"))?;
        Ok(JsonValue::Typed {
            value,
            datatype,
            lang,
        })
    }
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
