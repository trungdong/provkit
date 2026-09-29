//! W3C PROV data model with PROV-JSON and PROV-N read and write.
//!
//! The crate follows the PROV Recommendations and the member submissions,
//! and is tested against the shared conformance corpus.

pub mod datetime;
pub mod error;
pub mod namespaces;
pub mod qname;

#[cfg(test)]
pub(crate) mod test_util;

pub use datetime::DateTime;
pub use error::{Error, Result};
pub use namespaces::{Namespaces, Scope};
pub use qname::QName;
