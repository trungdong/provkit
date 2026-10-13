//! Tests for the record tables and [`Record::from_parts`].

use super::FormalAttr as F;
use super::*;
use crate::test_util::{dt, entity, ex, hash, prov, relation};
use crate::{Attributes, Value};

/// The generation of `ex:e1` at [`dt`].
fn generation(id: Option<QName>) -> Record {
    relation(
        Kind::Generation,
        id,
        vec![
            (F::Entity, Formal::Name(ex("e1"))),
            (F::Time, Formal::Time(dt())),
        ],
    )
}

/// A value for `attr` whose text names the formal, so a swapped field shows.
fn formal_for(attr: FormalAttr) -> Formal {
    if attr.is_time() {
        let position = FormalAttr::ALL.iter().position(|f| *f == attr).unwrap();
        let minute = u8::try_from(position).unwrap();
        Formal::Time(DateTime::new(2011, 11, 16, 16, minute, 0, 0, Some(0)).unwrap())
    } else {
        Formal::Name(ex(attr.local_name()))
    }
}

#[test]
fn a_formal_displays_as_its_prefixed_name() {
    assert_eq!(F::StartTime.to_string(), "prov:startTime");
    for attr in FormalAttr::ALL {
        assert_eq!(attr.to_string(), format!("prov:{}", attr.local_name()));
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
    let bare: Vec<Kind> = Kind::ALL.iter().copied().filter(|k| k.is_bare()).collect();
    assert_eq!(
        bare,
        [
            Kind::Specialization,
            Kind::Alternate,
            Kind::Mention,
            Kind::Membership
        ]
    );
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

#[test]
fn formal_tables_match_the_spec() {
    let table: Vec<(Kind, &[&str], &[usize])> = vec![
        (Kind::Entity, &[], &[0]),
        (Kind::Activity, &["startTime", "endTime"], &[0, 2]),
        (Kind::Agent, &[], &[0]),
        (Kind::Generation, &["entity", "activity", "time"], &[1, 3]),
        (Kind::Usage, &["activity", "entity", "time"], &[1, 3]),
        (Kind::Communication, &["informed", "informant"], &[2]),
        (
            Kind::Start,
            &["activity", "trigger", "starter", "time"],
            &[1, 4],
        ),
        (
            Kind::End,
            &["activity", "trigger", "ender", "time"],
            &[1, 4],
        ),
        (Kind::Invalidation, &["entity", "activity", "time"], &[1, 3]),
        (
            Kind::Derivation,
            &[
                "generatedEntity",
                "usedEntity",
                "activity",
                "generation",
                "usage",
            ],
            &[2, 5],
        ),
        (Kind::Attribution, &["entity", "agent"], &[2]),
        (Kind::Association, &["activity", "agent", "plan"], &[1, 3]),
        (
            Kind::Delegation,
            &["delegate", "responsible", "activity"],
            &[2, 3],
        ),
        (Kind::Influence, &["influencee", "influencer"], &[2]),
        (
            Kind::Specialization,
            &["specificEntity", "generalEntity"],
            &[2],
        ),
        (Kind::Alternate, &["alternate1", "alternate2"], &[2]),
        (
            Kind::Mention,
            &["specificEntity", "generalEntity", "bundle"],
            &[3],
        ),
        (Kind::Membership, &["collection", "entity"], &[2]),
    ];
    assert_eq!(
        table.iter().map(|(k, ..)| *k).collect::<Vec<_>>(),
        Kind::ALL
    );
    for (kind, names, arities) in table {
        let got: Vec<_> = kind.formals().iter().map(|f| f.local_name()).collect();
        assert_eq!(got, names, "{kind:?}");
        assert_eq!(kind.arities(), arities, "{kind:?}");
    }
}

#[test]
fn the_largest_arity_is_the_number_of_formals() {
    for &kind in Kind::ALL {
        assert_eq!(
            kind.arities().iter().copied().max(),
            Some(kind.formals().len()),
            "{}",
            kind.name()
        );
    }
}

#[test]
fn from_parts_builds_a_generation() {
    let r = generation(Some(ex("g1")));
    assert_eq!(r.kind(), Kind::Generation);
    assert_eq!(r.id(), Some(&ex("g1")));
    let Record::Generation(g) = &r else { panic!() };
    assert_eq!(g.entity, Some(ex("e1")));
    assert_eq!(g.activity, None);
    assert_eq!(g.time, Some(dt()));
    assert_eq!(
        r.formals(),
        vec![
            (FormalAttr::Entity, FormalRef::Name(&ex("e1"))),
            (FormalAttr::Time, FormalRef::Time(&dt())),
        ]
    );
}

#[test]
fn from_parts_builds_every_kind_empty() {
    for &kind in Kind::ALL {
        let id = kind.is_element().then(|| ex("x"));
        let r = Record::from_parts(kind, id.clone(), vec![], Attributes::new()).unwrap();
        assert_eq!(r.kind(), kind);
        assert_eq!(r.id(), id.as_ref());
        assert!(r.formals().is_empty(), "{:?}", r.formals());
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
        attributes.insert(ex("label"), Value::from("x"));
        let r = Record::from_parts(kind, Some(ex("r")), parts, attributes.clone()).unwrap();
        assert_eq!(r.kind(), kind);
        assert_eq!(r.id(), Some(&ex("r")));
        assert_eq!(r.attributes(), &attributes);

        let formals = r.formals();
        let attrs: Vec<_> = formals.iter().map(|(a, _)| *a).collect();
        assert_eq!(attrs, kind.formals(), "{kind:?} order");
        for (a, v) in &formals {
            assert_eq!(v.to_formal(), formal_for(*a), "{kind:?} {a:?}");
            assert_eq!(r.formal(*a), Some(*v), "{kind:?} {a:?}");
        }

        let rebuilt = Record::from_parts(
            kind,
            r.id().cloned(),
            formals
                .into_iter()
                .map(|(a, v)| (a, v.to_formal()))
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
        vec![(FormalAttr::Plan, Formal::Name(ex("p")))]
    ));
    assert!(bad(
        Kind::Entity,
        Some(ex("e")),
        vec![(FormalAttr::Entity, Formal::Name(ex("e")))]
    ));
    assert!(bad(
        Kind::Generation,
        None,
        vec![(FormalAttr::Entity, Formal::Time(dt()))]
    ));
    assert!(bad(
        Kind::Generation,
        None,
        vec![(FormalAttr::Time, Formal::Name(ex("t")))]
    ));
    assert!(bad(
        Kind::Generation,
        None,
        vec![
            (FormalAttr::Entity, Formal::Name(ex("a"))),
            (FormalAttr::Entity, Formal::Name(ex("b"))),
        ]
    ));
    // A repeated time formal is rejected as a repeated name is.
    assert!(bad(
        Kind::Generation,
        None,
        vec![
            (FormalAttr::Time, Formal::Time(dt())),
            (FormalAttr::Time, Formal::Time(dt())),
        ]
    ));
    assert!(bad(
        Kind::Activity,
        Some(ex("a")),
        vec![
            (FormalAttr::StartTime, Formal::Time(dt())),
            (FormalAttr::StartTime, Formal::Time(dt())),
        ]
    ));
}

#[test]
fn attribute_named_after_an_own_formal_is_rejected() {
    let mut clash = Attributes::new();
    clash.insert(prov("activity"), Value::QName(ex("a2")));
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
    fine.insert(prov("plan"), Value::QName(ex("p")));
    fine.insert(prov("type"), Value::from("t"));
    fine.insert(prov("label"), Value::from("l"));
    for kind in [Kind::Generation, Kind::Usage, Kind::Entity] {
        let r = Record::from_parts(kind, Some(ex("x")), vec![], fine.clone()).unwrap();
        assert_eq!(r.attributes().len(), 3);
    }
}

#[test]
fn records_compare_and_hash_by_value() {
    let build = |id: &str| {
        relation(
            Kind::Usage,
            Some(ex(id)),
            vec![(F::Activity, Formal::Name(ex("a")))],
        )
    };
    let r = build("u");
    assert_eq!(build("u"), r);
    assert_eq!(hash(&build("u")), hash(&r));
    assert_ne!(build("u"), build("v"));
    let unnamed = relation(
        Kind::Usage,
        None,
        r.formals()
            .into_iter()
            .map(|(a, v)| (a, v.to_formal()))
            .collect(),
    );
    assert_ne!(unnamed, r);
}

#[test]
fn attributes_mut_edits_every_variant() {
    for &kind in Kind::ALL {
        let mut r = Record::from_parts(kind, Some(ex("x")), vec![], Attributes::new()).unwrap();
        assert!(r.attributes_mut().insert(ex("label"), Value::from("x")));
        assert_eq!(r.attributes().iter().count(), 1, "{kind:?}");
    }
}

#[test]
fn formal_reads_one_value() {
    let r = generation(None);
    assert_eq!(
        r.formal(FormalAttr::Entity),
        Some(FormalRef::Name(&ex("e1")))
    );
    assert_eq!(r.formal(FormalAttr::Time), Some(FormalRef::Time(&dt())));
    // A formal of the kind that is absent, and a formal of another kind.
    assert_eq!(r.formal(FormalAttr::Activity), None);
    assert_eq!(r.formal(FormalAttr::Plan), None);
    // Every formal is absent from a record of a kind that has none.
    let e = entity("e");
    assert!(FormalAttr::ALL.iter().all(|&a| e.formal(a).is_none()));
}

#[test]
fn formal_ref_to_formal_copies_the_value() {
    let (name, time) = (ex("e1"), dt());
    assert_eq!(FormalRef::Name(&name).to_formal(), Formal::Name(ex("e1")));
    assert_eq!(FormalRef::Time(&time).to_formal(), Formal::Time(dt()));
    assert_eq!(Formal::from(FormalRef::Name(&name)), Formal::Name(ex("e1")));
}

#[test]
fn check_attributes_rejects_a_hand_built_own_formal() {
    let usage = |name: QName| {
        let mut attributes = Attributes::new();
        attributes.insert(name, Value::QName(ex("x")));
        Record::Usage(Usage {
            id: None,
            activity: Some(ex("a")),
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
