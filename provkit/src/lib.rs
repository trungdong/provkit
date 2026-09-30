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
//!   order, namespace tables, blank-node labels and the spelling of a
//!   number, boolean or `dateTime`.
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

mod attributes;
mod datetime;
mod document;
mod error;
#[cfg(feature = "json")]
pub mod json;
mod namespaces;
mod pn;
#[cfg(feature = "provn")]
pub mod provn;
mod qname;
mod record;
#[cfg(any(feature = "json", feature = "provn"))]
mod sink;
mod unify;
mod value;

#[cfg(test)]
pub(crate) mod test_util;

pub use attributes::Attributes;
pub use datetime::DateTime;
pub use document::{Bundle, Document};
pub use error::{Error, Result};
pub use namespaces::Namespaces;
pub use qname::{QName, well_known};
pub use record::{
    Activity, Agent, Alternate, Association, Attribution, Communication, Delegation, Derivation,
    End, Entity, Formal, FormalAttr, FormalRef, Generation, Influence, Invalidation, Kind,
    Membership, Mention, Record, Specialization, Start, Usage,
};
pub use value::{LangStr, Value};

/// `text` without one leading byte-order mark.
#[cfg(any(feature = "json", feature = "provn"))]
pub(crate) fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}
