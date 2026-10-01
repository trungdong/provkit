//! Wire structs to document-builder events.

use indexmap::IndexMap;

use super::wire::{JsonDocument, JsonRecord, JsonSection, JsonValue};
use crate::builder::DocumentBuilder;
use crate::error::clip;
use crate::namespaces::Scope;
use crate::value::trim_xsd_space;
use crate::well_known::{XSD_DATETIME, XSD_DOUBLE};
use crate::{
    Attributes, DateTime, Error, Formal, FormalAttr, Kind, LangStr, Namespaces, QName, Record,
    Result, Value,
};

fn structure(message: impl Into<String>) -> Error {
    Error::JsonStructure {
        message: message.into(),
    }
}

/// `error` as an error in the record or bundle under `key` of `section`.
fn in_record(section: &str, key: &str, error: Error) -> Error {
    Error::InRecord {
        section: section.to_owned(),
        key: key.to_owned(),
        error: Box::new(error),
    }
}

fn namespaces(prefix: &IndexMap<String, String>) -> Result<Namespaces> {
    let mut ns = Namespaces::new();
    for (p, iri) in prefix {
        if p == "default" {
            ns.set_default(iri.as_str());
        } else {
            ns.declare(p, iri)?;
        }
    }
    Ok(ns)
}

pub(super) fn emit(wire: &JsonDocument, builder: &mut DocumentBuilder) -> Result<()> {
    let doc_ns = namespaces(&wire.prefix)?;
    builder.start_document(doc_ns.clone());
    emit_sections(&wire.sections, Scope::of(&doc_ns), builder)?;
    for (key, bundle) in &wire.bundle {
        let bundle_ns = namespaces(&bundle.prefix)?;
        let scope = Scope::within(&bundle_ns, &doc_ns);
        let located = |e| in_record("bundle", key, e);
        if key.starts_with("_:") {
            return Err(located(structure(format!(
                "bundle `{}` needs a real identifier",
                clip(key)
            ))));
        }
        // The bundle's own name resolves in the bundle scope (PROV-N 3.7.4).
        let id = scope.resolve(key).map_err(located)?;
        builder
            .start_bundle(id, bundle_ns.clone())
            .map_err(located)?;
        emit_sections(&bundle.sections, scope, builder)?;
        builder.end_bundle();
    }
    builder.end_document();
    Ok(())
}

fn emit_sections(
    sections: &IndexMap<Kind, JsonSection>,
    scope: Scope<'_>,
    builder: &mut DocumentBuilder,
) -> Result<()> {
    for (&kind, records) in sections {
        for (key, group) in records {
            let located = |e| in_record(kind.name(), key, e);
            let id = record_id(kind, key, scope).map_err(located)?;
            for body in group.as_slice() {
                for record in records_from(kind, id.as_ref(), body, scope).map_err(located)? {
                    builder.record(record);
                }
            }
        }
    }
    Ok(())
}

/// `_:x` keys mean "no identifier" and are never resolved; elements must have one.
fn record_id(kind: Kind, key: &str, scope: Scope<'_>) -> Result<Option<QName>> {
    if key.starts_with("_:") {
        if kind.is_element() {
            return Err(structure(format!(
                "{} `{}` needs a real identifier",
                kind.name(),
                clip(key)
            )));
        }
        return Ok(None);
    }
    scope.resolve(key).map(Some)
}

/// One record, or several for `hadMember` with an array of members.
fn records_from(
    kind: Kind,
    id: Option<&QName>,
    body: &JsonRecord,
    scope: Scope<'_>,
) -> Result<Vec<Record>> {
    let mut formals: Vec<(FormalAttr, Formal)> = Vec::new();
    let mut members: Vec<Formal> = Vec::new();
    let mut attributes = Attributes::new();
    for (key, value) in body {
        let name = scope.resolve(key)?;
        let formal = crate::record::formal_named(kind, &name);
        match formal {
            Some(FormalAttr::Entity) if kind == Kind::Membership => {
                for v in value.items() {
                    members.push(formal_value(v, key, false, scope)?);
                }
            }
            Some(f) => formals.push((f, formal_value(value, key, f.is_time(), scope)?)),
            None => {
                for v in value.items() {
                    attributes.insert(name.clone(), decode_value(v, scope)?);
                }
            }
        }
    }
    if kind == Kind::Membership && !members.is_empty() {
        return members
            .into_iter()
            .map(|m| {
                let mut f = formals.clone();
                f.push((FormalAttr::Entity, m));
                Record::from_parts(kind, id.cloned(), f, attributes.clone())
            })
            .collect();
    }
    Ok(vec![Record::from_parts(
        kind,
        id.cloned(),
        formals,
        attributes,
    )?])
}

/// The value of the formal under `key`, a time when `time` and a name
/// otherwise. A plain string is the text: a time parses after the space,
/// tab, CR and LF around it are trimmed, and a name resolves in `scope`. A
/// value object with no `lang` goes through the rule both readers use for
/// typed literals and must give a `Value::QName` for a name; for a time its
/// `type` must resolve to `xsd:dateTime`. Anything else is not a formal
/// value.
fn formal_value(value: &JsonValue, key: &str, time: bool, scope: Scope<'_>) -> Result<Formal> {
    let expected = if time { "a dateTime" } else { "a name" };
    let wrong = |found: &str| structure(format!("`{}` must be {expected}, not {found}", clip(key)));
    let (text, datatype) = match value {
        JsonValue::Str(s) => (s, None),
        JsonValue::Typed { lang: Some(_), .. } => return Err(wrong("a language-tagged string")),
        JsonValue::Typed { datatype: None, .. } => return Err(wrong("an untyped value object")),
        JsonValue::Typed {
            value,
            datatype: Some(datatype),
            ..
        } => (value, Some(scope.resolve(datatype)?)),
        _ => return Err(wrong("a number, boolean or array")),
    };
    if time {
        if let Some(datatype) = datatype
            && datatype != *XSD_DATETIME
        {
            return Err(wrong(&format!(
                "a value typed <{}>",
                clip(&datatype.to_string())
            )));
        }
        return DateTime::parse(trim_xsd_space(text)).map(Formal::Time);
    }
    match datatype {
        None => scope.resolve(text).map(Formal::Name),
        Some(datatype) => match Value::typed_in(scope, text, datatype)? {
            Value::QName(name) => Ok(Formal::Name(name)),
            other => match other.datatype() {
                Some(datatype) => Err(wrong(&format!(
                    "a value typed <{}>",
                    clip(&datatype.to_string())
                ))),
                None => Err(wrong("a language-tagged string")),
            },
        },
    }
}

/// Reads a JSON attribute value as a [`Value`]. A string is a plain string,
/// an integer takes the narrowest of `xsd:int`, `xsd:long` and `xsd:integer`
/// and any other number is `xsd:double`, each keeping its text, a boolean is
/// `xsd:boolean`, and a value object is read by its `lang` or `type`.
fn decode_value(v: &JsonValue, scope: Scope<'_>) -> Result<Value> {
    Ok(match v {
        JsonValue::Str(s) => Value::Str(s.clone()),
        JsonValue::Number(text) => number(text)?,
        JsonValue::Bool(b) => Value::from(*b),
        // The wire layer rejects an array inside an array, so this is not reached.
        JsonValue::Many(_) => return Err(structure("nested arrays are not allowed")),
        JsonValue::Typed {
            value,
            datatype,
            lang,
        } => {
            if let Some(lang) = lang {
                // A `type` beside `lang` is still resolved strictly.
                if let Some(dt) = datatype {
                    scope.resolve(dt)?;
                }
                return Ok(Value::LangStr(LangStr::new(value.as_str(), lang.as_str())?));
            }
            match datatype {
                None => Value::Str(value.clone()),
                Some(dt) => Value::typed_in(scope, value, scope.resolve(dt)?)?,
            }
        }
    })
}

/// A JSON number typed from its text: digits with an optional `-` are an
/// integer, typed by range, and any other number is an `xsd:double`. The
/// text is kept, so `1.50` and `1e400` read back as written.
fn number(text: &str) -> Result<Value> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.bytes().all(|b| b.is_ascii_digit()) {
        Ok(Value::integer(text))
    } else {
        Value::typed(text, XSD_DOUBLE.clone())
    }
}

#[cfg(test)]
mod tests;
