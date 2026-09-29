//! Helpers shared by unit tests.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Hash a value with the default hasher.
pub(crate) fn hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
