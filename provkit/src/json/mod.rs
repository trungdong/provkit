//! PROV-JSON (W3C member submission, 2013) read and write.

mod read;
mod wire;
mod write;

pub use write::{to_string, write};

use crate::{Document, DocumentBuilder, Result, Sink};

/// Read a PROV-JSON document.
///
/// # Errors
///
/// Returns [`Error::Json`](crate::Error::Json) for malformed JSON, and the
/// other [`Error`](crate::Error) variants for names, literals or structure
/// that PROV-JSON does not allow.
pub fn read(text: &str) -> Result<Document> {
    let mut builder = DocumentBuilder::new();
    read_into(text, &mut builder)?;
    builder.finish()
}

/// Read PROV-JSON text, sending events to `sink`.
///
/// # Errors
///
/// As [`read`], plus any error the sink returns.
pub fn read_into(text: &str, sink: &mut impl Sink) -> Result<()> {
    let wire: wire::JsonDocument = serde_json::from_str(text)?;
    read::emit(&wire, sink)
}
