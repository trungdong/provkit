use super::*;
use crate::test_util::ex;

#[test]
fn tables_that_differ_only_in_the_default_are_unequal() {
    let mut a = Namespaces::new();
    a.declare("ex", "http://example.org/").unwrap();
    let mut b = a.clone();
    assert_eq!(a, b);
    b.set_default("http://default.org/").unwrap();
    assert_ne!(a, b);
    a.set_default("http://other.org/").unwrap();
    assert_ne!(a, b);
}

#[test]
fn resolves_prefixed_bare_and_predeclared() {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/").unwrap();
    ns.set_default("http://default.org/").unwrap();
    assert_eq!(ns.resolve("ex:e1").unwrap(), ex("e1"));
    assert_eq!(
        ns.resolve("e1").unwrap(),
        QName::new("http://default.org/", "e1").unwrap()
    );
    assert_eq!(
        ns.resolve("prov:type").unwrap(),
        QName::new(PROV, "type").unwrap()
    );
    assert_eq!(
        ns.resolve("xsd:int").unwrap(),
        QName::new(XSD, "int").unwrap()
    );
}

#[test]
fn undeclared_prefix_and_bare_without_default_fail() {
    let ns = Namespaces::new();
    assert!(matches!(ns.resolve("ex:e1"), Err(Error::UndeclaredPrefix(p)) if p == "ex"));
    assert!(matches!(ns.resolve("e1"), Err(Error::NoDefaultNamespace(l)) if l == "e1"));
}

#[test]
fn a_full_iri_is_an_undeclared_prefix_naming_its_scheme() {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/").unwrap();
    assert!(
        matches!(ns.resolve("http://example.org/e"), Err(Error::UndeclaredPrefix(p)) if p == "http")
    );
}

#[test]
fn malformed_names_are_invalid() {
    let mut ns = Namespaces::new();
    ns.set_default("http://default.org/").unwrap();
    assert!(matches!(ns.resolve(":e1"), Err(Error::InvalidQName(_))));
    assert!(matches!(ns.resolve(""), Err(Error::InvalidQName(_))));
}

#[test]
fn a_local_part_provn_cannot_write_does_not_resolve() {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/").unwrap();
    ns.set_default("http://default.org/").unwrap();
    for text in ["ex:a b", "a b", "ex:a%4"] {
        assert!(
            matches!(ns.resolve(text), Err(Error::InvalidQName(t)) if t == text),
            "{text:?}"
        );
    }
    assert!(matches!(
        Scope::of(&ns).resolve_parts("ex", "a\tb"),
        Err(Error::InvalidQName(t)) if t == "ex:a\tb"
    ));
    assert!(matches!(
        Scope::of(&ns).resolve_parts("", "a\tb"),
        Err(Error::InvalidQName(t)) if t == "a\tb"
    ));
    assert_eq!(ns.resolve("ex:").unwrap(), ex(""));
    // The prefix is looked up before the local part is checked.
    assert!(matches!(ns.resolve("zz:a b"), Err(Error::UndeclaredPrefix(p)) if p == "zz"));
}

#[test]
fn underscore_is_never_a_prefix() {
    let mut ns = Namespaces::new();
    assert!(matches!(
        ns.declare("_", "http://example.org/"),
        Err(Error::InvalidPrefix(p)) if p == "_"
    ));
    assert_eq!(ns.iter().count(), 0);
    // With no binding, `_:x` is still rejected rather than reported
    // as an undeclared prefix.
    assert!(matches!(ns.resolve("_:x"), Err(Error::InvalidQName(_))));
    let doc = Namespaces::new();
    let mut bundle = Namespaces::new();
    bundle.set_default("http://default.org/").unwrap();
    let merged = bundle.with_fallback(&doc);
    assert!(matches!(merged.resolve("_:x"), Err(Error::InvalidQName(_))));
}

#[test]
fn prefixes_must_be_pn_prefixes() {
    let mut ns = Namespaces::new();
    for bad in [
        "", "a:b", "a b", " a", "a\t", "1a", "-a", ".a", "a.", "é:", "_", "_a",
    ] {
        assert!(
            matches!(ns.declare(bad, "http://x/"), Err(Error::InvalidPrefix(p)) if p == bad),
            "{bad:?}"
        );
    }
    for good in ["a", "a1", "a.b", "a-b", "a_b", "é", "default", "ns1"] {
        ns.declare(good, "http://x/").unwrap();
    }
    assert_eq!(ns.iter().count(), 8);
}

#[test]
fn reserved_prefixes_cannot_move() {
    let mut ns = Namespaces::new();
    assert!(ns.declare("prov", PROV).is_ok());
    assert!(matches!(
        ns.declare("prov", "http://other/"),
        Err(Error::ReservedPrefix { .. })
    ));
    assert!(matches!(
        ns.declare("xsd", "http://other/"),
        Err(Error::ReservedPrefix { .. })
    ));
    assert_eq!(ns.iter().count(), 0);
}

#[test]
fn ensure_prefix_reuses_then_mints() {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/").unwrap();
    ns.declare("ns1", "http://taken/").unwrap();
    assert_eq!(ns.ensure_prefix("http://example.org/"), "ex");
    assert_eq!(ns.ensure_prefix("http://new/"), "ns2");
    assert_eq!(ns.ensure_prefix("http://new/"), "ns2");
    assert_eq!(ns.prefix_for(PROV), Some("prov"));
}

#[test]
fn render_parts_split_the_prefix_from_the_local_part() {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/").unwrap();
    ns.set_default("http://default.org/").unwrap();
    let q = |iri: &str, local: &str| QName::new(iri, local).unwrap();
    // The prefix alone comes back; `None` means bare.
    assert_eq!(
        WriteScope::of(&mut ns).render_parts(&q("http://example.org/", "a")),
        Some("ex".to_owned())
    );
    assert_eq!(
        WriteScope::of(&mut ns).render_parts(&q("http://default.org/", "b")),
        None
    );
    assert_eq!(
        WriteScope::of(&mut ns).render_parts(&q(PROV, "type")),
        Some("prov".to_owned())
    );
    assert_eq!(
        WriteScope::of(&mut ns).render_parts(&q("http://new/", "c")),
        Some("ns1".to_owned())
    );
    assert_eq!(ns.iter().count(), 2);
    // A name in the default namespace never mints a prefix.
    let mut plain = Namespaces::new();
    plain.set_default("http://default.org/").unwrap();
    assert_eq!(
        WriteScope::of(&mut plain).render_parts(&q("http://default.org/", "b")),
        None
    );
    assert_eq!(plain.iter().count(), 0);
    // A colon in the local part cannot be written bare.
    assert_eq!(
        WriteScope::of(&mut plain).render_parts(&q("http://default.org/", "x:y")),
        Some("ns1".to_owned())
    );
    assert_eq!(
        WriteScope::of(&mut plain).render_parts(&q("http://default.org/", "")),
        Some("ns1".to_owned())
    );
}

#[test]
fn a_bundle_table_falls_back_to_the_document() {
    let mut doc = Namespaces::new();
    doc.declare("ex", "http://example.org/").unwrap();
    let mut bundle = Namespaces::new();
    bundle.declare("b", "http://bundle.org/").unwrap();
    let merged = bundle.with_fallback(&doc);
    assert_eq!(merged.resolve("ex:e").unwrap(), ex("e"));
    assert_eq!(
        merged.resolve("b:e").unwrap(),
        QName::new("http://bundle.org/", "e").unwrap()
    );
}

#[test]
fn resolve_parts_keeps_a_colon_in_the_local_part() {
    let mut doc = Namespaces::new();
    doc.set_default("http://default.org/").unwrap();
    doc.declare("a", "http://a.org/").unwrap();
    let mut bundle = Namespaces::new();
    bundle.declare("a", "http://bundle.org/").unwrap();
    let merged = bundle.with_fallback(&doc);
    assert_eq!(
        Scope::of(&merged).resolve_parts("", "a:b").unwrap(),
        QName::new("http://default.org/", "a:b").unwrap()
    );
    assert_eq!(
        Scope::of(&merged).resolve_parts("a", "b").unwrap(),
        QName::new("http://bundle.org/", "b").unwrap()
    );
    assert!(
        matches!(Scope::of(&merged).resolve_parts("zz", "b"), Err(Error::UndeclaredPrefix(p)) if p == "zz")
    );
    assert!(matches!(
        Scope::of(&merged).resolve_parts("_", "b"),
        Err(Error::InvalidQName(_))
    ));
    let bare = Namespaces::new();
    assert!(matches!(
        Scope::of(&bare).resolve_parts("", "b"),
        Err(Error::NoDefaultNamespace(l)) if l == "b"
    ));
}

#[test]
fn a_merged_table_prefers_the_bundle_binding() {
    let mut doc = Namespaces::new();
    doc.declare("ex", "http://doc.org/").unwrap();
    doc.set_default("http://doc-default.org/").unwrap();
    let mut bundle = Namespaces::new();
    bundle.declare("ex", "http://bundle.org/").unwrap();
    let merged = bundle.with_fallback(&doc);
    assert_eq!(
        merged.resolve("ex:e").unwrap(),
        QName::new("http://bundle.org/", "e").unwrap()
    );
    assert_eq!(
        merged.resolve("e").unwrap(),
        QName::new("http://doc-default.org/", "e").unwrap()
    );
    let mut own_default = Namespaces::new();
    own_default
        .set_default("http://bundle-default.org/")
        .unwrap();
    assert_eq!(
        own_default.with_fallback(&doc).resolve("e").unwrap(),
        QName::new("http://bundle-default.org/", "e").unwrap()
    );
}

#[test]
fn the_first_declared_prefix_answers_for_a_shared_iri() {
    let mut ns = Namespaces::new();
    ns.declare("a", "http://x.org/").unwrap();
    ns.declare("b", "http://x.org/").unwrap();
    assert_eq!(ns.prefix_for("http://x.org/"), Some("a"));
    // Redeclaring keeps a prefix's place, so `a` still leads.
    ns.declare("b", "http://y.org/").unwrap();
    ns.declare("b", "http://x.org/").unwrap();
    assert_eq!(ns.prefix_for("http://x.org/"), Some("a"));
    assert_eq!(ns.prefix_for("http://y.org/"), None);
}

#[test]
fn a_redeclared_prefix_moves_to_its_new_iri() {
    let mut ns = Namespaces::new();
    ns.declare("a", "http://x.org/").unwrap();
    ns.declare("b", "http://x.org/").unwrap();
    ns.declare("a", "http://y.org/").unwrap();
    assert_eq!(ns.prefix_for("http://x.org/"), Some("b"));
    assert_eq!(ns.prefix_for("http://y.org/"), Some("a"));
    // `a` keeps its earlier position, so it leads a later shared IRI.
    ns.declare("b", "http://y.org/").unwrap();
    assert_eq!(ns.prefix_for("http://y.org/"), Some("a"));
    assert_eq!(ns.prefix_for("http://x.org/"), None);
}

#[test]
fn the_default_namespace_has_no_prefix() {
    let mut ns = Namespaces::new();
    ns.set_default("http://x.org/").unwrap();
    assert_eq!(ns.prefix_for("http://x.org/"), None);
}

#[test]
fn a_merged_table_answers_prefix_for() {
    let mut doc = Namespaces::new();
    doc.declare("d", "http://x.org/").unwrap();
    doc.declare("e", "http://z.org/").unwrap();
    let mut bundle = Namespaces::new();
    bundle.declare("b", "http://x.org/").unwrap();
    let merged = bundle.with_fallback(&doc);
    assert_eq!(merged.prefix_for("http://x.org/"), Some("b"));
    assert_eq!(merged.prefix_for("http://z.org/"), Some("e"));
}

#[test]
fn minted_prefixes_answer_prefix_for() {
    let mut ns = Namespaces::new();
    assert_eq!(ns.ensure_prefix("http://x.org/"), "ns1");
    assert_eq!(ns.prefix_for("http://x.org/"), Some("ns1"));
}

#[test]
fn equality_ignores_declaration_order() {
    let mut a = Namespaces::new();
    a.declare("p", "http://x.org/").unwrap();
    a.declare("q", "http://x.org/").unwrap();
    let mut b = Namespaces::new();
    b.declare("q", "http://x.org/").unwrap();
    b.declare("p", "http://x.org/").unwrap();
    assert_eq!(a, b);
}

#[test]
fn equality_ignores_the_reverse_index_after_a_redeclaration() {
    // `a` ends with `p` and `q` both on x.org, but the reverse index
    // lists `p` first in `a` and `q` first in `b`.
    let mut a = Namespaces::new();
    a.declare("p", "http://x.org/").unwrap();
    a.declare("q", "http://x.org/").unwrap();
    a.declare("p", "http://y.org/").unwrap();
    a.declare("p", "http://x.org/").unwrap();
    let mut b = Namespaces::new();
    b.declare("q", "http://x.org/").unwrap();
    b.declare("p", "http://x.org/").unwrap();
    assert_eq!(a, b);
    assert_eq!(a.prefix_for("http://x.org/"), Some("p"));
    assert_eq!(b.prefix_for("http://x.org/"), Some("q"));
    // A redeclared prefix is a different binding.
    b.declare("p", "http://y.org/").unwrap();
    assert_ne!(a, b);
}

#[test]
fn an_empty_local_part_resolves() {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/").unwrap();
    let name = ns.resolve("ex:").unwrap();
    assert_eq!(name.namespace(), "http://example.org/");
    assert_eq!(name.local(), "");
}

#[test]
fn absorb_keeps_the_first_default_and_the_first_binding() {
    let mut first = Namespaces::new();
    first.set_default("http://first.org/").unwrap();
    first.declare("p", "http://first.org/p/").unwrap();
    let mut second = Namespaces::new();
    second.set_default("http://second.org/").unwrap();
    second.declare("p", "http://second.org/p/").unwrap();
    second.declare("q", "http://second.org/q/").unwrap();
    first.absorb(&second);
    assert_eq!(first.default_namespace(), Some("http://first.org/"));
    assert_eq!(first.lookup("p"), Some("http://first.org/p/"));
    assert_eq!(first.lookup("q"), Some("http://second.org/q/"));
    assert_eq!(first.prefix_for("http://second.org/p/"), None);

    let mut empty = Namespaces::new();
    empty.absorb(&second);
    assert_eq!(empty.default_namespace(), Some("http://second.org/"));
}

/// A document table and a bundle table that shadow, share and extend
/// each other, with a default on each.
fn layered() -> (Namespaces, Namespaces) {
    let mut document = Namespaces::new();
    document.set_default("http://doc.org/").unwrap();
    document.declare("ex", "http://document.org/").unwrap();
    document.declare("o", "http://other.org/").unwrap();
    document.declare("alias", "http://other.org/").unwrap();
    document.declare("ns1", "http://taken.org/").unwrap();
    let mut bundle = Namespaces::new();
    bundle.declare("ex", "http://bundle.org/").unwrap();
    bundle.declare("o", "http://shadow.org/").unwrap();
    bundle.declare("b", "http://only-bundle.org/").unwrap();
    (document, bundle)
}

#[test]
fn a_scope_resolves_as_the_merged_table_does() {
    let (document, bundle) = layered();
    let merged = bundle.with_fallback(&document);
    let scope = Scope::within(&bundle, &document);
    for text in [
        "ex:a",
        "o:a",
        "alias:a",
        "b:a",
        "ns1:a",
        "prov:type",
        "xsd:int",
        "bare",
        "zz:a",
        "_:x",
        ":a",
        "",
        "ex:a b",
    ] {
        match (scope.resolve(text), merged.resolve(text)) {
            (Ok(a), Ok(b)) => assert_eq!(a, b, "{text}"),
            (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string(), "{text}"),
            (a, b) => panic!("{text}: {a:?} against {b:?}"),
        }
    }
    // With no default of its own, a bare name takes the document's.
    assert_eq!(
        scope.resolve("bare").unwrap().namespace(),
        "http://doc.org/"
    );
    let mut with_default = bundle.clone();
    with_default
        .set_default("http://bundle-default.org/")
        .unwrap();
    let scope = Scope::within(&with_default, &document);
    assert_eq!(
        scope.resolve("bare").unwrap().namespace(),
        "http://bundle-default.org/"
    );
}

/// Render each name through `bundle` in front of `document` and through the
/// two tables merged, assert the two agree, and return the bundle's table
/// with the prefixes minted on the way.
fn render_as_merged(
    document: &Namespaces,
    bundle: &Namespaces,
    names: &[(&str, &str)],
) -> Namespaces {
    let mut merged = bundle.with_fallback(document);
    let mut own = bundle.clone();
    let mut scope = WriteScope::within(&mut own, document);
    for &(namespace, local) in names {
        let name = QName::new(namespace, local).unwrap();
        assert_eq!(
            scope.render_parts(&name),
            WriteScope::of(&mut merged).render_parts(&name),
            "{namespace}{local}"
        );
    }
    own
}

#[test]
fn a_write_scope_renders_as_the_merged_table_does() {
    let (document, bundle) = layered();
    let names = [
        ("http://bundle.org/", "a"),
        ("http://document.org/", "a"),
        ("http://shadow.org/", "a"),
        // `o` is shadowed in the bundle, so the document's second prefix serves.
        ("http://other.org/", "a"),
        ("http://only-bundle.org/", "a"),
        ("http://taken.org/", "a"),
        ("http://doc.org/", "bare"),
        ("http://doc.org/", "with:colon"),
        ("http://doc.org/", ""),
        ("http://new.org/", "a"),
        ("http://newer.org/", "a"),
        ("http://new.org/", "b"),
        (PROV, "type"),
    ];
    let own = render_as_merged(&document, &bundle, &names);
    // The minted prefixes went to the bundle's table and skipped `ns1`,
    // which the document binds. The bundle shadows the document's `ex`,
    // and a default namespace cannot write a colon or an empty local
    // part, so those two namespaces take a prefix as well.
    assert_eq!(own.lookup("ns2"), Some("http://document.org/"));
    assert_eq!(own.lookup("ns3"), Some("http://doc.org/"));
    assert_eq!(own.lookup("ns4"), Some("http://new.org/"));
    assert_eq!(own.lookup("ns5"), Some("http://newer.org/"));
    assert_eq!(own.lookup("ns1"), None);
}

#[test]
fn a_write_scope_with_its_own_default_renders_as_the_merged_table_does() {
    let (document, mut bundle) = layered();
    bundle.set_default("http://bundle-default.org/").unwrap();
    let names = [
        ("http://bundle-default.org/", "bare"),
        ("http://bundle-default.org/", "with:colon"),
        ("http://bundle-default.org/", ""),
        // The document's default does not reach a bundle with its own.
        ("http://doc.org/", "bare"),
        ("http://document.org/", "a"),
        ("http://bundle.org/", "a"),
        ("http://new.org/", "a"),
    ];
    render_as_merged(&document, &bundle, &names);
}

#[test]
fn a_bundle_default_writes_bare_and_the_document_default_takes_a_prefix() {
    let (document, mut bundle) = layered();
    bundle.set_default("http://bundle-default.org/").unwrap();
    let mut scope = WriteScope::within(&mut bundle, &document);
    let own = QName::new("http://bundle-default.org/", "bare").unwrap();
    assert_eq!(scope.render_parts(&own), None);
    let inherited = QName::new("http://doc.org/", "bare").unwrap();
    assert_eq!(scope.render_parts(&inherited), Some("ns2".to_owned()));
}

#[test]
fn minting_continues_past_the_last_minted_number() {
    let mut ns = Namespaces::new();
    assert_eq!(ns.ensure_prefix("http://a/"), "ns1");
    ns.declare("ns2", "http://taken/").unwrap();
    assert_eq!(ns.ensure_prefix("http://b/"), "ns3");
    assert_eq!(ns.ensure_prefix("http://c/"), "ns4");
    // A minted prefix is a binding like any other to equality.
    let mut other = Namespaces::new();
    other.declare("ns1", "http://a/").unwrap();
    other.declare("ns2", "http://taken/").unwrap();
    other.declare("ns3", "http://b/").unwrap();
    other.declare("ns4", "http://c/").unwrap();
    assert_eq!(ns, other);
}

#[test]
fn an_empty_iri_is_refused_and_a_space_is_not() {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/").unwrap();
    ns.set_default("http://default.org/").unwrap();
    let before = ns.clone();
    assert!(matches!(ns.declare("z", ""), Err(Error::EmptyNamespace)));
    assert!(matches!(ns.declare("ex", ""), Err(Error::EmptyNamespace)));
    assert!(matches!(ns.set_default(""), Err(Error::EmptyNamespace)));
    assert_eq!(ns, before);
    assert_eq!(ns.default_namespace(), Some("http://default.org/"));
    // The IRI is never trimmed.
    ns.declare("z", " ").unwrap();
    ns.set_default(" ").unwrap();
    assert_eq!(ns.lookup("z"), Some(" "));
    assert_eq!(ns.default_namespace(), Some(" "));
}
