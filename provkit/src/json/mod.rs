//! PROV-JSON (W3C member submission, 2013) read and write.

mod read;
mod wire;
mod write;

pub use write::{to_string, write};

use crate::sink::DocumentBuilder;
use crate::{Document, Result, strip_bom};

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
    read_into(strip_bom(text), &mut builder)?;
    Ok(builder.finish())
}

/// Read PROV-JSON text, sending events to `builder`. On an error the builder
/// has already received the events that came before it.
pub(crate) fn read_into(text: &str, builder: &mut DocumentBuilder) -> Result<()> {
    let wire: wire::JsonDocument = serde_json::from_str(text)?;
    read::emit(&wire, builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_byte_order_mark_is_ignored() {
        let doc = read("\u{FEFF}{\"prefix\": {\"ex\": \"http://e/\"}, \"entity\": {\"ex:e\": {}}}")
            .unwrap();
        assert_eq!(doc.records.len(), 1);
    }

    #[test]
    fn a_byte_order_mark_elsewhere_is_not_stripped() {
        for text in ["\u{FEFF}\u{FEFF}{}", " \u{FEFF}{}", "{}\u{FEFF}"] {
            assert!(read(text).is_err(), "{text:?}");
        }
    }
}
