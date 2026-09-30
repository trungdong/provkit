//! Non-formal attributes of a record.

use std::hash::{DefaultHasher, Hash, Hasher};

use indexmap::IndexSet;

use crate::{QName, Value};

/// An ordered set of (name, value) pairs.
///
/// Insertion order is kept for output. A pair already present is not added
/// again. Two sets are equal when they hold the same pairs in any order.
/// Inserting and looking up a pair take constant time on average.
#[derive(Clone, Debug, Default)]
pub struct Attributes {
    pairs: IndexSet<(QName, Value)>,
}

impl Attributes {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pair. Returns `false` if it was already present.
    pub fn insert(&mut self, name: QName, value: Value) -> bool {
        self.pairs.insert((name, value))
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
        for (n, v) in other.iter() {
            self.insert(n.clone(), v.clone());
        }
    }
}

impl PartialEq for Attributes {
    fn eq(&self, other: &Self) -> bool {
        self.pairs.len() == other.pairs.len() && self.pairs.iter().all(|p| other.pairs.contains(p))
    }
}

impl Eq for Attributes {}

impl Hash for Attributes {
    // Order-independent: per-pair hashes combine by wrapping addition, so
    // equal sets in different orders hash alike.
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut sum: u64 = 0;
        for pair in &self.pairs {
            let mut h = DefaultHasher::new();
            pair.hash(&mut h);
            sum = sum.wrapping_add(h.finish());
        }
        self.pairs.len().hash(state);
        sum.hash(state);
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
