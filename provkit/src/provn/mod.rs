//! PROV-N (W3C Recommendation, 2013) read and write.

mod write;

// Task 15's `compare` renders bundle records with this table.
#[allow(unused_imports)]
pub(crate) use write::bundle_namespaces;
pub use write::{statement, to_string, write};
