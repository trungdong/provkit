//! Merging records that share an identifier, and rejecting the sharing
//! that PROV-CONSTRAINTS forbids.
//!
//! The merge follows the key constraints (22 and 23) with one difference:
//! an absent formal unifies with any value, in every position. In four
//! positions (`plan` of an association; `activity`, `generation` and
//! `usage` of a derivation) PROV-CONSTRAINTS reads `-` as "none", which
//! unifies only with another "none". Every formal here is an `Option`, so
//! "none" and "unknown" are one state.
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

use crate::error::clip;
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
            let value = value.to_formal();
            match &chosen {
                None => chosen = Some(value),
                Some(existing) if *existing == value => {}
                Some(existing) => {
                    return Err(Error::Unification {
                        id: id.clone(),
                        reason: format!(
                            "{attr} is {} in one record and {} in another",
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
    let text = match formal {
        Formal::Name(q) => q.to_string(),
        Formal::Time(t) => t.to_string(),
    };
    clip(&text).into_owned()
}

#[cfg(test)]
mod tests;
