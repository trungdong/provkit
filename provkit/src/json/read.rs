//! Wire structs to document-builder events.

use indexmap::IndexMap;

use super::wire::{JsonDocument, JsonRecord, JsonSection, JsonValue};
use crate::sink::DocumentBuilder;
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

pub(super) fn emit(wire: &JsonDocument, sink: &mut DocumentBuilder) -> Result<()> {
    let doc_ns = namespaces(&wire.prefix)?;
    sink.start_document(doc_ns.clone());
    emit_sections(&wire.sections, &doc_ns, sink)?;
    for (key, bundle) in &wire.bundle {
        let bundle_ns = namespaces(&bundle.prefix)?;
        let scope = bundle_ns.with_fallback(&doc_ns);
        if key.starts_with("_:") {
            return Err(structure(format!("bundle `{key}` needs a real identifier")));
        }
        // The bundle's own name resolves in the bundle scope (PROV-N 3.7.4).
        let id = scope.resolve(key)?;
        sink.start_bundle(id, bundle_ns.clone())?;
        emit_sections(&bundle.sections, &scope, sink)?;
        sink.end_bundle();
    }
    sink.end_document();
    Ok(())
}

fn emit_sections(
    sections: &IndexMap<String, JsonSection>,
    scope: &Namespaces,
    sink: &mut DocumentBuilder,
) -> Result<()> {
    for (section, records) in sections {
        let kind = Kind::from_name(section)
            .ok_or_else(|| structure(format!("unknown record kind `{section}`")))?;
        for (key, group) in records {
            let id = record_id(kind, key, scope)?;
            for body in group.as_slice() {
                for record in records_from(kind, id.as_ref(), body, scope)? {
                    sink.record(record);
                }
            }
        }
    }
    Ok(())
}

/// `_:x` keys mean "no identifier" and are never resolved; elements must have one.
fn record_id(kind: Kind, key: &str, scope: &Namespaces) -> Result<Option<QName>> {
    if key.starts_with("_:") {
        if kind.is_element() {
            return Err(structure(format!(
                "{} `{key}` needs a real identifier",
                kind.name()
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
    scope: &Namespaces,
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
fn formal_value(value: &JsonValue, key: &str, time: bool, scope: &Namespaces) -> Result<Formal> {
    let expected = if time { "a dateTime" } else { "a name" };
    let wrong = |found: &str| structure(format!("`{key}` must be {expected}, not {found}"));
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
            return Err(wrong(&format!("a value typed <{datatype}>")));
        }
        return DateTime::parse(trim_xsd_space(text)).map(Formal::Time);
    }
    match datatype {
        None => scope.resolve(text).map(Formal::Name),
        Some(datatype) => match Value::typed_in(scope, text, datatype)? {
            Value::QName(name) => Ok(Formal::Name(name)),
            other => match other.datatype() {
                Some(datatype) => Err(wrong(&format!("a value typed <{datatype}>"))),
                None => Err(wrong("a language-tagged string")),
            },
        },
    }
}

/// Reads a JSON attribute value as a [`Value`]. A string is a plain string,
/// an integer takes the narrowest of `xsd:int`, `xsd:long` and `xsd:integer`
/// and any other number is `xsd:double`, each keeping its text, a boolean is
/// `xsd:boolean`, and a value object is read by its `lang` or `type`.
fn decode_value(v: &JsonValue, scope: &Namespaces) -> Result<Value> {
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
mod tests {
    use super::*;
    use crate::json;
    use crate::test_util::ex;
    use crate::well_known::{XSD_BOOLEAN, XSD_INT, XSD_INTEGER, XSD_LONG};

    fn doc(json: &str) -> crate::Document {
        json::read(json).unwrap()
    }
    const PFX: &str =
        r#""prefix": {"ex": "http://example.org/", "default": "http://default.org/"}"#;

    #[test]
    fn a_language_string_reads_with_or_without_a_type() {
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{
            "ex:a": {{"$": "bonjour", "lang": "fr"}},
            "ex:a": {{"$": "bonjour", "type": "prov:InternationalizedString", "lang": "fr"}}
        }}}}}}"#
        ));
        let plain = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{"ex:a": {{"$": "bonjour", "lang": "fr"}}}}}}}}"#
        ));
        assert_eq!(d, plain);
    }

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
            Some(Value::Typed(t)) if t.lexical() == "9223372036854775808"
        ));
        assert_eq!(dt("neg"), *XSD_LONG);
        assert_eq!(dt("huge"), *XSD_INTEGER);
        assert!(matches!(
            e.attributes.get(&ex("huge")).next(),
            Some(Value::Typed(t)) if t.lexical() == "12345678901234567890123"
        ));
        assert_eq!(dt("exp"), *XSD_DOUBLE);
    }

    #[test]
    fn numbers_are_typed_from_their_text() {
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{
            "ex:zero": -0, "ex:low": -9223372036854775809, "ex:high": 18446744073709551616,
            "ex:huge": 1e400, "ex:frac": 1.50, "ex:exp": 1E2, "ex:negzero": -0.0
        }}}}}}"#
        ));
        let Record::Entity(e) = &d.records[0] else {
            panic!()
        };
        let get = |l: &str| e.attributes.get(&ex(l)).next().cloned().unwrap();
        let literal = |v: &Value| match v {
            Value::Typed(t) => (t.lexical().to_owned(), t.datatype().clone()),
            other => panic!("expected a typed literal, got {other:?}"),
        };
        assert_eq!(literal(&get("zero")), ("-0".into(), XSD_INT.clone()));
        assert_eq!(
            literal(&get("low")),
            ("-9223372036854775809".into(), XSD_INTEGER.clone())
        );
        assert_eq!(
            literal(&get("high")),
            ("18446744073709551616".into(), XSD_INTEGER.clone())
        );
        assert_eq!(literal(&get("huge")), ("1e400".into(), XSD_DOUBLE.clone()));
        assert_eq!(
            get("huge"),
            Value::typed("INF", XSD_DOUBLE.clone()).unwrap()
        );
        assert_eq!(literal(&get("frac")), ("1.50".into(), XSD_DOUBLE.clone()));
        assert_eq!(get("exp"), Value::typed("100", XSD_DOUBLE.clone()).unwrap());
        assert_eq!(literal(&get("negzero")).1, *XSD_DOUBLE);
    }

    #[cfg(feature = "provn")]
    #[test]
    fn bare_integers_read_alike_in_both_formats() {
        let json = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{"ex:n": [-0, 0, 3000000000, -9223372036854775809]}}}}}}"#
        ));
        let provn = crate::provn::read(
            "document\n default <http://default.org/>\n prefix ex <http://example.org/>\n \
             entity(ex:e, [ex:n=-0, ex:n=0, ex:n=3000000000, ex:n=-9223372036854775809])\nendDocument",
        )
        .unwrap();
        assert_eq!(json, provn);
    }

    #[test]
    fn nested_arrays_are_an_error_at_any_depth() {
        let nested = format!(r#"{{{PFX}, "entity": {{"ex:e": {{"ex:n": [[1]]}}}}}}"#);
        let message = json::read(&nested).unwrap_err().to_string();
        assert!(
            message.contains("nested arrays are not allowed"),
            "{message}"
        );
        let deep = format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{"ex:n": {}1{}}}}}}}"#,
            "[".repeat(100_000),
            "]".repeat(100_000)
        );
        assert!(json::read(&deep).is_err());
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
            Value::Typed(t)
                if t.lexical() == "http://x.example/"
                    && t.datatype().iri() == "http://www.w3.org/2001/XMLSchema#anyURI"
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
            matches!(&d.records[3], Record::Activity(a) if a.id == QName::new("http://default.org/", "a1").unwrap())
        );
    }

    #[test]
    fn member_fan_out_shares_one_attribute_allocation() {
        let n = 3_000;
        let members: Vec<String> = (0..n).map(|i| format!(r#""ex:m{i}""#)).collect();
        let attrs: Vec<String> = (0..n).map(|i| format!(r#""ex:k{i}""#)).collect();
        let d = doc(&format!(
            r#"{{{PFX}, "hadMember": {{"ex:c": {{"prov:collection": "ex:c",
                "prov:entity": [{}], "ex:tag": [{}]}}}}}}"#,
            members.join(","),
            attrs.join(",")
        ));
        assert_eq!(d.records.len(), n);
        let first = d.records[0].attributes();
        assert_eq!(first.len(), n);
        assert!(
            d.records
                .iter()
                .all(|r| r.attributes().shares_storage_with(first))
        );
    }

    #[test]
    fn errors() {
        let read = |s: &str| json::read(s);
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

    /// A document holding one entity `ex:e` with the attributes in `body`.
    fn entity(body: &str) -> String {
        format!(r#"{{{PFX}, "entity": {{"ex:e": {{{body}}}}}}}"#)
    }

    /// The values under `ex:<local>` of the first record.
    fn values(d: &crate::Document, local: &str) -> Vec<Value> {
        d.records[0].attributes().get(&ex(local)).cloned().collect()
    }

    #[test]
    fn ill_formed_literals_read_and_round_trip() {
        let literals = [
            ("2012-13-01T00:00:00", "xsd:dateTime"),
            ("nope", "xsd:dateTime"),
            ("abc", "xsd:int"),
            ("maybe", "xsd:boolean"),
            ("1,5", "xsd:decimal"),
        ];
        let body: Vec<String> = literals
            .iter()
            .enumerate()
            .map(|(i, (text, datatype))| {
                format!(r#""ex:k{i}": {{"$": "{text}", "type": "{datatype}"}}"#)
            })
            .collect();
        let d = doc(&entity(&body.join(", ")));
        for (i, (text, datatype)) in literals.iter().enumerate() {
            let value = &values(&d, &format!("k{i}"))[0];
            assert!(
                matches!(value, Value::Typed(t) if t.lexical() == *text),
                "{text:?} as {datatype}: {value:?}"
            );
        }
        // Each literal is written with the text and datatype it was read with.
        let written = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&written).unwrap();
        for (i, (text, datatype)) in literals.iter().enumerate() {
            assert_eq!(
                v["entity"]["ex:e"][format!("ex:k{i}")],
                serde_json::json!({"$": text, "type": datatype}),
                "{written}"
            );
        }
        assert_eq!(doc(&written), d);
    }

    #[test]
    fn a_name_that_cannot_be_written_in_provn_is_rejected() {
        // Each position a name appears in, with `@N@` for the name.
        let positions = [
            r#"{{PFX}, "entity": {"@N@": {}}}"#,
            r#"{{PFX}, "bundle": {"@N@": {}}}"#,
            r#"{{PFX}, "entity": {"ex:e": {"@N@": "v"}}}"#,
            r#"{{PFX}, "used": {"_:u": {"prov:entity": "@N@"}}}"#,
            r#"{{PFX}, "hadMember": {"ex:m": {"prov:collection": "ex:c", "prov:entity": ["ex:a", "@N@"]}}}"#,
            r#"{{PFX}, "entity": {"ex:e": {"ex:k": {"$": "v", "type": "@N@"}}}}"#,
            r#"{{PFX}, "entity": {"ex:e": {"ex:k": {"$": "@N@", "type": "xsd:QName"}}}}"#,
            r#"{{PFX}, "entity": {"ex:e": {"ex:k": {"$": "@N@", "type": "prov:QUALIFIED_NAME"}}}}"#,
        ];
        // The name as written in the JSON and the text the error carries.
        let names = [
            ("ex:a b", "ex:a b"),
            ("ex:a%4", "ex:a%4"),
            ("ex:a\\u0007b", "ex:a\u{7}b"),
            ("ex:<a>", "ex:<a>"),
            ("a b", "a b"),
            ("not a name", "not a name"),
            ("prov:agent ", "prov:agent "),
        ];
        for position in positions {
            for (name, carried) in names {
                let text = position.replace("{PFX}", PFX).replace("@N@", name);
                assert!(
                    matches!(
                        json::read(&text),
                        Err(Error::InvalidQName(t)) if t == carried
                    ),
                    "{text}"
                );
            }
        }
        // An empty local part after a prefix stays valid.
        let d = doc(&entity(
            r#""ex:k": {"$": "ex:", "type": "prov:QUALIFIED_NAME"}"#,
        ));
        assert_eq!(values(&d, "k"), [Value::QName(ex(""))]);
        // Text that is a qualified name must resolve.
        assert!(matches!(
            json::read(&entity(r#""ex:k": {"$": "zz:foo", "type": "prov:QUALIFIED_NAME"}"#)),
            Err(Error::UndeclaredPrefix(p)) if p == "zz"
        ));
        assert!(matches!(
            json::read(&entity(r#""ex:k": {"$": "", "type": "xsd:QName"}"#)),
            Err(Error::InvalidQName(t)) if t.is_empty()
        ));
    }

    #[test]
    fn a_formal_written_as_an_object_is_checked() {
        let doc = |formal: &str| {
            format!(
                r#"{{"prefix": {{"ex": "http://example.org/"}}, "wasGeneratedBy": {{"ex:g": {{"prov:entity": "ex:e", "prov:activity": {formal}}}}}}}"#
            )
        };
        assert!(json::read(&doc(r#"{"$": "ex:a", "type": "prov:QUALIFIED_NAME"}"#)).is_ok());
        assert!(json::read(&doc(r#"{"$": "ex:a", "type": "xsd:QName"}"#)).is_ok());
        for bad in [
            r#"{"$": "ex:a", "type": "zz:undeclared"}"#,
            r#"{"$": "ex:a", "type": "xsd:string"}"#,
            r#"{"$": "ex:a", "lang": "en"}"#,
            r#"{"$": "ex:a", "type": "prov:QUALIFIED_NAME", "lang": "en"}"#,
        ] {
            assert!(json::read(&doc(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_formal_object_of_the_wrong_shape_names_the_formal() {
        let generation = |formal: &str| {
            format!(
                r#"{{{PFX}, "wasGeneratedBy": {{"ex:g": {{"prov:entity": "ex:e", "prov:time": {formal}}}}}}}"#
            )
        };
        let d = doc(&generation(
            r#"{"$": "2011-11-16T16:05:33Z", "type": "xsd:dateTime"}"#,
        ));
        assert!(matches!(&d.records[0], Record::Generation(g) if g.time.is_some()));
        let names_the_formal = |text: &str, formal: &str| match json::read(text) {
            Err(Error::JsonStructure { message }) => {
                assert!(message.contains(formal), "{text}\n{message}");
            }
            other => panic!("{text}: {other:?}"),
        };
        for bad in [
            r#"{"$": "2011-11-16T16:05:33Z", "type": "xsd:string"}"#,
            r#"{"$": "2011-11-16T16:05:33Z", "type": "xsd:QName"}"#,
            r#"{"$": "2011-11-16T16:05:33Z", "type": "xsd:dateTime", "lang": "en"}"#,
            r#"{"$": "2011-11-16T16:05:33Z"}"#,
        ] {
            names_the_formal(&generation(bad), "prov:time");
        }
        for bad in [
            r#"{"$": "ex:a", "type": "xsd:dateTime"}"#,
            r#"{"$": "ex:a", "type": "xsd:string"}"#,
            r#"{"$": "ex:a", "lang": "en"}"#,
            r#"{"$": "ex:a"}"#,
        ] {
            names_the_formal(
                &format!(
                    r#"{{{PFX}, "wasGeneratedBy": {{"ex:g": {{"prov:entity": "ex:e", "prov:activity": {bad}}}}}}}"#
                ),
                "prov:activity",
            );
            names_the_formal(
                &format!(
                    r#"{{{PFX}, "hadMember": {{"ex:m": {{"prov:collection": "ex:c", "prov:entity": ["ex:a", {bad}]}}}}}}"#
                ),
                "prov:entity",
            );
        }
        // An undeclared prefix in the `type` is reported as such.
        assert!(matches!(
            json::read(&generation(r#"{"$": "x", "type": "zz:t"}"#)),
            Err(Error::UndeclaredPrefix(p)) if p == "zz"
        ));
    }

    #[test]
    fn an_empty_member_array_reads_as_one_membership_with_no_entity() {
        let d = doc(&format!(
            r#"{{{PFX}, "hadMember": {{"ex:m": {{"prov:collection": "ex:c", "prov:entity": []}}}}}}"#
        ));
        assert_eq!(d.records.len(), 1);
        let Record::Membership(m) = &d.records[0] else {
            panic!("{:?}", d.records[0])
        };
        assert_eq!(m.id, Some(ex("m")));
        assert_eq!(m.collection, Some(ex("c")));
        assert_eq!(m.entity, None);
    }

    #[test]
    fn a_padded_time_formal_reads_as_the_trimmed_time() {
        let generation = |time: &str| {
            format!(
                r#"{{{PFX}, "wasGeneratedBy": {{"ex:g": {{"prov:entity": "ex:e", "prov:time": {time}}}}}}}"#
            )
        };
        let plain = doc(&generation(r#""2011-11-16T16:05:33Z""#));
        for padded in [
            r#"" 2011-11-16T16:05:33Z""#,
            r#""\t2011-11-16T16:05:33Z\r\n""#,
            r#"{"$": " 2011-11-16T16:05:33Z ", "type": "xsd:dateTime"}"#,
        ] {
            assert_eq!(doc(&generation(padded)), plain, "{padded}");
        }
        // Other white space is not trimmed.
        assert!(matches!(
            json::read(&generation("\"\u{A0}2011-11-16T16:05:33Z\"")),
            Err(Error::InvalidLiteral { .. })
        ));
    }

    #[test]
    fn an_ill_formed_time_formal_is_an_error() {
        for (section, key) in [
            ("wasGeneratedBy", "prov:time"),
            ("activity", "prov:startTime"),
            ("activity", "prov:endTime"),
        ] {
            let text = format!(r#"{{{PFX}, "{section}": {{"ex:x": {{"{key}": "nope"}}}}}}"#);
            assert!(
                matches!(
                    json::read(&text),
                    Err(Error::InvalidLiteral { datatype, .. }) if datatype == "xsd:dateTime"
                ),
                "{key}"
            );
        }
    }

    #[test]
    fn literals_compare_by_value_and_keep_their_text() {
        let d = doc(&entity(
            r#""ex:n": 5, "ex:s": {"$": "005", "type": "xsd:int"},
               "ex:d": 1.0, "ex:b": true, "ex:c": {"$": "1.50", "type": "xsd:decimal"}"#,
        ));
        let same = doc(&entity(
            r#""ex:n": {"$": "+5", "type": "xsd:int"}, "ex:s": 5,
               "ex:d": {"$": "1.0E0", "type": "xsd:double"},
               "ex:b": {"$": "1", "type": "xsd:boolean"},
               "ex:c": {"$": "1.5", "type": "xsd:decimal"}"#,
        ));
        assert_eq!(d, same);
        let other = doc(&entity(
            r#""ex:n": {"$": "5", "type": "xsd:long"}, "ex:s": 5, "ex:d": 1.0, "ex:b": true,
               "ex:c": {"$": "1.5", "type": "xsd:decimal"}"#,
        ));
        assert_ne!(d, other);
        // A raw number or boolean and a `$` object are both written as a `$`
        // object, each with its own text.
        let written = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&written).unwrap();
        let body = &v["entity"]["ex:e"];
        assert_eq!(
            body["ex:n"],
            serde_json::json!({"$": "5", "type": "xsd:int"})
        );
        assert_eq!(
            body["ex:s"],
            serde_json::json!({"$": "005", "type": "xsd:int"})
        );
        assert_eq!(
            body["ex:d"],
            serde_json::json!({"$": "1.0", "type": "xsd:double"})
        );
        assert_eq!(
            body["ex:b"],
            serde_json::json!({"$": "true", "type": "xsd:boolean"})
        );
        assert_eq!(
            body["ex:c"],
            serde_json::json!({"$": "1.50", "type": "xsd:decimal"})
        );
    }

    #[test]
    fn equal_values_under_one_name_are_one_pair() {
        let d = doc(&entity(
            r#""ex:v": [{"$": " 5 ", "type": "xsd:int"}, 5, {"$": "5", "type": "xsd:int"},
                       {"$": "5", "type": "xsd:long"}]"#,
        ));
        // The first spelling stays; the `xsd:long` is another value.
        let read = values(&d, "v");
        assert_eq!(read.len(), 2);
        assert!(matches!(&read[0], Value::Typed(t) if t.lexical() == " 5 "));
        assert_eq!(read[1].datatype(), Some(XSD_LONG.clone()));
    }

    #[test]
    fn an_ill_formed_language_tag_is_an_error() {
        assert!(matches!(
            json::read(&entity(r#""ex:k": {"$": "x", "lang": "en_US"}"#)),
            Err(Error::InvalidLanguageTag(tag)) if tag == "en_US"
        ));
        let d = doc(&entity(
            r#""ex:k": {"$": "x", "lang": "en-GB-x-averylongsubtag"}"#,
        ));
        assert!(
            matches!(&values(&d, "k")[0], Value::LangStr(l) if l.lang() == "en-GB-x-averylongsubtag")
        );
    }

    #[test]
    fn json_errors_name_the_problem() {
        let message = |s: &str| json::read(s).unwrap_err().to_string();
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
                format!(r#"{{{PFX}, "entity": {{"ex:e": [3]}}}}"#),
                "invalid type: integer `3`, expected a map",
            ),
            (
                format!(
                    r#"{{{PFX}, "used": {{"_:u": {{"prov:entity": {{"$": "ex:e", "lang": "en"}}}}}}}}"#
                ),
                "`prov:entity` must be a name, not a language-tagged string",
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
            Some(&QName::new("http://b.org/", "e").unwrap())
        );
        assert_eq!(d.bundles[0].records[1].id(), Some(&ex("f")));
        assert_eq!(d.bundles[0].namespaces.iter().count(), 1);
    }

    #[test]
    fn strict_rules_and_shapes() {
        let read = |s: &str| json::read(s);
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
            Err(Error::Json(_))
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
        for text in [
            format!(r#"{{{PFX}, "prefix": {{}}}}"#),
            format!(r#"{{{PFX}, "bundle": {{}}, "bundle": {{}}}}"#),
            format!(r#"{{{PFX}, "bundle": {{"ex:b": {{"prefix": {{}}, "prefix": {{}}}}}}}}"#),
        ] {
            assert!(matches!(json::read(&text), Err(Error::Json(_))), "{text}");
        }
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
            Some(&QName::new("http://shadow.org/", "e").unwrap())
        );
    }

    #[test]
    fn bundle_id_resolves_in_bundle_scope() {
        let d = doc(r#"{"prefix": {"ex": "http://example.org/"},
            "bundle": {"ex:b": {"prefix": {"ex": "http://shadow.org/"}, "entity": {"ex:e": {}}}}}"#);
        assert_eq!(
            d.bundles[0].id,
            QName::new("http://shadow.org/", "b").unwrap()
        );
        assert_eq!(
            d.bundles[0].records[0].id(),
            Some(&QName::new("http://shadow.org/", "e").unwrap())
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
