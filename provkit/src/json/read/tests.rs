use super::*;
use crate::json;
use crate::test_util::{ex, round_trip, written};
use crate::well_known::{XSD_BOOLEAN, XSD_DATETIME, XSD_INT, XSD_INTEGER, XSD_LONG};

#[test]
fn a_json_syntax_error_carries_its_position_and_its_source() {
    let Err(Error::Json(e)) = json::read("{") else {
        panic!("expected Error::Json")
    };
    assert_eq!((e.line(), e.column()), (Some(1), Some(1)));
    assert!(e.to_string().ends_with(" at line 1 column 1"), "{e}");
    let source = std::error::Error::source(&e).expect("source");
    assert!(source.is::<serde_json::Error>());
    // The outer error prints the inner one and forwards its source.
    let outer = Error::Json(e);
    assert!(
        outer.to_string().ends_with(" at line 1 column 1"),
        "{outer}"
    );
    assert!(std::error::Error::source(&outer).is_some());
}

fn doc(json: &str) -> crate::Document {
    json::read(json).unwrap()
}
const PFX: &str = r#""prefix": {"ex": "http://example.org/", "default": "http://default.org/"}"#;

/// A document holding one record, `kind` `key`, whose body is `body`.
fn record(kind: &str, key: &str, body: &str) -> String {
    format!(r#"{{{PFX}, "{kind}": {{"{key}": {{{body}}}}}}}"#)
}

/// A document holding one entity `ex:e` with the attributes in `body`.
fn entity(body: &str) -> String {
    record("entity", "ex:e", body)
}

/// A one-line document with one record, `kind` `key`, whose body is `body`.
/// The text is fixed because tests pin columns in it.
fn one_record(kind: &str, key: &str, body: &str) -> String {
    format!(r#"{{"prefix": {{"ex": "http://example.org/"}}, "{kind}": {{"{key}": {{{body}}}}}}}"#)
}

/// The values under `ex:<local>` of `record`.
fn values_of(record: &Record, local: &str) -> Vec<Value> {
    record.attributes().get(&ex(local)).cloned().collect()
}

/// The values under `ex:<local>` of the first record.
fn values(d: &crate::Document, local: &str) -> Vec<Value> {
    values_of(&d.records[0], local)
}

/// Section, key and inner error of the `Error::InRecord` that reading `text` gives.
fn record_error(text: &str) -> (String, String, Error) {
    match json::read(text) {
        Err(Error::InRecord {
            section,
            key,
            error,
        }) => (section, key, *error),
        other => panic!("{text}: expected Error::InRecord, got {other:?}"),
    }
}

/// Section, key and message of an `Error::JsonStructure` inside a record.
fn record_structure(text: &str) -> (String, String, String) {
    match record_error(text) {
        (s, k, Error::JsonStructure { message }) => (s, k, message),
        (_, _, other) => panic!("{text}: expected JsonStructure, got {other:?}"),
    }
}

#[test]
fn a_language_string_reads_with_or_without_a_type() {
    let read = |member: &str| {
        let d = doc(&entity(&format!(r#""ex:a": {member}"#)));
        let Record::Entity(_) = &d.records[0] else {
            panic!("expected an entity, got {:?}", d.records[0]);
        };
        values(&d, "a")
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
    let d = doc(&entity(
        r#"
        "ex:s": "text",
        "ex:i": 5, "ex:l": 3000000000, "ex:b": 18446744073709551615, "ex:d": 1.5, "ex:t": true,
        "ex:o": {"$": "x"}, "ex:xs": {"$": "x", "type": "xsd:string"},
        "ex:lang": {"$": "bonjour", "type": "prov:InternationalizedString", "lang": "fr"},
        "ex:qn": {"$": "ex:q", "type": "xsd:QName"}, "ex:legacy": {"$": "ex:q", "type": "prov:QUALIFIED_NAME"},
        "ex:dt": {"$": "2011-11-16T16:05:33Z", "type": "xsd:dateTime"},
        "ex:ty": {"$": "1", "type": "xsd:long"},
        "ex:many": ["a", {"$": "2", "type": "xsd:int"}]
    "#,
    ));
    let Record::Entity(e) = &d.records[0] else {
        panic!()
    };
    let get = |l: &str| values(&d, l);
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

    // An entity has no `time` or `activity` formal, so these names are
    // ordinary attributes and each value keeps the type it was written with
    // (PROV-JSON section 2.2).
    let formal_name = |body: &str, local: &str| {
        let d = doc(&entity(body));
        let name = QName::new(crate::well_known::PROV, local).unwrap();
        d.records[0]
            .attributes()
            .get(&name)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        formal_name(r#""prov:time": "2012-01-01T00:00:00""#, "time"),
        [Value::Str("2012-01-01T00:00:00".into())]
    );
    assert_eq!(
        formal_name(r#""prov:time": "hello""#, "time"),
        [Value::Str("hello".into())]
    );
    let timed = formal_name(
        r#""prov:time": {"$": "2012-01-01T00:00:00", "type": "xsd:dateTime"}"#,
        "time",
    );
    assert_eq!(timed.len(), 1);
    assert!(matches!(timed[0], Value::DateTime(_)));
    assert_eq!(
        formal_name(r#""prov:activity": "ex:a""#, "activity"),
        [Value::Str("ex:a".into())]
    );
    for qualified in ["xsd:QName", "prov:QUALIFIED_NAME"] {
        assert_eq!(
            formal_name(
                &format!(r#""prov:activity": {{"$": "ex:a", "type": "{qualified}"}}"#),
                "activity"
            ),
            [Value::QName(ex("a"))]
        );
    }
}

#[test]
fn integers_type_by_range() {
    let d = doc(&entity(
        r#"
        "ex:min": -2147483648, "ex:max": 2147483647, "ex:over": 2147483648,
        "ex:low": -2147483649, "ex:i64": 9223372036854775807,
        "ex:u64": 9223372036854775808, "ex:neg": -9223372036854775808,
        "ex:huge": 12345678901234567890123, "ex:exp": 1e3
    "#,
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
    let d = doc(&entity(
        r#"
        "ex:zero": -0, "ex:low": -9223372036854775809, "ex:high": 18446744073709551616,
        "ex:huge": 1e400, "ex:frac": 1.50, "ex:exp": 1E2, "ex:negzero": -0.0
    "#,
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
    let json = doc(&entity(
        r#""ex:n": [-0, 0, 3000000000, -9223372036854775809]"#,
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
    let (_, _, message) = record_structure(&entity(r#""ex:n": [[1]]"#));
    assert!(message.contains("nested arrays"), "{message}");
    let deep = entity(&format!(
        r#""ex:n": {}1{}"#,
        "[".repeat(100_000),
        "]".repeat(100_000)
    ));
    // The value's text is captured by a scan that does not recurse, and
    // the second bracket is refused by its first byte, before it is parsed.
    // So the depth is never recursed into, and serde_json's recursion limit
    // is not what stops it.
    let (_, _, message) = record_structure(&deep);
    assert!(message.contains("nested arrays"), "{message}");
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
fn a_string_serde_quotes_is_cut_without_a_count() {
    use serde_json::json;
    // serde prints the string escaped: each `"` as two characters, and the
    // odd one out puts a backslash where the cut falls.
    let quotes = "\"".repeat(1 << 20);
    let odd = format!("x{quotes}");
    let cases = [
        // A string where a record belongs.
        (
            json!({"entity": {"ex:e": quotes.as_str()}}),
            "expected a record object or an array of record objects",
        ),
        // A string where the namespace table belongs.
        (json!({"prefix": odd.as_str()}), "expected a map"),
        (json!({"prefix": "x".repeat(1 << 20)}), "expected a map"),
    ];
    for (document, fragment) in cases {
        let message = json::read(&document.to_string())
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(message.len() < 300, "{fragment}: {} bytes", message.len());
        assert!(message.contains(fragment), "{message}");
        assert!(message.contains("...\", expected"), "{message}");
        assert!(!message.contains("\\...\", expected"), "{message}");
        assert!(!message.contains("characters)"), "{message}");
    }
}

#[test]
fn any_uri_stays_a_typed_literal() {
    let d = doc(&entity(
        r#""ex:u": {"$": "http://x.example/", "type": "xsd:anyURI"}"#,
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
    let d = doc(&record(
        "hadMember",
        "ex:c",
        &format!(
            r#""prov:collection": "ex:c",
            "prov:entity": [{}], "ex:tag": [{}]"#,
            members.join(","),
            attrs.join(",")
        ),
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
fn the_shape_of_the_json_decides_the_variant() {
    let positioned = |text: &str, error: Error| match error {
        Error::JsonStructure { message } => {
            assert!(message.contains(" at line 1 column "), "{text}\n{message}");
        }
        other => panic!("{text}: expected JsonStructure, got {other:?}"),
    };
    let wrong_shape = [
        "[]".to_owned(),
        r#"{"entity": 3}"#.to_owned(),
        format!(r#"{{{PFX}, "prefix": {{}}}}"#),
    ];
    for text in wrong_shape {
        positioned(&text, json::read(&text).unwrap_err());
    }
    // A value of the wrong shape is inside a record.
    let values = [entity(r#""ex:a": null"#), entity(r#""ex:a": {"$": 1}"#)];
    for text in values {
        let (_, _, error) = record_error(&text);
        positioned(&text, error);
    }
    for text in [
        "{",
        r#"{"a": 01}"#,
        r#"{"entity": 01}"#,
        "",
        r#"{"a": }"#,
        r#"{"entity": }"#,
    ] {
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
            r#"{"hadDictionaryMember": {"_:d": {"prov:dictionary": "ex:d", "prov:key": "k"}}}"#
                .to_owned(),
            "unknown record kind `hadDictionaryMember`",
        ),
        (
            r#"{"derivedByRemovalFrom": {"_:d": {"prov:after": "ex:d2", "prov:before": "ex:d1"}}}"#
                .to_owned(),
            "unknown record kind `derivedByRemovalFrom`",
        ),
        (
            record(
                "bundle",
                "ex:b",
                r#""bundle": {"ex:c": {"entity": {"ex:e": {}}}}"#,
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
    let (section, key, error) = record_error(r#"{"entity": {"zz:e": {}}}"#);
    assert_eq!((section.as_str(), key.as_str()), ("entity", "zz:e"));
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));

    let text = record("activity", "ex:a", r#""prov:startTime": "soon""#);
    let e = json::read(&text).unwrap_err();
    assert!(e.to_string().starts_with("activity `ex:a`: "), "{e}");
    let (_, _, error) = record_error(&text);
    assert!(matches!(error, Error::InvalidLiteral { .. }));

    let (section, key, _) = record_error(&record("bundle", "zz:b", ""));
    assert_eq!((section.as_str(), key.as_str()), ("bundle", "zz:b"));
}

#[test]
fn errors() {
    let (_, _, error) = record_error(&record("entity", "_:e", ""));
    assert!(matches!(error, Error::JsonStructure { .. }));
    assert!(matches!(
        json::read(&record("foo", "ex:e", "")),
        Err(Error::JsonStructure { .. })
    ));
    let (_, _, error) = record_error(r#"{"entity": {"zz:e": {}}}"#);
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
    let (_, _, error) = record_error(&record("activity", "ex:a", r#""prov:startTime": "soon""#));
    assert!(matches!(error, Error::InvalidLiteral { .. }));
    assert!(matches!(json::read("{"), Err(Error::Json(_))));
    assert!(matches!(
        json::read(r#"{"prefix": {"prov": "http://x/"}}"#),
        Err(Error::ReservedPrefix { .. })
    ));
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
    let (written, v) = round_trip(&d);
    for (i, (text, datatype)) in literals.iter().enumerate() {
        assert_eq!(
            v["entity"]["ex:e"][format!("ex:k{i}")],
            serde_json::json!({"$": text, "type": datatype}),
            "{written}"
        );
    }
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
            let (_, _, error) = record_error(&text);
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
    let (_, _, error) = record_error(&entity(
        r#""ex:k": {"$": "zz:foo", "type": "prov:QUALIFIED_NAME"}"#,
    ));
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
    let (_, _, error) = record_error(&entity(r#""ex:k": {"$": "", "type": "xsd:QName"}"#));
    assert!(matches!(error, Error::InvalidQName(t) if t.is_empty()));
}

#[test]
fn a_formal_written_as_an_object_is_checked() {
    let generation = |formal: &str| {
        one_record(
            "wasGeneratedBy",
            "ex:g",
            &format!(r#""prov:entity": "ex:e", "prov:activity": {formal}"#),
        )
    };
    for accepted in [
        r#"{"$": "ex:a", "type": "prov:QUALIFIED_NAME"}"#,
        r#"{"$": "ex:a", "type": "xsd:QName"}"#,
    ] {
        let d = doc(&generation(accepted));
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
        let (_, _, message) = record_structure(&generation(bad));
        assert!(message.contains("prov:activity"), "{bad}\n{message}");
    }
    let undeclared = r#"{"$": "ex:a", "type": "zz:undeclared"}"#;
    let (_, _, error) = record_error(&generation(undeclared));
    assert!(
        matches!(error, Error::UndeclaredPrefix(ref p) if p == "zz"),
        "{error:?}"
    );
}

#[test]
fn a_formal_object_of_the_wrong_shape_names_the_formal() {
    let generation = |formal: &str| {
        record(
            "wasGeneratedBy",
            "ex:g",
            &format!(r#""prov:entity": "ex:e", "prov:time": {formal}"#),
        )
    };
    let d = doc(&generation(
        r#"{"$": "2011-11-16T16:05:33Z", "type": "xsd:dateTime"}"#,
    ));
    assert!(matches!(&d.records[0], Record::Generation(g) if g.time.is_some()));
    let names_the_formal = |text: &str, formal: &str| {
        let (_, _, message) = record_structure(text);
        assert!(message.contains(formal), "{text}\n{message}");
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
            &record(
                "wasGeneratedBy",
                "ex:g",
                &format!(r#""prov:entity": "ex:e", "prov:activity": {bad}"#),
            ),
            "prov:activity",
        );
        names_the_formal(
            &record(
                "hadMember",
                "ex:m",
                &format!(r#""prov:collection": "ex:c", "prov:entity": ["ex:a", {bad}]"#),
            ),
            "prov:entity",
        );
    }
    // An undeclared prefix in the `type` is reported as such.
    let (_, _, error) = record_error(&generation(r#"{"$": "x", "type": "zz:t"}"#));
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
}

#[test]
fn an_empty_member_array_is_refused_and_one_or_many_members_read() {
    let members = |entity: &str| {
        record(
            "hadMember",
            "ex:m",
            &format!(r#""prov:collection": "ex:c", "prov:entity": {entity}"#),
        )
    };
    let (section, key, error) = record_error(&members("[]"));
    assert_eq!((section.as_str(), key.as_str()), ("hadMember", "ex:m"));
    assert!(
        matches!(&error, Error::InvalidRecord { kind: Kind::Membership, message } if message == "prov:entity is an empty array"),
        "{error:?}"
    );
    assert_eq!(doc(&members(r#"["ex:a"]"#)).records.len(), 1);
    assert_eq!(
        doc(&members(r#"["ex:a", "ex:b", "ex:d"]"#)).records.len(),
        3
    );
}

#[test]
fn a_relation_without_a_mandatory_party_is_refused() {
    let cases = [
        ("hadMember", "prov:collection", "prov:entity"),
        ("alternateOf", "prov:alternate1", "prov:alternate2"),
        (
            "specializationOf",
            "prov:specificEntity",
            "prov:generalEntity",
        ),
        ("mentionOf", "prov:specificEntity", "prov:generalEntity"),
    ];
    for (section, first, second) in cases {
        for (absent, given) in [(first, second), (second, first)] {
            let body = format!(r#""{given}": "ex:p""#);
            let (found, _, error) = record_error(&record(section, "ex:r", &body));
            assert_eq!(found, section);
            assert!(
                matches!(&error, Error::InvalidRecord { message, .. } if *message == format!("needs {absent}")),
                "{section} {absent}: {error:?}"
            );
        }
        let (_, _, error) = record_error(&record(section, "ex:r", ""));
        assert!(
            matches!(&error, Error::InvalidRecord { message, .. } if *message == format!("needs {first}")),
            "{section}: {error:?}"
        );
    }
    // A mention needs no bundle.
    let body = r#""prov:specificEntity": "ex:s", "prov:generalEntity": "ex:g""#;
    assert_eq!(doc(&record("mentionOf", "ex:r", body)).records.len(), 1);
}

#[test]
fn a_padded_time_formal_reads_as_the_trimmed_time() {
    let generation = |time: &str| {
        record(
            "wasGeneratedBy",
            "ex:g",
            &format!(r#""prov:entity": "ex:e", "prov:time": {time}"#),
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
    let (_, _, error) = record_error(&generation("\"\u{A0}2011-11-16T16:05:33Z\""));
    assert!(matches!(error, Error::InvalidLiteral { .. }));
}

#[test]
fn an_ill_formed_time_formal_is_an_error() {
    for (section, key) in [
        ("wasGeneratedBy", "prov:time"),
        ("activity", "prov:startTime"),
        ("activity", "prov:endTime"),
    ] {
        let text = record(section, "ex:x", &format!(r#""{key}": "nope""#));
        let (_, _, error) = record_error(&text);
        assert!(
            matches!(&error, Error::InvalidLiteral { datatype, .. } if *datatype == *XSD_DATETIME),
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
    let (_, v) = written(&d);
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
    // An `xsd:float` compares in single precision, so these are one value.
    let d = doc(&entity(
        r#""ex:f": [{"$": "16777216", "type": "xsd:float"}, {"$": "16777217", "type": "xsd:float"}]"#,
    ));
    let read = values(&d, "f");
    assert_eq!(read.len(), 1);
    assert!(matches!(&read[0], Value::Typed(t) if t.lexical() == "16777216"));
}

#[test]
fn an_ill_formed_language_tag_is_an_error() {
    let (_, _, error) = record_error(&entity(r#""ex:k": {"$": "x", "lang": "en_US"}"#));
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
            "expected a string as the `$` of a value object",
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
            record(
                "used",
                "_:u",
                r#""prov:entity": {"$": "ex:e", "lang": "en"}"#,
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
            record("bundle", "ex:b", r#""bundle": {}"#),
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
    let (_, _, error) = record_error(&entity(r#""ex:a": {"$": "1", "type": "zz:t"}"#));
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
    let (_, _, error) = record_error(&entity(
        r#""ex:a": {"$": "1", "type": "zz:t", "lang": "en"}"#,
    ));
    assert!(matches!(error, Error::UndeclaredPrefix(p) if p == "zz"));
    let (_, _, error) = record_error(&entity(r#""ex:a": [["x"]]"#));
    assert!(matches!(error, Error::JsonStructure { .. }));
    let (_, _, error) = record_error(&record("used", "_:u", r#""prov:entity": 5"#));
    assert!(matches!(error, Error::JsonStructure { .. }));
    let (section, _, error) =
        record_error(r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"_:b": {}}}"#);
    assert_eq!(section, "bundle");
    assert!(matches!(error, Error::JsonStructure { .. }));
}

#[test]
fn repeated_keys_in_a_document_or_bundle() {
    // A repeated `prefix` or `bundle` is an error.
    for text in [
        format!(r#"{{{PFX}, "prefix": {{}}}}"#),
        format!(r#"{{{PFX}, "bundle": {{}}, "bundle": {{}}}}"#),
        record("bundle", "ex:b", r#""prefix": {}, "prefix": {}"#),
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
    let d = doc(&record(
        "activity",
        "ex:a",
        r#""prov:endTime": "2011-11-16T16:05:33Z", "ex:f": false"#,
    ));
    let Record::Activity(a) = &d.records[0] else {
        panic!()
    };
    assert!(a.end_time.is_some() && a.start_time.is_none());
    assert_eq!(
        values(&d, "f"),
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

#[test]
fn a_formal_given_under_two_spellings_is_an_invalid_record() {
    // A key that spells an own formal is the formal, never an attribute, so
    // the only way to meet the check is to give one formal twice.
    let text = r#"{"prefix": {"ex": "http://example.org/", "w3": "http://www.w3.org/ns/"},
        "wasGeneratedBy": {"_:g": {"prov:activity": "ex:a", "w3:prov#activity": "ex:b"}}}"#;
    let (section, _, error) = record_error(text);
    assert_eq!(section, "wasGeneratedBy");
    assert!(
        matches!(
            error,
            Error::InvalidRecord {
                kind: Kind::Generation,
                ..
            }
        ),
        "{error:?}"
    );
    // On a `hadMember`, `prov:entity` is a member list, so two spellings
    // give the members of both.
    let text = r#"{"prefix": {"ex": "http://example.org/", "w3": "http://www.w3.org/ns/"},
        "hadMember": {"ex:m": {"prov:collection": "ex:c", "prov:entity": "ex:a", "w3:prov#entity": "ex:b"}}}"#;
    let d = doc(text);
    let members: Vec<_> = d
        .records
        .iter()
        .map(|r| match r {
            Record::Membership(m) => m.entity.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(members, [Some(ex("a")), Some(ex("b"))]);
}

#[test]
fn a_prefix_that_is_not_a_pn_prefix_is_invalid() {
    let table = |prefix: &str| format!(r#"{{"prefix": {{"{prefix}": "http://example.org/"}}}}"#);
    for bad in ["_", "a/b", "1a", "a.", "a b", "-a"] {
        assert!(
            matches!(json::read(&table(bad)), Err(Error::InvalidPrefix(p)) if p == bad),
            "{bad}"
        );
    }
    let in_bundle = r#"{"bundle": {"ex:b": {"prefix": {"1a": "http://example.org/"}}}}"#;
    let (section, key, error) = record_error(in_bundle);
    assert_eq!((section.as_str(), key.as_str()), ("bundle", "ex:b"));
    assert!(matches!(error, Error::InvalidPrefix(p) if p == "1a"));
    assert!(json::read(&table("a-b")).is_ok());
}

#[test]
fn a_blank_namespace_iri_is_an_error_naming_the_declaration() {
    let blank = [
        (r#"{"prefix": {"z": ""}}"#, "z"),
        (r#"{"prefix": {"z": " "}}"#, "z"),
        (r#"{"prefix": {"z": " "}, "entity": {"z:e": {}}}"#, "z"),
        (r#"{"prefix": {"z": "\t\u00a0"}}"#, "z"),
        (r#"{"prefix": {"default": ""}}"#, ""),
        (r#"{"prefix": {"default": " "}}"#, ""),
    ];
    for (text, want) in blank {
        assert!(
            matches!(
                json::read(text),
                Err(Error::EmptyNamespace { prefix: Some(p) }) if p == want
            ),
            "{text}"
        );
    }
    let in_bundle = [
        (r#"{"bundle": {"ex:b": {"prefix": {"z": " "}}}}"#, "z"),
        (
            r#"{"bundle": {"ex:b": {"prefix": {"z": ""}, "entity": {"z:e": {}}}}}"#,
            "z",
        ),
        (r#"{"bundle": {"ex:b": {"prefix": {"default": " "}}}}"#, ""),
    ];
    for (text, want) in in_bundle {
        let (section, key, error) = record_error(text);
        assert_eq!(
            (section.as_str(), key.as_str()),
            ("bundle", "ex:b"),
            "{text}"
        );
        assert!(
            matches!(error, Error::EmptyNamespace { prefix: Some(p) } if p == want),
            "{text}"
        );
    }
    // An IRI that holds more than whitespace reads as it did.
    let d = doc(r#"{"prefix": {"z": " x"}, "entity": {"z:e": {}}}"#);
    assert_eq!(d.records.len(), 1);
    assert_eq!(d.namespaces.lookup("z"), Some(" x"));
}

#[test]
fn an_empty_array_reads_as_nothing() {
    // No value under an attribute key.
    let d = doc(&entity(r#""ex:a": [], "ex:b": "kept""#));
    assert!(values(&d, "a").is_empty(), "{:?}", values(&d, "a"));
    assert_eq!(values(&d, "b"), [Value::Str("kept".into())]);
    assert_eq!(d.records[0].attributes().len(), 1);
    // No record under an identifier.
    let d = doc(&format!(r#"{{{PFX}, "entity": {{"ex:e": []}}}}"#));
    assert!(d.records.is_empty(), "{:?}", d.records);
}

#[test]
fn a_formal_written_as_a_one_element_array_is_an_error() {
    for body in [
        r#""prov:activity": ["ex:a"]"#,
        r#""prov:time": ["2011-11-16T16:05:33Z"]"#,
    ] {
        let text = record(
            "wasGeneratedBy",
            "ex:g",
            &format!(r#""prov:entity": "ex:e", {body}"#),
        );
        let (_, _, error) = record_error(&text);
        assert!(matches!(error, Error::JsonStructure { .. }), "{body}");
    }
}

#[test]
fn a_null_attribute_value_is_an_error() {
    for body in [r#""ex:a": null"#, r#""ex:a": ["x", null]"#] {
        let (_, _, message) = record_structure(&entity(body));
        assert!(message.contains("null"), "{body}\n{message}");
    }
}

#[test]
fn a_value_object_with_an_unknown_key_or_a_non_string_dollar_is_an_error() {
    for body in [
        r#""ex:a": {"$": "1", "datatype": "xsd:int"}"#,
        r#""ex:a": {"$": "1", "type": "xsd:int", "extra": 1}"#,
        r#""ex:a": {"$": 1}"#,
        r#""ex:a": {"$": true, "type": "xsd:boolean"}"#,
        r#""ex:a": {"$": null}"#,
    ] {
        let (_, _, error) = record_error(&entity(body));
        assert!(matches!(error, Error::JsonStructure { .. }), "{body}");
    }
}

#[test]
fn a_member_fan_out_gives_every_record_the_identifier_and_the_attributes() {
    let d = doc(&record(
        "hadMember",
        "ex:m",
        r#""prov:collection": "ex:c",
            "prov:entity": ["ex:a", "ex:b", "ex:c2"], "ex:tag": "t""#,
    ));
    assert_eq!(d.records.len(), 3);
    for record in &d.records {
        assert_eq!(record.id(), Some(&ex("m")));
        assert_eq!(values_of(record, "tag"), [Value::Str("t".into())]);
    }
}

#[test]
fn a_value_error_is_placed_at_its_token_and_names_its_record() {
    let text = "{\n  \"prefix\": {\"ex\": \"http://example.org/\"},\n  \"entity\": {\n    \"ex:e\": {\n      \"ex:a\": [\n        1,\n        {\"$\": 5},\n        2,\n        3\n      ]\n    }\n  }\n}\n";
    let (section, key, message) = record_structure(text);
    assert_eq!((section.as_str(), key.as_str()), ("entity", "ex:e"));
    // The `5` is the fifteenth byte of line 7.
    assert!(message.ends_with(" at line 7 column 15"), "{message}");
    assert!(message.contains("`$`"), "{message}");
}

#[test]
fn a_value_error_sits_where_serde_json_puts_it() {
    // Each bad value with the line and column of its token, the column in
    // characters.
    let cases = [
        (r#"{"$": 5}"#, 77),
        ("null", 74),
        ("{}", 72),
        (r#"{"$":"x","zz":1}"#, 83),
        ("[1, [2]]", 75),
        (r#"["é", null]"#, 80),
    ];
    for (value, column) in cases {
        let text = one_record("entity", "ex:e", &format!(r#""ex:a": {value}"#));
        let (_, _, error) = record_error(&text);
        let expected = format!(" at line 1 column {column}");
        assert!(error.to_string().ends_with(&expected), "{value}: {error}");
        // A byte order mark is not counted.
        let (_, _, error) = record_error(&format!("\u{FEFF}{text}"));
        assert!(error.to_string().ends_with(&expected), "{value}: {error}");
    }
    // A value that spans lines: the column counts from its own line.
    let text = entity("\"ex:a\": {\"$\": \"x\",\n \"zz\": 1}");
    let (_, _, error) = record_error(&text);
    assert!(
        error.to_string().ends_with(" at line 2 column 5"),
        "{error}"
    );
}

#[test]
fn every_kind_of_bad_value_names_its_record() {
    for value in ["null", "[1,[2]]", r#"{"$":5}"#, "{}", r#"{"$":"x","zz":1}"#] {
        let text = entity(&format!(r#""ex:a": {value}"#));
        let (section, key, error) = record_error(&text);
        assert_eq!(
            (section.as_str(), key.as_str()),
            ("entity", "ex:e"),
            "{value}"
        );
        assert!(matches!(error, Error::JsonStructure { .. }), "{value}");
    }
    // A record inside a bundle, and a formal.
    let text = record(
        "bundle",
        "ex:b",
        r#""used": {"_:u": {"prov:entity": null}}"#,
    );
    let (section, key, error) = record_error(&text);
    assert_eq!((section.as_str(), key.as_str()), ("bundle", "ex:b"));
    let Error::InRecord {
        section,
        key,
        error,
    } = error
    else {
        panic!("expected a nested InRecord, got {error:?}")
    };
    assert_eq!((section.as_str(), key.as_str()), ("used", "_:u"));
    assert!(matches!(*error, Error::JsonStructure { .. }));
}

#[test]
fn text_the_parser_rejects_is_a_json_error() {
    for text in [
        "{",
        r#"{"zzz" 1}"#,
        r#"{"zzz": 01}"#,
        r#"{"entity": 01}"#,
        r#"{"prefix": {"ex": "\ud800"}}"#,
        r#"{"prefix": {}, "prefix" 1}"#,
        r#"{"bundle": {"ex:b": {"bundle" 1}}}"#,
    ] {
        assert!(matches!(json::read(text), Err(Error::Json(_))), "{text}");
    }
}

#[test]
fn a_string_that_cannot_be_decoded_in_a_value_is_a_structure_error_in_its_record() {
    // Each body with the column of the closing quote of its `"\ud800"`,
    // which is where serde_json finds the escape has no second half.
    let cases = [
        ("entity", "ex:e", r#""ex:a": "\ud800""#, 78),
        ("entity", "ex:e", r#""ex:a": ["ok", "\ud800"]"#, 85),
        ("entity", "ex:e", r#""ex:a": {"$": "\ud800"}"#, 84),
        (
            "entity",
            "ex:e",
            r#""ex:a": {"$": "x", "type": "\ud800"}"#,
            97,
        ),
        (
            "entity",
            "ex:e",
            r#""ex:a": {"$": "x", "lang": "\ud800"}"#,
            97,
        ),
        ("used", "_:u", r#""prov:entity": "\ud800""#, 82),
    ];
    for (kind, id, body, column) in cases {
        let text = one_record(kind, id, body);
        let (section, key, message) = record_structure(&text);
        assert_eq!((section.as_str(), key.as_str()), (kind, id), "{body}");
        let expected = format!(" at line 1 column {column}");
        assert!(message.ends_with(&expected), "{body}: {message}");
    }
}

#[test]
fn a_value_error_is_placed_whatever_comes_before_it() {
    // Column 93 is the closing quote of `"\ud800"`, where serde_json finds
    // the escape has no second half. The `1e400` before it is a valid
    // number that does not fit an `f64`, and it is not the error.
    let text = one_record("entity", "ex:e", r#""ex:n": 1e400, "ex:a": "\ud800""#);
    let (_, _, message) = record_structure(&text);
    assert!(message.ends_with(" at line 1 column 93"), "{message}");
    assert!(message.contains("hex escape"), "{message}");
}

#[test]
fn a_value_object_member_that_is_not_a_string_names_the_member() {
    for member in ["$", "type", "lang"] {
        for value in ["1e400", "5", "true", "null", "[1]", r#"{"a": 1}"#] {
            let body = match member {
                "$" => format!(r#""ex:a": {{"$": {value}}}"#),
                _ => format!(r#""ex:a": {{"$": "x", "{member}": {value}}}"#),
            };
            let text = one_record("entity", "ex:e", &body);
            let (section, key, message) = record_structure(&text);
            assert_eq!(
                (section.as_str(), key.as_str()),
                ("entity", "ex:e"),
                "{body}"
            );
            let expected = format!("expected a string as the `{member}` of a value object");
            assert!(message.contains(&expected), "{body}: {message}");
        }
    }
    // The position is the first byte of the member's value.
    let text = one_record("entity", "ex:e", r#""ex:a": {"$": 1e400}"#);
    let (_, _, error) = record_error(&text);
    assert!(
        error.to_string().ends_with(" at line 1 column 77"),
        "{error}"
    );
}

/// The 1-based character column just past the last `fragment` in the
/// one-line `text`, where an error raised once a value has been read is
/// placed.
fn column_after(text: &str, fragment: &str) -> usize {
    let end = text.rfind(fragment).unwrap() + fragment.len();
    text[..end].chars().count() + 1
}

/// The 1-based character column of the first byte of the last `fragment`.
fn column_of(text: &str, fragment: &str) -> usize {
    text[..text.rfind(fragment).unwrap()].chars().count() + 1
}

#[test]
fn columns_count_characters_in_every_path() {
    // One `é` on the line before each fault. A byte count would put every
    // column one further right.
    let prefix = r#"{"prefix": {"ex": "http://é.example/"}, "#;

    // A value error inside a fragment, on the fragment's first line.
    let text = format!(r#"{prefix}"entity": {{"ex:e": {{"ex:a": null}}}}}}"#);
    let (_, _, message) = record_structure(&text);
    let expected = format!(" at line 1 column {}", column_after(&text, "null") - 1);
    assert!(message.ends_with(&expected), "{text}\n{message}");

    // A value error on a fragment's second line: the column counts from
    // that line. The unknown key `é` is placed at its closing quote, the
    // fourth character and the fifth byte of its line (serde_json says 5).
    let text =
        format!("{prefix}\"entity\": {{\"ex:e\": {{\"ex:a\": {{\"$\": \"x\",\n \"é\": 1}}}}}}}}");
    let (_, _, message) = record_structure(&text);
    assert!(message.ends_with(" at line 2 column 4"), "{message}");

    // An error after a repeated attribute.
    let text = format!(r#"{prefix}"entity": {{"ex:e": {{"ex:a": 1, "ex:a": 2}}}}}}"#);
    let (_, _, message) = record_structure(&text);
    let expected = format!(
        "duplicate field `ex:a` at line 1 column {}",
        column_after(&text, "2")
    );
    assert_eq!(message, expected);

    // An unknown section, refused by the outer pass once its value is read.
    let text = format!(r#"{prefix}"zz": {{}}}}"#);
    let Err(Error::JsonStructure { message }) = json::read(&text) else {
        panic!("expected JsonStructure")
    };
    let expected = format!(
        "unknown record kind `zz` at line 1 column {}",
        column_after(&text, "{}")
    );
    assert_eq!(message, expected);

    // A JSON syntax error at a multi-byte character: serde_json counts the
    // character's first byte, so the character is in the column. `é` alone
    // is column 1, and the `é` in `{"a": é}` is the seventh character.
    for (text, column) in [("é", 1), (r#"{"a": é}"#, 7), (r#"{"a": ["é", é]}"#, 13)] {
        let Err(Error::Json(e)) = json::read(text) else {
            panic!("{text}: expected Error::Json")
        };
        assert_eq!(e.column(), Some(column), "{text}");
        assert_eq!(column, column_of(text, "é"), "{text}");
    }

    // A JSON syntax error.
    let text = format!(r#"{prefix}"entity": "#);
    let Err(Error::Json(e)) = json::read(&text) else {
        panic!("expected Error::Json")
    };
    assert_eq!(e.column(), Some(text.chars().count()));
    assert!(
        e.to_string()
            .ends_with(&format!(" at line 1 column {}", text.chars().count())),
        "{e}"
    );
}

#[test]
fn a_repeated_key_is_an_error_after_its_second_value() {
    // Outside a record: a prefix, a section, a record key, and a record key
    // inside a bundle.
    for (text, repeated, value) in [
        (
            r#"{"prefix": {"ex": "http://example.org/", "ex": "http://b.org/"}}"#,
            "ex",
            r#""http://b.org/""#,
        ),
        (
            r#"{"prefix": {"ex": "http://example.org/"}, "entity": {"ex:e": {}}, "entity": {"ex:f": {}}}"#,
            "entity",
            r#"{"ex:f": {}}"#,
        ),
        (
            r#"{"prefix": {"ex": "http://example.org/"}, "entity": {"ex:e": {"ex:a": "1"}, "ex:e": {"ex:b": "2"}}}"#,
            "ex:e",
            r#"{"ex:b": "2"}"#,
        ),
        (
            r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"ex:b": {"entity": {"ex:e": {}, "ex:e": [{}]}}}}"#,
            "ex:e",
            "[{}]",
        ),
    ] {
        let message = match json::read(text) {
            Err(Error::JsonStructure { message }) => message,
            other => panic!("{text}: expected JsonStructure, got {other:?}"),
        };
        let column = column_after(text, value);
        let expected = format!("duplicate field `{repeated}` at line 1 column {column}");
        assert_eq!(message, expected, "{text}");
    }
    // Inside a record: an attribute, an attribute in one of several records
    // under one key, and a member of a value object.
    for (body, repeated, value) in [
        (
            r#""ex:a": "1", "ex:a": {"$": "2"}"#,
            "ex:a",
            r#"{"$": "2"}"#,
        ),
        (r#""ex:a": "1", "ex:b": "2", "ex:a": 3"#, "ex:a", "3"),
        (r#""ex:a": {"$": "1", "$": "2"}"#, "$", r#""2""#),
        (
            r#""ex:a": ["x", {"$": "1", "type": "xsd:int", "type": "xsd:long"}]"#,
            "type",
            r#""xsd:long""#,
        ),
        (
            r#""ex:a": {"$": "x", "lang": "en", "lang": "fr"}"#,
            "lang",
            r#""fr""#,
        ),
    ] {
        let text = one_record("entity", "ex:e", body);
        let (section, key, message) = record_structure(&text);
        assert_eq!((section.as_str(), key.as_str()), ("entity", "ex:e"));
        let column = column_after(&text, value);
        let expected = format!("duplicate field `{repeated}` at line 1 column {column}");
        assert_eq!(message, expected, "{body}");
    }
    let text = r#"{"prefix": {"ex": "http://example.org/"}, "used": {"_:u": [{}, {"ex:a": 1, "ex:a": 2}]}}"#;
    let (section, key, message) = record_structure(text);
    assert_eq!((section.as_str(), key.as_str()), ("used", "_:u"));
    let column = column_after(text, "2");
    assert_eq!(
        message,
        format!("duplicate field `ex:a` at line 1 column {column}")
    );
    // What follows the value places the error, in a record as outside one:
    // a closing brace is taken, a comma is not.
    for (tail, line, attribute_column, section_column) in [
        ("\n}", 4, 1, 1),
        ("\n, \"ex:z\": {}}", 4, 0, 0),
        ("  }", 3, 12, 15),
        ("\r\n}", 4, 1, 1),
        ("\r\n, \"ex:z\": {}}", 4, 0, 0),
    ] {
        let text = format!(
            "{{\"prefix\": {{\"ex\": \"http://example.org/\"}},\n\"entity\": {{\"ex:e\": {{\"ex:a\": 1,\n\"ex:a\": 2{tail}}}}}"
        );
        let (_, _, message) = record_structure(&text);
        let expected = format!("duplicate field `ex:a` at line {line} column {attribute_column}");
        assert_eq!(message, expected, "{text}");
        let text = format!(
            "{{\"prefix\": {{\"ex\": \"http://example.org/\"}},\n\"entity\": {{}},\n\"entity\": {{}}{tail}"
        );
        let Err(Error::JsonStructure { message }) = json::read(&text) else {
            panic!("{text}")
        };
        let expected = format!("duplicate field `entity` at line {line} column {section_column}");
        assert_eq!(message, expected, "{text}");
    }
}

#[test]
fn a_repeated_bundle_key_is_a_duplicate_bundle() {
    // Two empty bundles; a second bundle holding a value the wire layer
    // rejects; and two spellings of one identifier.
    for text in [
        r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"ex:b": {}, "ex:b": {}}}"#,
        r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"ex:b": {}, "ex:b": {"entity": {"ex:e": {"ex:a": null}}}}}"#,
        r#"{"prefix": {"ex": "http://example.org/", "ex2": "http://example.org/"}, "bundle": {"ex:b": {}, "ex2:b": {"entity": {"ex:e": {"ex:a": null}}}}}"#,
    ] {
        let (section, key, error) = record_error(text);
        assert_eq!(section, "bundle", "{text}");
        assert!(key == "ex:b" || key == "ex2:b", "{text}: {key}");
        assert!(
            matches!(error, Error::DuplicateBundle(ref id) if *id == ex("b")),
            "{text}: {error:?}"
        );
    }
}

#[test]
fn a_bundle_key_spelt_twice_is_refused_before_its_table_is_read() {
    // The second bundle rebinds the key's prefix, or declares a reserved
    // one. Either way the repeated key is reported, with the first
    // bundle's identifier.
    for table in [r#""ex": "http://other.org/""#, r#""prov": "http://x.org/""#] {
        let text = format!(
            r#"{{"prefix": {{"ex": "http://example.org/"}}, "bundle": {{"ex:b": {{}}, "ex:b": {{"prefix": {{{table}}}}}}}}}"#
        );
        let (section, key, error) = record_error(&text);
        assert_eq!(
            (section.as_str(), key.as_str()),
            ("bundle", "ex:b"),
            "{text}"
        );
        assert!(
            matches!(error, Error::DuplicateBundle(ref id) if *id == ex("b")),
            "{text}: {error:?}"
        );
    }
}

#[test]
fn a_repeated_bundle_key_is_reported_whatever_its_value_holds() {
    for value in [
        r#"{"entity": {"ex:e": {}, "ex:e": {}}}"#,
        r#"{"entity": {}, "entity": {}}"#,
        r#"{"prefix": {"a": "http://a.org/", "a": "http://b.org/"}}"#,
        r#"{"zzz": {}}"#,
        r#"{"bundle": {}}"#,
        "1",
    ] {
        let text = format!(
            r#"{{"prefix": {{"ex": "http://example.org/"}}, "bundle": {{"ex:b": {{}}, "ex:b": {value}}}}}"#
        );
        let (section, key, error) = record_error(&text);
        assert_eq!(
            (section.as_str(), key.as_str()),
            ("bundle", "ex:b"),
            "{text}"
        );
        assert!(
            matches!(error, Error::DuplicateBundle(ref id) if *id == ex("b")),
            "{text}: {error:?}"
        );
    }
    // A fault the parser finds inside the first bundle comes first.
    let text =
        r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"ex:b": {"zzz": {}}, "ex:b": {}}}"#;
    let Err(Error::JsonStructure { message }) = json::read(text) else {
        panic!("{text}: expected JsonStructure")
    };
    assert!(
        message.starts_with("unknown record kind `zzz`"),
        "{message}"
    );
}

#[test]
fn an_id_or_a_qualified_section_inside_a_bundle_is_an_unknown_kind() {
    for section in [
        r#""@id": "ex:b""#,
        r#""qualifiedSpecializationOf": {"_:q": {}}"#,
    ] {
        let text = format!(
            r#"{{"prefix": {{"ex": "http://example.org/"}}, "bundle": {{"ex:b": {{{section}}}}}}}"#
        );
        let name = section.split('"').nth(1).unwrap();
        let Err(Error::JsonStructure { message }) = json::read(&text) else {
            panic!("{text}: expected JsonStructure")
        };
        assert!(
            message.starts_with(&format!("unknown record kind `{name}`")),
            "{message}"
        );
    }
}

#[test]
fn a_bundle_key_fault_comes_after_the_body_and_before_the_bundle_body() {
    // A wire-level fault in the document body is reported first.
    let text = r#"{"prefix": {"ex": "http://example.org/"}, "entity": {"ex:e": {"ex:a": null}}, "bundle": {"zz:b": {}}}"#;
    let (section, _, _) = record_error(text);
    assert_eq!(section, "entity");
    // So is an error building a record in the document body.
    let text = r#"{"prefix": {"ex": "http://example.org/"}, "entity": {"zz:e": {}}, "bundle": {"ex:b": {}, "ex:b": {}}}"#;
    let (section, key, error) = record_error(text);
    assert_eq!((section.as_str(), key.as_str()), ("entity", "zz:e"));
    assert!(
        matches!(error, Error::UndeclaredPrefix(ref p) if p == "zz"),
        "{error:?}"
    );
    // A bundle key that does not resolve is reported before a wire-level
    // fault inside that bundle.
    let text = r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"zz:b": {"entity": {"ex:e": {"ex:a": null}}}}}"#;
    let (section, key, error) = record_error(text);
    assert_eq!((section.as_str(), key.as_str()), ("bundle", "zz:b"));
    assert!(
        matches!(error, Error::UndeclaredPrefix(ref p) if p == "zz"),
        "{error:?}"
    );
}

#[test]
fn errors_in_a_record_are_reported_in_the_order_of_the_text() {
    for (body, first) in [
        (
            r#""ex:a": null, "ex:a": 1"#,
            "null is not an attribute value",
        ),
        (
            r#""ex:a": 1, "ex:b": null, "ex:a": 2"#,
            "null is not an attribute value",
        ),
        (
            r#""ex:a": 1, "ex:a": 2, "ex:b": null"#,
            "duplicate field `ex:a`",
        ),
        // Inside a value object, an invalid member before a repeated one.
        (
            r#""ex:a": {"$": 1, "$": "x"}"#,
            "expected a string as the `$` of a value object",
        ),
        // Inside a value object, an invalid member before an unknown one.
        (
            r#""ex:a": {"$": 1, "zz": 2}"#,
            "expected a string as the `$` of a value object",
        ),
        (
            r#""ex:a": {"$": "\ud800", "lang": "en", "lang": "fr"}"#,
            "unexpected end of hex escape",
        ),
        (
            r#""ex:a": {"$": "x", "lang": "en", "lang": "fr"}"#,
            "duplicate field `lang`",
        ),
        // A repeated member with more members after it.
        (
            r#""ex:a": {"$": 1, "$": "x", "lang": "en"}"#,
            "expected a string as the `$` of a value object at line 1 column 77",
        ),
        (
            r#""ex:a": {"type": 5, "type": "x", "$": "y"}"#,
            "expected a string as the `type` of a value object at line 1 column 80",
        ),
        (
            r#""ex:a": {"$": "x", "lang": "en", "lang": "fr", "type": "xsd:string"}"#,
            "duplicate field `lang`",
        ),
    ] {
        let (_, _, message) = record_structure(&one_record("entity", "ex:e", body));
        assert!(message.starts_with(first), "{body}\n{message}");
    }
}

#[test]
fn of_several_invalid_members_the_first_in_the_text_comes_first() {
    for (value, first) in [
        (r#"{"type": 5, "$": 1}"#, "`type`"),
        (r#"{"lang": 5, "type": 5, "$": 1}"#, "`lang`"),
        (r#"{"type": 5, "lang": 5, "$": 1, "$": "x"}"#, "`type`"),
        (r#"{"lang": 5, "$": "x", "type": 5}"#, "`lang`"),
        // An unknown key after two invalid members.
        (r#"{"type": 1, "$": 2, "zz": 3}"#, "`type`"),
    ] {
        let text = one_record("entity", "ex:e", &format!(r#""ex:a": {value}"#));
        let (_, _, message) = record_structure(&text);
        let expected = format!("expected a string as the {first} of a value object");
        assert!(message.starts_with(&expected), "{value}\n{message}");
    }
    // A `type` holding a lone surrogate, before an invalid `$` and an
    // unknown key.
    let text = one_record(
        "entity",
        "ex:e",
        r#""ex:a": {"type": "\ud800", "$": 1, "zz": 2}"#,
    );
    let (_, _, message) = record_structure(&text);
    assert!(
        message.starts_with("unexpected end of hex escape"),
        "{message}"
    );
}

#[test]
fn an_unknown_member_of_a_value_object_is_placed_at_its_key() {
    let text = one_record("entity", "ex:e", r#""ex:a": {"$": "x", "zz": 2 }"#);
    let (_, _, message) = record_structure(&text);
    let column = column_after(&text, r#""zz""#) - 1;
    let expected = format!(
        "unknown key `zz` in a value object, expected `$`, `type` or `lang` at line 1 column {column}"
    );
    assert_eq!(message, expected);
}

#[test]
fn an_error_inside_a_bundle_names_the_bundle() {
    for body in [r#""zz:e": {}"#, r#""ex:e": {"ex:a": null}"#] {
        let text = format!(
            r#"{{"prefix": {{"ex": "http://example.org/"}}, "bundle": {{"ex:b": {{"entity": {{{body}}}}}}}}}"#
        );
        let (section, key, inner) = record_error(&text);
        assert_eq!(
            (section.as_str(), key.as_str()),
            ("bundle", "ex:b"),
            "{text}"
        );
        let Error::InRecord { section, key, .. } = inner else {
            panic!("{text}: expected a nested InRecord, got {inner:?}")
        };
        assert_eq!(section, "entity", "{text}");
        assert!(key == "zz:e" || key == "ex:e", "{text}");
    }
    let text = r#"{"prefix": {"ex": "http://example.org/"}, "bundle": {"ex:b": {"entity": {"zz:e": {}}}}}"#;
    assert_eq!(
        json::read(text).unwrap_err().to_string(),
        "bundle `ex:b`: entity `zz:e`: undeclared prefix `zz`"
    );
}

#[test]
fn a_bad_namespace_table_in_a_bundle_names_the_bundle() {
    for (entry, reserved) in [
        (r#""1a": "http://example.org/""#, false),
        (r#""prov": "http://x/""#, true),
    ] {
        let text = format!(
            r#"{{"prefix": {{"ex": "http://example.org/"}}, "bundle": {{"ex:b": {{"prefix": {{{entry}}}}}}}}}"#
        );
        let (section, key, inner) = record_error(&text);
        assert_eq!(
            (section.as_str(), key.as_str()),
            ("bundle", "ex:b"),
            "{text}"
        );
        match inner {
            Error::InvalidPrefix(p) if !reserved => assert_eq!(p, "1a", "{text}"),
            Error::ReservedPrefix { .. } if reserved => {}
            other => panic!("{text}: unexpected {other:?}"),
        }
    }
    // The document's own table stays unwrapped.
    assert!(matches!(
        json::read(r#"{"prefix": {"prov": "http://x/"}}"#),
        Err(Error::ReservedPrefix { .. })
    ));
}
