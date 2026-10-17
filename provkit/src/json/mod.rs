//! PROV-JSON (W3C member submission, 2013) read and write.
//!
//! Every position in an error from this module is a 1-based line and a
//! 1-based column counted in characters.

mod read;
mod wire;
mod write;

pub use write::{to_string, write};

use crate::builder::DocumentBuilder;
use crate::error::QUOTED;
use crate::{Document, Error, JsonError, Result, strip_bom};

// ANCHOR: json_read
/// Read a PROV-JSON document.
///
/// # Errors
///
/// - [`Error::Json`] for text the JSON parser rejects, holding a
///   [`JsonError`] with the line and column, in characters, where parsing
///   stopped.
/// - [`Error::JsonStructure`] for JSON that is not a PROV document. An
///   error found while reading the JSON ends with the line and column of
///   the offending token, the column counted in characters, as the PROV-N
///   reader counts it. An attribute value the reader cannot decode is
///   such an error, a string with a `\u` escape that names a lone surrogate
///   included. For a key the reader refuses (an unknown section, a key
///   given twice in one object, a `bundle` inside a bundle) the position is
///   where `serde_json` puts an error raised once the key's value has been
///   read: past the white space after the value, and past a closing brace
///   that follows it. An unknown member of a value object is placed at its
///   key, and a bundle key given twice is the exception below. An error found while building a record (a
///   formal of the wrong form, a blank identifier on an element or bundle)
///   carries no position.
/// - [`Error::InRecord`] wrapping an error inside a record: a bad attribute
///   value, a repeated attribute or value-object key, a formal of the wrong
///   form, a blank identifier, or a name or literal PROV does not allow. It
///   names the section and the record's key, which locates an error that
///   carries no position. Inside a bundle the record's error is wrapped once
///   more in an `InRecord` whose section is `bundle` and whose key is the
///   bundle's, so the message reads ``bundle `ex:b`: entity `ex:e`: ...``.
/// - [`Error::DuplicateBundle`], inside [`Error::InRecord`], for two bundles
///   with one identifier, a bundle key given twice included. The value of a
///   repeated bundle key is checked only as JSON, not as a bundle, so the
///   repeat is reported whatever that value holds, unless it is not JSON. A bundle key that repeats, does not
///   resolve or has a bad namespace table is reported after the document
///   body and before any fault inside a bundle that the JSON parser does not
///   itself refuse. A fault the parser refuses, such as text that is not
///   JSON, an unknown section or a key repeated among the sections, is
///   reported in the order of the text, ahead of every other fault.
/// - [`Error::InvalidPrefix`] and [`Error::ReservedPrefix`] for a `prefix`
///   entry that PROV-N could not write, returned as they are.
pub fn read(text: &str) -> Result<Document> {
    let text = strip_bom(text);
    let wire: wire::JsonDocument = serde_json::from_str(text).map_err(|e| shape_error(e, text))?;
    let mut builder = DocumentBuilder::new();
    read::emit(&wire, text, &mut builder)?;
    Ok(builder.finish())
}
// ANCHOR_END: json_read

/// `serde_json` reports JSON of the wrong shape as a data error. That is
/// valid JSON which is not PROV-JSON, so it is a structure error here. Any
/// other error is text that is not JSON. Either way the position is
/// re-counted in characters.
fn shape_error(e: serde_json::Error, text: &str) -> Error {
    let (line, column) = (e.line(), char_column(text, e.line(), e.column()));
    if e.classify() == serde_json::error::Category::Data {
        return structure(format!(
            "{} at line {line} column {column}",
            without_position(&e)
        ));
    }
    Error::Json(JsonError::positioned(e, line, column))
}

/// The character column that `serde_json`'s byte column names on `line`
/// of `text`, both 1-based: the number of characters in the part of that
/// line the byte column covers. A byte column inside a multi-byte
/// character counts that character, because the column covers its first
/// byte.
pub(super) fn char_column(text: &str, line: usize, byte_column: usize) -> usize {
    let line_text = text.split('\n').nth(line.saturating_sub(1)).unwrap_or("");
    let mut end = byte_column.min(line_text.len());
    while !line_text.is_char_boundary(end) {
        end += 1;
    }
    line_text[..end].chars().count()
}

/// The message of a `serde_json` error without the ` at line L column C`
/// it ends with. A string the message quotes is cut to the length every
/// message keeps to.
pub(super) fn without_position(e: &serde_json::Error) -> String {
    let message = e.to_string();
    let end = message.rfind(" at line ").unwrap_or(message.len());
    clip_quoted(&message[..end])
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
