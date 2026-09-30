//! Merging records that share an identifier (PROV-CONSTRAINTS key
//! constraints 22 and 23) and rejecting the sharing that PROV-CONSTRAINTS
//! forbids.
//!
//! - Constraint 53: `used`, `wasGeneratedBy`, `wasInvalidatedBy`,
//!   `wasStartedBy`, `wasEndedBy`, `wasInformedBy`, `wasAttributedTo`,
//!   `wasAssociatedWith` and `actedOnBehalfOf` are pairwise disjoint, so no
//!   identifier names two of them.
//! - Constraint 54: an entity, activity or agent identifier cannot also
//!   identify any of those nine relations, `wasInfluencedBy` or
//!   `wasDerivedFrom`.
//! - Constraint 55: `entity` and `activity` are disjoint. An agent may also
//!   be an entity or an activity.
//!
//! `wasInfluencedBy` and `wasDerivedFrom` are outside Constraint 53, so either
//! may share an identifier with any relation, including each other.

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::{Attributes, Bundle, Document, Error, Formal, FormalAttr, Kind, QName, Record, Result};

impl Document {
    /// A copy of the document in which records sharing an identifier and a
    /// kind are merged.
    ///
    /// Formal attributes unify position by position. An absent value takes
    /// the concrete one, equal values unify, and two different concrete
    /// values are an [`Error::Unification`] naming the identifier, the
    /// attribute and both values. Non-formal attributes are the union in
    /// first-seen order. The merged record takes the first member's position.
    ///
    /// Kinds that PROV-CONSTRAINTS forbids under one identifier are an
    /// error:
    ///
    /// - two different kinds among the nine relations of Constraint 53
    ///   (`used`, `wasGeneratedBy`, `wasInvalidatedBy`, `wasStartedBy`,
    ///   `wasEndedBy`, `wasInformedBy`, `wasAttributedTo`,
    ///   `wasAssociatedWith`, `actedOnBehalfOf`);
    /// - an element and any of the eleven relations of Constraint 54 (those
    ///   nine plus `wasInfluencedBy` and `wasDerivedFrom`);
    /// - an entity and an activity (Constraint 55).
    ///
    /// An agent may overlap with an entity or an activity and stays a
    /// separate record. `wasInfluencedBy` and `wasDerivedFrom` may share an
    /// identifier with any relation. Records without an identifier never
    /// merge, and neither do `hadMember`, `alternateOf`, `specializationOf`
    /// and `mentionOf` records, which PROV-DM gives no identifier.
    ///
    /// Bundles unify independently. Bundles that share an identifier are
    /// combined first, as document equality treats them.
    ///
    /// # Errors
    ///
    /// - [`Error::Unification`] when two records conflict.
    /// - [`Error::InvalidRecord`] from [`Record::from_parts`] when a merged
    ///   record carries an attribute that spells one of its own formals
    ///   (`prov:activity` on a generation). Readers never produce such a
    ///   record, but a hand-built one can.
    pub fn unified(&self) -> Result<Document> {
        let mut bundles = Vec::new();
        for bundle in self.merged_bundles().iter() {
            bundles.push(Bundle {
                id: bundle.id.clone(),
                namespaces: bundle.namespaces.clone(),
                records: unify_records(&bundle.records)?,
            });
        }
        Ok(Document {
            namespaces: self.namespaces.clone(),
            records: unify_records(&self.records)?,
            bundles,
        })
    }
}

fn unify_records(records: &[Record]) -> Result<Vec<Record>> {
    // Positions per identifier, groups in first-seen order. PROV-DM gives
    // these four kinds no identifier, so one carried by PROV-JSON is only a
    // grouping key and never merges.
    let mut groups: IndexMap<&QName, Vec<usize>> = IndexMap::new();
    for (i, r) in records.iter().enumerate() {
        if let Some(id) = r.id().filter(|_| !r.kind().is_bare()) {
            groups.entry(id).or_default().push(i);
        }
    }

    // Position -> merged record for a group's first member of a kind, or
    // `None` for a later member that the merged record absorbs.
    let mut replacement: HashMap<usize, Option<Record>> = HashMap::new();
    for (id, positions) in &groups {
        if positions.len() < 2 {
            continue;
        }
        check_kinds(id, positions.iter().map(|&i| records[i].kind()))?;

        let mut by_kind: IndexMap<Kind, Vec<usize>> = IndexMap::new();
        for &i in positions {
            by_kind.entry(records[i].kind()).or_default().push(i);
        }
        for (kind, members) in by_kind {
            let merged = merge(
                kind,
                id,
                &members.iter().map(|&i| &records[i]).collect::<Vec<_>>(),
            )?;
            replacement.insert(members[0], Some(merged));
            for &i in &members[1..] {
                replacement.insert(i, None);
            }
        }
    }

    Ok(records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match replacement.remove(&i) {
            Some(merged) => merged,
            None => Some(r.clone()),
        })
        .collect())
}

/// Constraint 53: these relation kinds cannot share an identifier with one
/// another.
const DISJOINT_RELATIONS: [Kind; 9] = [
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

/// Whether PROV-CONSTRAINTS lets two different kinds share an identifier.
fn may_share_an_identifier(a: Kind, b: Kind) -> bool {
    let in_53 = |k| DISJOINT_RELATIONS.contains(&k);
    // Constraint 54 adds influence and derivation to the relations an
    // element excludes.
    let in_54 = |k| in_53(k) || matches!(k, Kind::Influence | Kind::Derivation);
    match (a.is_element(), b.is_element()) {
        // Constraint 55: entity and activity are disjoint, an agent may be
        // either.
        (true, true) => !matches!(
            (a, b),
            (Kind::Entity, Kind::Activity) | (Kind::Activity, Kind::Entity)
        ),
        (true, false) => !in_54(b),
        (false, true) => !in_54(a),
        (false, false) => !(in_53(a) && in_53(b)),
    }
}

/// Reject the pairs of kinds that Constraints 53, 54 and 55 forbid under one
/// identifier.
fn check_kinds(id: &QName, kinds: impl Iterator<Item = Kind>) -> Result<()> {
    let mut seen: Vec<Kind> = Vec::new();
    for kind in kinds {
        if seen.contains(&kind) {
            continue;
        }
        for &other in &seen {
            if !may_share_an_identifier(kind, other) {
                return Err(Error::Unification {
                    id: id.clone(),
                    reason: format!(
                        "{} and {} cannot share an identifier",
                        other.name(),
                        kind.name()
                    ),
                });
            }
        }
        seen.push(kind);
    }
    Ok(())
}

/// Merge records of one kind and identifier.
fn merge(kind: Kind, id: &QName, members: &[&Record]) -> Result<Record> {
    let mut formals: Vec<(FormalAttr, Formal)> = Vec::new();
    for &attr in kind.formals() {
        let mut chosen: Option<Formal> = None;
        for record in members {
            let Some(value) = record.formal(attr) else {
                continue;
            };
            let value = value.to_owned();
            match &chosen {
                None => chosen = Some(value),
                Some(existing) if *existing == value => {}
                Some(existing) => {
                    return Err(Error::Unification {
                        id: id.clone(),
                        reason: format!(
                            "prov:{} is {} in one record and {} in another",
                            attr.local_name(),
                            show(existing),
                            show(&value)
                        ),
                    });
                }
            }
        }
        if let Some(value) = chosen {
            formals.push((attr, value));
        }
    }

    let mut attributes = Attributes::new();
    for record in members {
        attributes.extend_from(record.attributes());
    }
    Record::from_parts(kind, Some(id.clone()), formals, attributes)
}

fn show(formal: &Formal) -> String {
    match formal {
        Formal::Name(q) => q.to_string(),
        Formal::Time(t) => t.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::{bundle, ex};
    use crate::well_known::PROV_LABEL;
    use crate::{
        Attributes, Bundle, DateTime, Document, Error, Formal, FormalAttr as F, Kind, Namespaces,
        Record,
    };

    fn rec(kind: Kind, id: &str, formals: Vec<(F, Formal)>, label: Option<&str>) -> Record {
        let mut a = Attributes::new();
        if let Some(l) = label {
            a.insert(PROV_LABEL.clone(), l.into());
        }
        Record::from_parts(kind, Some(ex(id)), formals, a).unwrap()
    }

    fn anon(kind: Kind, formals: Vec<(F, Formal)>) -> Record {
        Record::from_parts(kind, None, formals, Attributes::new()).unwrap()
    }

    fn t(s: &str) -> Formal {
        Formal::Time(DateTime::parse(s).unwrap())
    }

    fn name(l: &str) -> Formal {
        Formal::Name(ex(l))
    }

    #[test]
    fn merges_formals_and_attributes() {
        let mut d = Document::new();
        d.records.push(rec(
            Kind::Generation,
            "g",
            vec![(F::Entity, name("e"))],
            Some("one"),
        ));
        d.records.push(rec(Kind::Entity, "e", vec![], None));
        d.records.push(rec(
            Kind::Generation,
            "g",
            vec![(F::Activity, name("a"))],
            Some("two"),
        ));
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
            let mut d = Document::new();
            for act in [first, second] {
                let formals = act.map(|a| (F::Activity, name(a))).into_iter().collect();
                d.records.push(rec(Kind::Generation, "g", formals, None));
            }
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
        let mut d = Document::new();
        for _ in 0..2 {
            d.records.push(rec(
                Kind::Generation,
                "g",
                vec![(F::Time, t("2014-06-23T12:28:53Z"))],
                None,
            ));
        }
        assert_eq!(d.unified().unwrap().records.len(), 1);
    }

    #[test]
    fn conflicting_times_fail() {
        let mut d = Document::new();
        d.records.push(rec(
            Kind::Generation,
            "g",
            vec![(F::Time, t("2014-06-23T12:28:53Z"))],
            None,
        ));
        d.records.push(rec(
            Kind::Generation,
            "g",
            vec![(F::Time, t("2012-12-03T21:08:16Z"))],
            None,
        ));
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
        let mut d = Document::new();
        for a in ["a1", "a2"] {
            d.records
                .push(rec(Kind::Usage, "u", vec![(F::Activity, name(a))], None));
        }
        let Err(Error::Unification { reason, .. }) = d.unified() else {
            panic!("expected a unification error")
        };
        assert!(
            reason.contains("prov:activity") && reason.contains("a1") && reason.contains("a2"),
            "{reason}"
        );
    }

    fn pair(a: Kind, b: Kind) -> crate::Result<Document> {
        let mut d = Document::new();
        d.records.push(rec(a, "x", vec![], None));
        d.records.push(rec(b, "x", vec![], None));
        d.unified()
    }

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
        let mut d = Document::new();
        for kind in [Kind::Entity, Kind::Agent, Kind::Activity] {
            d.records.push(rec(kind, "x", vec![], None));
        }
        assert!(matches!(d.unified(), Err(Error::Unification { .. })));
    }

    #[test]
    fn identifier_less_kinds_are_never_merged_or_checked() {
        let with = |kind: Kind, formals: Vec<(F, Formal)>| rec(kind, "x", formals, None);
        let mut d = Document::new();
        for (a1, a2) in [("a", "b"), ("c", "d")] {
            d.records.push(with(
                Kind::Alternate,
                vec![(F::Alternate1, name(a1)), (F::Alternate2, name(a2))],
            ));
            d.records.push(with(
                Kind::Specialization,
                vec![(F::SpecificEntity, name(a1))],
            ));
            d.records
                .push(with(Kind::Mention, vec![(F::SpecificEntity, name(a1))]));
            d.records
                .push(with(Kind::Membership, vec![(F::Collection, name(a1))]));
        }
        // None of the eight clashes with the entity under the same identifier,
        // and none merges with its twin.
        d.records.push(with(Kind::Entity, vec![]));
        let u = d.unified().unwrap();
        assert_eq!(u.records, d.records);
    }

    #[test]
    fn overlapping_kinds_merge_per_kind() {
        let mut d = Document::new();
        d.records.push(rec(Kind::Entity, "x", vec![], Some("e1")));
        d.records.push(rec(Kind::Agent, "x", vec![], None));
        d.records.push(rec(Kind::Entity, "x", vec![], Some("e2")));
        let u = d.unified().unwrap();
        assert_eq!(u.records.len(), 2);
        assert_eq!(u.records[0].kind(), Kind::Entity);
        assert_eq!(u.records[0].attributes().len(), 2);
        assert_eq!(u.records[1].kind(), Kind::Agent);
    }

    #[test]
    fn output_order_follows_first_members() {
        let mut d = Document::new();
        d.records.push(rec(Kind::Entity, "b", vec![], None));
        d.records.push(rec(Kind::Entity, "a", vec![], None));
        d.records.push(rec(Kind::Entity, "b", vec![], Some("l")));
        d.records.push(rec(Kind::Entity, "c", vec![], None));
        let u = d.unified().unwrap();
        let ids: Vec<_> = u.records.iter().map(|r| r.id().unwrap().clone()).collect();
        assert_eq!(ids, [ex("b"), ex("a"), ex("c")]);
    }

    #[test]
    fn records_without_identifiers_never_merge() {
        let mut d = Document::new();
        for _ in 0..2 {
            d.records
                .push(anon(Kind::Generation, vec![(F::Entity, name("e"))]));
        }
        d.records.push(anon(
            Kind::Generation,
            vec![(F::Time, t("2014-06-23T12:28:53Z"))],
        ));
        d.records.push(anon(
            Kind::Generation,
            vec![(F::Time, t("2012-12-03T21:08:16Z"))],
        ));
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
        let mut d = Document::new();
        d.records.push(member("a"));
        d.records.push(member("b"));
        let u = d.unified().unwrap();
        assert_eq!(u.records, d.records);
        // Sharing an identifier with another kind is not a conflict either.
        d.records.push(rec(Kind::Entity, "m", vec![], None));
        assert_eq!(d.unified().unwrap().records, d.records);
    }

    #[test]
    fn bundles_unify_independently() {
        let mut d = Document::new();
        d.records.push(rec(Kind::Entity, "e", vec![], Some("top")));
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
        let mut d = Document::new();
        d.records.push(rec(Kind::Entity, "x", vec![], None));
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
        let split = |ns: &str, local: &str| crate::QName::new(ns, local).unwrap();
        let mut d = Document::new();
        for (ns, local, label) in [
            ("http://example.org/", "foo/bar", "one"),
            ("http://example.org/foo/", "bar", "two"),
        ] {
            let mut a = Attributes::new();
            a.insert(PROV_LABEL.clone(), label.into());
            d.records.push(
                Record::from_parts(
                    Kind::Generation,
                    Some(split(ns, local)),
                    vec![(F::Entity, Formal::Name(split(ns, "e")))],
                    a,
                )
                .unwrap(),
            );
        }
        // `e` differs between the two, so the merge must see a conflict
        // rather than two unrelated records.
        assert!(matches!(d.unified(), Err(Error::Unification { .. })));
        let Record::Generation(g) = &mut d.records[1] else {
            panic!()
        };
        g.entity = Some(split("http://example.org/", "e"));
        let u = d.unified().unwrap();
        assert_eq!(u.records.len(), 1);
        assert_eq!(u.records[0].attributes().len(), 2);
    }

    #[test]
    fn merging_a_hand_built_formal_named_attribute_is_an_invalid_record() {
        let mut a = Attributes::new();
        a.insert(
            crate::QName::new(crate::well_known::PROV, "activity").unwrap(),
            ex("x").into(),
        );
        let generation = |attributes: Attributes| {
            Record::Generation(crate::Generation {
                id: Some(ex("g")),
                entity: Some(ex("e")),
                activity: None,
                time: None,
                attributes,
            })
        };
        let mut d = Document::new();
        d.records.push(generation(a));
        d.records.push(generation(Attributes::new()));
        assert!(matches!(
            d.unified(),
            Err(Error::InvalidRecord {
                kind: Kind::Generation,
                ..
            })
        ));
    }
}
