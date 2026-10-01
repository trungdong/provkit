use crate::provn::Profile;
use crate::test_util::ex;
use crate::{Document, Error, Kind, QName, Record, Value, provn};

const HEAD: &str = "document\n  default <http://default.org/>\n  prefix ex <http://example.org/>\n";

fn read(body: &str, profile: Profile) -> crate::Result<Document> {
    provn::read_with(&format!("{HEAD}{body}\nendDocument"), profile)
}

fn doc(body: &str, profile: Profile) -> Document {
    read(body, profile).unwrap()
}

fn err(body: &str, profile: Profile) -> Error {
    read(body, profile).unwrap_err()
}

fn syntax_message(e: Error) -> String {
    match e {
        Error::Syntax { message, .. } => message,
        other => panic!("expected a syntax error, got {other:?}"),
    }
}

/// The position and the inner error of an [`Error::At`].
fn located(e: Error) -> ((usize, usize), Error) {
    match e {
        Error::At {
            line,
            column,
            error,
        } => ((line, column), *error),
        other => panic!("expected Error::At, got {other:?}"),
    }
}

/// The values of the first record's attributes, in order.
fn values(d: &Document) -> Vec<&Value> {
    d.records[0].attributes().iter().map(|(_, v)| v).collect()
}

#[test]
fn attribute_named_after_an_own_formal_is_rejected() {
    let text = "document\nprefix ex <http://example.org/>\n\
                wasGeneratedBy(ex:e, ex:a, -, [prov:activity='ex:b'])\nendDocument\n";
    let e = provn::read(text).unwrap_err();
    // The message names the position and the kind, once each.
    assert_eq!(
        e.to_string(),
        "line 3, column 1: wasGeneratedBy: prov:activity is given as an attribute; \
         its value belongs in the positional argument"
    );
    let (position, inner) = located(e);
    assert_eq!(position, (3, 1));
    assert!(
        matches!(
            inner,
            Error::InvalidRecord {
                kind: Kind::Generation,
                ..
            }
        ),
        "{inner:?}"
    );
    // prov:type and prov:label on a relation stay attributes.
    let d = doc(
        "wasGeneratedBy(ex:e, ex:a, -, [prov:type='ex:T', prov:label=\"l\"])",
        Profile::Strict,
    );
    assert_eq!(d.records[0].attributes().len(), 2);
}

#[test]
fn elements_relations_and_ids() {
    let d = doc(
        "entity(ex:e1)\nactivity(a1, 2011-11-16T16:05:33Z, -)\n\
         wasGeneratedBy(ex:g1; ex:e1, a1, -)\nwasGeneratedBy(ex:e1)\n\
         wasGeneratedBy(-; ex:e1, -, -)",
        Profile::Strict,
    );
    assert_eq!(d.records.len(), 5);
    assert!(
        matches!(&d.records[1], Record::Activity(a) if a.start_time.is_some() && a.end_time.is_none())
    );
    assert!(
        matches!(&d.records[2], Record::Generation(g) if g.id == Some(ex("g1")) && g.activity == Some(QName::new("http://default.org/", "a1").unwrap()))
    );
    assert!(
        matches!(&d.records[3], Record::Generation(g) if g.id.is_none() && g.activity.is_none())
    );
    assert!(
        matches!(&d.records[4], Record::Generation(g) if g.id.is_none() && g.entity == Some(ex("e1")))
    );
}

#[test]
fn attribute_values() {
    let d = doc(
        r#"entity(ex:e, [prov:type='ex:T', ex:n=1, ex:s="a\"b", prov:label="x"@en, ex:d="2011-11-16T16:05:33Z" %% xsd:dateTime, ex:f="1.5" %% xsd:double, ex:t="t" %% xsd:string, ex:q="ex:v" %% xsd:QName, ex:l="ex:w" %% prov:QUALIFIED_NAME])"#,
        Profile::Strict,
    );
    let Record::Entity(e) = &d.records[0] else {
        panic!("expected an entity")
    };
    let vals: Vec<&Value> = e.attributes.iter().map(|(_, v)| v).collect();
    assert!(matches!(vals[0], Value::QName(q) if *q == ex("T")));
    assert!(matches!(vals[1], Value::Typed(t) if t.lexical() == "1"));
    assert_eq!(*vals[2], Value::Str("a\"b".into()));
    assert!(matches!(vals[3], Value::LangStr(_)));
    assert!(matches!(vals[4], Value::DateTime(_)));
    assert!(matches!(vals[5], Value::Typed(t) if t.lexical() == "1.5"));
    assert_eq!(*vals[6], Value::Str("t".into()));
    assert_eq!(*vals[7], Value::QName(ex("v")));
    assert_eq!(*vals[8], Value::QName(ex("w")));
    // An untyped dateTime is not a literal.
    assert!(matches!(
        err(
            "entity(ex:e, [ex:d=2011-11-16T16:05:33Z])",
            Profile::Default
        ),
        Error::Syntax { .. }
    ));
}

#[test]
fn profiles() {
    assert!(matches!(
        err("mentionOf(ex:a, ex:b, ex:c)", Profile::Strict),
        Error::Syntax { .. }
    ));
    assert_eq!(
        doc("mentionOf(ex:a, ex:b, ex:c)", Profile::Default).records[0].kind(),
        Kind::Mention
    );
    assert_eq!(
        doc("prov:mentionOf(ex:a, ex:b, ex:c)", Profile::Strict).records[0].kind(),
        Kind::Mention
    );
    assert!(matches!(
        err("wasGeneratedBy(-, ex:a, -)", Profile::Strict),
        Error::Syntax { .. }
    ));
    assert!(
        matches!(&doc("wasGeneratedBy(-, ex:a, -)", Profile::Default).records[0], Record::Generation(g) if g.entity.is_none())
    );
    // Beyond the leading required positions, `-` is fine under Strict.
    assert!(read("wasDerivedFrom(ex:a, ex:b, -, -, -)", Profile::Strict).is_ok());
}

#[test]
fn mention_takes_attributes() {
    assert!(
        read(
            "prov:mentionOf(ex:a, ex:b, ex:c, [ex:k=\"v\"])",
            Profile::Strict
        )
        .is_ok()
    );
}

/// The document with `body` under `profile`, and the expressions left out.
fn report(body: &str, profile: Profile) -> (Document, Vec<provn::Ignored>) {
    let provn::ReadReport {
        document, ignored, ..
    } = provn::read_report(&format!("{HEAD}{body}\nendDocument"), profile).unwrap();
    (document, ignored)
}

/// `prov:mentionOf` is a mention only in the shape of one: three names,
/// then `)` or an attribute block. Any other shape is an extensibility
/// expression under production [49], parsed and left out.
#[test]
fn prov_mention_of_is_a_statement_only_in_its_own_shape() {
    for profile in [Profile::Strict, Profile::Default] {
        for body in [
            "prov:mentionOf(ex:e2, ex:e1, ex:b)",
            "prov:mentionOf(ex:e2, ex:e1, ex:b, [ex:k=\"v\"])",
            "prov:mentionOf(12, ex:e1, ex:b)",
        ] {
            let (doc, ignored) = report(body, profile);
            assert!(ignored.is_empty(), "{body}");
            assert!(
                matches!(&doc.records[0], Record::Mention(m) if m.id.is_none() && m.bundle == Some(ex("b"))),
                "{body}: {:?}",
                doc.records
            );
        }
        for body in [
            "prov:mentionOf(ex:e2, ex:e1)",
            "prov:mentionOf(ex:e2, ex:e1, \"b\")",
            "prov:mentionOf(ex:e2, ex:e1, ex:b, 5)",
            "prov:mentionOf(ex:e2, ex:e1, {ex:b})",
            "prov:mentionOf(ex:e2, ex:e1, 2012-01-01T00:00:00)",
        ] {
            let (doc, ignored) = report(body, profile);
            assert!(doc.records.is_empty(), "{body}: {:?}", doc.records);
            assert_eq!(ignored.len(), 1, "{body}");
            assert_eq!(ignored[0].name, "prov:mentionOf", "{body}");
        }
    }
}

/// Under `Strict` a `-` in `prov:mentionOf` is outside the mention shape,
/// so the statement is an extensibility expression.
#[test]
fn prov_mention_of_with_a_marker_is_an_extension_under_strict() {
    let (doc, ignored) = report("prov:mentionOf(ex:e2, ex:e1, -)", Profile::Strict);
    assert!(doc.records.is_empty(), "{:?}", doc.records);
    assert_eq!(ignored.len(), 1);
    assert_eq!(ignored[0].name, "prov:mentionOf");
    assert!(matches!(
        err("mentionOf(ex:e2, ex:e1, -)", Profile::Strict),
        Error::Syntax { .. }
    ));
}

/// `Default` reads `-` in a required position of `prov:mentionOf` as it
/// does for bare `mentionOf`: a mention with that formal absent.
#[test]
fn prov_mention_of_with_a_marker_is_a_mention_under_default() {
    for body in [
        "prov:mentionOf(ex:e2, ex:e1, -)",
        "mentionOf(ex:e2, ex:e1, -)",
        "prov:mentionOf(-, -, -)",
    ] {
        let (doc, ignored) = report(body, Profile::Default);
        assert!(ignored.is_empty(), "{body}");
        assert!(
            matches!(&doc.records[0], Record::Mention(m) if m.bundle.is_none()),
            "{body}: {:?}",
            doc.records
        );
    }
}

/// PROV-DM gives `mentionOf` no identifier. Under `Strict` one makes
/// `prov:mentionOf` an extensibility expression; `Default` reads it as the
/// mention's identifier, as it does for bare `mentionOf`.
#[test]
fn an_identifier_on_prov_mention_of_is_an_extension_under_strict() {
    let body = "prov:mentionOf(ex:m; ex:e2, ex:e1, ex:b)";
    let (doc, ignored) = report(body, Profile::Strict);
    assert!(doc.records.is_empty());
    assert_eq!(ignored.len(), 1);
    let (doc, ignored) = report(body, Profile::Default);
    assert!(ignored.is_empty());
    assert!(matches!(&doc.records[0], Record::Mention(m) if m.id == Some(ex("m"))));
    let (doc, _) = report("prov:mentionOf(-; ex:e2, ex:e1, ex:b)", Profile::Default);
    assert!(matches!(&doc.records[0], Record::Mention(m) if m.id.is_none()));
    // Bare `mentionOf` with an identifier: a syntax error under `Strict`,
    // because the keyword is bare, and a mention under `Default`.
    assert!(matches!(
        err("mentionOf(ex:m; ex:e2, ex:e1, ex:b)", Profile::Strict),
        Error::Syntax { .. }
    ));
    let (doc, _) = report("mentionOf(ex:m; ex:e2, ex:e1, ex:b)", Profile::Default);
    assert!(matches!(&doc.records[0], Record::Mention(m) if m.id == Some(ex("m"))));
}

#[test]
fn strict_is_the_whole_grammar() {
    let p = "prefix ex <http://example.org/>\n";
    let cases = [
        (
            format!("document\n{p}default <http://d/>\nentity(e)\nendDocument\n"),
            (3, 1),
        ),
        (
            format!("document\n{p}bundle ex:b\nendBundle\nentity(ex:f)\nendDocument\n"),
            (5, 1),
        ),
        (
            format!(
                "document\n{p}bundle ex:b\nprefix ey <http://e/>\ndefault <http://d/>\nendBundle\nendDocument\n"
            ),
            (5, 1),
        ),
        (
            format!("document\n{p}hadMember(ex:m; ex:c, ex:e)\nendDocument\n"),
            (3, 11),
        ),
        (
            format!("document\n{p}alternateOf(ex:a; ex:b, ex:c)\nendDocument\n"),
            (3, 13),
        ),
        (
            format!("document\n{p}specializationOf(ex:a, ex:b, [ex:k=\"x\"])\nendDocument\n"),
            (3, 30),
        ),
        (
            format!(
                "document\n{p}prefix prov <http://www.w3.org/ns/prov#>\nentity(ex:e)\nendDocument\n"
            ),
            (3, 8),
        ),
        (
            format!(
                "document\n{p}prefix xsd <http://www.w3.org/2001/XMLSchema#>\nentity(ex:e)\nendDocument\n"
            ),
            (3, 8),
        ),
    ];
    for (text, position) in cases {
        assert!(provn::read_with(&text, Profile::Default).is_ok(), "{text}");
        match provn::read_with(&text, Profile::Strict) {
            Err(Error::Syntax { line, column, .. }) => {
                assert_eq!((line, column), position, "{text}");
            }
            other => panic!("{text}: expected a syntax error, got {other:?}"),
        }
    }
}

#[test]
fn strict_reads_an_empty_bundle() {
    let text = "document\nprefix ex <http://example.org/>\nbundle ex:b\nendBundle\nendDocument\n";
    for profile in [Profile::Strict, Profile::Default] {
        let d = provn::read_with(text, profile).unwrap();
        assert_eq!(d.bundles.len(), 1);
        assert!(d.bundles[0].records.is_empty());
    }
}

#[test]
fn a_second_default_and_a_moved_reserved_prefix_fail_under_both_profiles() {
    let twice = "document\ndefault <http://a/>\ndefault <http://b/>\nendDocument\n";
    let moved = "document\nprefix prov <http://other/>\nendDocument\n";
    for profile in [Profile::Strict, Profile::Default] {
        assert!(matches!(
            provn::read_with(twice, profile),
            Err(Error::Syntax {
                line: 3,
                column: 1,
                ..
            })
        ));
        let (position, inner) = located(provn::read_with(moved, profile).unwrap_err());
        assert_eq!(position, (2, 8));
        assert!(matches!(inner, Error::ReservedPrefix { .. }), "{inner:?}");
    }
}

#[test]
fn integer_literals_type_by_range() {
    let d = doc(
        "entity(ex:e, [ex:a=42, ex:b=3000000000, ex:c=99999999999999999999, \
         ex:d=-2147483649, ex:e=007])",
        Profile::Strict,
    );
    let datatypes: Vec<_> = values(&d)
        .iter()
        .map(|v| v.datatype().unwrap().local().to_owned())
        .collect();
    assert_eq!(datatypes, ["int", "long", "integer", "long", "int"]);
    let texts: Vec<_> = values(&d)
        .iter()
        .map(|v| match v {
            Value::Typed(t) => t.lexical(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        texts,
        [
            "42",
            "3000000000",
            "99999999999999999999",
            "-2147483649",
            "007"
        ]
    );
}

#[test]
fn a_bare_minus_zero_is_an_int_and_a_plus_sign_is_a_syntax_error() {
    let d = doc("entity(ex:e, [ex:a=-0])", Profile::Strict);
    assert!(matches!(
        values(&d)[0],
        Value::Typed(t) if t.lexical() == "-0" && t.datatype().local() == "int"
    ));
    for profile in [Profile::Strict, Profile::Default] {
        assert!(matches!(
            err("entity(ex:e, [ex:a=+5])", profile),
            Error::Syntax { .. }
        ));
    }
}

#[test]
fn a_language_tag_ends_at_an_underscore_and_the_rest_is_a_syntax_error() {
    for profile in [Profile::Strict, Profile::Default] {
        assert!(matches!(
            err(r#"entity(ex:e, [ex:a="x" @en_US])"#, profile),
            Error::Syntax { .. }
        ));
    }
}

#[test]
fn a_brace_outside_a_dictionary_is_a_syntax_error_at_the_brace() {
    match provn::read_with("document { endDocument", Profile::Default) {
        Err(Error::Syntax { line, column, .. }) => assert_eq!((line, column), (1, 10)),
        other => panic!("expected a syntax error, got {other:?}"),
    }
}

#[test]
fn a_datatype_may_be_all_digits() {
    let d = doc(r#"entity(ex:e, [ex:k="x" %% 123])"#, Profile::Strict);
    assert_eq!(
        values(&d)[0].datatype().unwrap().iri(),
        "http://default.org/123"
    );
}

#[test]
fn arity_and_structure_errors() {
    let message = syntax_message(err("wasGeneratedBy(ex:e, ex:a)", Profile::Default));
    assert!(message.contains("1 or 3"), "{message}");
    assert!(message.starts_with("expected"), "{message}");
    assert!(matches!(
        err("entity()", Profile::Default),
        Error::Syntax { .. }
    ));
    assert!(matches!(
        err(
            "bundle ex:b\n bundle ex:c\n endBundle\nendBundle",
            Profile::Default
        ),
        Error::Syntax { .. }
    ));
    assert!(matches!(
        located(err("entity(zz:e)", Profile::Default)),
        (_, Error::UndeclaredPrefix(p)) if p == "zz"
    ));
    assert!(matches!(
        provn::read("entity(ex:e)"),
        Err(Error::Syntax {
            line: 1,
            column: 1,
            ..
        })
    ));
    // A time where a name belongs, and a name where a time belongs.
    assert!(matches!(
        err(
            "wasGeneratedBy(ex:e, 2011-11-16T16:05:33Z, -)",
            Profile::Default
        ),
        Error::Syntax { .. }
    ));
    assert!(matches!(
        err("wasGeneratedBy(ex:e, ex:a, ex:t)", Profile::Default),
        Error::Syntax { .. }
    ));
}

/// A syntax error sits at the token the grammar rejects, or at the end of
/// the input when the text stops short.
#[test]
fn errors_carry_positions() {
    assert!(matches!(
        err("entity(ex:e]", Profile::Default),
        Error::Syntax {
            line: 4,
            column: 12,
            ..
        }
    ));
    assert!(matches!(
        provn::read("document\nprefix ex <http://e/>\nentity(ex:e)\n"),
        Err(Error::Syntax {
            line: 4,
            column: 1,
            ..
        })
    ));
}

#[test]
fn each_layer_error_sits_at_its_token() {
    let read = |body: &str| {
        let text = format!("document\nprefix ex <http://example.org/>\n{body}\nendDocument\n");
        located(provn::read(&text).unwrap_err())
    };
    // An undeclared prefix, at the token that carries it: an attribute
    // name, a datatype, a name argument, a bundle identifier, and a
    // literal's first token for a qualified-name value.
    for (body, position) in [
        ("entity(ex:e, [zz:k=1])", (3, 15)),
        ("entity(ex:e, [ex:k=\"1\" %% zz:t])", (3, 27)),
        ("used(ex:a, zz:e, -)", (3, 12)),
        ("bundle zz:b\nendBundle", (3, 8)),
        ("entity(ex:e, [ex:k=\"zz:v\" %% xsd:QName])", (3, 20)),
    ] {
        let (found, inner) = read(body);
        assert_eq!(found, position, "{body}");
        assert!(
            matches!(&inner, Error::UndeclaredPrefix(p) if p == "zz"),
            "{body}: {inner:?}"
        );
    }
    assert!(matches!(
        read("entity(e)"),
        ((3, 8), Error::NoDefaultNamespace(l)) if l == "e"
    ));
    // A bare qualified-name value, at the literal's first token.
    assert!(matches!(
        read("entity(ex:e, [ex:k=\"v\" %% xsd:QName])"),
        ((3, 20), Error::NoDefaultNamespace(l)) if l == "v"
    ));
    // A dateTime argument, at its own token.
    assert!(matches!(
        read("activity(ex:a, -, 2012-13-01T00:00:00)"),
        ((3, 19), Error::InvalidLiteral { .. })
    ));
    // A reserved prefix, at the prefix token.
    let text = "document\nprefix prov <http://example.org/>\nendDocument\n";
    assert!(matches!(
        located(provn::read(text).unwrap_err()),
        ((2, 8), Error::ReservedPrefix { .. })
    ));
    // A record error, at the statement's keyword.
    assert!(matches!(
        read("wasGeneratedBy(ex:e, ex:a, -, [prov:activity='ex:b'])"),
        ((3, 1), Error::InvalidRecord { .. })
    ));
    // A duplicate bundle, at the second bundle's identifier.
    assert!(matches!(
        read("bundle ex:b\nendBundle\nbundle ex:b\nendBundle"),
        ((5, 8), Error::DuplicateBundle(_))
    ));
}

#[test]
fn only_a_closing_paren_follows_attributes() {
    let message = syntax_message(err("entity(ex:e, [ex:a=1], ex:b)", Profile::Default));
    assert!(message.starts_with("expected ')'"), "{message}");
}

#[test]
fn nothing_follows_end_document() {
    match provn::read("document\nendDocument\nentity(ex:e)") {
        Err(Error::Syntax {
            line,
            column,
            message,
            ..
        }) => {
            assert_eq!((line, column), (3, 1));
            assert!(message.starts_with("expected end of input"), "{message}");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_prefix_is_not_an_integer() {
    let text = "document\n prefix 123 <http://a/>\nendDocument";
    assert!(matches!(
        provn::read(text),
        Err(Error::Syntax { line: 2, .. })
    ));
}

#[test]
fn digit_names_and_empty_attributes() {
    let d = doc("entity(123, [])\nwasDerivedFrom(ex:a, 42)", Profile::Strict);
    assert_eq!(
        d.records[0].id(),
        Some(&QName::new("http://default.org/", "123").unwrap())
    );
    assert!(
        matches!(&d.records[1], Record::Derivation(r) if r.used_entity == Some(QName::new("http://default.org/", "42").unwrap()))
    );
}

#[test]
fn escaped_colon_in_a_bare_name_stays_local() {
    let d = doc(r"entity(ex\:e1)", Profile::Strict);
    assert_eq!(
        d.records[0].id(),
        Some(&QName::new("http://default.org/", "ex:e1").unwrap())
    );
}

#[test]
fn qualified_name_datatypes_read_as_names() {
    let d = doc(
        r#"entity(ex:e, [ex:a="ex:v" %% prov:QUALIFIED_NAME, ex:b="ex:v" %% xsd:QName])"#,
        Profile::Strict,
    );
    assert_eq!(values(&d), [&Value::QName(ex("v")), &Value::QName(ex("v"))]);
    assert!(matches!(
        located(err(r#"entity(ex:e, [ex:a="zz:v" %% prov:QUALIFIED_NAME])"#, Profile::Default)),
        (_, Error::UndeclaredPrefix(p)) if p == "zz"
    ));
    // A name the grammar writes with an escape is spelled here without it.
    let d = doc(
        r#"entity(ex:e, [ex:a="ex:a=b" %% xsd:QName, ex:b='ex:a\=b'])"#,
        Profile::Strict,
    );
    assert_eq!(values(&d), [&Value::QName(ex("a=b")); 2]);
}

#[test]
fn a_qualified_name_literal_whose_text_is_not_a_name_is_an_error() {
    for (literal, text) in [
        (r#""not a name" %% prov:QUALIFIED_NAME"#, "not a name"),
        (r#""ex:a b" %% xsd:QName"#, "ex:a b"),
        (r#""ex:a%4" %% xsd:QName"#, "ex:a%4"),
        (r#""" %% xsd:QName"#, ""),
    ] {
        let (position, inner) = located(err(
            &format!("entity(ex:e, [ex:a={literal}])"),
            Profile::Strict,
        ));
        assert_eq!(position, (4, 20), "{literal}");
        assert!(
            matches!(&inner, Error::InvalidQName(t) if t == text),
            "{literal}: {inner:?}"
        );
    }
}

#[test]
fn a_local_part_provn_cannot_write_is_a_syntax_error() {
    // The lexer ends a name at a `%` without two hex digits and takes
    // no `\u` escape in a name, so neither reaches the resolver.
    for body in [r"entity(ex:a%4)", r"entity(ex:a\u00E9)", "entity(ex:a b)"] {
        assert!(
            matches!(err(body, Profile::Strict), Error::Syntax { .. }),
            "{body}"
        );
    }
}

#[test]
fn ill_formed_literals_read_and_round_trip() {
    let literals = [
        (
            r#""2012-13-01T00:00:00" %% xsd:dateTime"#,
            "2012-13-01T00:00:00",
        ),
        (r#""x" %% xsd:dateTime"#, "x"),
        (r#""abc" %% xsd:int"#, "abc"),
        (r#""maybe" %% xsd:boolean"#, "maybe"),
        (r#""1,5" %% xsd:decimal"#, "1,5"),
    ];
    let pairs: Vec<String> = literals
        .iter()
        .enumerate()
        .map(|(i, (literal, _))| format!("ex:k{i}={literal}"))
        .collect();
    let d = doc(
        &format!("entity(ex:e, [{}])", pairs.join(", ")),
        Profile::Strict,
    );
    let read = values(&d);
    assert_eq!(read.len(), literals.len());
    for (value, (literal, text)) in read.iter().zip(literals) {
        assert!(
            matches!(value, Value::Typed(t) if t.lexical() == text),
            "{literal}: {value:?}"
        );
    }
    // Each literal is written with the text it was read with.
    let written = provn::to_string(&d).unwrap();
    for (literal, _) in literals {
        assert!(written.contains(literal), "{literal}\n{written}");
    }
    assert_eq!(provn::read(&written).unwrap(), d);
    #[cfg(feature = "json")]
    {
        let json = crate::json::to_string(&d).unwrap();
        assert_eq!(crate::json::read(&json).unwrap(), d, "{json}");
    }
}

#[test]
fn long_language_subtags_read() {
    let d = doc(
        r#"entity(ex:e, [ex:k="x"@abcdefghi, ex:l="x"@en-GB-x-averylongsubtag])"#,
        Profile::Strict,
    );
    let tags: Vec<&str> = values(&d)
        .into_iter()
        .map(|v| match v {
            Value::LangStr(l) => l.lang(),
            other => panic!("expected a tagged string, got {other:?}"),
        })
        .collect();
    assert_eq!(tags, ["abcdefghi", "en-GB-x-averylongsubtag"]);
}

#[test]
fn an_ill_formed_time_argument_is_an_error() {
    for body in [
        "activity(ex:a, 2012-13-01T00:00:00, -)",
        "wasGeneratedBy(ex:e, ex:a, 2012-02-30T00:00:00)",
    ] {
        let (_, inner) = located(err(body, Profile::Default));
        assert!(
            matches!(&inner, Error::InvalidLiteral { datatype, .. } if datatype == "xsd:dateTime"),
            "{body}: {inner:?}"
        );
    }
}

#[test]
fn literals_compare_by_value_and_keep_their_text() {
    let body = r#"entity(ex:e, [ex:i=005, ex:j="+5" %% xsd:int, ex:d="1.0E0" %% xsd:double, ex:b="1" %% xsd:boolean, ex:c="1.50" %% xsd:decimal])"#;
    let other = r#"entity(ex:e, [ex:i=5, ex:j=" 5 " %% xsd:int, ex:d="1" %% xsd:double, ex:b="true" %% xsd:boolean, ex:c="1.5" %% xsd:decimal])"#;
    let d = doc(body, Profile::Strict);
    assert_eq!(d, doc(other, Profile::Strict));
    assert_ne!(d, doc(&other.replace("ex:i=5", "ex:i=6"), Profile::Strict));
    assert_ne!(
        d,
        doc(
            &other.replace("ex:i=5", r#"ex:i="5" %% xsd:long"#),
            Profile::Strict
        )
    );
    let written = provn::to_string(&d).unwrap();
    for literal in [
        "ex:i=005",
        r#"ex:j="+5" %% xsd:int"#,
        r#"ex:d="1.0E0" %% xsd:double"#,
        r#"ex:b="1" %% xsd:boolean"#,
        r#"ex:c="1.50" %% xsd:decimal"#,
    ] {
        assert!(written.contains(literal), "{literal}\n{written}");
    }
    let lexicals = |d: &Document| -> Vec<String> {
        values(d)
            .into_iter()
            .map(|v| match v {
                Value::Typed(t) => t.lexical().to_owned(),
                other => panic!("expected a typed literal, got {other:?}"),
            })
            .collect()
    };
    assert_eq!(lexicals(&provn::read(&written).unwrap()), lexicals(&d));
    #[cfg(feature = "json")]
    {
        let json = crate::json::to_string(&d).unwrap();
        assert_eq!(lexicals(&crate::json::read(&json).unwrap()), lexicals(&d));
    }
}

#[test]
fn equal_values_under_one_name_are_one_pair() {
    let d = doc(
        r#"entity(ex:e, [ex:v=" 5 " %% xsd:int, ex:v="5" %% xsd:int, ex:v=5, ex:v="5" %% xsd:long])"#,
        Profile::Strict,
    );
    // The first spelling stays; the `xsd:long` is another value.
    let read = values(&d);
    assert_eq!(read.len(), 2);
    assert!(matches!(read[0], Value::Typed(t) if t.lexical() == " 5 "));
    assert_eq!(
        read[1].datatype(),
        Some(crate::well_known::XSD_LONG.clone())
    );
}

#[test]
fn bundles_scope_prefixes() {
    let d = doc(
        "bundle ex:b\n  prefix b <http://b.org/>\n  entity(b:e)\n  entity(ex:f)\nendBundle",
        Profile::Strict,
    );
    assert_eq!(d.bundles[0].id, ex("b"));
    assert_eq!(
        d.bundles[0].records[0].id(),
        Some(&QName::new("http://b.org/", "e").unwrap())
    );
    assert_eq!(d.bundles[0].records[1].id(), Some(&ex("f")));
    assert_eq!(d.bundles[0].namespaces.iter().count(), 1);
}

#[test]
fn bundle_identifier_resolves_in_its_own_scope() {
    let d = doc(
        "bundle ex:b\n  prefix ex <http://bundle.org/>\n  entity(ex:e)\nendBundle\nentity(ex:e)",
        Profile::Default,
    );
    assert_eq!(
        d.bundles[0].id,
        QName::new("http://bundle.org/", "b").unwrap()
    );
    assert_eq!(
        d.bundles[0].records[0].id(),
        Some(&QName::new("http://bundle.org/", "e").unwrap())
    );
    // The bundle's declarations end with the bundle.
    assert_eq!(d.records[0].id(), Some(&ex("e")));
}

#[test]
fn duplicate_prefix_in_one_scope_is_rejected() {
    let text = "document\n prefix ex <http://a/>\n prefix ex <http://b/>\nendDocument";
    assert!(matches!(
        provn::read(text),
        Err(Error::Syntax { line: 3, .. })
    ));
}

#[test]
fn a_prefix_is_a_pn_prefix_as_written() {
    for profile in [Profile::Strict, Profile::Default] {
        for prefix in [r"a\-b", r"a\.b", r"a\:b", "a%41", "_", "a/b"] {
            let text = format!("document\nprefix {prefix} <http://x/>\nentity(a-b:e)\nendDocument");
            match provn::read_with(&text, profile) {
                Err(Error::Syntax { line, column, .. }) => {
                    assert_eq!((line, column), (2, 8), "{prefix}");
                }
                other => panic!("{prefix}: expected a syntax error, got {other:?}"),
            }
        }
        // A prefix with a hyphen needs no escape, and reads.
        let text = "document\nprefix a-b <http://x/>\nentity(a-b:e)\nendDocument";
        assert!(provn::read_with(text, profile).is_ok());
    }
}

#[test]
fn many_prefix_declarations_stay_linear() {
    use std::fmt::Write as _;
    let mut text = String::from("document\n");
    for i in 0..100_000 {
        writeln!(text, "prefix p{i} <http://e/{i}/>").unwrap();
    }
    text.push_str("endDocument\n");
    // A scan of the earlier declarations per `prefix` took minutes in a
    // debug build.
    let start = std::time::Instant::now();
    provn::read(&text).unwrap();
    assert!(start.elapsed().as_secs() < 5, "took {:?}", start.elapsed());
}
