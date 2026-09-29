//! The PROV-JSON shape, as serde sees it. Private; the model never sees these types.

use indexmap::IndexMap;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A record's attributes, key order kept.
pub(super) type JsonRecord = IndexMap<String, JsonValue>;
/// Records of one kind, keyed by identifier.
pub(super) type JsonSection = IndexMap<String, OneOrMany<JsonRecord>>;

#[derive(Serialize, Deserialize)]
pub(super) struct JsonDocument {
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub prefix: IndexMap<String, String>,
    #[serde(flatten)]
    pub sections: IndexMap<String, JsonSection>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub bundle: IndexMap<String, JsonBundle>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct JsonBundle {
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub prefix: IndexMap<String, String>,
    #[serde(flatten)]
    pub sections: IndexMap<String, JsonSection>,
}

/// PROV-JSON lists several records under one identifier as an array.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
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
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    Typed {
        value: String,
        datatype: Option<String>,
        lang: Option<String>,
    },
    Many(Vec<JsonValue>),
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
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
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<JsonValue, E> {
                Ok(JsonValue::Int(v))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<JsonValue, E> {
                Ok(JsonValue::UInt(v))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<JsonValue, E> {
                Ok(JsonValue::Float(v))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<JsonValue, E> {
                Ok(JsonValue::Bool(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsonValue, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(JsonValue::Many(items))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsonValue, A::Error> {
                let (mut value, mut datatype, mut lang) = (None, None, None);
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "$" => value = Some(map.next_value::<String>()?),
                        "type" => datatype = Some(map.next_value::<String>()?),
                        "lang" => lang = Some(map.next_value::<String>()?),
                        other => {
                            return Err(de::Error::unknown_field(other, &["$", "type", "lang"]));
                        }
                    }
                }
                let value = value.ok_or_else(|| de::Error::missing_field("$"))?;
                Ok(JsonValue::Typed {
                    value,
                    datatype,
                    lang,
                })
            }
        }
        d.deserialize_any(V)
    }
}

impl Serialize for JsonValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            JsonValue::Str(v) => s.serialize_str(v),
            JsonValue::Int(v) => s.serialize_i64(*v),
            JsonValue::UInt(v) => s.serialize_u64(*v),
            JsonValue::Float(v) => s.serialize_f64(*v),
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
