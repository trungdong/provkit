//! PROV-N (W3C Recommendation, 2013) read and write.

mod chars;
mod extension;
mod lexer;
mod parser;
mod write;

use crate::sink::DocumentBuilder;
use crate::{Document, Result, strip_bom};

pub use write::{name, statement, to_string, write};

/// How closely the reader follows the Recommendation's grammar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Profile {
    /// The whole grammar of the Recommendation and the MUST statements of
    /// its prose, with the departures `CONFORMANCE.md` records. `Strict`
    /// rejects:
    ///
    /// - `mentionOf` without the `prov:` prefix.
    /// - `-` in a required position of a relation.
    /// - `default` after a `prefix` (production \[45\]).
    /// - A statement after a bundle at document level (production \[1\]).
    /// - An identifier or attributes on `alternateOf`, `specializationOf` and
    ///   `hadMember` (productions \[24\] to \[26\]), and an identifier on
    ///   `mentionOf`.
    /// - A declaration of `prov` or `xsd` at its predeclared IRI (section
    ///   3.7.4).
    ///
    /// `prov:mentionOf` (PROV-LINKS) is a mention only when three names
    /// follow it, with or without an attribute block. Under `Strict` an
    /// identifier or any other argument makes it an extensibility
    /// expression (production \[49\]), parsed and left out of the document.
    ///
    /// Both profiles reject a repeated `default`, a prefix that is not a
    /// `PN_PREFIX`, and every other text the grammar rejects.
    Strict,
    /// The grammar plus the six extensions that `Strict` rejects, which
    /// `ProvToolbox` and `ProvPy` write. `prov:mentionOf` with an identifier
    /// is a mention under `Default`, as bare `mentionOf` is.
    #[default]
    Default,
}

/// An extensibility expression the reader parsed and left out of the document.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Ignored {
    /// The expression's name as written, for example `dictExt:hadMembers`.
    pub name: String,
    /// 1-based line of the name.
    pub line: usize,
    /// 1-based column of the name.
    pub column: usize,
}

/// Read a PROV-N document under [`Profile::Default`].
///
/// An extensibility expression (Recommendation section 5, a statement whose
/// keyword has a prefix, other than `prov:mentionOf` in the shape of a
/// mention; see [`Profile`]) is parsed and left out of the document.
/// [`read_report`] lists the ones left out.
///
/// # Errors
///
/// Every error carries a line and column:
///
/// - [`Error::Syntax`](crate::Error::Syntax) for text the grammar rejects.
/// - [`Error::At`](crate::Error::At) for a name, literal or record that
///   parses but that PROV does not allow. Its `error` field is the error at
///   that position, such as [`Error::UndeclaredPrefix`](crate::Error::UndeclaredPrefix)
///   or [`Error::InvalidRecord`](crate::Error::InvalidRecord).
pub fn read(text: &str) -> Result<Document> {
    read_with(text, Profile::Default)
}

/// Read a PROV-N document under `profile`.
///
/// # Errors
///
/// As [`read`].
pub fn read_with(text: &str, profile: Profile) -> Result<Document> {
    read_report(text, profile).map(|(document, _)| document)
}

/// Read a PROV-N document under `profile`, and list the extensibility
/// expressions it holds. PROV defines no meaning for them, so they are
/// not in the document.
///
/// The list holds one entry per outermost expression, in source order.
///
/// # Errors
///
/// As [`read`].
pub fn read_report(text: &str, profile: Profile) -> Result<(Document, Vec<Ignored>)> {
    let mut builder = DocumentBuilder::new();
    let ignored = read_into_with(strip_bom(text), profile, &mut builder)?;
    Ok((builder.finish(), ignored))
}

/// Read PROV-N text under `profile`, sending events to `builder` as each
/// statement is parsed. Returns the extensibility expressions left out. On an
/// error the builder has already received the events that came before it.
pub(crate) fn read_into_with(
    text: &str,
    profile: Profile,
    builder: &mut DocumentBuilder,
) -> Result<Vec<Ignored>> {
    let tokens = lexer::tokenize(text)?;
    let mut parser = parser::Parser::new(text, &tokens, profile);
    parser.document(builder)?;
    Ok(parser.ignored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_byte_order_mark_is_ignored() {
        let with_bom = "\u{FEFF}document prefix ex <http://e/> entity(ex:e) endDocument";
        assert_eq!(read(with_bom).unwrap().records.len(), 1);
        assert_eq!(
            read_with(with_bom, Profile::Strict).unwrap().records.len(),
            1
        );
    }

    #[test]
    fn a_byte_order_mark_elsewhere_is_not_stripped() {
        // U+FEFF is a name character, so a second mark or a mark after
        // whitespace spoils the `document` keyword.
        for text in [
            "\u{FEFF}\u{FEFF}document endDocument",
            " \u{FEFF}document endDocument",
            "document \u{FEFF}endDocument",
        ] {
            assert!(read(text).is_err(), "{text:?}");
        }
    }
}
