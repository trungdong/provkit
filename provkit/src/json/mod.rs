//! PROV-JSON (W3C member submission, 2013) read and write.

mod read;
mod wire;
mod write;

pub use write::{to_string, write};

use crate::sink::{DocumentBuilder, Sink};
use crate::{Document, Result};

/// Read a PROV-JSON document.
///
/// # Errors
///
/// - [`Error::Json`](crate::Error::Json) for malformed JSON.
/// - [`Error::JsonStructure`](crate::Error::JsonStructure) for JSON that is
///   not a PROV document.
/// - The other [`Error`](crate::Error) variants for names, literals or
///   records that PROV does not allow.
pub fn read(text: &str) -> Result<Document> {
    let mut builder = DocumentBuilder::new();
    read_into(text, &mut builder)?;
    Ok(builder.finish())
}

/// Read PROV-JSON text, sending events to `sink`.
pub(crate) fn read_into(text: &str, sink: &mut impl Sink) -> Result<()> {
    let wire: wire::JsonDocument = serde_json::from_str(text)?;
    read::emit(&wire, sink)
}
