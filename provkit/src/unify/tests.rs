use crate::test_util::{agent, bundle, document, entity, ex, prov, relation};
use crate::well_known::PROV_LABEL;
use crate::{
    Association, Attributes, Bundle, DateTime, Document, Error, Formal, FormalAttr as F,
    Generation, Kind, Namespaces, QName, Record,
};

fn rec(kind: Kind, id: &str, formals: Vec<(F, Formal)>, label: Option<&str>) -> Record {
    let mut a = Attributes::new();
    if let Some(l) = label {
        a.insert(PROV_LABEL.clone(), l.into());
    }
    Record::from_parts(kind, Some(ex(id)), formals, a).unwrap()
}

fn t(s: &str) -> Formal {
    Formal::Time(DateTime::parse(s).unwrap())
}

/// A generation at `time`, with no other formal.
fn gen_at(id: Option<&str>, time: &str) -> Record {
    relation(Kind::Generation, id.map(ex), vec![(F::Time, t(time))])
}

/// The generation `ex:g` of `ex:e`, with `prov:activity` as an attribute
/// when `clash` is set.
fn hand_built(clash: bool) -> Record {
    let mut attributes = Attributes::new();
    if clash {
        attributes.insert(prov("activity"), ex("a").into());
    }
    Record::Generation(Generation {
        id: Some(ex("g")),
        entity: Some(ex("e")),
        activity: None,
        time: None,
        attributes,
    })
}

fn name(l: &str) -> Formal {
    Formal::Name(ex(l))
}

#[test]
fn merges_formals_and_attributes() {
    let d = document(vec![
        rec(
            Kind::Generation,
            "g",
            vec![(F::Entity, name("e"))],
            Some("one"),
        ),
        entity("e"),
        rec(
            Kind::Generation,
            "g",
            vec![(F::Activity, name("a"))],
            Some("two"),
        ),
    ]);
    let u = d.unified().unwrap();
    assert_eq!(u.records.len(), 2);
    let Record::Generation(g) = &u.records[0] else {
        panic!(
            "merged record should keep the first position: {:?}",
            u.records
        )
    };
    assert_eq!(
        (g.entity.clone(), g.activity.clone()),
        (Some(ex("e")), Some(ex("a")))
    );
    let labels: Vec<_> = g.attributes.get(&PROV_LABEL).collect();
    assert_eq!(
        labels,
        ["one", "two"].map(Into::into).iter().collect::<Vec<_>>()
    );
    assert_eq!(d.records.len(), 3, "original untouched");
}

#[test]
fn absent_takes_the_concrete_value_in_either_order() {
    for (first, second) in [(None, Some("a")), (Some("a"), None)] {
        let d = document(
            [first, second]
                .into_iter()
                .map(|act| {
                    let formals = act.map(|a| (F::Activity, name(a))).into_iter().collect();
                    rec(Kind::Generation, "g", formals, None)
                })
                .collect(),
        );
        let u = d.unified().unwrap();
        assert_eq!(u.records.len(), 1);
        let Record::Generation(g) = &u.records[0] else {
            panic!()
        };
        assert_eq!(g.activity, Some(ex("a")));
    }
}

#[test]
fn equal_values_unify() {
    let d = document(vec![gen_at(Some("g"), "2014-06-23T12:28:53Z"); 2]);
    assert_eq!(d.unified().unwrap().records.len(), 1);
}

#[test]
fn conflicting_times_fail() {
    let d = document(vec![
        gen_at(Some("g"), "2014-06-23T12:28:53Z"),
        gen_at(Some("g"), "2012-12-03T21:08:16Z"),
    ]);
    match d.unified() {
        Err(Error::Unification { id, reason }) => {
            assert_eq!(id, ex("g"));
            assert!(
                reason.contains("prov:time")
                    && reason.contains("2014-06-23T12:28:53Z")
                    && reason.contains("2012-12-03T21:08:16Z"),
                "{reason}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn conflicting_names_fail() {
    let d = document(
        ["a1", "a2"]
            .map(|a| rec(Kind::Usage, "u", vec![(F::Activity, name(a))], None))
            .to_vec(),
    );
    let Err(Error::Unification { reason, .. }) = d.unified() else {
        panic!("expected a unification error")
    };
    assert!(
        reason.contains("prov:activity") && reason.contains("a1") && reason.contains("a2"),
        "{reason}"
    );
}

#[test]
fn a_conflict_between_long_names_gives_a_short_message() {
    let long = |c: char| Formal::Name(ex(&c.to_string().repeat(1 << 20)));
    let d = document(
        ['a', 'b']
            .map(|c| rec(Kind::Usage, "u", vec![(F::Activity, long(c))], None))
            .to_vec(),
    );
    let Err(Error::Unification { reason, .. }) = d.unified() else {
        panic!("expected a unification error")
    };
    assert!(reason.len() < 600, "{} bytes", reason.len());
}

#[test]
fn a_lone_record_with_an_own_formal_attribute_is_invalid() {
    let d = document(vec![hand_built(true)]);
    assert!(matches!(
        d.unified(),
        Err(Error::InvalidRecord {
            kind: Kind::Generation,
            ..
        })
    ));
}

#[test]
fn unified_keeps_the_namespace_tables() {
    let mut d = Document::new();
    d.namespaces.declare("ex", "http://example.org/").unwrap();
    let mut inner = bundle("b", vec![entity("e")]);
    inner.namespaces.declare("in", "http://inner.org/").unwrap();
    d.bundles.push(inner);
    let u = d.unified().unwrap();
    assert_eq!(u.namespaces, d.namespaces);
    assert_eq!(u.bundles[0].namespaces, d.bundles[0].namespaces);
}

#[test]
fn an_absent_formal_unifies_with_a_present_one_in_every_position() {
    // PROV-CONSTRAINTS makes `plan` non-expandable, so `-` there means
    // "no plan" and would not unify with a plan. The model holds every
    // formal as an `Option`, so it cannot tell the two apart, and merges.
    let association = |plan: Option<QName>| {
        Record::Association(Association {
            id: Some(ex("assoc")),
            activity: Some(ex("a")),
            agent: Some(ex("ag")),
            plan,
            attributes: Attributes::new(),
        })
    };
    let d = document(vec![association(None), association(Some(ex("pl")))]);
    let u = d.unified().unwrap();
    assert_eq!(u.records, vec![association(Some(ex("pl")))]);

    // Every formal of every kind that merges, in either order.
    for &kind in Kind::ALL.iter().filter(|k| !k.is_bare()) {
        for &attr in kind.formals() {
            let present = if attr.is_time() {
                t("2011-11-16T16:05:33Z")
            } else {
                name("v")
            };
            let with = rec(kind, "x", vec![(attr, present)], None);
            let without = rec(kind, "x", vec![], None);
            for pair in [[&with, &without], [&without, &with]] {
                let d = document(pair.iter().map(|&r| r.clone()).collect());
                assert_eq!(
                    d.unified().unwrap().records,
                    vec![with.clone()],
                    "{kind:?} {attr}"
                );
            }
        }
    }
}

fn pair(a: Kind, b: Kind) -> crate::Result<Document> {
    document(vec![rec(a, "x", vec![], None), rec(b, "x", vec![], None)]).unified()
}

/// Constraint 53 spelt out independently of `DISJOINT_RELATIONS`, so
/// a change to the production list fails here.
const CONSTRAINT_53: [Kind; 9] = [
    Kind::Usage,
    Kind::Generation,
    Kind::Invalidation,
    Kind::Start,
    Kind::End,
    Kind::Communication,
    Kind::Attribution,
    Kind::Association,
    Kind::Delegation,
];

#[test]
fn influence_and_derivation_share_with_any_relation() {
    for (a, b) in [
        (Kind::Influence, Kind::Derivation),
        (Kind::Generation, Kind::Influence),
        (Kind::Derivation, Kind::Usage),
    ] {
        assert_eq!(pair(a, b).unwrap().records.len(), 2, "{a:?} and {b:?}");
        assert_eq!(pair(b, a).unwrap().records.len(), 2, "{b:?} and {a:?}");
    }
    for k in CONSTRAINT_53 {
        for other in [Kind::Influence, Kind::Derivation] {
            assert!(pair(k, other).is_ok(), "{k:?} and {other:?}");
        }
    }
}

#[test]
fn constraint_53_relations_are_pairwise_disjoint() {
    for (i, &a) in CONSTRAINT_53.iter().enumerate() {
        for &b in &CONSTRAINT_53[i + 1..] {
            for (x, y) in [(a, b), (b, a)] {
                assert!(
                    matches!(pair(x, y), Err(Error::Unification { .. })),
                    "{x:?} and {y:?}"
                );
            }
        }
    }
}

#[test]
fn constraint_54_keeps_elements_apart_from_eleven_relations() {
    let relations = CONSTRAINT_53
        .into_iter()
        .chain([Kind::Influence, Kind::Derivation]);
    for relation in relations {
        for element in [Kind::Entity, Kind::Activity, Kind::Agent] {
            for (x, y) in [(element, relation), (relation, element)] {
                assert!(
                    matches!(pair(x, y), Err(Error::Unification { .. })),
                    "{x:?} and {y:?}"
                );
            }
        }
    }
}

#[test]
fn constraint_55_entity_and_activity_are_disjoint_and_agents_overlap() {
    for (a, b) in [
        (Kind::Entity, Kind::Activity),
        (Kind::Activity, Kind::Entity),
    ] {
        assert!(matches!(pair(a, b), Err(Error::Unification { .. })));
    }
    for (a, b) in [
        (Kind::Entity, Kind::Agent),
        (Kind::Agent, Kind::Entity),
        (Kind::Activity, Kind::Agent),
        (Kind::Agent, Kind::Activity),
    ] {
        assert_eq!(pair(a, b).unwrap().records.len(), 2, "{a:?} and {b:?}");
    }
}

#[test]
fn an_entity_agent_and_activity_under_one_identifier_fail() {
    let d = document(
        [Kind::Entity, Kind::Agent, Kind::Activity]
            .map(|kind| rec(kind, "x", vec![], None))
            .to_vec(),
    );
    assert!(matches!(d.unified(), Err(Error::Unification { .. })));
}

#[test]
fn identifier_less_kinds_are_never_merged_or_checked() {
    let with = |kind: Kind, formals: Vec<(F, Formal)>| rec(kind, "x", formals, None);
    let mut records = Vec::new();
    for (a1, a2) in [("a", "b"), ("c", "d")] {
        records.extend([
            with(
                Kind::Alternate,
                vec![(F::Alternate1, name(a1)), (F::Alternate2, name(a2))],
            ),
            with(Kind::Specialization, vec![(F::SpecificEntity, name(a1))]),
            with(Kind::Mention, vec![(F::SpecificEntity, name(a1))]),
            with(Kind::Membership, vec![(F::Collection, name(a1))]),
        ]);
    }
    // None of the eight clashes with the entity under the same identifier,
    // and none merges with its twin.
    records.push(entity("x"));
    let d = document(records);
    let u = d.unified().unwrap();
    assert_eq!(u.records, d.records);
}

#[test]
fn overlapping_kinds_merge_per_kind() {
    let d = document(vec![
        rec(Kind::Entity, "x", vec![], Some("e1")),
        agent("x"),
        rec(Kind::Entity, "x", vec![], Some("e2")),
    ]);
    let u = d.unified().unwrap();
    assert_eq!(u.records.len(), 2);
    assert_eq!(u.records[0].kind(), Kind::Entity);
    assert_eq!(u.records[0].attributes().len(), 2);
    assert_eq!(u.records[1].kind(), Kind::Agent);
}

#[test]
fn output_order_follows_first_members() {
    let d = document(vec![
        entity("b"),
        entity("a"),
        rec(Kind::Entity, "b", vec![], Some("l")),
        entity("c"),
    ]);
    let u = d.unified().unwrap();
    let ids: Vec<_> = u.records.iter().map(|r| r.id().unwrap().clone()).collect();
    assert_eq!(ids, [ex("b"), ex("a"), ex("c")]);
}

#[test]
fn records_without_identifiers_never_merge() {
    let of_e = relation(Kind::Generation, None, vec![(F::Entity, name("e"))]);
    let d = document(vec![
        of_e.clone(),
        of_e,
        gen_at(None, "2014-06-23T12:28:53Z"),
        gen_at(None, "2012-12-03T21:08:16Z"),
    ]);
    let u = d.unified().unwrap();
    assert_eq!(u.records.len(), 4);
    assert_eq!(u.records, d.records);
}

#[test]
fn identified_memberships_survive_unchanged() {
    // `hadMember` with an identifier and two members, as PROV-JSON fans it out.
    let member = |e: &str| {
        rec(
            Kind::Membership,
            "m",
            vec![(F::Collection, name("c")), (F::Entity, name(e))],
            None,
        )
    };
    let mut d = document(vec![member("a"), member("b")]);
    let u = d.unified().unwrap();
    assert_eq!(u.records, d.records);
    // Sharing an identifier with another kind is not a conflict either.
    d.records.push(entity("m"));
    assert_eq!(d.unified().unwrap().records, d.records);
}

#[test]
fn bundles_unify_independently() {
    let mut d = document(vec![rec(Kind::Entity, "e", vec![], Some("top"))]);
    d.bundles.push(bundle(
        "b",
        vec![
            rec(Kind::Entity, "e", vec![], Some("one")),
            rec(Kind::Entity, "e", vec![], Some("two")),
        ],
    ));
    let u = d.unified().unwrap();
    assert_eq!(u.records.len(), 1);
    assert_eq!(u.bundles[0].records.len(), 1);
    assert_eq!(u.bundles[0].records[0].attributes().len(), 2);
    assert_eq!(d.bundles[0].records.len(), 2, "original untouched");

    // A conflict inside a bundle is not visible from outside it, and the
    // same identifier in different scopes never conflicts.
    let mut d = document(vec![entity("x")]);
    d.bundles
        .push(bundle("b", vec![rec(Kind::Activity, "x", vec![], None)]));
    assert!(d.unified().is_ok());
}

#[test]
fn same_identifier_bundles_with_clashing_prefixes_unify() {
    let bundle = |iri: &str, label: &str| {
        let mut ns = Namespaces::new();
        ns.declare("o", iri).unwrap();
        Bundle {
            id: ex("b"),
            namespaces: ns,
            records: vec![rec(Kind::Entity, "e", vec![], Some(label))],
        }
    };
    let mut d = Document::new();
    d.bundles.push(bundle("http://one.org/", "one"));
    d.bundles.push(bundle("http://two.org/", "two"));
    let u = d.unified().unwrap();
    assert_eq!(u.bundles.len(), 1);
    assert_eq!(u.bundles[0].records.len(), 1);
    assert_eq!(u.bundles[0].namespaces.lookup("o"), Some("http://one.org/"));
}

#[test]
fn bundles_sharing_an_identifier_merge_first() {
    let mut d = Document::new();
    d.bundles.push(bundle(
        "b",
        vec![rec(Kind::Entity, "e", vec![], Some("one"))],
    ));
    d.bundles.push(bundle(
        "b",
        vec![rec(Kind::Entity, "e", vec![], Some("two"))],
    ));
    let u = d.unified().unwrap();
    assert_eq!(u.bundles.len(), 1);
    assert_eq!(u.bundles[0].records.len(), 1);
    assert_eq!(u.bundles[0].records[0].attributes().len(), 2);
}

#[test]
fn two_spellings_of_one_iri_merge() {
    let split = |ns: &str, local: &str| QName::new(ns, local).unwrap();
    let mut d = document(
        [
            ("http://example.org/", "foo/bar", "one"),
            ("http://example.org/foo/", "bar", "two"),
        ]
        .map(|(ns, local, label)| {
            let mut a = Attributes::new();
            a.insert(PROV_LABEL.clone(), label.into());
            Record::from_parts(
                Kind::Generation,
                Some(split(ns, local)),
                vec![(F::Entity, Formal::Name(split(ns, "e")))],
                a,
            )
            .unwrap()
        })
        .to_vec(),
    );
    // `e` differs between the two, so the merge must see a conflict
    // rather than two unrelated records.
    assert!(matches!(d.unified(), Err(Error::Unification { .. })));
    let Record::Generation(g) = &mut d.records[1] else {
        panic!()
    };
    g.entity = Some(ex("e"));
    let u = d.unified().unwrap();
    assert_eq!(u.records.len(), 1);
    assert_eq!(u.records[0].attributes().len(), 2);
}

#[test]
fn merging_a_hand_built_formal_named_attribute_is_an_invalid_record() {
    let d = document(vec![hand_built(true), hand_built(false)]);
    assert!(matches!(
        d.unified(),
        Err(Error::InvalidRecord {
            kind: Kind::Generation,
            ..
        })
    ));
}
