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
        let position = FormalAttr::ALL.iter().position(|f| *f == attr).unwrap();
        let minute = u8::try_from(position).unwrap();
        Formal::Time(DateTime::new(2011, 11, 16, 16, minute, 0, 0, Some(0)).unwrap())
    } else {
        Formal::Name(q(attr.local_name()))
    }
}

#[test]
fn tables() {
    assert_eq!(Kind::ALL.len(), 18);
    assert_eq!(Kind::from_name("wasGeneratedBy"), Some(Kind::Generation));
    assert_eq!(Kind::from_name("wasGenerated"), None);
    assert_eq!(Kind::from_name("Generation"), None);
    assert_eq!(Kind::Entity.name(), "entity");
    assert_eq!(Kind::Generation.name(), "wasGeneratedBy");
    assert_eq!(Kind::Mention.name(), "mentionOf");
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
fn names_and_local_names_round_trip() {
    for &kind in Kind::ALL {
        assert_eq!(Kind::from_name(kind.name()), Some(kind));
    }
    assert_eq!(FormalAttr::ALL.len(), 26);
    for &attr in FormalAttr::ALL {
        assert_eq!(FormalAttr::from_local_name(attr.local_name()), Some(attr));
    }
    let times: Vec<_> = FormalAttr::ALL.iter().filter(|f| f.is_time()).collect();
    assert_eq!(times, [&F::Time, &F::StartTime, &F::EndTime]);
}

/// Three rows of the table against PROV-DM and the PROV-N grammar, spelt out
/// by hand: the keyword, the shape, the formals in order and, for PROV-N, the
/// argument counts.
#[test]
fn table_rows_match_prov_dm() {
    let row = |kind: Kind, name: &str, is_element: bool, formals: &[F], arities: &[usize]| {
        assert_eq!(kind.name(), name);
        assert_eq!(kind.is_element(), is_element, "{name}");
        assert_eq!(kind.formals(), formals, "{name}");
        #[cfg(feature = "provn")]
        assert_eq!(kind.arities(), arities, "{name}");
        #[cfg(not(feature = "provn"))]
        let _ = arities;
    };
    row(
        Kind::Activity,
        "activity",
        true,
        &[F::StartTime, F::EndTime],
        &[0, 2],
    );
    row(
        Kind::Derivation,
        "wasDerivedFrom",
        false,
        &[
            F::GeneratedEntity,
            F::UsedEntity,
            F::Activity,
            F::Generation,
            F::Usage,
        ],
        &[2, 5],
    );
    row(
        Kind::Delegation,
        "actedOnBehalfOf",
        false,
        &[F::Delegate, F::Responsible, F::Activity],
        &[2, 3],
    );
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
    for &kind in Kind::ALL {
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
    for &kind in Kind::ALL {
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
            assert_eq!(v.to_owned(), formal_for(*a), "{kind:?} {a:?}");
            assert_eq!(r.formal(*a), Some(*v), "{kind:?} {a:?}");
        }

        let rebuilt = Record::from_parts(
            kind,
            r.id().cloned(),
            formals
                .into_iter()
                .map(|(a, v)| (a, v.to_owned()))
                .collect(),
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
            .map(|(a, v)| (a, v.to_owned()))
            .collect(),
        Attributes::new(),
    )
    .unwrap();
    assert_ne!(unnamed, r);
}

#[test]
fn attributes_mut_edits_every_variant() {
    for &kind in Kind::ALL {
        let mut r = Record::from_parts(kind, Some(q("x")), vec![], Attributes::new()).unwrap();
        assert!(r.attributes_mut().insert(q("label"), Value::from("x")));
        assert_eq!(r.attributes().iter().count(), 1, "{kind:?}");
    }
}

#[test]
fn formal_reads_one_value() {
    let r = Record::from_parts(
        Kind::Generation,
        None,
        vec![
            (FormalAttr::Entity, Formal::Name(q("e1"))),
            (FormalAttr::Time, Formal::Time(t())),
        ],
        Attributes::new(),
    )
    .unwrap();
    assert_eq!(
        r.formal(FormalAttr::Entity),
        Some(FormalRef::Name(&q("e1")))
    );
    assert_eq!(r.formal(FormalAttr::Time), Some(FormalRef::Time(&t())));
    // A formal of the kind that is absent, and a formal of another kind.
    assert_eq!(r.formal(FormalAttr::Activity), None);
    assert_eq!(r.formal(FormalAttr::Plan), None);
    // Every formal is absent from a record of a kind that has none.
    let e = crate::test_util::entity("e");
    assert!(FormalAttr::ALL.iter().all(|&a| e.formal(a).is_none()));
}

#[test]
fn formal_ref_to_owned_copies_the_value() {
    let (name, time) = (q("e1"), t());
    assert_eq!(FormalRef::Name(&name).to_owned(), Formal::Name(q("e1")));
    assert_eq!(FormalRef::Time(&time).to_owned(), Formal::Time(t()));
}

#[cfg(any(feature = "json", feature = "provn"))]
#[test]
fn check_attributes_rejects_a_hand_built_own_formal() {
    let prov = |l: &str| QName::new(PROV, l);
    let usage = |name: QName| {
        let mut attributes = Attributes::new();
        attributes.insert(name, Value::QName(q("x")));
        Record::Usage(Usage {
            id: None,
            activity: Some(q("a")),
            entity: None,
            time: None,
            attributes,
        })
    };
    let e = usage(prov("entity")).check_attributes().unwrap_err();
    assert!(
        matches!(
            e,
            Error::InvalidRecord {
                kind: Kind::Usage,
                ..
            }
        ),
        "{e:?}"
    );
    // The text is the one `Record::from_parts` gives.
    assert_eq!(
        e.to_string(),
        "used: prov:entity is given as an attribute; \
         its value belongs in the positional argument"
    );
    // A formal of another kind is a plain attribute.
    assert!(usage(prov("plan")).check_attributes().is_ok());
}
