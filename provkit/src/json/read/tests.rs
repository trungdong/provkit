use super::*;
use crate::json;
use crate::test_util::ex;
use crate::well_known::{XSD_BOOLEAN, XSD_INT, XSD_INTEGER, XSD_LONG};

fn doc(json: &str) -> crate::Document {
    json::read(json).unwrap()
}
const PFX: &str = r#""prefix": {"ex": "http://example.org/", "default": "http://default.org/"}"#;

#[test]
fn a_language_string_reads_with_or_without_a_type() {
    let read = |member: &str| {
        let d = doc(&format!(
            r#"{{{PFX}, "entity": {{"ex:e": {{"ex:a": {member}}}}}}}"#
        ));
        let Record::Entity(e) = &d.records[0] else {
            panic!("expected an entity, got {:?}", d.records[0]);
        };
        e.attributes.get(&ex("a")).cloned().collect::<Vec<_>>()
    };
    let expected = [Value::from(LangStr::new("bonjour", "fr").unwrap())];
    assert_eq!(read(r#"{"$": "bonjour", "lang": "fr"}"#), expected);
    assert_eq!(
        read(r#"{"$": "bonjour", "type": "prov:InternationalizedString", "lang": "fr"}"#),
        expected
    );
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
    // The reader stops at the second bracket, not at serde_json's
    // recursion limit.
    assert!(matches!(
        json::read(&deep),
        Err(Error::JsonStructure { ref message }) if message.contains("nested arrays")
    ));
}

#[test]
fn a_megabyte_of_input_text_stays_out_of_every_message() {
    use serde_json::json;
    let big = "x".repeat(1 << 20);
    let prefix = json!({
        "ex": "http://example.org/",
        "d": format!("http://{big}/"),
        big.as_str(): "http://www.w3.org/ns/prov#",
    });
    // Each document with the fragment its message must hold, so the
    // case reaches the site it names.
    let cases = [
        // An unknown section name.
        (json!({big.as_str(): {}}), "unknown record kind"),
        // An unknown key in a value object.
        (
            json!({"prefix": prefix, "entity": {"ex:e": {"ex:a": {"$": "v", big.as_str(): "w"}}}}),
            "unknown key",
        ),
        // A formal key, under a prefix bound to the PROV namespace.
        (
            json!({"prefix": prefix, "wasGeneratedBy": {"ex:g": {format!("{big}:activity"): 5}}}),
            "must be a name, not a number",
        ),
        // A datatype in a namespace of that length, on a time formal.
        (
            json!({"prefix": prefix, "wasGeneratedBy": {"ex:g": {
                "prov:time": {"$": "2011-11-16T16:05:33Z", "type": "d:t"}
            }}}),
            "must be a dateTime, not a value typed <",
        ),
        // The same, on a name formal.
        (
            json!({"prefix": prefix, "wasGeneratedBy": {"ex:g": {
                "prov:activity": {"$": "ex:a", "type": "d:t"}
            }}}),
            "must be a name, not a value typed <",
        ),
        // A record key with an undeclared prefix of that length.
        (
            json!({"prefix": prefix, "entity": {format!("{big}y:e"): {}}}),
            "undeclared prefix",
        ),
    ];
    for (document, fragment) in cases {
        let text = document.to_string();
        let message = json::read(&text).map(|_| ()).unwrap_err().to_string();
        assert!(
            message.contains(fragment),
            "{fragment}: {}",
            &message[..200]
        );
        assert!(
            message.contains("characters)"),
            "{fragment}: {}",
            &message[..200]
        );
        assert!(message.len() < 400, "{fragment}: {} bytes", message.len());
    }
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

/// The section, key and inner error of an [`Error::InRecord`].
fn in_record(e: Error) -> (String, String, Error) {
    match e {
        Error::InRecord {
            section,
            key,
            error,
        } => (section, key, *error),
        other => panic!("expected Error::InRecord, got {other:?}"),
    }
}

#[test]
fn the_shape_of_the_json_decides_the_variant() {
    let structure = [
        "[]".to_owned(),
        r#"{"entity": 3}"#.to_owned(),
        format!(r#"{{{PFX}, "entity": {{"ex:e": {{"ex:a": null}}}}}}"#),
        format!(r#"{{{PFX}, "entity": {{"ex:e": {{"ex:a": {{"$": 1}}}}}}}}"#),
        format!(r#"{{{PFX}, "prefix": {{}}}}"#),
    ];
    for text in structure {
        match json::read(&text) {
            Err(Error::JsonStructure { message }) => {
                assert!(message.contains(" at line 1 column "), "{text}\n{message}");
            }
            other => panic!("{text}: expected JsonStructure, got {other:?}"),
        }
    }
    for text in ["{", r#"{"entity": 01}"#, "", r#"{"entity": }"#] {
        assert!(matches!(json::read(text), Err(Error::Json(_))), "{text}");
    }
}

#[test]
fn a_section_is_checked_by_name_before_its_body() {
    let cases = [
        (r#"{"foo": "bar"}"#.to_owned(), "unknown record kind `foo`"),
        (
            r#"{"@context": {"ex": "http://example.org/"}}"#.to_owned(),
            "unknown record kind `@context`",
        ),
        (
            r#"{"derivedByInsertionFrom": {"_:d": {"prov:key-entity-set": [{"key": "k", "$": "e"}]}}}"#
                .to_owned(),
            "unknown record kind `derivedByInsertionFrom`",
        ),
        (
            format!(
                r#"{{{PFX}, "bundle": {{"ex:b": {{"bundle": {{"ex:c": {{"entity": {{"ex:e": {{}}}}}}}}}}}}}}"#
            ),
            "a bundle cannot hold a bundle",
        ),
    ];
    for (text, needle) in cases {
        match json::read(&text) {
            Err(Error::JsonStructure { message }) => {
                assert!(message.contains(needle), "{text}\n{message}");
            }
            other => panic!("{text}: expected JsonStructure, got {other:?}"),
        }
    }
}

#[test]
fn an_error_in_a_record_names_the_record() {
    let (section, key, error) = in_record(json::read(r#"{"entity": {"zz:e": {}}}"#).unwrap_err());
    assert_eq!((section.as_str(), key.as_str()), ("entity", "zz:e"));
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));

    let text = format!(r#"{{{PFX}, "activity": {{"ex:a": {{"prov:startTime": "soon"}}}}}}"#);
    let e = json::read(&text).unwrap_err();
    assert!(e.to_string().starts_with("activity `ex:a`: "), "{e}");
    let (_, _, error) = in_record(e);
    assert!(matches!(error, Error::InvalidLiteral { .. }));

    let text = format!(r#"{{{PFX}, "bundle": {{"zz:b": {{}}}}}}"#);
    let (section, key, _) = in_record(json::read(&text).unwrap_err());
    assert_eq!((section.as_str(), key.as_str()), ("bundle", "zz:b"));
}

#[test]
fn errors() {
    let read = |s: &str| json::read(s);
    let (_, _, error) =
        in_record(read(&format!(r#"{{{PFX}, "entity": {{"_:e": {{}}}}}}"#)).unwrap_err());
    assert!(matches!(error, Error::JsonStructure { .. }));
    assert!(matches!(
        read(&format!(r#"{{{PFX}, "foo": {{"ex:e": {{}}}}}}"#)),
        Err(Error::JsonStructure { .. })
    ));
    let (_, _, error) = in_record(read(r#"{"entity": {"zz:e": {}}}"#).unwrap_err());
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
    let (_, _, error) = in_record(
        read(&format!(
            r#"{{{PFX}, "activity": {{"ex:a": {{"prov:startTime": "soon"}}}}}}"#
        ))
        .unwrap_err(),
    );
    assert!(matches!(error, Error::InvalidLiteral { .. }));
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
            let (_, _, error) = in_record(json::read(&text).unwrap_err());
            assert!(
                matches!(&error, Error::InvalidQName(t) if t == carried),
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
    let (_, _, error) = in_record(
        json::read(&entity(
            r#""ex:k": {"$": "zz:foo", "type": "prov:QUALIFIED_NAME"}"#,
        ))
        .unwrap_err(),
    );
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
    let (_, _, error) =
        in_record(json::read(&entity(r#""ex:k": {"$": "", "type": "xsd:QName"}"#)).unwrap_err());
    assert!(matches!(error, Error::InvalidQName(t) if t.is_empty()));
}

#[test]
fn a_formal_written_as_an_object_is_checked() {
    let doc = |formal: &str| {
        format!(
            r#"{{"prefix": {{"ex": "http://example.org/"}}, "wasGeneratedBy": {{"ex:g": {{"prov:entity": "ex:e", "prov:activity": {formal}}}}}}}"#
        )
    };
    for accepted in [
        r#"{"$": "ex:a", "type": "prov:QUALIFIED_NAME"}"#,
        r#"{"$": "ex:a", "type": "xsd:QName"}"#,
    ] {
        let d = json::read(&doc(accepted)).unwrap();
        assert!(
            matches!(&d.records[0], Record::Generation(g) if g.activity == Some(ex("a"))),
            "{accepted}: {:?}",
            d.records
        );
    }
    for bad in [
        r#"{"$": "ex:a", "type": "xsd:string"}"#,
        r#"{"$": "ex:a", "lang": "en"}"#,
        r#"{"$": "ex:a", "type": "prov:QUALIFIED_NAME", "lang": "en"}"#,
    ] {
        let (_, _, error) = in_record(json::read(&doc(bad)).unwrap_err());
        match error {
            Error::JsonStructure { message } => {
                assert!(message.contains("prov:activity"), "{bad}\n{message}");
            }
            other => panic!("{bad}: {other:?}"),
        }
    }
    let undeclared = r#"{"$": "ex:a", "type": "zz:undeclared"}"#;
    let (_, _, error) = in_record(json::read(&doc(undeclared)).unwrap_err());
    assert!(
        matches!(error, Error::UndeclaredPrefix(ref p) if p == "zz"),
        "{error:?}"
    );
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
    let names_the_formal = |text: &str, formal: &str| {
        let (_, _, error) = in_record(json::read(text).unwrap_err());
        match error {
            Error::JsonStructure { message } => {
                assert!(message.contains(formal), "{text}\n{message}");
            }
            other => panic!("{text}: {other:?}"),
        }
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
    let (_, _, error) =
        in_record(json::read(&generation(r#"{"$": "x", "type": "zz:t"}"#)).unwrap_err());
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
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
    let (_, _, error) =
        in_record(json::read(&generation("\"\u{A0}2011-11-16T16:05:33Z\"")).unwrap_err());
    assert!(matches!(error, Error::InvalidLiteral { .. }));
}

#[test]
fn an_ill_formed_time_formal_is_an_error() {
    for (section, key) in [
        ("wasGeneratedBy", "prov:time"),
        ("activity", "prov:startTime"),
        ("activity", "prov:endTime"),
    ] {
        let text = format!(r#"{{{PFX}, "{section}": {{"ex:x": {{"{key}": "nope"}}}}}}"#);
        let (_, _, error) = in_record(json::read(&text).unwrap_err());
        assert!(
            matches!(&error, Error::InvalidLiteral { datatype, .. } if datatype == "xsd:dateTime"),
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
    let (_, _, error) =
        in_record(json::read(&entity(r#""ex:k": {"$": "x", "lang": "en_US"}"#)).unwrap_err());
    assert!(matches!(error, Error::InvalidLanguageTag(tag) if tag == "en_US"));
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
    let cases = [
        (
            entity(r#""ex:a": {"$": 1}"#),
            "invalid type: integer `1`, expected a string as the `$` of a value object",
        ),
        (
            entity(r#""ex:a": {"$": "1", "type": 2}"#),
            "expected a string as the `type` of a value object",
        ),
        (
            entity(r#""ex:a": {"$": "1", "datatype": "xsd:int"}"#),
            "unknown key `datatype` in a value object, expected `$`, `type` or `lang`",
        ),
        (
            entity(r#""ex:a": {"type": "xsd:int"}"#),
            "a value object needs a `$` key",
        ),
        (entity(r#""ex:a": null"#), "null is not an attribute value"),
        (
            entity(r#""ex:a": ["x", null]"#),
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
            "a bundle cannot hold a bundle",
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
    let (_, _, error) =
        in_record(read(&entity(r#""ex:a": {"$": "1", "type": "zz:t"}"#)).unwrap_err());
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
    let (_, _, error) = in_record(
        read(&entity(
            r#""ex:a": {"$": "1", "type": "zz:t", "lang": "en"}"#,
        ))
        .unwrap_err(),
    );
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
    assert!(matches!(
        read(&entity(r#""ex:a": [["x"]]"#)),
        Err(Error::JsonStructure { .. })
    ));
    let (_, _, error) = in_record(
        read(&format!(
            r#"{{{PFX}, "used": {{"_:u": {{"prov:entity": 5}}}}}}"#
        ))
        .unwrap_err(),
    );
    assert!(matches!(error, Error::JsonStructure { .. }));
    let (section, _, error) = in_record(
        read(r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"_:b": {}}}"#).unwrap_err(),
    );
    assert_eq!(section, "bundle");
    assert!(matches!(error, Error::JsonStructure { .. }));
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
        assert!(
            matches!(json::read(&text), Err(Error::JsonStructure { .. })),
            "{text}"
        );
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
