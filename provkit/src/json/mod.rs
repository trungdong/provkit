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
/// - [`Error::Json`] for text the JSON parser rejects.
/// - [`Error::JsonStructure`] for JSON that is not a PROV document. An
///   error found while reading the JSON ends with the line and column of
///   the offending token. For a key the reader refuses (an unknown section,
///   a repeated `prefix` or `bundle`, a `bundle` inside a bundle) the
///   position is just past that key's value. An attribute value the reader
///   cannot decode is such an error, a string with a `\u` escape that names
///   a lone surrogate included. An error found while building a record (a
///   formal of the wrong form, a blank identifier on an element or bundle)
///   carries no position.
/// - [`Error::InRecord`] wrapping an error inside a record: a bad attribute
///   value, a formal of the wrong form, a blank identifier, or a name or
///   literal PROV does not allow. It names the section and the record's
///   key, which locates an error that carries no position.
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
        structure(clip_quoted(&e.to_string()))
    } else {
        Error::Json(e)
    }
}

fn structure(message: impl Into<String>) -> Error {
    Error::JsonStructure {
        message: message.into(),
    }
}

/// A serde message with the string it quotes cut to the length every
/// message keeps to. serde quotes a string it did not expect whole, between
/// double quotes and in front of `, expected` (`invalid type: string "...",
/// expected a map`).
///
/// serde prints the string escaped, so its true length is not known here
/// and the cut is marked with `...` alone. A backslash at the cut that is
/// half an escape is dropped: the last of an odd run of backslashes.
fn clip_quoted(message: &str) -> String {
    let quoted = message.rfind("\", expected").and_then(|close| {
        let open = message[..close].find('"')? + 1;
        let (cut, _) = message[open..close].char_indices().nth(QUOTED)?;
        let kept = &message[open..open + cut];
        let backslashes = kept.len() - kept.trim_end_matches('\\').len();
        Some((open + cut - backslashes % 2, close))
    });
    match quoted {
        Some((cut, close)) => format!("{}...{}", &message[..cut], &message[close..]),
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
        // Whole pairs of backslashes stay: 200 of them are escaped as 400,
        // and the cut after 80 splits no pair.
        let backslashes = "\\".repeat(400);
        assert_eq!(
            clip_quoted(&format!(
                "invalid type: string \"{backslashes}\", expected a map"
            )),
            format!(
                "invalid type: string \"{}...\", expected a map",
                "\\".repeat(80)
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
