//! PROV-JSON (W3C member submission, 2013) read and write.

mod read;
mod wire;
mod write;

pub use write::{to_string, write};

use crate::sink::DocumentBuilder;
use crate::{Document, Error, Result, strip_bom};

/// Read a PROV-JSON document.
///
/// # Errors
///
/// - [`Error::Json`] for text that is not JSON.
/// - [`Error::JsonStructure`] for JSON that is
///   not a PROV document. The message ends with the line and column where
///   the reader knows them.
/// - [`Error::InRecord`] around the error for a
///   name, literal or record that PROV does not allow, naming the record.
pub fn read(text: &str) -> Result<Document> {
    let wire: wire::JsonDocument = serde_json::from_str(strip_bom(text)).map_err(shape_error)?;
    let mut builder = DocumentBuilder::new();
    read::emit(&wire, &mut builder)?;
    Ok(builder.finish())
}

/// `serde_json` reports JSON of the wrong shape as a data error. That is
/// valid JSON which is not PROV-JSON, so it is a structure error here.
fn shape_error(e: serde_json::Error) -> Error {
    if e.classify() == serde_json::error::Category::Data {
        Error::JsonStructure {
            message: e.to_string(),
        }
    } else {
        Error::Json(e)
    }
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
