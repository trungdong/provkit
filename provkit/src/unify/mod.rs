//! Merging records that share an identifier, and rejecting the sharing
//! that PROV-CONSTRAINTS forbids.
//!
//! The merge follows the key constraints (22 and 23). An absent formal is
//! an existential and unifies with any value, except in the positions
//! Definition 4 makes non-expandable, where `-` is the constant "none" and
//! unifies only with another absent value. These are the `plan` of an
//! association, the `activity` of a derivation, and the `generation` and
//! `usage` of a derivation whose `activity` is absent.
//!
//! The kinds that may share an identifier are listed on
//! [`Document::unified`].

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use crate::{
    Attributes, Bundle, Document, Error, Formal, FormalAttr, FormalRef, Kind, QName, Record,
    Result, UnificationCause,
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
    /// After the merge, a record equal to an earlier one in the same scope
    /// is dropped, so the result holds each record once; two records that
    /// differ in any formal or attribute both stay. Bundles unify
    /// independently and the rule does not cross a bundle boundary.
    /// In four positions an absent value is the constant "none" and conflicts
    /// with a present one: the `plan` of `wasAssociatedWith`, the `activity`
    /// of `wasDerivedFrom`, and the `generation` and `usage` of a
    /// `wasDerivedFrom` whose `activity` is absent (PROV-CONSTRAINTS
    /// Definition 4, Table 3, Constraint 23).
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
    /// - [`Error::Unification`] when two records conflict. Its
    ///   [`UnificationCause`](crate::UnificationCause) says whether two kinds or
    ///   one formal attribute clash.
    /// - [`Error::InvalidRecord`] when a record carries an attribute that
    ///   spells one of its own formals (`prov:activity` on a generation).
    ///   Readers never produce such a record, but a hand-built one can.
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
    // A hand-built record can carry an attribute named after one of its own
    // formals. The readers and the writers refuse it, and so does this.
    for record in records {
        record.check_attributes()?;
    }

    // Positions per identifier, groups in first-seen order. PROV-DM gives a
    // bare kind no identifier, so one carried by PROV-JSON is only a
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
        let mut by_kind: IndexMap<Kind, Vec<usize>> = IndexMap::new();
        for &i in positions {
            by_kind.entry(records[i].kind()).or_default().push(i);
        }
        check_kinds(id, by_kind.keys().copied())?;

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

    let merged = records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match replacement.remove(&i) {
            Some(merged) => merged,
            None => Some(r.clone()),
        })
        .collect();
    Ok(each_once(merged))
}

/// `records` with each record kept once: a record equal to an earlier one
/// is dropped and the earlier one keeps its position. A document is a set
/// of records, so the normal form holds each once.
fn each_once(records: Vec<Record>) -> Vec<Record> {
    let keep: Vec<bool> = {
        let mut seen = HashSet::new();
        records.iter().map(|r| seen.insert(r)).collect()
    };
    records
        .into_iter()
        .zip(keep)
        .filter_map(|(record, keep)| keep.then_some(record))
        .collect()
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

/// PROV-CONSTRAINTS Definition 4 and Table 3: the positions in which an
/// absent value is always the constant "none" rather than an existential, so
/// it unifies only with another absent value (Constraint 23).
const NONE_POSITIONS: [(Kind, FormalAttr); 2] = [
    (Kind::Association, FormalAttr::Plan),
    (Kind::Derivation, FormalAttr::Activity),
];

/// Whether `attr` of `record` is a position where an absent value is "none".
///
/// Besides [`NONE_POSITIONS`], Table 3 gives wasDerivedFrom(id; e2, e1, -, g,
/// u) non-expandable `g` and `u`, so the `generation` and `usage` of a
/// derivation are "none" when its `activity` is absent.
fn is_none_position(kind: Kind, attr: FormalAttr, record: &Record) -> bool {
    NONE_POSITIONS.contains(&(kind, attr))
        || (kind == Kind::Derivation
            && matches!(attr, FormalAttr::Generation | FormalAttr::Usage)
            && record.formal(FormalAttr::Activity).is_none())
}

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
/// identifier. `kinds` holds each kind once, in first-seen order.
fn check_kinds(id: &QName, kinds: impl Iterator<Item = Kind>) -> Result<()> {
    let mut seen: Vec<Kind> = Vec::new();
    for kind in kinds {
        for &other in &seen {
            if !may_share_an_identifier(kind, other) {
                return Err(Error::Unification {
                    id: id.clone(),
                    cause: UnificationCause::Kinds {
                        first: other,
                        second: kind,
                    },
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
        let conflict =
            |first: Option<FormalRef<'_>>, second: Option<FormalRef<'_>>| Error::Unification {
                id: id.clone(),
                cause: UnificationCause::Formal {
                    attr,
                    first: first.map(FormalRef::to_formal),
                    second: second.map(FormalRef::to_formal),
                },
            };
        // The activity position has already agreed by the time a derivation
        // reaches its generation and usage, so the first member decides.
        if is_none_position(kind, attr, members[0]) {
            // A constant: every member must hold the same value, absent
            // included.
            let first = members[0].formal(attr);
            for record in &members[1..] {
                let value = record.formal(attr);
                if value != first {
                    return Err(conflict(first, value));
                }
            }
            if let Some(value) = first {
                formals.push((attr, value.to_formal()));
            }
            continue;
        }
        let mut chosen: Option<FormalRef<'_>> = None;
        for value in members.iter().filter_map(|record| record.formal(attr)) {
            match chosen {
                None => chosen = Some(value),
                Some(existing) if existing == value => {}
                Some(existing) => return Err(conflict(Some(existing), Some(value))),
            }
        }
        if let Some(value) = chosen {
            formals.push((attr, value.to_formal()));
        }
    }

    let mut attributes = Attributes::new();
    for record in members {
        attributes.extend_from(record.attributes());
    }
    Record::from_parts(kind, Some(id.clone()), formals, attributes)
}

#[cfg(test)]
mod tests;
