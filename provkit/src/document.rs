//! Documents and bundles.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::{Namespaces, QName, Record};

/// A named bundle of records. Bundles do not nest.
///
/// Equality ignores the namespace table, record order and duplicate records.
#[derive(Clone, Debug)]
pub struct Bundle {
    /// The bundle's identifier.
    pub id: QName,
    /// Prefixes declared inside the bundle.
    pub namespaces: Namespaces,
    /// The bundle's records.
    pub records: Vec<Record>,
}

/// A PROV document: namespaces, top-level records and named bundles.
///
/// Records refer to each other by identifier. The document holds no
/// references into itself, so it can be cloned, compared and sent between
/// threads freely.
///
/// Two documents are equal when they assert the same set of records at the
/// top level and in each bundle, with bundles matched by identifier.
/// Bundles that share an identifier compare as the union of their records.
/// Namespace tables, record order, bundle order and duplicate records take
/// no part.
#[derive(Clone, Debug, Default)]
pub struct Document {
    /// Prefixes declared at the top level.
    pub namespaces: Namespaces,
    /// Top-level records, in assertion order.
    pub records: Vec<Record>,
    /// Named bundles, in assertion order.
    pub bundles: Vec<Bundle>,
}

impl Document {
    /// An empty document.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The first bundle with `id`, if any.
    ///
    /// A hand-built document can hold several bundles with one identifier.
    /// [`merged_bundles`](Self::merged_bundles) reads them as one bundle.
    #[must_use]
    pub fn bundle(&self, id: &QName) -> Option<&Bundle> {
        self.bundles.iter().find(|b| b.id == *id)
    }

    /// The bundles as document equality and the writers read them: bundles
    /// sharing an identifier become one.
    ///
    /// The first copy keeps its position, records follow in document order
    /// and the first binding of a prefix wins. Names are resolved IRIs, so a
    /// dropped binding only changes how a writer spells a name.
    ///
    /// When every identifier is unique, the result borrows the document's own
    /// bundles and copies nothing.
    #[must_use]
    pub fn merged_bundles(&self) -> Cow<'_, [Bundle]> {
        let mut seen = HashSet::new();
        if self.bundles.iter().all(|b| seen.insert(&b.id)) {
            return Cow::Borrowed(&self.bundles);
        }
        let mut merged: Vec<Bundle> = Vec::new();
        // Identifier -> position in `merged`.
        let mut at: HashMap<&QName, usize> = HashMap::new();
        for bundle in &self.bundles {
            if let Some(&i) = at.get(&bundle.id) {
                merged[i].namespaces.absorb(&bundle.namespaces);
                merged[i].records.extend(bundle.records.iter().cloned());
            } else {
                at.insert(&bundle.id, merged.len());
                merged.push(bundle.clone());
            }
        }
        Cow::Owned(merged)
    }

    /// Top-level records carrying `id`, in assertion order.
    pub fn records_with_id<'a>(&'a self, id: &'a QName) -> impl Iterator<Item = &'a Record> + 'a {
        self.records.iter().filter(move |r| r.id() == Some(id))
    }

    /// Every record, with the top level first and then each bundle in order.
    pub fn all_records(&self) -> impl Iterator<Item = &Record> {
        self.records
            .iter()
            .chain(self.bundles.iter().flat_map(|b| b.records.iter()))
    }
}

/// Compares as sets, so order and duplicates do not matter.
fn same_records(a: &[Record], b: &[Record]) -> bool {
    let a: HashSet<&Record> = a.iter().collect();
    let b: HashSet<&Record> = b.iter().collect();
    a == b
}

/// Records per bundle identifier, merged across bundles that share one.
fn bundle_sets(d: &Document) -> HashMap<&QName, HashSet<&Record>> {
    let mut sets: HashMap<&QName, HashSet<&Record>> = HashMap::new();
    for b in &d.bundles {
        sets.entry(&b.id).or_default().extend(&b.records);
    }
    sets
}

impl PartialEq for Bundle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && same_records(&self.records, &other.records)
    }
}

impl Eq for Bundle {}

impl PartialEq for Document {
    // Grouping bundles by identifier keeps `==` reflexive when a hand-built
    // document repeats a bundle identifier.
    fn eq(&self, other: &Self) -> bool {
        same_records(&self.records, &other.records) && bundle_sets(self) == bundle_sets(other)
    }
}

impl Eq for Document {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{agent, entity, q};

    fn bundle(id: &str, records: Vec<Record>) -> Bundle {
        Bundle {
            id: q(id),
            namespaces: Namespaces::new(),
            records,
        }
    }

    #[test]
    fn equality_is_a_set_over_records() {
        let mut a = Document::new();
        a.records.extend([entity("e1"), entity("e2")]);
        let mut b = Document::new();
        b.records.extend([entity("e2"), entity("e1"), entity("e1")]);
        assert_eq!(a, b);
        b.records.push(entity("e3"));
        assert_ne!(a, b);
    }

    #[test]
    fn namespace_tables_are_ignored() {
        let mut a = Document::new();
        a.records.push(entity("e1"));
        a.bundles.push(bundle("b", vec![entity("e2")]));
        let mut b = a.clone();
        b.namespaces.declare("ex", "http://example.org/").unwrap();
        b.bundles[0]
            .namespaces
            .declare("other", "http://other.example/")
            .unwrap();
        assert_ne!(a.namespaces, b.namespaces);
        assert_ne!(a.bundles[0].namespaces, b.bundles[0].namespaces);
        assert_eq!(a, b);
    }

    #[test]
    fn bundles_match_by_identifier() {
        let mut a = Document::new();
        a.bundles.push(bundle("b", vec![entity("e1")]));
        a.bundles.push(bundle("c", vec![entity("e2")]));
        let mut b = a.clone();
        b.bundles.reverse();
        b.bundles[0].records.push(entity("e2"));
        assert_eq!(a, b);
        b.bundles[0].records.clear();
        assert_ne!(a, b);
        b.bundles[0].records.push(entity("e2"));
        b.bundles[0].id = q("d");
        assert_ne!(a, b);
    }

    #[test]
    fn repeated_bundle_identifiers_merge() {
        let mut a = Document::new();
        a.bundles.push(bundle("b", vec![entity("e1")]));
        a.bundles.push(bundle("b", vec![entity("e2")]));
        assert_eq!(a, a);
        let mut b = Document::new();
        b.bundles.push(bundle("b", vec![entity("e2")]));
        b.bundles.push(bundle("b", vec![entity("e1")]));
        assert_eq!(a, b);
        assert_eq!(b, a);
        let mut c = Document::new();
        c.bundles
            .push(bundle("b", vec![entity("e1"), entity("e2")]));
        assert_eq!(a, c);
        c.bundles.push(bundle("c", vec![entity("e1")]));
        assert_ne!(a, c);
        assert_ne!(c, a);
    }

    #[test]
    fn lookups() {
        let mut d = Document::new();
        d.records
            .extend([entity("e1"), entity("e2"), agent("e1"), agent("e3")]);
        d.bundles.push(bundle("b", vec![entity("e4")]));
        d.bundles.push(bundle("c", vec![entity("e1"), agent("e5")]));
        assert_eq!(d.bundle(&q("c")), Some(&d.bundles[1]));
        assert_eq!(d.bundle(&q("e1")), None);
        assert_eq!(
            d.records_with_id(&q("e1")).collect::<Vec<_>>(),
            [&entity("e1"), &agent("e1")]
        );
        assert_eq!(
            d.all_records().collect::<Vec<_>>(),
            [
                &entity("e1"),
                &entity("e2"),
                &agent("e1"),
                &agent("e3"),
                &entity("e4"),
                &entity("e1"),
                &agent("e5"),
            ]
        );
    }

    #[test]
    fn merged_bundles_keep_first_positions() {
        let bundles = vec![
            bundle("b", vec![entity("e1")]),
            bundle("c", vec![entity("e2")]),
            bundle("b", vec![entity("e3")]),
        ];
        let doc = Document {
            bundles,
            ..Document::default()
        };
        let merged = doc.merged_bundles();
        assert!(matches!(merged, Cow::Owned(_)));
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, q("b"));
        assert_eq!(merged[0].records, [entity("e1"), entity("e3")]);
        assert_eq!(merged[1].id, q("c"));
    }

    #[test]
    fn merged_bundles_borrow_when_identifiers_are_unique() {
        let doc = Document {
            bundles: vec![
                bundle("b", vec![entity("e1")]),
                bundle("c", vec![entity("e2")]),
            ],
            ..Document::default()
        };
        let merged = doc.merged_bundles();
        assert!(matches!(merged, Cow::Borrowed(_)));
        assert!(std::ptr::eq(merged.as_ptr(), doc.bundles.as_ptr()));
        assert!(Document::new().merged_bundles().is_empty());
    }

    #[test]
    fn bundle_returns_the_first_of_several_with_one_identifier() {
        let doc = Document {
            bundles: vec![
                bundle("b", vec![entity("e1")]),
                bundle("b", vec![entity("e2")]),
            ],
            ..Document::default()
        };
        assert_eq!(doc.bundle(&q("b")).unwrap().records, [entity("e1")]);
    }
}
