//! W3C
//! [PROV data model](https://www.w3.org/TR/2013/REC-prov-dm-20130430/)
//! with
//! [PROV-JSON](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/)
//! and
//! [PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/)
//! read and write.
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
//!   keeps each record once, and returns [`Error::Unification`] when their
//!   formal attributes conflict, when an absent value in one of the four
//!   non-expandable positions of
//!   [PROV-CONSTRAINTS](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/)
//!   Table 3 meets a present
//!   one, or when PROV-CONSTRAINTS 53 to 55 forbid the sharing.
//! - [`provn::Profile`] selects how strictly PROV-N input is checked.
//!   `Strict` reads the whole grammar and the MUST statements of its prose,
//!   with the departures
//!   [CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
//!   records, and `Default` adds six extensions that the
//!   `default` profile of
//!   [Python `prov`](https://github.com/trungdong/prov)
//!   3.3.0 also reads, which that file lists.
//!
//! [CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
//! in the repository lists where provkit departs from the PROV
//! Recommendations, where it differs from Python `prov` 3.3.0 and from
//! `ProvToolbox` 2.5.0, and its implementation limits.
//!
//! # Features
//!
//! - `json` (default): the [`json`] module, with `serde` and `serde_json`.
//! - `provn` (default): the [`provn`] module.

// Several crate-private helpers serve one format only. With either format
// switched off they have no caller, which is not dead code in the default
// build, where the lint stays on.
#![cfg_attr(
    not(all(feature = "json", feature = "provn")),
    allow(dead_code, unused_imports)
)]

/// Compiles the examples in `README.md` as doc tests.
#[cfg(all(doctest, feature = "json", feature = "provn"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// ANCHOR: modules
mod attributes;
mod builder;
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
mod unify;
mod value;
// ANCHOR_END: modules

#[cfg(test)]
pub(crate) mod test_util;

// ANCHOR: exports
pub use attributes::{Attributes, IntoIter as AttributesIntoIter, Iter as AttributesIter};
pub use datetime::DateTime;
pub use document::{Bundle, Document};
#[cfg(feature = "json")]
pub use error::JsonError;
pub use error::{Error, Result, UnificationCause};
pub use namespaces::Namespaces;
pub use qname::{QName, well_known};
pub use record::{
    Activity, Agent, Alternate, Association, Attribution, Communication, Delegation, Derivation,
    End, Entity, Formal, FormalAttr, FormalRef, Generation, Influence, Invalidation, Kind,
    Membership, Mention, Record, Specialization, Start, Usage,
};
pub use value::{LangStr, TypedLiteral, Value};
// ANCHOR_END: exports

/// `text` without one leading byte-order mark.
pub(crate) fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}
