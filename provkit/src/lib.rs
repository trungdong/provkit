//! W3C PROV data model with PROV-JSON and PROV-N read and write.
//!
//! The crate follows the PROV Recommendations and the member submissions,
//! and is tested against the shared conformance corpus.

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
pub use qname::QName;
pub use record::{
    Activity, Agent, Alternate, Association, Attribution, Communication, Delegation, Derivation,
    End, Entity, Formal, FormalAttr, FormalRef, Generation, Influence, Invalidation, Kind,
    Membership, Mention, Record, Specialization, Start, Usage,
};
pub use sink::{DocumentBuilder, Sink};
pub use value::{LangStr, Value};
