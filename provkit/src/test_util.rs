//! Helpers shared by unit tests.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{Agent, Attributes, Bundle, DateTime, Entity, Namespaces, QName, Record};
#[cfg(feature = "provn")]
use crate::{Document, Formal, FormalAttr, Kind};

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

/// The dateTime `2011-11-16T16:05:33Z`.
pub(crate) fn dt() -> DateTime {
    DateTime::parse("2011-11-16T16:05:33Z").unwrap()
}

// Only the PROV-N writer tests build records this way.

/// An element of `kind` with `attrs` and no formals.
#[cfg(feature = "provn")]
pub(crate) fn element(kind: Kind, id: QName, attrs: Attributes) -> Record {
    Record::from_parts(kind, Some(id), vec![], attrs).unwrap()
}

/// A relation of `kind` with `formals` and no attributes.
#[cfg(feature = "provn")]
pub(crate) fn relation(
    kind: Kind,
    id: Option<QName>,
    formals: Vec<(FormalAttr, Formal)>,
) -> Record {
    Record::from_parts(kind, id, formals, Attributes::new()).unwrap()
}

/// A document in `http://example.org/` under `ex`, holding `records`.
#[cfg(feature = "provn")]
pub(crate) fn document(records: Vec<Record>) -> Document {
    let mut d = Document::new();
    d.namespaces.declare("ex", "http://example.org/").unwrap();
    d.records = records;
    d
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
