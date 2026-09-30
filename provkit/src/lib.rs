//! W3C PROV data model with PROV-JSON and PROV-N read and write.
//!
//! A [`Document`] holds [`Record`]s, one enum variant per PROV-DM type, and
//! named [`Bundle`]s. Readers and writers are free functions in [`json`] and
//! [`provn`]. Readers take `&str` and writers take `&mut impl std::io::Write`
//! (`to_string` returns a `String`), so the library never touches files.
//!
//! ```
//! # #[cfg(all(feature = "json", feature = "provn"))]
//! # fn main() -> Result<(), provkit::Error> {
//! use provkit::{json, provn, Attributes, Entity, QName, Record};
//!
//! let text = r#"{"prefix": {"ex": "http://example.org/"}, "entity": {"ex:e1": {}}}"#;
//! let mut doc = json::read(text)?;
//! doc.records.push(Record::Entity(Entity {
//!     id: QName::new("http://example.org/", "e2")?,
//!     attributes: Attributes::new(),
//! }));
//! let out = provn::to_string(&doc)?;
//! assert!(out.contains("entity(ex:e2)"));
//! # Ok(())
//! # }
//! # #[cfg(not(all(feature = "json", feature = "provn")))]
//! # fn main() {}
//! ```
//!
//! Comparison and merging:
//!
//! - `==` on [`Document`] is set-based. It ignores record order, attribute
//!   order, namespace tables, blank-node labels and the spelling of a
//!   number, boolean or `dateTime`.
//! - [`Document::unified`] merges records that share an identifier and kind,
//!   and returns [`Error::Unification`] when their formal attributes conflict
//!   or when PROV-CONSTRAINTS 53 to 55 forbid the sharing.
//! - [`provn::Profile`] selects how strictly PROV-N input is checked:
//!   `Strict` reads the whole grammar and the MUST statements of its prose,
//!   and `Default` adds the extensions that `ProvToolbox` and `ProvPy` write.
//!
//! `CONFORMANCE.md` in the repository lists where provkit departs from the
//! PROV Recommendations, where it differs from `ProvPy` 3.2.2, and its
//! implementation limits.
//!
//! # Features
//!
//! - `json` (default): the [`json`] module, with `serde` and `serde_json`.
//! - `provn` (default): the [`provn`] module.

/// Compiles the examples in `README.md` as doc tests.
#[cfg(all(doctest, feature = "json", feature = "provn"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

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
