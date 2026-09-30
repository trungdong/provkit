//! Round trips over generated names, values and namespace IRIs. Whatever the
//! PROV-N writer accepts reads back as an equal document.

#![cfg(all(feature = "json", feature = "provn"))]

use provkit::well_known::{self, XSD_INT, XSD_LONG};
use provkit::{
    Attributes, Bundle, DateTime, Document, Error, Formal, FormalAttr as F, Kind, LangStr,
    Namespaces, QName, Record, Value, json, provn,
};

fn ex(l: &str) -> QName {
    QName::new("http://example.org/", l).unwrap()
}

fn dt() -> DateTime {
    DateTime::parse("2011-11-16T16:05:33Z").unwrap()
}

fn element(kind: Kind, id: QName, attrs: Attributes) -> Record {
    Record::from_parts(kind, Some(id), vec![], attrs).unwrap()
}

fn relation(kind: Kind, id: Option<QName>, formals: Vec<(F, Formal)>) -> Record {
    Record::from_parts(kind, id, formals, Attributes::new()).unwrap()
}

/// A document in `http://example.org/` under `ex`, holding `records`.
fn document(records: Vec<Record>) -> Document {
    let mut d = Document::new();
    d.namespaces.declare("ex", "http://example.org/").unwrap();
    d.records = records;
    d
}

/// Write `d`; when the writer accepts it, it must read back equal.
/// Returns whether the writer accepted it.
fn written_reads_back(d: &Document) -> bool {
    match provn::to_string(d) {
        Ok(text) => {
            let back = provn::read(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
            assert_eq!(back, *d, "{text}");
            true
        }
        Err(Error::Unwritable { .. } | Error::UnwritableNamespace { .. }) => false,
        Err(e) => panic!("unexpected error {e:?} for {d:?}"),
    }
}

/// A document using `local` in every name position, under a declared
/// prefix and bare in the default namespace.
fn every_position(local: &str) -> Document {
    let default = "http://default.org/";
    let mut d = document(vec![]);
    d.namespaces.set_default(default);
    for ns in ["http://example.org/", default] {
        let n = QName::new(ns, local).unwrap();
        let mut a = Attributes::new();
        a.insert(n.clone(), Value::QName(n.clone()));
        a.insert(ex("t"), Value::typed("x", n.clone()).unwrap());
        d.records.push(element(Kind::Entity, n.clone(), a));
        d.records.push(relation(
            Kind::Usage,
            Some(n.clone()),
            vec![(F::Entity, Formal::Name(n.clone()))],
        ));
        d.bundles.push(Bundle {
            id: n,
            namespaces: Namespaces::new(),
            records: vec![],
        });
    }
    d
}

/// Every ASCII character and a sample beyond it, including combining
/// marks, `·`, `‿`, non-characters and separators.
fn palette() -> Vec<char> {
    let mut chars: Vec<char> = (0u8..0x80).map(char::from).collect();
    chars.extend([
        'é', '\u{A0}', '\u{B7}', '\u{D7}', '\u{300}', '\u{36F}', '\u{1680}', '\u{203F}',
        '\u{2040}', '\u{2028}', '\u{FEFF}', '\u{FFFE}', '😀',
    ]);
    chars
}

/// Local parts and lexical forms: every palette character alone and in
/// context, pseudo-random mixes, and hand-picked edge cases.
fn sample_texts() -> Vec<String> {
    let chars = palette();
    let mut locals: Vec<String> = Vec::new();
    for &c in &chars {
        locals.extend([
            c.to_string(),
            format!("a{c}"),
            format!("a{c}b"),
            format!("{c}a"),
        ]);
    }
    // Pseudo-random strings of one to four palette characters, so pairs
    // such as `%4` followed by a letter, or `\.` mixes, are covered.
    let mut seed: u64 = 0x5eed;
    for _ in 0..3000 {
        let mut local = String::new();
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        for _ in 0..=(seed >> 60) % 4 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let i = usize::try_from(seed >> 33).unwrap() % chars.len();
            local.push(chars[i]);
        }
        locals.push(local);
    }
    locals.extend(
        [
            "",
            "12",
            "%41",
            "a%4",
            "%zz",
            "a%",
            "%%41",
            "a.",
            ".a",
            "-1",
            "a\\",
            "//x",
            "/*y",
            "a//x",
            "a/*y",
            "\u{1680}a",
            "a\u{1680}",
            "\u{A0}a",
            "a\u{A0}",
            "\u{2028}a",
            "a\u{2028}",
        ]
        .map(String::from),
    );
    locals
}

/// Values from every public constructor: the `From` conversions, and
/// [`Value::typed`] over sample lexical forms and datatypes.
fn sample_values() -> Vec<Value> {
    let mut values = vec![
        Value::from(0i32),
        Value::from(i32::MIN),
        Value::from(i64::MAX),
        Value::from(1.5f32),
        Value::from(f32::NAN),
        Value::from(f64::INFINITY),
        Value::from(f64::NEG_INFINITY),
        Value::from(-0.0f64),
        Value::from(true),
        Value::from(false),
        Value::from("a\"b\\c\nd\re\tf"),
        Value::from(String::from("é\u{2028}😀")),
        Value::from(ex("q")),
        Value::from(dt()),
        Value::from(LangStr::new("hi", "en-GB").unwrap()),
    ];
    let datatypes = [
        XSD_INT.clone(),
        XSD_LONG.clone(),
        well_known::XSD_STRING.clone(),
        well_known::XSD_DATETIME.clone(),
        well_known::XSD_DOUBLE.clone(),
        QName::new(well_known::XSD, "anyURI").unwrap(),
        ex("dt"),
    ];
    for lexical in sample_texts() {
        for datatype in &datatypes {
            values.push(Value::typed(lexical.as_str(), datatype.clone()).unwrap());
        }
    }
    for lexical in ["2011-11-16T16:05:33Z", "2011-11-16T16:05:33.25+01:00"] {
        values.push(Value::typed(lexical, well_known::XSD_DATETIME.clone()).unwrap());
    }
    values
}

#[test]
fn a_qualified_name_literal_resolves_exactly_when_a_name_can_be_built() {
    let (mut resolved, mut rejected) = (0, 0);
    for local in sample_texts() {
        let text = serde_json::json!({
            "prefix": {"ex": "http://example.org/"},
            "entity": {"ex:e": {"ex:v": {"$": format!("ex:{local}"), "type": "xsd:QName"}}},
        })
        .to_string();
        match (QName::new("http://example.org/", &local), json::read(&text)) {
            (Ok(q), Ok(d)) => {
                let mut a = Attributes::new();
                a.insert(ex("v"), Value::QName(q));
                let expected = document(vec![element(Kind::Entity, ex("e"), a)]);
                assert_eq!(d, expected, "{local:?}");
                // It is written, and reads back equal from both formats.
                assert!(written_reads_back(&d), "{local:?}");
                let json = json::to_string(&d).unwrap();
                assert_eq!(json::read(&json).unwrap(), d, "{json}");
                resolved += 1;
            }
            (Err(_), Err(Error::InvalidQName(l))) => {
                assert_eq!(l, local);
                rejected += 1;
            }
            (built, read) => panic!("{local:?}: {built:?} against {read:?}"),
        }
    }
    assert!(resolved > 1000 && rejected > 100, "{resolved} {rejected}");
}

#[test]
fn whatever_the_writer_accepts_reads_back_equal() {
    let chars = palette();
    let locals = sample_texts();
    // Every name that can be built is written, and reads back equal.
    let (mut accepted, mut rejected) = (0, 0);
    for local in &locals {
        if QName::new("http://example.org/", local).is_err() {
            rejected += 1;
            continue;
        }
        assert!(written_reads_back(&every_position(local)), "{local:?}");
        accepted += 1;
    }
    assert!(accepted > 1000 && rejected > 100, "{accepted} {rejected}");

    // Every value the public constructors build is accepted, and reads
    // back equal through PROV-JSON too.
    for v in sample_values() {
        let mut a = Attributes::new();
        a.insert(ex("v"), v);
        let d = document(vec![element(Kind::Entity, ex("e"), a)]);
        assert!(written_reads_back(&d));
        {
            let text = json::to_string(&d).unwrap();
            assert_eq!(json::read(&text).unwrap(), d, "{text}");
        }
    }

    // Namespace IRIs, minted and default, and declared prefixes.
    for &c in &chars {
        let iri = format!("http://example.org/{c}/");
        let mut minted = Document::new();
        minted.records.push(element(
            Kind::Entity,
            QName::new(iri.as_str(), "e").unwrap(),
            Attributes::new(),
        ));
        written_reads_back(&minted);
        let mut default = Document::new();
        default.namespaces.set_default(iri.as_str());
        written_reads_back(&default);
        for prefix in [format!("p{c}"), format!("{c}p"), format!("p{c}q")] {
            let mut declared = Document::new();
            if declared
                .namespaces
                .declare(&prefix, "http://p.org/")
                .is_ok()
            {
                declared.records.push(element(
                    Kind::Entity,
                    QName::new("http://p.org/", "e").unwrap(),
                    Attributes::new(),
                ));
                written_reads_back(&declared);
            }
        }
    }
}
