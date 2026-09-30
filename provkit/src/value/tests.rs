use super::*;
use crate::test_util::hash;

fn typed(lexical: &str, datatype: &QName) -> Value {
    Value::typed(lexical, datatype.clone()).unwrap()
}

/// Every form in `forms` is one value of `datatype`: equal and hashing alike.
fn assert_one_value(datatype: &QName, forms: &[&str]) {
    let first = typed(forms[0], datatype);
    for form in forms {
        let other = typed(form, datatype);
        assert_eq!(first, other, "{:?} and {form:?} as {datatype}", forms[0]);
        assert_eq!(
            hash(&first),
            hash(&other),
            "{:?} and {form:?} as {datatype}",
            forms[0]
        );
    }
}

/// No two forms in `forms` are one value of `datatype`.
fn assert_distinct_values(datatype: &QName, forms: &[&str]) {
    for (i, a) in forms.iter().enumerate() {
        for b in &forms[i + 1..] {
            assert_ne!(
                typed(a, datatype),
                typed(b, datatype),
                "{a:?} and {b:?} as {datatype}"
            );
        }
    }
}

#[test]
fn typed_folds_string_and_parses_datetime() {
    assert_eq!(
        Value::typed("x", XSD_STRING.clone()).unwrap(),
        Value::Str("x".into())
    );
    assert!(matches!(
        Value::typed("2011-11-16T16:05:33Z", XSD_DATETIME.clone()).unwrap(),
        Value::DateTime(_)
    ));
    assert_eq!(
        Value::typed("1", XSD_INT.clone()).unwrap(),
        Value::typed("1", XSD_INT.clone()).unwrap()
    );
}

#[test]
fn a_padded_datetime_reads_as_a_datetime_and_prints_trimmed() {
    let padded = typed(" \t2011-11-16T16:05:33Z\r\n", &XSD_DATETIME);
    let Value::DateTime(parsed) = &padded else {
        panic!("expected Value::DateTime, got {padded:?}");
    };
    assert_eq!(parsed.to_string(), "2011-11-16T16:05:33Z");
    assert_eq!(padded, typed("2011-11-16T16:05:33Z", &XSD_DATETIME));
    assert!(padded.has_one_representation());
    // Other white space is not trimmed, so the text stays ill-formed.
    let other = typed("\u{A0}2011-11-16T16:05:33Z", &XSD_DATETIME);
    assert!(matches!(other, Value::Typed { .. }));
    assert!(other.has_one_representation());
    // A padded `Typed` that should have been parsed is reported.
    let unfolded = Value::Typed {
        lexical: " 2011-11-16T16:05:33Z ".into(),
        datatype: XSD_DATETIME.clone(),
    };
    assert!(!unfolded.has_one_representation());
}

#[test]
fn a_well_formed_key_never_equals_the_text_key_of_the_same_content() {
    // Through `Key::of`, the well-formed spellings meet and the
    // ill-formed ones stay apart from them.
    assert_eq!(Key::of("+5", &XSD_INT), Key::of("5", &XSD_INT));
    assert_ne!(Key::of("5", &XSD_INT), Key::of("5.0", &XSD_INT));
    assert!(matches!(Key::of("5.0", &XSD_INT), Key::Text("5.0")));
    assert!(matches!(Key::of("1", &XSD_BOOLEAN), Key::Boolean(true)));
    assert!(matches!(Key::of("yes", &XSD_BOOLEAN), Key::Text("yes")));
}

#[test]
fn ill_formed_literals_are_kept() {
    for (lexical, datatype) in [
        ("abc", &*XSD_INT),
        ("maybe", &*XSD_BOOLEAN),
        ("2012-13-01T00:00:00", &*XSD_DATETIME),
        ("2012-01-01", &*XSD_DATETIME),
    ] {
        let value = Value::typed(lexical, datatype.clone()).unwrap();
        assert!(
            matches!(&value, Value::Typed { lexical: l, .. } if l == lexical),
            "{lexical}"
        );
        assert_eq!(value.datatype().as_ref(), Some(datatype));
    }
}

#[test]
fn integers_compare_by_value_under_one_datatype() {
    let long = "1234567890123456789012345678901234567890";
    for datatype in INTEGER_TYPES {
        assert_one_value(datatype, &[" 5 ", "5", "+5", "005", "\t5\r\n", "+0005"]);
        assert_one_value(datatype, &["0", "-0", "+0", "000", "-000"]);
        assert_one_value(datatype, &["-5", "-05", " -5"]);
        // Forty digits compare exactly, and no range is checked.
        assert_one_value(datatype, &[long, &format!("+000{long}")]);
        assert_distinct_values(
            datatype,
            &[
                "5",
                "-5",
                "6",
                "50",
                "0",
                long,
                &format!("{}1", &long[..39]),
                &format!("-{long}"),
            ],
        );
    }
}

#[test]
fn decimals_compare_by_value() {
    let dt = &*XSD_DECIMAL;
    assert_one_value(dt, &["1.50", "1.5", "+1.5", "01.5", " 1.500 "]);
    assert_one_value(dt, &["5", "5.0", "5.", "+05.00"]);
    assert_one_value(dt, &["0", "-0.0", "+0", "0.", ".0", "-.000", "000"]);
    assert_one_value(dt, &[".5", "0.5", "+.50", "00.500"]);
    assert_one_value(dt, &["-1.5", "-01.50"]);
    let long = "1234567890123456789012345678901234567890";
    assert_one_value(
        dt,
        &[&format!("{long}.{long}"), &format!("0{long}.{long}0")],
    );
    assert_distinct_values(
        dt,
        &[
            "1.5",
            "-1.5",
            "1.05",
            "15",
            "0.15",
            "0",
            &format!("{long}.{long}"),
            &format!("{long}.{long}1"),
        ],
    );
}

#[test]
fn doubles_and_floats_compare_by_value() {
    for dt in [&*XSD_DOUBLE, &*XSD_FLOAT] {
        assert_one_value(
            dt,
            &[
                "1", "1.0", "1e0", "1E0", "+1", "1.", "1.e0", "10e-1", ".1e1", " 1 ", "0.01E+2",
            ],
        );
        assert_one_value(dt, &["NaN", "NaN", " NaN\n"]);
        assert_one_value(dt, &["0", "-0", "0.0", "-0.0", "+0", "-.0e5", "0E0"]);
        // An exponent too large for the type rounds to infinity.
        assert_one_value(dt, &["INF", "+INF", "1e999", " INF "]);
        assert_one_value(dt, &["-INF", "-1e999"]);
        assert_one_value(dt, &[".5", "0.5", "5e-1", "5.E-1"]);
        assert_distinct_values(dt, &["1", "2", "-1", "0", "NaN", "INF", "-INF", "1.5"]);
    }
}

#[test]
fn floats_compare_in_single_precision() {
    // Both round to one `f32` and to two `f64`s.
    let forms = ["0.1", "0.100000001"];
    assert_one_value(&XSD_FLOAT, &forms);
    assert_distinct_values(&XSD_DOUBLE, &forms);
    assert_one_value(&XSD_FLOAT, &["INF", "1e39"]);
    assert_distinct_values(&XSD_DOUBLE, &["INF", "1e39"]);
}

#[test]
fn spellings_outside_the_xsd_pattern_are_not_numbers() {
    // Rust's float parser reads all of these; XSD reads none.
    let rust_only = [
        "inf", "+inf", "-inf", "infinity", "Infinity", "INFINITY", "nan", "NAN",
    ];
    for dt in [&*XSD_DOUBLE, &*XSD_FLOAT] {
        assert_distinct_values(
            dt,
            &["INF", "inf", "+inf", "infinity", "Infinity", "INFINITY"],
        );
        assert_distinct_values(dt, &["-INF", "-inf", "-infinity"]);
        assert_distinct_values(dt, &["NaN", "nan", "NAN", "+NaN", "-NaN"]);
        assert_distinct_values(
            dt,
            &["1", "1e", "1e+", "+", ".", "e1", "", " ", "1 0", "0x1"],
        );
        for form in rust_only {
            assert_eq!(typed(form, dt), typed(form, dt));
            assert_ne!(typed(form, dt), typed(&format!(" {form}"), dt), "{form:?}");
        }
    }
    for dt in INTEGER_TYPES {
        assert_distinct_values(dt, &["5", "5.0", "5e0", "+", "-", "", "5 5", "٥", "0x5"]);
        // Only space, tab, CR and LF are trimmed.
        assert_distinct_values(dt, &["5", "\u{A0}5", "5\u{2003}", "\u{C}5", "5\u{B}"]);
    }
    assert_distinct_values(&XSD_DECIMAL, &["1", "1e0", ".", "+.", "1.0.0", "", "1,0"]);
}

#[test]
fn booleans_compare_by_value() {
    let dt = &*XSD_BOOLEAN;
    assert_one_value(dt, &["true", "1", " true ", "\t1"]);
    assert_one_value(dt, &["false", "0", "0\n"]);
    assert_distinct_values(dt, &["true", "false", "TRUE", "True", "yes", "01", "maybe"]);
}

#[test]
fn ill_formed_text_compares_untrimmed() {
    assert_one_value(&XSD_INT, &["abc", "abc"]);
    assert_distinct_values(&XSD_INT, &["abc", "abd", " abc", "abc "]);
    assert_distinct_values(
        &XSD_DATETIME,
        &["2012-13-01T00:00:00", "2012-13-01T00:00:00 "],
    );
    // Other datatypes have no value space here, so text is compared as written.
    let any_uri = QName::new(crate::well_known::XSD, "anyURI").unwrap();
    assert_distinct_values(&any_uri, &["5", " 5 ", "05"]);
}

#[test]
fn values_compare_within_one_datatype_only() {
    assert_ne!(typed("5", &XSD_INT), typed("5", &XSD_LONG));
    assert_ne!(typed("5", &XSD_INT), typed("5", &XSD_INTEGER));
    assert_ne!(typed("abc", &XSD_INT), typed("abc", &XSD_DOUBLE));
    assert_ne!(typed("1", &XSD_DOUBLE), typed("1", &XSD_FLOAT));
    assert_ne!(typed("1", &XSD_DECIMAL), typed("1", &XSD_DOUBLE));
    assert_ne!(typed("1", &XSD_BOOLEAN), typed("1", &XSD_INT));
    assert_ne!(typed("5", &XSD_INT), Value::from("5"));
    assert_ne!(
        typed("nope", &XSD_DATETIME),
        typed("2011-11-16T16:05:33Z", &XSD_DATETIME)
    );
    // The datatype is compared as a name: by its IRI, however it is split.
    let split = QName::new("http://www.w3.org/2001/", "XMLSchema#int").unwrap();
    assert_one_value(&split, &["5", " 005 "]);
    assert_eq!(typed("+5", &split), typed("5", &XSD_INT));
    assert_eq!(hash(&typed("+5", &split)), hash(&typed("5", &XSD_INT)));
}

#[test]
fn each_value_keeps_its_text() {
    for (lexical, datatype) in [
        (" 5 ", &*XSD_INT),
        ("005", &*XSD_INT),
        ("1.50", &*XSD_DECIMAL),
        ("1E0", &*XSD_DOUBLE),
        ("1", &*XSD_BOOLEAN),
    ] {
        assert!(
            matches!(typed(lexical, datatype), Value::Typed { lexical: l, .. } if l == lexical)
        );
    }
}

#[test]
fn typed_rejects_qualified_name_datatypes() {
    for (datatype, spelling) in [
        (XSD_QNAME.clone(), "xsd:QName"),
        (PROV_QUALIFIED_NAME.clone(), "prov:QUALIFIED_NAME"),
    ] {
        for text in ["ex:foo", "not a name"] {
            match Value::typed(text, datatype.clone()) {
                Err(Error::InvalidLiteral {
                    datatype: shown,
                    lexical,
                    reason,
                }) => {
                    assert_eq!(shown, spelling);
                    assert_eq!(lexical, text);
                    assert_eq!(
                        reason,
                        "a qualified-name value must be resolved against a namespace table; \
                         use Value::QName"
                    );
                }
                other => panic!("expected InvalidLiteral, got {other:?}"),
            }
        }
    }
}

fn example_names() -> Namespaces {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/").unwrap();
    ns
}

#[test]
fn typed_in_resolves_qualified_names_and_delegates_the_rest() {
    let ns = example_names();
    let foo = QName::new("http://example.org/", "foo").unwrap();
    for datatype in [XSD_QNAME.clone(), PROV_QUALIFIED_NAME.clone()] {
        assert_eq!(
            Value::typed_in(&ns, "ex:foo", datatype).unwrap(),
            Value::QName(foo.clone())
        );
    }
    assert_eq!(
        Value::typed_in(&ns, "x", XSD_STRING.clone()).unwrap(),
        Value::Str("x".into())
    );
    assert_eq!(
        Value::typed_in(&ns, "1", XSD_INT.clone()).unwrap(),
        Value::from(1i32)
    );
    assert!(matches!(
        Value::typed_in(&ns, "nope", XSD_DATETIME.clone()).unwrap(),
        Value::Typed { lexical, .. } if lexical == "nope"
    ));
}

#[test]
fn typed_in_rejects_text_that_is_not_a_qualified_name() {
    let mut with_default = example_names();
    with_default.set_default("http://default.org/");
    for datatype in [XSD_QNAME.clone(), PROV_QUALIFIED_NAME.clone()] {
        // Text no table resolves.
        for ns in [&example_names(), &with_default] {
            for text in ["", ":foo", "_:foo"] {
                assert!(
                    matches!(
                        Value::typed_in(ns, text, datatype.clone()),
                        Err(Error::InvalidQName(t)) if t == text
                    ),
                    "{text:?}"
                );
            }
        }
        // A local part PROV-N cannot write, under a prefix or bare.
        for local in ["a b", "a\\.b", "a%", "<a>", "\u{301}a"] {
            let text = format!("ex:{local}");
            assert!(
                matches!(
                    Value::typed_in(&with_default, &text, datatype.clone()),
                    Err(Error::InvalidQName(t)) if t == text
                ),
                "{local:?}"
            );
        }
        assert!(matches!(
            Value::typed_in(&with_default, "not a name", datatype.clone()),
            Err(Error::InvalidQName(l)) if l == "not a name"
        ));
        // A prefix that is not a `PN_PREFIX` can never be declared.
        assert!(matches!(
            Value::typed_in(&with_default, "1x:foo", datatype.clone()),
            Err(Error::UndeclaredPrefix(p)) if p == "1x"
        ));
        // Without a default namespace, bare text fails on that first.
        assert!(matches!(
            Value::typed_in(&example_names(), "not a name", datatype),
            Err(Error::NoDefaultNamespace(l)) if l == "not a name"
        ));
    }
}

#[test]
fn a_qualified_name_value_must_resolve() {
    let ns = example_names();
    for datatype in [XSD_QNAME.clone(), PROV_QUALIFIED_NAME.clone()] {
        assert!(matches!(
            Value::typed_in(&ns, "zz:foo", datatype.clone()),
            Err(Error::UndeclaredPrefix(p)) if p == "zz"
        ));
        assert!(matches!(
            Value::typed_in(&ns, "foo", datatype),
            Err(Error::NoDefaultNamespace(l)) if l == "foo"
        ));
    }
    // The text spells the name as the model holds it, without escapes.
    let name = |local: &str| Value::QName(QName::new("http://example.org/", local).unwrap());
    for local in ["a=b", "a:b", "a.b.", "-a", "a(1)", "a%2Fb", "12", ""] {
        assert_eq!(
            Value::typed_in(&ns, &format!("ex:{local}"), XSD_QNAME.clone()).unwrap(),
            name(local),
            "{local:?}"
        );
    }
}

#[test]
fn every_constructor_keeps_one_representation() {
    let ns = example_names();
    let values = [
        Value::from("s"),
        Value::from(1i32),
        Value::from(f64::NAN),
        Value::typed("x", XSD_STRING.clone()).unwrap(),
        Value::typed("2011-11-16T16:05:33Z", XSD_DATETIME.clone()).unwrap(),
        Value::typed("nope", XSD_DATETIME.clone()).unwrap(),
        Value::typed("x", QName::new("http://example.org/", "dt").unwrap()).unwrap(),
        Value::typed_in(&ns, "ex:foo", XSD_QNAME.clone()).unwrap(),
    ];
    assert!(values.iter().all(Value::has_one_representation));
    // The shapes no constructor builds.
    let typed = |lexical: &str, datatype: &QName| Value::Typed {
        lexical: lexical.to_owned(),
        datatype: datatype.clone(),
    };
    for folded in [
        typed("x", &XSD_STRING),
        typed("2011-11-16T16:05:33Z", &XSD_DATETIME),
        typed("ex:foo", &XSD_QNAME),
        typed("foo", &PROV_QUALIFIED_NAME),
        typed("not a name", &XSD_QNAME),
    ] {
        assert!(!folded.has_one_representation(), "{folded:?}");
    }
}

#[test]
fn datatype_reports_the_asserted_type() {
    assert_eq!(Value::from("s").datatype(), Some(XSD_STRING.clone()));
    assert_eq!(Value::from(1i32).datatype(), Some(XSD_INT.clone()));
    let tagged = Value::from(LangStr::new("hi", "en").unwrap());
    assert_eq!(tagged.datatype(), None);
}

#[test]
fn lang_tags_compare_case_insensitively() {
    let a = LangStr::new("hi", "en-GB").unwrap();
    let b = LangStr::new("hi", "EN-gb").unwrap();
    assert_eq!(a, b);
    assert_eq!(hash(&a), hash(&b));
    assert_ne!(a, LangStr::new("hi", "en-US").unwrap());
    assert_ne!(a, LangStr::new("bye", "en-GB").unwrap());
    assert_eq!(a.value(), "hi");
    assert_eq!(b.lang(), "EN-gb");
}

#[test]
fn lang_tags_follow_the_langtag_shape() {
    for tag in [
        "en",
        "a",
        "EN",
        "en-GB",
        "zh-Hant-TW",
        "en-1",
        "x-1-2",
        "abcdefghi",
        "en-GB-x-averylongprivatesubtag01",
    ] {
        let tagged = LangStr::new("hi", tag).unwrap();
        assert_eq!(tagged.lang(), tag);
    }
    for tag in [
        "", "en_US", "en-", "-en", "en--GB", "en-GB-", "1a", "1", "é", "en-é", "en GB", " en",
        "@en", "-",
    ] {
        assert!(
            matches!(LangStr::new("hi", tag), Err(Error::InvalidLanguageTag(t)) if t == tag),
            "{tag:?}"
        );
    }
}

#[test]
fn native_conversions_are_exact() {
    assert_eq!(
        Value::from(1i32),
        Value::typed("1", XSD_INT.clone()).unwrap()
    );
    assert_eq!(
        Value::from(1i64),
        Value::typed("1", XSD_LONG.clone()).unwrap()
    );
    assert_eq!(
        Value::from(1.5f64),
        Value::typed("1.5", XSD_DOUBLE.clone()).unwrap()
    );
    assert_eq!(
        Value::from(1.5f32),
        Value::typed("1.5", XSD_FLOAT.clone()).unwrap()
    );
    assert_eq!(
        Value::from(true),
        Value::typed("true", XSD_BOOLEAN.clone()).unwrap()
    );
    assert_eq!(Value::from("s"), Value::Str("s".into()));
}

#[test]
fn native_floats_equal_their_xsd_spellings() {
    // `to_string` writes a float without an exponent, and the key reads it back.
    for v in [
        0.0,
        -0.0,
        1.0,
        1e300,
        1e-300,
        f64::MAX,
        f64::MIN_POSITIVE,
        0.1,
    ] {
        assert_eq!(Value::from(v), typed(&format!("{v:e}"), &XSD_DOUBLE), "{v}");
    }
    for v in [0.0f32, 1.0, 1e30, f32::MAX, f32::MIN_POSITIVE, 0.1] {
        assert_eq!(Value::from(v), typed(&format!("{v:e}"), &XSD_FLOAT), "{v}");
    }
    assert_eq!(Value::from(f64::INFINITY), typed("+INF", &XSD_DOUBLE));
    assert_eq!(Value::from(f64::NAN), Value::from(-f64::NAN));
    assert_eq!(hash(&Value::from(f64::NAN)), hash(&Value::from(-f64::NAN)));
}

#[test]
fn non_finite_floats_use_xsd_spellings() {
    let lexical = |v: Value| match v {
        Value::Typed { lexical, .. } => lexical,
        other => panic!("expected Typed, got {other:?}"),
    };
    assert_eq!(lexical(Value::from(f64::NAN)), "NaN");
    assert_eq!(lexical(Value::from(f64::INFINITY)), "INF");
    assert_eq!(lexical(Value::from(f64::NEG_INFINITY)), "-INF");
    assert_eq!(lexical(Value::from(f32::NAN)), "NaN");
    assert_eq!(lexical(Value::from(f32::INFINITY)), "INF");
    assert_eq!(lexical(Value::from(f32::NEG_INFINITY)), "-INF");
}

#[test]
fn same_lexical_different_datatype_differs() {
    assert_ne!(Value::from(1i32), Value::from(1i64));
}

#[test]
fn string_conversions_give_str() {
    assert_eq!(Value::from(String::from("s")), Value::Str("s".into()));
    assert_eq!(Value::from(String::new()), Value::from(""));
}
