//! Helpers shared by unit tests.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{Agent, Attributes, Entity, QName, Record};

/// Hash a value with the default hasher.
pub(crate) fn hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// A name in `http://example.org/`.
pub(crate) fn q(local: &str) -> QName {
    QName::new("http://example.org/", local).unwrap()
}

/// An entity with no attributes.
pub(crate) fn entity(local: &str) -> Record {
    Record::Entity(Entity {
        id: q(local),
        attributes: Attributes::new(),
    })
}

/// An agent with no attributes.
pub(crate) fn agent(local: &str) -> Record {
    Record::Agent(Agent {
        id: q(local),
        attributes: Attributes::new(),
    })
}
