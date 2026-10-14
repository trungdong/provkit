use crate::test_util::{Broken, bundle, document, dt, element, entity, entity_in, ex, relation};
use crate::well_known::{PROV_LABEL, PROV_TYPE, XSD_INT, XSD_INTEGER, XSD_LONG};
use crate::{
    Attributes, Bundle, Document, Error, Formal, FormalAttr as F, Kind, LangStr, Namespaces, QName,
    Record, Value, provn,
};

/// The single statement line of a one-record document.
fn line(record: Record) -> String {
    let mut d = document(vec![record]);
    provn::statement(&d.records[0], &mut d.namespaces).unwrap()
}

/// The statement line of entity `ex:e` with `pairs` as its attributes.
fn entity_line(pairs: impl IntoIterator<Item = (QName, Value)>) -> String {
    line(element(Kind::Entity, ex("e"), pairs.into_iter().collect()))
}

#[test]
fn prints_the_expected_text() {
    let mut d = Document::new();
    d.namespaces.set_default("http://default.org/");
    d.namespaces.declare("ex", "http://example.org/").unwrap();
    let a = [
        (PROV_TYPE.clone(), ex("T").into()),
        (ex("n"), 1i32.into()),
        (ex("s"), "a\"b\n".into()),
        (PROV_LABEL.clone(), LangStr::new("x", "en").unwrap().into()),
        (ex("d"), dt().into()),
        (ex("f"), 1.5f64.into()),
    ];
    d.records
        .push(element(Kind::Entity, ex("e1"), a.into_iter().collect()));
    d.records.push(relation(
        Kind::Activity,
        Some(QName::new("http://default.org/", "a1").unwrap()),
        vec![(F::StartTime, Formal::Time(dt()))],
    ));
    let a1 = QName::new("http://default.org/", "a1").unwrap();
    d.records.push(relation(
        Kind::Generation,
        Some(ex("g1")),
        vec![
            (F::Entity, Formal::Name(ex("e1"))),
            (F::Activity, Formal::Name(a1)),
        ],
    ));
    d.records.push(relation(
        Kind::Generation,
        None,
        vec![(F::Entity, Formal::Name(ex("e1")))],
    ));
    d.records.push(relation(
        Kind::Mention,
        None,
        vec![
            (F::SpecificEntity, Formal::Name(ex("e2"))),
            (F::GeneralEntity, Formal::Name(ex("e1"))),
            (F::Bundle, Formal::Name(ex("b"))),
        ],
    ));
    d.records.push(entity("e.2"));
    d.bundles.push(bundle("b", vec![entity("e2")]));
    let expected = "\
document
  default <http://default.org/>
  prefix ex <http://example.org/>

  entity(ex:e1, [prov:type='ex:T', ex:n=1, ex:s=\"a\\\"b\\n\", prov:label=\"x\"@en, ex:d=\"2011-11-16T16:05:33Z\" %% xsd:dateTime, ex:f=\"1.5\" %% xsd:double])
  activity(a1, 2011-11-16T16:05:33Z, -)
  wasGeneratedBy(ex:g1; ex:e1, a1, -)
  wasGeneratedBy(ex:e1, -, -)
  mentionOf(ex:e2, ex:e1, ex:b)
  entity(ex:e\\.2)
  bundle ex:b
    entity(ex:e2)
  endBundle
endDocument
";
    assert_eq!(provn::to_string(&d).unwrap(), expected);
    let mut names = d.namespaces.clone();
    assert_eq!(
        provn::statement(&d.records[3], &mut names).unwrap(),
        "wasGeneratedBy(ex:e1, -, -)"
    );
}

#[test]
fn relations_have_full_arity() {
    let r = relation(
        Kind::Derivation,
        Some(ex("d1")),
        vec![(F::UsedEntity, Formal::Name(ex("e0")))],
    );
    assert_eq!(line(r), "wasDerivedFrom(ex:d1; -, ex:e0, -, -, -)");
    let r = relation(Kind::Start, None, vec![(F::Time, Formal::Time(dt()))]);
    assert_eq!(line(r), "wasStartedBy(-, -, -, 2011-11-16T16:05:33Z)");
}

#[test]
fn activity_writes_both_times_or_neither() {
    let neither = element(Kind::Activity, ex("a"), Attributes::new());
    assert_eq!(line(neither), "activity(ex:a)");
    let end_only = relation(
        Kind::Activity,
        Some(ex("a")),
        vec![(F::EndTime, Formal::Time(dt()))],
    );
    assert_eq!(line(end_only), "activity(ex:a, -, 2011-11-16T16:05:33Z)");
    let attrs = [(ex("k"), "v".into())].into_iter().collect();
    assert_eq!(
        line(element(Kind::Activity, ex("a"), attrs)),
        "activity(ex:a, [ex:k=\"v\"])"
    );
}

#[test]
fn identified_relation_leads_with_its_id_and_a_semicolon() {
    let r = relation(
        Kind::Usage,
        Some(ex("u")),
        vec![(F::Activity, Formal::Name(ex("a")))],
    );
    assert_eq!(line(r), "used(ex:u; ex:a, -, -)");
}

#[test]
fn an_identifier_on_a_bare_kind_is_written_and_reads_back() {
    // PROV-JSON gives `hadMember` a key, which is the record's identifier.
    let r = relation(
        Kind::Membership,
        Some(ex("m")),
        vec![
            (F::Collection, Formal::Name(ex("c"))),
            (F::Entity, Formal::Name(ex("e"))),
        ],
    );
    assert_eq!(line(r.clone()), "hadMember(ex:m; ex:c, ex:e)");
    let d = document(vec![r]);
    let text = provn::to_string(&d).unwrap();
    assert_eq!(provn::read(&text).unwrap(), d);
    // The grammar gives these kinds no identifier, so `Strict` refuses what
    // the writer wrote, as it refuses `-` in a required argument.
    assert!(
        matches!(
            provn::read_with(&text, provn::Profile::Strict),
            Err(Error::Syntax { .. })
        ),
        "{text}"
    );
}

#[test]
fn mention_is_written_bare() {
    let r = relation(
        Kind::Mention,
        None,
        vec![(F::SpecificEntity, Formal::Name(ex("s")))],
    );
    assert_eq!(line(r), "mentionOf(ex:s, -, -)");
}

#[test]
fn values_follow_the_grammar() {
    let a = [
        (ex("q"), ex("name").into()),
        (ex("i"), 1i32.into()),
        (ex("neg"), Value::typed("-42", XSD_INT.clone()).unwrap()),
        (ex("zeros"), Value::typed("005", XSD_INT.clone()).unwrap()),
        (ex("l"), 7i64.into()),
        (ex("c"), Value::typed("x", ex("dt")).unwrap()),
    ];
    assert_eq!(
        entity_line(a),
        "entity(ex:e, [ex:q='ex:name', ex:i=1, ex:neg=-42, ex:zeros=005, \
         ex:l=\"7\" %% xsd:long, ex:c=\"x\" %% ex:dt])"
    );
}

#[test]
fn an_integer_is_bare_only_as_an_xsd_int() {
    // Only an xsd:int is written bare. Every reader types a bare integer
    // within the 32-bit range as xsd:int, and ProvToolbox reads no wider
    // bare integer.
    let cases = [
        ("5", &XSD_INT, true),
        ("-2147483648", &XSD_INT, true),
        ("2147483648", &XSD_INT, false),
        ("3000000000", &XSD_LONG, false),
        ("5", &XSD_LONG, false),
        ("-2147483649", &XSD_LONG, false),
        ("9223372036854775808", &XSD_LONG, false),
        ("99999999999999999999", &XSD_INTEGER, false),
        ("5", &XSD_INTEGER, false),
        ("3000000000", &XSD_INTEGER, false),
    ];
    for (text, datatype, bare) in cases {
        let value = Value::typed(text, QName::clone(datatype)).unwrap();
        let shown = if bare {
            text.to_owned()
        } else {
            format!("\"{text}\" %% xsd:{}", datatype.local())
        };
        assert_eq!(
            entity_line([(ex("i"), value)]),
            format!("entity(ex:e, [ex:i={shown}])"),
            "{text} as {datatype:?}"
        );
    }
}

#[test]
fn a_lone_minus_or_empty_int_is_not_bare() {
    for text in ["-", "", "+1", "1.0", "--1", "1 ", " 5 ", "abc"] {
        let value = Value::typed(text, XSD_INT.clone()).unwrap();
        assert_eq!(
            entity_line([(ex("i"), value)]),
            format!("entity(ex:e, [ex:i=\"{text}\" %% xsd:int])"),
            "lexical form {text:?}"
        );
    }
}

#[test]
fn local_parts_and_strings_escape() {
    let all = ex("a=b'c(d)e,f:g-h[i]j.k;l");
    assert_eq!(
        line(element(Kind::Entity, all, Attributes::new())),
        "entity(ex:a\\=b\\'c\\(d\\)e\\,f\\:g\\-h\\[i\\]j\\.k\\;l)"
    );
    assert_eq!(
        entity_line([(ex("s"), "q\" b\\ n\n r\r t\t".into())]),
        "entity(ex:e, [ex:s=\"q\\\" b\\\\ n\\n r\\r t\\t\"])"
    );
}

#[test]
fn undeclared_namespaces_get_minted_prefixes_and_the_document_is_untouched() {
    let mut d = Document::new();
    d.records.push(entity_in("http://new.org/", "e"));
    let before = d.namespaces.clone();
    assert_eq!(
        provn::to_string(&d).unwrap(),
        "document\n  prefix ns1 <http://new.org/>\n\n  entity(ns1:e)\nendDocument\n"
    );
    assert_eq!(d.namespaces, before);
}

#[test]
fn default_namespace_names_mint_no_prefix() {
    let mut d = Document::new();
    d.namespaces.set_default("http://default.org/");
    d.records.push(entity_in("http://default.org/", "e"));
    assert_eq!(
        provn::to_string(&d).unwrap(),
        "document\n  default <http://default.org/>\n\n  entity(e)\nendDocument\n"
    );
}

#[test]
fn empty_document_has_no_blank_line() {
    assert_eq!(
        provn::to_string(&Document::new()).unwrap(),
        "document\nendDocument\n"
    );
}

#[test]
fn bundles_sharing_an_identifier_are_written_once() {
    let mut d = document(vec![]);
    for id in ["e1", "e2"] {
        d.bundles.push(bundle("b", vec![entity(id)]));
    }
    let text = provn::to_string(&d).unwrap();
    assert_eq!(text.matches("bundle ex:b").count(), 1);
    assert!(text.contains("entity(ex:e1)") && text.contains("entity(ex:e2)"));
}

#[test]
fn bundle_prefix_overrides_the_documents_for_its_own_identifier() {
    let bundle_ns = "http://bundle.org/";
    let mut d = document(vec![]);
    let mut inner = Namespaces::new();
    inner.declare("ex", bundle_ns).unwrap();
    d.bundles.push(Bundle {
        id: QName::new(bundle_ns, "b").unwrap(),
        namespaces: inner,
        records: vec![entity_in(bundle_ns, "in"), entity("out")],
    });
    let text = provn::to_string(&d).unwrap();
    // The identifier and the bundle's own names use the bundle's `ex`;
    // the document's `ex` namespace needs a fresh prefix inside.
    let expected = "\
document
  prefix ex <http://example.org/>

  bundle ex:b
    prefix ex <http://bundle.org/>
    prefix ns1 <http://example.org/>
    entity(ex:in)
    entity(ns1:out)
  endBundle
endDocument
";
    assert_eq!(text, expected);
}

#[test]
fn bundle_inherits_and_does_not_repeat_document_declarations() {
    let mut d = document(vec![]);
    d.namespaces.set_default("http://default.org/");
    d.bundles
        .push(bundle("b", vec![entity_in("http://default.org/", "e")]));
    assert_eq!(
        provn::to_string(&d).unwrap(),
        "document\n  default <http://default.org/>\n  prefix ex <http://example.org/>\n\n  \
         bundle ex:b\n    entity(e)\n  endBundle\nendDocument\n"
    );
}

/// The name and reason of the `Unwritable` error from writing `d`.
fn unwritable(d: &Document) -> (QName, String) {
    match provn::to_string(d) {
        Err(Error::Unwritable { name, reason }) => (name, reason),
        other => panic!("expected Error::Unwritable, got {other:?}"),
    }
}

#[test]
fn unrepresentable_iris_are_errors() {
    let name_in = |iri: &str| entity_in(iri, "e");
    // Met while rendering a name.
    let mut minted = Document::new();
    minted.records.push(name_in("http://x.org/a b/"));
    let (name, reason) = unwritable(&minted);
    assert_eq!(name, QName::new("http://x.org/a b/", "e").unwrap());
    assert!(reason.contains("' ' (U+0020)"), "{reason}");

    // Met while writing a declaration. The default namespace has an
    // empty prefix.
    let mut default = Document::new();
    default.namespaces.set_default("http://x.org/>");
    assert!(matches!(
        provn::to_string(&default),
        Err(Error::UnwritableNamespace { prefix, iri, .. })
            if prefix.is_empty() && iri == "http://x.org/>"
    ));

    let mut declared = Document::new();
    declared.namespaces.declare("q", "http://x.org/\"").unwrap();
    assert!(matches!(
        provn::to_string(&declared),
        Err(Error::UnwritableNamespace { prefix, iri, .. })
            if prefix == "q" && iri == "http://x.org/\""
    ));

    let mut names = Namespaces::new();
    let bad = QName::new("http://x.org/{", "e").unwrap();
    assert!(matches!(
        provn::name(&bad, &mut names),
        Err(Error::Unwritable { name, .. }) if name == bad
    ));
    assert!(matches!(
        provn::statement(&name_in("http://x.org/|"), &mut names),
        Err(Error::Unwritable { .. })
    ));
}

#[test]
fn an_attribute_spelling_an_own_formal_cannot_be_written() {
    let a = [(
        QName::new(crate::well_known::PROV, "entity").unwrap(),
        ex("x").into(),
    )];
    let r = Record::Usage(crate::Usage {
        id: None,
        activity: Some(ex("a")),
        entity: None,
        time: None,
        attributes: a.into_iter().collect(),
    });
    let d = document(vec![r]);
    assert!(matches!(
        provn::to_string(&d),
        Err(Error::InvalidRecord {
            kind: Kind::Usage,
            ..
        })
    ));
}

#[test]
fn io_failure_is_reported_as_io() {
    let err = provn::write(&Document::new(), &mut Broken).unwrap_err();
    assert!(matches!(err, Error::Io(_)), "{err:?}");
    let mut ok = Vec::new();
    provn::write(&Document::new(), &mut ok).unwrap();
    assert_eq!(ok, b"document\nendDocument\n");
}

#[test]
fn minted_prefixes_stay_with_the_caller() {
    let mut names = Namespaces::new();
    let a = provn::name(&QName::new("http://one.org/", "12").unwrap(), &mut names).unwrap();
    let b = provn::name(&QName::new("http://two.org/", "34").unwrap(), &mut names).unwrap();
    let again = provn::name(&QName::new("http://one.org/", "56").unwrap(), &mut names).unwrap();
    assert_eq!(
        (a.as_str(), b.as_str(), again.as_str()),
        ("ns1:12", "ns2:34", "ns1:56")
    );
    assert_eq!(names.lookup("ns1"), Some("http://one.org/"));
    assert_eq!(names.lookup("ns2"), Some("http://two.org/"));
}

#[test]
fn a_minted_prefix_skips_one_the_table_already_binds() {
    let mut names = Namespaces::new();
    names.declare("ns1", "http://taken.org/").unwrap();
    let minted = provn::name(&QName::new("http://one.org/", "e").unwrap(), &mut names).unwrap();
    assert_eq!(minted, "ns2:e");
    assert_eq!(names.lookup("ns1"), Some("http://taken.org/"));
    assert_eq!(names.lookup("ns2"), Some("http://one.org/"));
}

#[test]
fn statement_hands_back_the_prefixes_it_mints() {
    let mut names = Namespaces::new();
    let e = entity_in("http://one.org/", "e");
    assert_eq!(provn::statement(&e, &mut names).unwrap(), "entity(ns1:e)");
    assert_eq!(names.lookup("ns1"), Some("http://one.org/"));
    assert_eq!(provn::statement(&e, &mut names).unwrap(), "entity(ns1:e)");
    assert_eq!(names.iter().count(), 1);
}

/// Formals by attribute and local name in `http://example.org/`; a time
/// formal ignores its name.
type Formals = &'static [(F, &'static str)];

#[test]
// The table of fifteen relations is the test.
#[allow(clippy::too_many_lines)]
fn every_relation_writes_its_formals_in_grammar_order() {
    // Each relation with every formal present, and the statement PROV-N
    // gives it. The four relations without an identifier in the grammar
    // carry none.
    let time = "2011-11-16T16:05:33Z";
    let cases: [(Kind, bool, Formals, String); 15] = [
        (
            Kind::Generation,
            true,
            &[(F::Entity, "e"), (F::Activity, "a"), (F::Time, "")],
            format!("wasGeneratedBy(ex:r; ex:e, ex:a, {time})"),
        ),
        (
            Kind::Usage,
            true,
            &[(F::Activity, "a"), (F::Entity, "e"), (F::Time, "")],
            format!("used(ex:r; ex:a, ex:e, {time})"),
        ),
        (
            Kind::Communication,
            true,
            &[(F::Informed, "a2"), (F::Informant, "a1")],
            "wasInformedBy(ex:r; ex:a2, ex:a1)".to_owned(),
        ),
        (
            Kind::Start,
            true,
            &[
                (F::Activity, "a"),
                (F::Trigger, "trigger"),
                (F::Starter, "starter"),
                (F::Time, ""),
            ],
            format!("wasStartedBy(ex:r; ex:a, ex:trigger, ex:starter, {time})"),
        ),
        (
            Kind::End,
            true,
            &[
                (F::Activity, "a"),
                (F::Trigger, "trigger"),
                (F::Ender, "ender"),
                (F::Time, ""),
            ],
            format!("wasEndedBy(ex:r; ex:a, ex:trigger, ex:ender, {time})"),
        ),
        (
            Kind::Invalidation,
            true,
            &[(F::Entity, "e"), (F::Activity, "a"), (F::Time, "")],
            format!("wasInvalidatedBy(ex:r; ex:e, ex:a, {time})"),
        ),
        (
            Kind::Derivation,
            true,
            &[
                (F::GeneratedEntity, "e2"),
                (F::UsedEntity, "e1"),
                (F::Activity, "a"),
                (F::Generation, "g"),
                (F::Usage, "u"),
            ],
            "wasDerivedFrom(ex:r; ex:e2, ex:e1, ex:a, ex:g, ex:u)".to_owned(),
        ),
        (
            Kind::Attribution,
            true,
            &[(F::Entity, "e"), (F::Agent, "ag")],
            "wasAttributedTo(ex:r; ex:e, ex:ag)".to_owned(),
        ),
        (
            Kind::Association,
            true,
            &[(F::Activity, "a"), (F::Agent, "ag"), (F::Plan, "pl")],
            "wasAssociatedWith(ex:r; ex:a, ex:ag, ex:pl)".to_owned(),
        ),
        (
            Kind::Delegation,
            true,
            &[
                (F::Delegate, "ag2"),
                (F::Responsible, "ag1"),
                (F::Activity, "a"),
            ],
            "actedOnBehalfOf(ex:r; ex:ag2, ex:ag1, ex:a)".to_owned(),
        ),
        (
            Kind::Influence,
            true,
            &[(F::Influencee, "o2"), (F::Influencer, "o1")],
            "wasInfluencedBy(ex:r; ex:o2, ex:o1)".to_owned(),
        ),
        (
            Kind::Specialization,
            false,
            &[(F::SpecificEntity, "spec"), (F::GeneralEntity, "gen")],
            "specializationOf(ex:spec, ex:gen)".to_owned(),
        ),
        (
            Kind::Alternate,
            false,
            &[(F::Alternate1, "alt1"), (F::Alternate2, "alt2")],
            "alternateOf(ex:alt1, ex:alt2)".to_owned(),
        ),
        (
            Kind::Mention,
            false,
            &[
                (F::SpecificEntity, "spec"),
                (F::GeneralEntity, "gen"),
                (F::Bundle, "b"),
            ],
            "mentionOf(ex:spec, ex:gen, ex:b)".to_owned(),
        ),
        (
            Kind::Membership,
            false,
            &[(F::Collection, "c"), (F::Entity, "e")],
            "hadMember(ex:c, ex:e)".to_owned(),
        ),
    ];
    for (kind, has_id, formals, expected) in cases {
        let formals = formals
            .iter()
            .map(|&(attr, local)| match attr {
                F::Time => (attr, Formal::Time(dt())),
                _ => (attr, Formal::Name(ex(local))),
            })
            .collect();
        let record = relation(kind, has_id.then(|| ex("r")), formals);
        assert_eq!(line(record), expected, "{kind:?}");
    }
}
