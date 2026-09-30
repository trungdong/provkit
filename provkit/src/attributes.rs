//! Non-formal attributes of a record.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use indexmap::IndexSet;

use crate::{QName, Value};

/// An ordered set of (name, value) pairs.
///
/// Insertion order is kept for output. A pair already present is not added
/// again. Two sets are equal when they hold the same pairs in any order.
/// Inserting and looking up a pair take constant time on average.
///
/// Clones share one allocation. The first write to a clone copies the pairs.
#[derive(Clone, Debug, Default)]
pub struct Attributes {
    pairs: Arc<IndexSet<(QName, Value)>>,
}

impl Attributes {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pair. Returns `false` if it was already present.
    pub fn insert(&mut self, name: QName, value: Value) -> bool {
        Arc::make_mut(&mut self.pairs).insert((name, value))
    }

    /// Remove a pair, keeping the order of the rest. Returns `false` if it
    /// was not present.
    pub fn remove(&mut self, name: &QName, value: &Value) -> bool {
        let pair = (name.clone(), value.clone());
        if !self.pairs.contains(&pair) {
            return false;
        }
        Arc::make_mut(&mut self.pairs).shift_remove(&pair)
    }

    /// Pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&QName, &Value)> {
        self.pairs.iter().map(|(n, v)| (n, v))
    }

    /// Every value under `name`, in insertion order.
    pub fn get<'a>(&'a self, name: &'a QName) -> impl Iterator<Item = &'a Value> + 'a {
        self.pairs
            .iter()
            .filter(move |(n, _)| n == name)
            .map(|(_, v)| v)
    }

    /// Number of pairs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether there are no pairs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Add every pair of `other` not already present.
    pub fn extend_from(&mut self, other: &Attributes) {
        if self.is_empty() {
            self.pairs = Arc::clone(&other.pairs);
            return;
        }
        for (n, v) in other.iter() {
            self.insert(n.clone(), v.clone());
        }
    }
}

#[cfg(test)]
impl Attributes {
    /// Whether both sets point at the same allocation.
    pub(crate) fn shares_storage_with(&self, other: &Attributes) -> bool {
        Arc::ptr_eq(&self.pairs, &other.pairs)
    }
}

type PairRef<'a> = (&'a QName, &'a Value);

impl FromIterator<(QName, Value)> for Attributes {
    fn from_iter<I: IntoIterator<Item = (QName, Value)>>(iter: I) -> Self {
        Self {
            pairs: Arc::new(iter.into_iter().collect()),
        }
    }
}

impl Extend<(QName, Value)> for Attributes {
    fn extend<I: IntoIterator<Item = (QName, Value)>>(&mut self, iter: I) {
        Arc::make_mut(&mut self.pairs).extend(iter);
    }
}

impl IntoIterator for Attributes {
    type Item = (QName, Value);
    type IntoIter = indexmap::set::IntoIter<(QName, Value)>;

    /// Owned pairs in insertion order. Copies the pairs only if the storage
    /// is shared.
    fn into_iter(self) -> Self::IntoIter {
        Arc::unwrap_or_clone(self.pairs).into_iter()
    }
}

impl<'a> IntoIterator for &'a Attributes {
    type Item = PairRef<'a>;
    type IntoIter = std::iter::Map<
        indexmap::set::Iter<'a, (QName, Value)>,
        fn(&'a (QName, Value)) -> PairRef<'a>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.pairs.iter().map(|(n, v)| (n, v))
    }
}

impl PartialEq for Attributes {
    fn eq(&self, other: &Self) -> bool {
        self.pairs.len() == other.pairs.len() && self.pairs.iter().all(|p| other.pairs.contains(p))
    }
}

impl Eq for Attributes {}

impl Hash for Attributes {
    // Order-independent: per-pair hashes are sorted before they reach the
    // outer hasher. `DefaultHasher::new` has fixed keys, so equal sets hash
    // alike in every instance.
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut hashes: Vec<u64> = self
            .pairs
            .iter()
            .map(|pair| {
                let mut h = DefaultHasher::new();
                pair.hash(&mut h);
                h.finish()
            })
            .collect();
        hashes.sort_unstable();
        hashes.len().hash(state);
        for h in hashes {
            h.hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::hash;
    use crate::well_known::{PROV_LABEL, PROV_TYPE};

    #[test]
    fn ordered_deduplicated_set_equal() {
        let mut a = Attributes::new();
        assert!(a.is_empty());
        assert!(a.insert(PROV_TYPE.clone(), "t".into()));
        assert!(a.insert(PROV_LABEL.clone(), "l".into()));
        assert!(!a.insert(PROV_TYPE.clone(), "t".into()));
        assert_eq!(a.len(), 2);
        let names: Vec<_> = a.iter().map(|(n, _)| n.local().to_owned()).collect();
        assert_eq!(names, ["type", "label"]);

        let mut b = Attributes::new();
        b.insert(PROV_LABEL.clone(), "l".into());
        b.insert(PROV_TYPE.clone(), "t".into());
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));

        b.insert(PROV_TYPE.clone(), "u".into());
        assert_ne!(a, b);
        assert_eq!(b.get(&PROV_TYPE).count(), 2);
    }

    fn q(l: &str) -> QName {
        QName::new("http://example.org/", l)
    }

    #[test]
    fn clones_share_until_written() {
        let mut a = Attributes::new();
        a.insert(PROV_LABEL.clone(), Value::from("x"));
        let mut b = a.clone();
        assert!(a.shares_storage_with(&b));
        b.insert(PROV_LABEL.clone(), Value::from("y"));
        assert!(!a.shares_storage_with(&b));
        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn removing_an_absent_pair_keeps_sharing() {
        let mut a = Attributes::new();
        a.insert(PROV_LABEL.clone(), Value::from("x"));
        let mut b = a.clone();
        assert!(!b.remove(&PROV_LABEL, &Value::from("absent")));
        assert!(a.shares_storage_with(&b));
    }

    #[test]
    fn remove_keeps_the_order_of_the_rest() {
        let mut a: Attributes = [("a", "1"), ("b", "2"), ("c", "3")]
            .into_iter()
            .map(|(n, v)| (q(n), Value::from(v)))
            .collect();
        assert!(a.remove(&q("b"), &Value::from("2")));
        assert!(!a.remove(&q("b"), &Value::from("2")));
        let names: Vec<_> = (&a)
            .into_iter()
            .map(|(n, _)| n.local().to_owned())
            .collect();
        assert_eq!(names, ["a", "c"]);
    }

    #[test]
    fn extend_and_collect_drop_duplicates_and_keep_first_position() {
        let pairs =
            [("a", "1"), ("b", "2"), ("a", "1"), ("c", "3")].map(|(n, v)| (q(n), Value::from(v)));
        let mut a: Attributes = pairs.iter().cloned().collect();
        assert_eq!(a.len(), 3);
        a.extend([(q("b"), Value::from("2")), (q("d"), Value::from("4"))]);
        let names: Vec<_> = a.into_iter().map(|(n, _)| n.local().to_owned()).collect();
        assert_eq!(names, ["a", "b", "c", "d"]);
    }

    #[test]
    fn into_iter_yields_owned_pairs_from_shared_storage() {
        let a: Attributes = [(q("a"), Value::from("1")), (q("b"), Value::from("2"))]
            .into_iter()
            .collect();
        let b = a.clone();
        let owned: Vec<_> = a.into_iter().collect();
        assert_eq!(owned.len(), 2);
        assert_eq!(owned[0].0, q("a"));
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn hash_ignores_order_and_separates_sets() {
        let fwd: Attributes = (0..100).map(|i| (q("k"), Value::from(i))).collect();
        let rev: Attributes = (0..100).rev().map(|i| (q("k"), Value::from(i))).collect();
        assert_eq!(fwd, rev);
        assert_eq!(hash(&fwd), hash(&rev));
        let other: Attributes = (1..101).map(|i| (q("k"), Value::from(i))).collect();
        assert_ne!(hash(&fwd), hash(&other));
    }

    #[test]
    fn extend_from_an_empty_set_shares_storage() {
        let mut src = Attributes::new();
        src.insert(PROV_TYPE.clone(), "t".into());
        let mut dst = Attributes::new();
        dst.extend_from(&src);
        assert!(dst.shares_storage_with(&src));
    }

    #[test]
    fn extend_from_skips_present_pairs() {
        let mut a = Attributes::new();
        a.insert(PROV_TYPE.clone(), "t".into());
        let mut b = Attributes::new();
        b.insert(PROV_TYPE.clone(), "t".into());
        b.insert(PROV_LABEL.clone(), "l".into());
        a.extend_from(&b);
        assert_eq!(a.len(), 2);
        assert_eq!(a, b);
    }

    #[test]
    fn large_sets_insert_and_compare_without_pairwise_scans() {
        // Quadratic insert or equality would take minutes at this size.
        let n = 50_000;
        let mut a = Attributes::new();
        let mut b = Attributes::new();
        for i in 0..n {
            assert!(a.insert(PROV_LABEL.clone(), Value::from(i)));
            b.insert(PROV_LABEL.clone(), Value::from(n - 1 - i));
        }
        assert!(!a.insert(PROV_LABEL.clone(), Value::from(0)));
        assert_eq!(a.len(), 50_000);
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
        assert_eq!(a.iter().next().unwrap().1, &Value::from(0));
    }
}
