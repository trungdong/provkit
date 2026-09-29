//! W3C PROV data model with PROV-JSON and PROV-N read and write.
//!
//! A [`Document`] holds [`Record`]s, one enum variant per PROV-DM type, and
//! named [`Bundle`]s. Readers and writers are free functions in [`json`] and
//! [`provn`]. They take and return text, so the library never touches files.
//!
//! ```
//! use provkit::{json, provn, Attributes, Entity, QName, Record};
//!
//! let text = r#"{"prefix": {"ex": "http://example.org/"}, "entity": {"ex:e1": {}}}"#;
//! let mut doc = json::read(text)?;
//! doc.records.push(Record::Entity(Entity {
//!     id: QName::new("http://example.org/", "e2"),
//!     attributes: Attributes::new(),
//! }));
//! let out = provn::to_string(&doc)?;
//! assert!(out.contains("entity(ex:e2)"));
//! # Ok::<(), provkit::Error>(())
//! ```
//!
//! Comparison and merging:
//!
//! - `==` on [`Document`] is set-based. It ignores record order, attribute
//!   order, namespace tables, blank-node labels and `dateTime` spelling.
//! - [`Document::unified`] merges records that share an identifier and kind,
//!   and returns [`Error::Unification`] when their formal attributes conflict.
//! - [`provn::Profile`] selects how strictly PROV-N input is checked.
//!
//! Departures from the PROV Recommendations, `ProvPy` and `ProvToolbox` are
//! listed in `CONFORMANCE.md` in the repository.
//!
//! # Features
//!
//! - `json` (default): the [`json`] module, with `serde` and `serde_json`.
//! - `provn` (default): the [`provn`] module.

pub mod attributes;
pub mod datetime;
pub mod document;
pub mod error;
#[cfg(feature = "json")]
pub mod json;
pub mod namespaces;
#[cfg(feature = "provn")]
pub mod provn;
pub mod qname;
pub mod record;
pub mod sink;
mod unify;
pub mod value;

#[cfg(test)]
pub(crate) mod test_util;

pub use attributes::Attributes;
pub use datetime::DateTime;
pub use document::{Bundle, Document};
pub use error::{Error, Result};
pub use namespaces::{Namespaces, Scope};
#[cfg(feature = "provn")]
pub use provn::Profile;
pub use qname::QName;
pub use record::{
    Activity, Agent, Alternate, Association, Attribution, Communication, Delegation, Derivation,
    End, Entity, Formal, FormalAttr, FormalRef, Generation, Influence, Invalidation, Kind,
    Membership, Mention, Record, Specialization, Start, Usage,
};
pub use sink::{DocumentBuilder, Sink};
pub use value::{LangStr, Value};
