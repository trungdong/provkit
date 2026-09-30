//! Wire structs to Sink events.

use indexmap::IndexMap;

use super::wire::{JsonDocument, JsonRecord, JsonSection, JsonValue};
use crate::qname::well_known::{XSD_INT, XSD_INTEGER, XSD_LONG};
use crate::sink::Sink;
use crate::{
    Attributes, DateTime, Error, Formal, FormalAttr, Kind, LangStr, Namespaces, QName, Record,
    Result, Scope, Value,
};

fn structure(message: impl Into<String>) -> Error {
    Error::JsonStructure {
        message: message.into(),
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

pub(super) fn emit(wire: &JsonDocument, sink: &mut impl Sink) -> Result<()> {
    let doc_ns = namespaces(&wire.prefix)?;
    sink.start_document(doc_ns.clone())?;
    emit_sections(&wire.sections, &Scope::new(&doc_ns, None), sink)?;
    for (key, bundle) in &wire.bundle {
        let bundle_ns = namespaces(&bundle.prefix)?;
        let scope = Scope::new(&doc_ns, Some(&bundle_ns));
        if key.starts_with("_:") {
            return Err(structure(format!("bundle `{key}` needs a real identifier")));
        }
        // The bundle's own name resolves in the bundle scope (PROV-N 3.7.4).
        let id = scope.resolve(key)?;
        sink.start_bundle(id, bundle_ns.clone())?;
        emit_sections(&bundle.sections, &scope, sink)?;
        sink.end_bundle()?;
    }
    sink.end_document()
}

fn emit_sections(
    sections: &IndexMap<String, JsonSection>,
    scope: &Scope<'_>,
    sink: &mut impl Sink,
) -> Result<()> {
    for (section, records) in sections {
        let kind = Kind::from_json_key(section)
            .ok_or_else(|| structure(format!("unknown record kind `{section}`")))?;
        for (key, group) in records {
            let id = record_id(kind, key, scope)?;
            for body in group.as_slice() {
                for record in records_from(kind, id.as_ref(), body, scope)? {
                    sink.record(record)?;
                }
            }
        }
    }
    Ok(())
}

/// `_:x` keys mean "no identifier" and are never resolved; elements must have one.
fn record_id(kind: Kind, key: &str, scope: &Scope<'_>) -> Result<Option<QName>> {
    if key.starts_with("_:") {
        if kind.is_element() {
            return Err(structure(format!(
                "{} `{key}` needs a real identifier",
                kind.json_key()
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
    scope: &Scope<'_>,
) -> Result<Vec<Record>> {
    let mut formals: Vec<(FormalAttr, Formal)> = Vec::new();
    let mut members: Vec<QName> = Vec::new();
    let mut attributes = Attributes::new();
    for (key, value) in body {
        let name = scope.resolve(key)?;
        let formal = crate::record::formal_named(kind, &name);
        match formal {
            Some(FormalAttr::Entity) if kind == Kind::Membership => {
                for v in value.items() {
                    members.push(scope.resolve(v.expect_str(key)?)?);
                }
            }
            Some(f) if f.is_time() => {
                let text = value.expect_str(key)?;
                formals.push((f, Formal::Time(DateTime::parse(text)?)));
            }
            Some(f) => {
                let text = value.expect_str(key)?;
                formals.push((f, Formal::Name(scope.resolve(text)?)));
            }
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
                f.push((FormalAttr::Entity, Formal::Name(m)));
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

impl JsonValue {
    /// A formal attribute value must be a plain string (a name or a dateTime).
    fn expect_str(&self, key: &str) -> Result<&str> {
        match self {
            JsonValue::Str(s) | JsonValue::Typed { value: s, .. } => Ok(s),
            _ => Err(structure(format!("`{key}` must be a string"))),
        }
    }

    /// An array is several values; anything else is one.
    fn items(&self) -> &[JsonValue] {
        match self {
            JsonValue::Many(items) => items,
            other => std::slice::from_ref(other),
        }
    }
}

/// The design note's reading table.
fn decode_value(v: &JsonValue, scope: &Scope<'_>) -> Result<Value> {
    Ok(match v {
        JsonValue::Str(s) => Value::Str(s.clone()),
        JsonValue::Int(i) => integer(i.to_string(), i128::from(*i))?,
        JsonValue::UInt(u) => integer(u.to_string(), i128::from(*u))?,
        JsonValue::Float(f) => Value::from(*f),
        JsonValue::Bool(b) => Value::from(*b),
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

/// Raw JSON integers type by range. Integers beyond `i64` and within `u64`
/// are `xsd:integer`; larger numbers arrive as floats and read as `xsd:double`.
fn integer(lexical: String, n: i128) -> Result<Value> {
    let datatype = if i32::try_from(n).is_ok() {
        XSD_INT.clone()
    } else if i64::try_from(n).is_ok() {
        XSD_LONG.clone()
    } else {
        XSD_INTEGER.clone()
    };
    Value::typed(lexical, datatype)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qname::well_known::{XSD_BOOLEAN, XSD_DOUBLE};
    use crate::{Error, Record, Value};

    fn doc(json: &str) -> crate::Document {
        crate::json::read(json).unwrap()
    }
    fn ex(l: &str) -> QName {
        QName::new("http://example.org/", l)
    }
    const PFX: &str =
        r#""prefix": {"ex": "http://example.org/", "default": "http://default.org/"}"#;

    #[test]
    fn values_decode_per_table() {
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{
            "ex:s": "text",
            "ex:i": 5, "ex:l": 3000000000, "ex:b": 18446744073709551615, "ex:d": 1.5, "ex:t": true,
            "ex:o": {{"$": "x"}}, "ex:xs": {{"$": "x", "type": "xsd:string"}},
            "ex:lang": {{"$": "bonjour", "type": "prov:InternationalizedString", "lang": "fr"}},
            "ex:qn": {{"$": "ex:q", "type": "xsd:QName"}}, "ex:legacy": {{"$": "ex:q", "type": "prov:QUALIFIED_NAME"}},
            "ex:dt": {{"$": "2011-11-16T16:05:33Z", "type": "xsd:dateTime"}},
            "ex:ty": {{"$": "1", "type": "xsd:long"}},
            "ex:many": ["a", {{"$": "2", "type": "xsd:int"}}]
        }}}}}}"#
        ));
        let Record::Entity(e) = &d.records[0] else {
            panic!()
        };
        let get = |l: &str| e.attributes.get(&ex(l)).cloned().collect::<Vec<_>>();
        assert_eq!(get("s"), [Value::Str("text".into())]);
        assert_eq!(get("i"), [Value::typed("5", XSD_INT.clone()).unwrap()]);
        assert_eq!(
            get("l"),
            [Value::typed("3000000000", XSD_LONG.clone()).unwrap()]
        );
        assert_eq!(
            get("b"),
            [Value::typed("18446744073709551615", XSD_INTEGER.clone()).unwrap()]
        );
        assert_eq!(get("b")[0].datatype().unwrap(), *XSD_INTEGER);
        assert_eq!(get("d"), [Value::typed("1.5", XSD_DOUBLE.clone()).unwrap()]);
        assert_eq!(
            get("t"),
            [Value::typed("true", XSD_BOOLEAN.clone()).unwrap()]
        );
        assert_eq!(get("o"), [Value::Str("x".into())]);
        assert_eq!(get("xs"), [Value::Str("x".into())]);
        assert!(matches!(&get("lang")[0], Value::LangStr(l) if l.lang() == "fr"));
        assert_eq!(get("qn"), [Value::QName(ex("q"))]);
        assert_eq!(get("legacy"), [Value::QName(ex("q"))]);
        assert!(matches!(get("dt")[0], Value::DateTime(_)));
        assert_eq!(get("ty"), [Value::typed("1", XSD_LONG.clone()).unwrap()]);
        assert_eq!(get("many").len(), 2);
        assert_eq!(e.attributes.len(), 15);
    }

    #[test]
    fn integers_type_by_range() {
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{
            "ex:min": -2147483648, "ex:max": 2147483647, "ex:over": 2147483648,
            "ex:low": -2147483649, "ex:i64": 9223372036854775807,
            "ex:u64": 9223372036854775808, "ex:neg": -9223372036854775808,
            "ex:huge": 12345678901234567890123, "ex:exp": 1e3
        }}}}}}"#
        ));
        let Record::Entity(e) = &d.records[0] else {
            panic!()
        };
        let dt = |l: &str| e.attributes.get(&ex(l)).next().unwrap().datatype().unwrap();
        assert_eq!(dt("min"), *XSD_INT);
        assert_eq!(dt("max"), *XSD_INT);
        assert_eq!(dt("over"), *XSD_LONG);
        assert_eq!(dt("low"), *XSD_LONG);
        assert_eq!(dt("i64"), *XSD_LONG);
        assert_eq!(dt("u64"), *XSD_INTEGER);
        assert!(matches!(
            e.attributes.get(&ex("u64")).next(),
            Some(Value::Typed { lexical, .. }) if &**lexical == "9223372036854775808"
        ));
        assert_eq!(dt("neg"), *XSD_LONG);
        assert_eq!(dt("huge"), *XSD_DOUBLE);
        assert_eq!(dt("exp"), *XSD_DOUBLE);
    }

    #[test]
    fn any_uri_stays_a_typed_literal() {
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{"ex:u": {{"$": "http://x.example/", "type": "xsd:anyURI"}}}}}}}}"#
        ));
        let Record::Entity(e) = &d.records[0] else {
            panic!()
        };
        let name = ex("u");
        let v = e.attributes.get(&name).next().unwrap();
        assert!(matches!(
            v,
            Value::Typed { lexical, datatype }
                if &**lexical == "http://x.example/"
                    && datatype.iri() == "http://www.w3.org/2001/XMLSchema#anyURI"
        ));
    }

    #[test]
    fn duplicated_keys_keep_the_last() {
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{}}, "ex:e": {{"ex:a": "second"}}}}}}"#
        ));
        assert_eq!(d.records.len(), 1);
        let Record::Entity(e) = &d.records[0] else {
            panic!()
        };
        assert_eq!(e.attributes.len(), 1);
    }

    #[test]
    fn formals_blank_ids_and_fan_out() {
        let d = doc(&format!(
            r#"{{{PFX},
            "wasGeneratedBy": {{"_:g": {{"prov:entity": "ex:e", "prov:time": "2011-11-16T16:05:33Z", "prov:role": "r"}}}},
            "hadMember": {{"ex:m": {{"prov:collection": "ex:c", "prov:entity": ["ex:a", "ex:b"]}}}},
            "activity": {{"a1": {{"prov:startTime": "2011-11-16T16:05:33Z"}}}}
        }}"#
        ));
        assert_eq!(d.records.len(), 4);
        let Record::Generation(g) = &d.records[0] else {
            panic!()
        };
        assert_eq!(g.id, None);
        assert_eq!(g.entity, Some(ex("e")));
        assert!(g.time.is_some());
        assert_eq!(g.attributes.len(), 1);
        assert!(matches!(&d.records[1], Record::Membership(m) if m.entity == Some(ex("a"))));
        assert!(matches!(&d.records[2], Record::Membership(m) if m.entity == Some(ex("b"))));
        assert!(
            matches!(&d.records[3], Record::Activity(a) if a.id == QName::new("http://default.org/", "a1"))
        );
    }

    #[test]
    fn errors() {
        let read = |s: &str| crate::json::read(s);
        assert!(matches!(
            read(&format!(r#"{{{PFX}, "entity": {{"_:e": {{}}}}}}"#)),
            Err(Error::JsonStructure { .. })
        ));
        assert!(matches!(
            read(&format!(r#"{{{PFX}, "foo": {{"ex:e": {{}}}}}}"#)),
            Err(Error::JsonStructure { .. })
        ));
        assert!(
            matches!(read(r#"{"entity": {"zz:e": {}}}"#), Err(Error::UndeclaredPrefix(p)) if p == "zz")
        );
        assert!(matches!(
            read(&format!(
                r#"{{{PFX}, "activity": {{"ex:a": {{"prov:startTime": "soon"}}}}}}"#
            )),
            Err(Error::InvalidLiteral { .. })
        ));
        assert!(matches!(read("{"), Err(Error::Json(_))));
        assert!(matches!(
            read(r#"{"prefix": {"prov": "http://x/"}}"#),
            Err(Error::ReservedPrefix { .. })
        ));
    }

    #[test]
    fn json_errors_name_the_problem() {
        let message = |s: &str| crate::json::read(s).unwrap_err().to_string();
        let ent = |body: &str| format!(r#"{{{PFX}, "entity": {{"ex:e": {{{body}}}}}}}"#);
        let cases = [
            (
                ent(r#""ex:a": {"$": 1}"#),
                "invalid type: integer `1`, expected a string as the `$` of a value object",
            ),
            (
                ent(r#""ex:a": {"$": "1", "type": 2}"#),
                "expected a string as the `type` of a value object",
            ),
            (
                ent(r#""ex:a": {"$": "1", "datatype": "xsd:int"}"#),
                "unknown key `datatype` in a value object, expected `$`, `type` or `lang`",
            ),
            (
                ent(r#""ex:a": {"type": "xsd:int"}"#),
                "a value object needs a `$` key",
            ),
            (ent(r#""ex:a": null"#), "null is not an attribute value"),
            (
                ent(r#""ex:a": ["x", null]"#),
                "null is not an attribute value",
            ),
            (
                format!(r#"{{{PFX}, "entity": {{"ex:e": null}}}}"#),
                "invalid type: null, expected a record object or an array of record objects",
            ),
            (
                format!(r#"{{{PFX}, "entity": {{"ex:e": [{{"ex:a": {{"$": 1}}}}]}}}}"#),
                "expected a string as the `$` of a value object",
            ),
            (
                "[]".to_owned(),
                "invalid type: sequence, expected a PROV-JSON document, which must be a JSON object",
            ),
            (
                "\"x\"".to_owned(),
                "expected a PROV-JSON document, which must be a JSON object",
            ),
            (
                r#"{"bundle": {"ex:b": 3}}"#.to_owned(),
                "expected a bundle, which must be a JSON object",
            ),
            (
                r#"{"prefix": 3}"#.to_owned(),
                "invalid type: integer `3`, expected a map",
            ),
            (
                r#"{"prefix": {"ex": 3}}"#.to_owned(),
                "invalid type: integer `3`, expected a string",
            ),
            (
                format!(r#"{{{PFX}, "entity": 3}}"#),
                "invalid type: integer `3`, expected a map",
            ),
            (
                format!(r#"{{{PFX}, "entity": {{"ex:e": {{}}}}, "foo": {{}}}}"#),
                "unknown record kind `foo`",
            ),
            (
                format!(r#"{{{PFX}, "bundle": {{"ex:b": {{"bundle": {{}}}}}}}}"#),
                "unknown record kind `bundle`",
            ),
        ];
        for (text, needle) in cases {
            let m = message(&text);
            assert!(m.contains(needle), "{text}\n{m}");
            for private in ["OneOrMany", "JsonValue", "JsonDocument", "untagged"] {
                assert!(!m.contains(private), "{text}\n{m}");
            }
        }
    }

    #[test]
    fn bundles_scope_their_prefixes() {
        let d = doc(r#"{"prefix": {"ex": "http://example.org/"},
            "bundle": {"ex:b": {"prefix": {"b": "http://b.org/"}, "entity": {"b:e": {}, "ex:f": {}}}}}"#);
        assert_eq!(d.bundles.len(), 1);
        assert_eq!(d.bundles[0].id, ex("b"));
        assert_eq!(
            d.bundles[0].records[0].id(),
            Some(&QName::new("http://b.org/", "e"))
        );
        assert_eq!(d.bundles[0].records[1].id(), Some(&ex("f")));
        assert_eq!(d.bundles[0].namespaces.iter().count(), 1);
    }

    #[test]
    fn strict_rules_and_shapes() {
        let read = |s: &str| crate::json::read(s);
        let ent = |body: &str| format!(r#"{{{PFX}, "entity": {{"ex:e": {{{body}}}}}}}"#);
        assert!(matches!(
            read(&ent(r#""ex:a": {"$": "1", "type": "zz:t"}"#)),
            Err(Error::UndeclaredPrefix(p)) if p == "zz"
        ));
        assert!(matches!(
            read(&ent(r#""ex:a": {"$": "1", "type": "zz:t", "lang": "en"}"#)),
            Err(Error::UndeclaredPrefix(p)) if p == "zz"
        ));
        assert!(matches!(
            read(&ent(r#""ex:a": [["x"]]"#)),
            Err(Error::JsonStructure { .. })
        ));
        assert!(matches!(
            read(&format!(
                r#"{{{PFX}, "used": {{"_:u": {{"prov:entity": 5}}}}}}"#
            )),
            Err(Error::JsonStructure { .. })
        ));
        assert!(matches!(
            read(r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"_:b": {}}}"#),
            Err(Error::JsonStructure { .. })
        ));
    }

    #[test]
    fn repeated_keys_in_a_document_or_bundle() {
        // A repeated section keeps the last, as any repeated JSON key does.
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:a": {{}}}}, "entity": {{"ex:e": {{}}}}}}"#
        ));
        assert_eq!(d.records.len(), 1);
        assert_eq!(d.records[0].id(), Some(&ex("e")));
        // A repeated `prefix` or `bundle` is an error.
        let message = |s: &str| crate::json::read(s).unwrap_err().to_string();
        assert!(
            message(&format!(r#"{{{PFX}, "prefix": {{}}}}"#)).contains("duplicate field `prefix`")
        );
        assert!(
            message(&format!(r#"{{{PFX}, "bundle": {{}}, "bundle": {{}}}}"#))
                .contains("duplicate field `bundle`")
        );
        assert!(
            message(&format!(
                r#"{{{PFX}, "bundle": {{"ex:b": {{"prefix": {{}}, "prefix": {{}}}}}}}}"#
            ))
            .contains("duplicate field `prefix`")
        );
    }

    #[test]
    fn many_records_under_one_id() {
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": [{{"ex:a": "1"}}, {{"ex:a": "2"}}]}}}}"#
        ));
        assert_eq!(d.records.len(), 2);
    }

    #[test]
    fn end_time_and_false() {
        let d = doc(&format!(
            r#"{{{PFX}, "activity": {{"ex:a": {{"prov:endTime": "2011-11-16T16:05:33Z", "ex:f": false}}}}}}"#
        ));
        let Record::Activity(a) = &d.records[0] else {
            panic!()
        };
        assert!(a.end_time.is_some() && a.start_time.is_none());
        assert_eq!(
            a.attributes.get(&ex("f")).cloned().collect::<Vec<_>>(),
            [Value::typed("false", XSD_BOOLEAN.clone()).unwrap()]
        );
    }

    #[test]
    fn bundle_prefix_shadows_document_prefix() {
        let d = doc(
            r#"{"prefix": {"ex": "http://example.org/", "o": "http://o.org/"},
            "bundle": {"o:b": {"prefix": {"ex": "http://shadow.org/"}, "entity": {"ex:e": {}}}}}"#,
        );
        assert_eq!(
            d.bundles[0].records[0].id(),
            Some(&QName::new("http://shadow.org/", "e"))
        );
    }

    #[test]
    fn bundle_id_resolves_in_bundle_scope() {
        let d = doc(r#"{"prefix": {"ex": "http://example.org/"},
            "bundle": {"ex:b": {"prefix": {"ex": "http://shadow.org/"}, "entity": {"ex:e": {}}}}}"#);
        assert_eq!(d.bundles[0].id, QName::new("http://shadow.org/", "b"));
        assert_eq!(
            d.bundles[0].records[0].id(),
            Some(&QName::new("http://shadow.org/", "e"))
        );
    }

    #[test]
    fn a_formal_spelled_with_another_split_is_the_formal() {
        let d = doc(
            r#"{"prefix": {"ex": "http://example.org/", "w3": "http://www.w3.org/ns/"},
            "wasGeneratedBy": {"_:g": {"w3:prov#entity": "ex:e"}}}"#,
        );
        let Record::Generation(g) = &d.records[0] else {
            panic!()
        };
        assert_eq!(g.entity, Some(ex("e")));
        assert!(g.attributes.is_empty());
    }
}
