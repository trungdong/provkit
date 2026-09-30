//! Helpers shared by unit tests.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{Agent, Attributes, Bundle, Entity, Namespaces, QName, Record};

/// Hash a value with the default hasher.
pub(crate) fn hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// A name in `http://example.org/`.
pub(crate) fn ex(local: &str) -> QName {
    QName::new("http://example.org/", local).unwrap()
}

/// An entity with no attributes.
pub(crate) fn entity(local: &str) -> Record {
    Record::Entity(Entity {
        id: ex(local),
        attributes: Attributes::new(),
    })
}

/// An agent with no attributes.
pub(crate) fn agent(local: &str) -> Record {
    Record::Agent(Agent {
        id: ex(local),
        attributes: Attributes::new(),
    })
}

/// A bundle named `id` in `http://example.org/`, with an empty namespace table.
pub(crate) fn bundle(id: &str, records: Vec<Record>) -> Bundle {
    Bundle {
        id: ex(id),
        namespaces: Namespaces::new(),
        records,
    }
}

/// A writer whose every write fails.
#[cfg(any(feature = "json", feature = "provn"))]
pub(crate) struct Broken;

#[cfg(any(feature = "json", feature = "provn"))]
impl std::io::Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("broken"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
