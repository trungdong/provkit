//! Wire structs to document-builder events.

use indexmap::IndexMap;

use super::structure;
use super::wire::{
    DEFAULT, InvalidValue, JSON_SPACE, JsonDocument, JsonRecord, JsonSection, JsonValue, Place,
};
use crate::builder::DocumentBuilder;
use crate::error::clip;
use crate::namespaces::Scope;
use crate::value::{is_int_literal, is_qname_datatype, trim_xsd_space};
use crate::well_known::{XSD_DATETIME, XSD_DOUBLE};
use crate::{
    Attributes, DateTime, Error, Formal, FormalAttr, Kind, LangStr, Namespaces, QName, Record,
    Result, Value,
};

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
        if p == DEFAULT {
            ns.set_default(iri.as_str());
        } else {
            ns.declare(p, iri)?;
        }
    }
    Ok(ns)
}

/// Where a fragment of `input` starts, as a 1-based line and column, when
/// the fragment lies inside `input`. The column counts bytes, as
/// `serde_json` counts them.
fn position_of(input: &str, start: usize) -> Option<(usize, usize)> {
    let offset = start.checked_sub(input.as_ptr().addr())?;
    if offset >= input.len() {
        return None;
    }
    let before = input.get(..offset)?;
    let line = 1 + before.matches('\n').count();
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    Some((line, offset - line_start + 1))
}

/// Where `serde_json` places an error that a map's visitor raises once a
/// value has been read, given the address of the byte after that value. It
/// skips the white space after the value and reads a closing brace that
/// follows, but not a comma. The column is the number of bytes on the line
/// before the next byte to read.
fn after_value(input: &str, end: usize) -> Option<(usize, usize)> {
    let offset = end.checked_sub(input.as_ptr().addr())?;
    let rest = input.get(offset..)?;
    let after = rest.trim_start_matches(JSON_SPACE);
    let next = input.len() - after.len() + usize::from(after.starts_with('}'));
    let before = &input[..next];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    Some((1 + before.matches('\n').count(), next - line_start))
}

/// The error for a value the wire layer could not read. It holds the wire
/// layer's message and the position of the error in `input`, the text the
/// document was read from. The position of an error inside a fragment is
/// the fragment's, moved on by the error's position inside the fragment.
/// Each equals the position `serde_json` gives the same error when it reads
/// the document in one pass.
fn invalid_value(invalid: &InvalidValue, input: &str) -> Error {
    let position = match invalid.at {
        Place::Inside {
            start,
            line: inner_line,
            column: inner_column,
        } => position_of(input, start)
            .filter(|_| inner_line > 0)
            .map(|(line, column)| match inner_line {
                1 => (line, (column + inner_column).saturating_sub(1)),
                inner => (line + inner - 1, inner_column),
            }),
        Place::After(end) => after_value(input, end),
    };
    structure(match position {
        Some((line, column)) => format!("{} at line {line} column {column}", invalid.message),
        None => invalid.message.clone(),
    })
}

/// Refuses the first value in `sections` that the wire layer could not
/// read, naming its record.
fn refuse_invalid(sections: &IndexMap<Kind, JsonSection>, input: &str) -> Result<()> {
    for (kind, records) in sections {
        for (key, group) in records {
            let invalid = group
                .as_slice()
                .iter()
                .flat_map(IndexMap::values)
                .find_map(|value| match value {
                    JsonValue::Invalid(invalid) => Some(invalid),
                    _ => None,
                });
            if let Some(invalid) = invalid {
                return Err(in_record(kind.name(), key, invalid_value(invalid, input)));
            }
        }
    }
    Ok(())
}

/// Builds the document `wire` holds. `input` is the text `wire` was read
/// from. A value the wire layer could not read is refused before anything
/// is built, so it is reported ahead of an error in a name or a literal,
/// as an error of the JSON parser is.
pub(super) fn emit(wire: &JsonDocument, input: &str, builder: &mut DocumentBuilder) -> Result<()> {
    refuse_invalid(&wire.sections, input)?;
    for (_, bundle) in &wire.bundle {
        refuse_invalid(&bundle.sections, input)?;
    }
    let doc_ns = namespaces(&wire.prefix)?;
    builder.start_document(doc_ns.clone());
    emit_sections(&wire.sections, Scope::of(&doc_ns), builder)?;
    for (key, bundle) in &wire.bundle {
        let bundle_ns = namespaces(&bundle.prefix)?;
        let scope = Scope::within(&bundle_ns, &doc_ns);
        let located = |e| in_record("bundle", key, e);
        if key.starts_with("_:") {
            return Err(located(needs_identifier("bundle", key)));
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
            return Err(needs_identifier(kind.name(), key));
        }
        return Ok(None);
    }
    scope.resolve(key).map(Some)
}

/// The error for a blank key in `section`, an element kind or `bundle`.
fn needs_identifier(section: &str, key: &str) -> Error {
    structure(format!("{section} `{}` needs a real identifier", clip(key)))
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
/// value object with no `lang` is read the same way when its `type`
/// resolves to `xsd:dateTime` for a time, or to `xsd:QName` or
/// `prov:QUALIFIED_NAME` for a name. Anything else is not a formal value.
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
        // `emit` refuses invalid values first, so this is not reached.
        JsonValue::Invalid(invalid) => return Err(structure(invalid.message.as_str())),
        JsonValue::Number(_) | JsonValue::Bool(_) | JsonValue::Many(_) => {
            return Err(wrong("a number, boolean or array"));
        }
    };
    if let Some(datatype) = datatype {
        let fits = if time {
            datatype == *XSD_DATETIME
        } else {
            is_qname_datatype(&datatype)
        };
        if !fits {
            return Err(wrong(&format!(
                "a value typed <{}>",
                clip(&datatype.to_string())
            )));
        }
    }
    if time {
        DateTime::parse(trim_xsd_space(text)).map(Formal::Time)
    } else {
        scope.resolve(text).map(Formal::Name)
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
        // The wire layer makes an array inside an array an invalid value,
        // and `emit` refuses invalid values first, so neither is reached.
        JsonValue::Many(_) => return Err(structure("nested arrays are not allowed")),
        JsonValue::Invalid(invalid) => return Err(structure(invalid.message.as_str())),
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
    if is_int_literal(text) {
        Ok(Value::integer(text))
    } else {
        Value::typed(text, XSD_DOUBLE.clone())
    }
}

#[cfg(test)]
mod tests;
