//! Merging records that share an identifier (PROV-CONSTRAINTS key
//! constraints 22 and 23, and the compatibility table of constraint 55).

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::document::merged_bundles;
use crate::{
    Attributes, Bundle, Document, Error, Formal, FormalAttr, FormalRef, Kind, QName, Record, Result,
};

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
    /// Kinds that PROV-CONSTRAINTS forbids under one identifier (an entity
    /// and an activity, two relation kinds, an element and a relation) are
    /// an error. An agent may overlap with an entity or an activity and
    /// stays a separate record. Records without an identifier never merge,
    /// and neither do `hadMember` records, which PROV-DM gives no identifier.
    ///
    /// Bundles unify independently. Bundles that share an identifier are
    /// combined first, as document equality treats them.
    ///
    /// # Errors
    ///
    /// - [`Error::Unification`] when two records conflict.
    /// - [`Error::Structure`] from [`Record::from_parts`] when a merged
    ///   record carries an attribute that spells one of its own formals
    ///   (`prov:activity` on a generation). Readers never produce such a
    ///   record, but a hand-built one can.
    pub fn unified(&self) -> Result<Document> {
        let mut bundles = Vec::new();
        for bundle in merged_bundles(&self.bundles) {
            bundles.push(Bundle {
                records: unify_records(&bundle.records)?,
                ..bundle
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
    // `hadMember` no identifier, so a Membership id is only a PROV-JSON
    // grouping key and never merges.
    let mut groups: IndexMap<&QName, Vec<usize>> = IndexMap::new();
    for (i, r) in records.iter().enumerate() {
        if let Some(id) = r.id().filter(|_| r.kind() != Kind::Membership) {
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

/// Constraint 55: entity and activity are disjoint, an agent may be either,
/// and relation kinds are disjoint from each other and from elements.
fn check_kinds(id: &QName, kinds: impl Iterator<Item = Kind>) -> Result<()> {
    let mut seen: Vec<Kind> = Vec::new();
    for kind in kinds {
        if seen.contains(&kind) {
            continue;
        }
        for &other in &seen {
            let compatible = matches!(
                (kind, other),
                (Kind::Entity | Kind::Activity, Kind::Agent)
                    | (Kind::Agent, Kind::Entity | Kind::Activity)
            );
            if !compatible {
                return Err(Error::Unification {
                    id: id.clone(),
                    reason: format!(
                        "{} and {} cannot share an identifier",
                        other.json_key(),
                        kind.json_key()
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
            let Some((_, value)) = record.formals().into_iter().find(|(a, _)| *a == attr) else {
                continue;
            };
            let value = match value {
                FormalRef::Name(q) => Formal::Name(q.clone()),
                FormalRef::Time(t) => Formal::Time(t.clone()),
            };
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
    use crate::qname::well_known::PROV_LABEL;
    use crate::test_util::q;
    use crate::{
        Attributes, Bundle, DateTime, Document, Error, Formal, FormalAttr as F, Kind, Namespaces,
        Record,
    };

    fn rec(kind: Kind, id: &str, formals: Vec<(F, Formal)>, label: Option<&str>) -> Record {
        let mut a = Attributes::new();
        if let Some(l) = label {
            a.insert(PROV_LABEL.clone(), l.into());
        }
        Record::from_parts(kind, Some(q(id)), formals, a).unwrap()
    }

    fn anon(kind: Kind, formals: Vec<(F, Formal)>) -> Record {
        Record::from_parts(kind, None, formals, Attributes::new()).unwrap()
    }

    fn t(s: &str) -> Formal {
        Formal::Time(DateTime::parse(s).unwrap())
    }

    fn name(l: &str) -> Formal {
        Formal::Name(q(l))
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
            (Some(q("e")), Some(q("a")))
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
            assert_eq!(g.activity, Some(q("a")));
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
                assert_eq!(id, q("g"));
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

    #[test]
    fn kind_compatibility() {
        let pair = |a: Kind, b: Kind| {
            let mut d = Document::new();
            d.records.push(rec(a, "x", vec![], None));
            d.records.push(rec(b, "x", vec![], None));
            d.unified()
        };
        assert_eq!(pair(Kind::Entity, Kind::Agent).unwrap().records.len(), 2);
        assert_eq!(pair(Kind::Agent, Kind::Entity).unwrap().records.len(), 2);
        assert_eq!(pair(Kind::Activity, Kind::Agent).unwrap().records.len(), 2);
        assert_eq!(pair(Kind::Agent, Kind::Activity).unwrap().records.len(), 2);
        for (a, b) in [
            (Kind::Entity, Kind::Activity),
            (Kind::Generation, Kind::Usage),
            (Kind::Entity, Kind::Generation),
            (Kind::Agent, Kind::Usage),
        ] {
            assert!(
                matches!(pair(a, b), Err(Error::Unification { .. })),
                "{a:?} and {b:?}"
            );
        }
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
        assert_eq!(ids, [q("b"), q("a"), q("c")]);
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
        let bundle = |id: &str, records: Vec<Record>| Bundle {
            id: q(id),
            namespaces: Namespaces::new(),
            records,
        };
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
                id: q("b"),
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
        assert_eq!(
            u.bundles[0].namespaces.lookup("o").as_deref(),
            Some("http://one.org/")
        );
    }

    #[test]
    fn bundles_sharing_an_identifier_merge_first() {
        let bundle = |records: Vec<Record>| Bundle {
            id: q("b"),
            namespaces: Namespaces::new(),
            records,
        };
        let mut d = Document::new();
        d.bundles
            .push(bundle(vec![rec(Kind::Entity, "e", vec![], Some("one"))]));
        d.bundles
            .push(bundle(vec![rec(Kind::Entity, "e", vec![], Some("two"))]));
        let u = d.unified().unwrap();
        assert_eq!(u.bundles.len(), 1);
        assert_eq!(u.bundles[0].records.len(), 1);
        assert_eq!(u.bundles[0].records[0].attributes().len(), 2);
    }

    #[test]
    fn two_spellings_of_one_iri_merge() {
        let split = |ns: &str, local: &str| crate::QName::new(ns, local);
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
    fn merging_a_hand_built_formal_named_attribute_is_a_structure_error() {
        let mut a = Attributes::new();
        a.insert(
            crate::QName::new(crate::namespaces::PROV, "activity"),
            q("x").into(),
        );
        let generation = |attributes: Attributes| {
            Record::Generation(crate::Generation {
                id: Some(q("g")),
                entity: Some(q("e")),
                activity: None,
                time: None,
                attributes,
            })
        };
        let mut d = Document::new();
        d.records.push(generation(a));
        d.records.push(generation(Attributes::new()));
        match d.unified() {
            Err(Error::Structure { format, message }) => {
                assert_eq!(format, "record");
                assert!(message.contains("prov:activity"), "{message}");
            }
            other => panic!("expected a structure error, got {other:?}"),
        }
    }
}
