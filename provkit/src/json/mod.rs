//! PROV-JSON (W3C member submission, 2013) read and write.

mod read;
mod wire;
mod write;

pub use write::{to_string, write};

use crate::builder::DocumentBuilder;
use crate::error::QUOTED;
use crate::{Document, Error, Result, strip_bom};

/// Read a PROV-JSON document.
///
/// # Errors
///
/// - [`Error::Json`] for text that is not JSON, wherever it is.
/// - [`Error::JsonStructure`] for JSON that is not a PROV document. The
///   message ends with the line and column of the offending token where
///   the reader knows them.
/// - [`Error::InRecord`] wrapping the error for an attribute value, name,
///   literal or record PROV does not allow, and naming the record.
/// - [`Error::InvalidPrefix`] and [`Error::ReservedPrefix`] for a `prefix`
///   entry that PROV-N could not write, returned as they are.
pub fn read(text: &str) -> Result<Document> {
    let text = strip_bom(text);
    let wire: wire::JsonDocument = serde_json::from_str(text).map_err(shape_error)?;
    let mut builder = DocumentBuilder::new();
    read::emit(&wire, text, &mut builder)?;
    Ok(builder.finish())
}

/// `serde_json` reports JSON of the wrong shape as a data error. That is
/// valid JSON which is not PROV-JSON, so it is a structure error here.
fn shape_error(e: serde_json::Error) -> Error {
    if e.classify() == serde_json::error::Category::Data {
        Error::JsonStructure {
            message: clip_quoted(&e.to_string()),
        }
    } else {
        Error::Json(e)
    }
}

/// A serde message with the string it quotes cut to the length every
/// message keeps to. serde quotes a string it did not expect whole, between
/// double quotes and in front of `, expected` (`invalid type: string "...",
/// expected a map`).
///
/// serde prints the string escaped, so its true length is not known here
/// and the cut is marked with `...` alone. A backslash at the cut is
/// dropped, because it would be half an escape.
fn clip_quoted(message: &str) -> String {
    let quoted = message.rfind("\", expected").and_then(|close| {
        let open = message[..close].find('"')? + 1;
        let (cut, _) = message[open..close].char_indices().nth(QUOTED)?;
        Some((open + cut, close))
    });
    match quoted {
        Some((cut, close)) => format!(
            "{}...{}",
            message[..cut].trim_end_matches('\\'),
            &message[close..]
        ),
        None => message.to_owned(),
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
    fn a_quoted_string_in_a_serde_message_is_clipped() {
        let long = "x".repeat(200);
        let eighty = "x".repeat(80);
        assert_eq!(
            clip_quoted(&format!(
                "invalid type: string \"{long}\", expected a map at line 1 column 9"
            )),
            format!("invalid type: string \"{eighty}...\", expected a map at line 1 column 9")
        );
        // The quoted string may itself hold a quote and the word `expected`.
        let tricky = format!("a\\\", expected {long}");
        assert_eq!(
            clip_quoted(&format!(
                "invalid type: string \"{tricky}\", expected a map"
            )),
            format!(
                "invalid type: string \"{}...\", expected a map",
                &tricky[..80]
            )
        );
        // A backslash where the cut falls is dropped with what follows it.
        let escaped = format!("{}\\\"{long}", "x".repeat(79));
        assert_eq!(
            clip_quoted(&format!(
                "invalid type: string \"{escaped}\", expected a map"
            )),
            format!(
                "invalid type: string \"{}...\", expected a map",
                "x".repeat(79)
            )
        );
        // A short string, and a message that quotes nothing, are unchanged.
        for whole in [
            "invalid type: string \"abc\", expected a map",
            "invalid type: integer `3`, expected a map",
            "a value object needs a `$` key",
        ] {
            assert_eq!(clip_quoted(whole), whole);
        }
    }

    #[test]
    fn a_byte_order_mark_elsewhere_is_not_stripped() {
        for text in ["\u{FEFF}\u{FEFF}{}", " \u{FEFF}{}", "{}\u{FEFF}"] {
            assert!(read(text).is_err(), "{text:?}");
        }
    }
}
