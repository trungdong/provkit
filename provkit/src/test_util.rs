//! Helpers shared by unit tests.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{Agent, Attributes, Bundle, DateTime, Entity, Namespaces, QName, Record};
use crate::{Document, Error, Formal, FormalAttr, Kind};

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

/// A name in the PROV namespace.
pub(crate) fn prov(local: &str) -> QName {
    QName::new(crate::well_known::PROV, local).unwrap()
}

/// An entity with no attributes.
pub(crate) fn entity(local: &str) -> Record {
    Record::Entity(Entity {
        id: ex(local),
        attributes: Attributes::new(),
    })
}

/// An entity named `local` in `namespace`, with no attributes.
pub(crate) fn entity_in(namespace: &str, local: &str) -> Record {
    Record::Entity(Entity {
        id: QName::new(namespace, local).unwrap(),
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

/// An element of `kind` with `attrs` and no formals.
pub(crate) fn element(kind: Kind, id: QName, attrs: Attributes) -> Record {
    Record::from_parts(kind, Some(id), vec![], attrs).unwrap()
}

/// A relation of `kind` with `formals` and no attributes.
pub(crate) fn relation(
    kind: Kind,
    id: Option<QName>,
    formals: Vec<(FormalAttr, Formal)>,
) -> Record {
    Record::from_parts(kind, id, formals, Attributes::new()).unwrap()
}

/// A document in `http://example.org/` under `ex`, holding `records`.
pub(crate) fn document(records: Vec<Record>) -> Document {
    let mut d = Document::new();
    d.namespaces.declare("ex", "http://example.org/").unwrap();
    d.records = records;
    d
}

// ANCHOR: broken
/// A writer whose every write fails.
pub(crate) struct Broken;

impl std::io::Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("broken"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
// ANCHOR_END: broken

/// The position and message of an [`Error::Syntax`].
pub(crate) fn syntax(e: Error) -> ((usize, usize), String) {
    match e {
        Error::Syntax {
            line,
            column,
            message,
        } => ((line, column), message),
        other => panic!("expected a syntax error, got {other:?}"),
    }
}

/// The position and the inner error of an [`Error::At`].
pub(crate) fn located(e: Error) -> ((usize, usize), Error) {
    match e {
        Error::At {
            line,
            column,
            error,
        } => ((line, column), *error),
        other => panic!("expected Error::At, got {other:?}"),
    }
}

/// `d` written as PROV-JSON, as text and as a JSON value.
#[cfg(feature = "json")]
pub(crate) fn written(d: &Document) -> (String, serde_json::Value) {
    let text = crate::json::to_string(d).unwrap();
    let value = serde_json::from_str(&text).unwrap();
    (text, value)
}

// ANCHOR: round_trip
/// As [`written`], after checking that the text reads back equal to `d`.
#[cfg(feature = "json")]
pub(crate) fn round_trip(d: &Document) -> (String, serde_json::Value) {
    let (text, value) = written(d);
    assert_eq!(crate::json::read(&text).unwrap(), *d, "{text}");
    (text, value)
}
// ANCHOR_END: round_trip
