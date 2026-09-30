//! Tests for the record tables and [`Record::from_parts`].

use super::*;
use crate::test_util::{hash, q};
use crate::{Attributes, Value};

fn t() -> DateTime {
    DateTime::parse("2011-11-16T16:05:33Z").unwrap()
}

/// A value for `attr` whose text names the formal, so a swapped field shows.
fn formal_for(attr: FormalAttr) -> Formal {
    if attr.is_time() {
        let minute = u8::try_from(ALL_FORMALS.iter().position(|f| *f == attr).unwrap()).unwrap();
        Formal::Time(DateTime::new(2011, 11, 16, 16, minute, 0, 0, Some(0)).unwrap())
    } else {
        Formal::Name(q(attr.local_name()))
    }
}

fn owned(r: FormalRef<'_>) -> Formal {
    match r {
        FormalRef::Name(q) => Formal::Name(q.clone()),
        FormalRef::Time(t) => Formal::Time(t.clone()),
    }
}

#[test]
fn tables() {
    assert_eq!(Kind::ALL.len(), 18);
    assert_eq!(
        Kind::from_json_key("wasGeneratedBy"),
        Some(Kind::Generation)
    );
    assert_eq!(Kind::from_json_key("wasGenerated"), None);
    assert_eq!(Kind::Entity.json_key(), "entity");
    assert_eq!(Kind::Generation.json_key(), "wasGeneratedBy");
    assert_eq!(Kind::Generation.provn_keyword(), "wasGeneratedBy");
    assert_eq!(Kind::Mention.provn_keyword(), "mentionOf");
    assert_eq!(
        Kind::Generation.formals(),
        &[FormalAttr::Entity, FormalAttr::Activity, FormalAttr::Time]
    );
    assert_eq!(Kind::Derivation.formals().len(), 5);
    assert!(Kind::Entity.is_element() && !Kind::Usage.is_element());
    assert_eq!(
        FormalAttr::from_local_name("generatedEntity"),
        Some(FormalAttr::GeneratedEntity)
    );
    assert!(FormalAttr::Time.is_time() && !FormalAttr::Entity.is_time());
}

#[test]
fn keys_and_local_names_round_trip() {
    for kind in Kind::ALL {
        assert_eq!(Kind::from_json_key(kind.json_key()), Some(kind));
    }
    for attr in ALL_FORMALS {
        assert_eq!(FormalAttr::from_local_name(attr.local_name()), Some(attr));
    }
    let times: Vec<_> = ALL_FORMALS.into_iter().filter(|f| f.is_time()).collect();
    assert_eq!(times, [F::Time, F::StartTime, F::EndTime]);
}

#[test]
fn formal_tables_match_the_spec() {
    let table: Vec<(Kind, &[&str])> = vec![
        (Kind::Entity, &[]),
        (Kind::Activity, &["startTime", "endTime"]),
        (Kind::Agent, &[]),
        (Kind::Generation, &["entity", "activity", "time"]),
        (Kind::Usage, &["activity", "entity", "time"]),
        (Kind::Communication, &["informed", "informant"]),
        (Kind::Start, &["activity", "trigger", "starter", "time"]),
        (Kind::End, &["activity", "trigger", "ender", "time"]),
        (Kind::Invalidation, &["entity", "activity", "time"]),
        (
            Kind::Derivation,
            &[
                "generatedEntity",
                "usedEntity",
                "activity",
                "generation",
                "usage",
            ],
        ),
        (Kind::Attribution, &["entity", "agent"]),
        (Kind::Association, &["activity", "agent", "plan"]),
        (Kind::Delegation, &["delegate", "responsible", "activity"]),
        (Kind::Influence, &["influencee", "influencer"]),
        (Kind::Specialization, &["specificEntity", "generalEntity"]),
        (Kind::Alternate, &["alternate1", "alternate2"]),
        (
            Kind::Mention,
            &["specificEntity", "generalEntity", "bundle"],
        ),
        (Kind::Membership, &["collection", "entity"]),
    ];
    assert_eq!(table.iter().map(|(k, _)| *k).collect::<Vec<_>>(), Kind::ALL);
    for (kind, names) in table {
        let got: Vec<_> = kind.formals().iter().map(|f| f.local_name()).collect();
        assert_eq!(got, names, "{kind:?}");
    }
}

#[test]
fn from_parts_builds_a_generation() {
    let r = Record::from_parts(
        Kind::Generation,
        Some(q("g1")),
        vec![
            (FormalAttr::Entity, Formal::Name(q("e1"))),
            (FormalAttr::Time, Formal::Time(t())),
        ],
        Attributes::new(),
    )
    .unwrap();
    assert_eq!(r.kind(), Kind::Generation);
    assert_eq!(r.id(), Some(&q("g1")));
    let Record::Generation(g) = &r else { panic!() };
    assert_eq!(g.entity, Some(q("e1")));
    assert_eq!(g.activity, None);
    assert_eq!(g.time, Some(t()));
    assert_eq!(
        r.formals(),
        vec![
            (FormalAttr::Entity, FormalRef::Name(&q("e1"))),
            (FormalAttr::Time, FormalRef::Time(&t())),
        ]
    );
}

#[test]
fn from_parts_builds_every_kind_empty() {
    for kind in Kind::ALL {
        let id = kind.is_element().then(|| q("x"));
        let r = Record::from_parts(kind, id.clone(), vec![], Attributes::new()).unwrap();
        assert_eq!(r.kind(), kind);
        assert_eq!(r.id(), id.as_ref());
        assert!(r.formals().is_empty());
        assert!(r.attributes().is_empty());
    }
}

#[test]
fn formals_round_trip_through_from_parts_for_every_kind() {
    for kind in Kind::ALL {
        // Given in reverse, so table order in the output is not an echo of the input.
        let parts: Vec<_> = kind
            .formals()
            .iter()
            .rev()
            .map(|&a| (a, formal_for(a)))
            .collect();
        let mut attributes = Attributes::new();
        attributes.insert(q("label"), Value::from("x"));
        let r = Record::from_parts(kind, Some(q("r")), parts, attributes.clone()).unwrap();
        assert_eq!(r.kind(), kind);
        assert_eq!(r.id(), Some(&q("r")));
        assert_eq!(r.attributes(), &attributes);

        let formals = r.formals();
        let attrs: Vec<_> = formals.iter().map(|(a, _)| *a).collect();
        assert_eq!(attrs, kind.formals(), "{kind:?} order");
        for (a, v) in &formals {
            assert_eq!(owned(*v), formal_for(*a), "{kind:?} {a:?}");
        }

        let rebuilt = Record::from_parts(
            kind,
            r.id().cloned(),
            formals.into_iter().map(|(a, v)| (a, owned(v))).collect(),
            r.attributes().clone(),
        )
        .unwrap();
        assert_eq!(rebuilt, r, "{kind:?}");
        assert_eq!(hash(&rebuilt), hash(&r), "{kind:?}");
    }
}

#[test]
fn from_parts_rejects_bad_parts() {
    let bad = |kind: Kind, id, formals| {
        matches!(
            Record::from_parts(kind, id, formals, Attributes::new()),
            Err(Error::InvalidRecord { kind: k, .. }) if k == kind
        )
    };
    assert!(bad(Kind::Entity, None, vec![]));
    assert!(bad(Kind::Activity, None, vec![]));
    assert!(bad(Kind::Agent, None, vec![]));
    assert!(bad(
        Kind::Generation,
        None,
        vec![(FormalAttr::Plan, Formal::Name(q("p")))]
    ));
    assert!(bad(
        Kind::Entity,
        Some(q("e")),
        vec![(FormalAttr::Entity, Formal::Name(q("e")))]
    ));
    assert!(bad(
        Kind::Generation,
        None,
        vec![(FormalAttr::Entity, Formal::Time(t()))]
    ));
    assert!(bad(
        Kind::Generation,
        None,
        vec![(FormalAttr::Time, Formal::Name(q("t")))]
    ));
    assert!(bad(
        Kind::Generation,
        None,
        vec![
            (FormalAttr::Entity, Formal::Name(q("a"))),
            (FormalAttr::Entity, Formal::Name(q("b"))),
        ]
    ));
}

#[test]
fn attribute_named_after_an_own_formal_is_rejected() {
    let prov = |l: &str| QName::new(PROV, l);
    let mut clash = Attributes::new();
    clash.insert(prov("activity"), Value::QName(q("a2")));
    let e = Record::from_parts(Kind::Generation, None, vec![], clash).unwrap_err();
    assert!(
        matches!(
            e,
            Error::InvalidRecord {
                kind: Kind::Generation,
                ..
            }
        ),
        "{e:?}"
    );
    // The message names the kind, the attribute and where its value goes.
    assert_eq!(
        e.to_string(),
        "wasGeneratedBy: prov:activity is given as an attribute; \
         its value belongs in the positional argument"
    );
    // A formal of another kind, and prov:type and prov:label, are plain attributes.
    let mut fine = Attributes::new();
    fine.insert(prov("plan"), Value::QName(q("p")));
    fine.insert(prov("type"), Value::from("t"));
    fine.insert(prov("label"), Value::from("l"));
    for kind in [Kind::Generation, Kind::Usage, Kind::Entity] {
        let r = Record::from_parts(kind, Some(q("x")), vec![], fine.clone()).unwrap();
        assert_eq!(r.attributes().len(), 3);
    }
}

#[test]
fn records_compare_and_hash_by_value() {
    let build = |id: &str| {
        Record::from_parts(
            Kind::Usage,
            Some(q(id)),
            vec![(FormalAttr::Activity, Formal::Name(q("a")))],
            Attributes::new(),
        )
        .unwrap()
    };
    let r = build("u");
    assert_eq!(r.clone(), r);
    assert_eq!(hash(&r.clone()), hash(&r));
    assert_ne!(build("u"), build("v"));
    let unnamed = Record::from_parts(
        Kind::Usage,
        None,
        r.formals()
            .into_iter()
            .map(|(a, v)| (a, owned(v)))
            .collect(),
        Attributes::new(),
    )
    .unwrap();
    assert_ne!(unnamed, r);
}

#[test]
fn attributes_mut_edits_every_variant() {
    for kind in Kind::ALL {
        let mut r = Record::from_parts(kind, Some(q("x")), vec![], Attributes::new()).unwrap();
        assert!(r.attributes_mut().insert(q("label"), Value::from("x")));
        assert_eq!(r.attributes().iter().count(), 1, "{kind:?}");
    }
}
