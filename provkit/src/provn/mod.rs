//! PROV-N (W3C Recommendation, 2013) read and write.

mod lexer;
mod parser;
mod write;

use crate::{Document, DocumentBuilder, Result, Sink};

pub use write::{statement, statement_name, to_string, write};

/// How closely the reader follows the Recommendation's grammar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Profile {
    /// The formal grammar only.
    Strict,
    /// The grammar plus bare `mentionOf` and `-` in required positions,
    /// which is what `ProvToolbox` and `ProvPy` write.
    #[default]
    Default,
}

/// Read a PROV-N document under [`Profile::Default`].
///
/// # Errors
///
/// Returns [`Error::Syntax`](crate::Error::Syntax) for text the grammar
/// rejects, and the other [`Error`](crate::Error) variants for names,
/// literals or structure that PROV does not allow.
pub fn read(text: &str) -> Result<Document> {
    read_with(text, Profile::Default)
}

/// Read a PROV-N document under `profile`.
///
/// # Errors
///
/// As [`read`].
pub fn read_with(text: &str, profile: Profile) -> Result<Document> {
    let mut builder = DocumentBuilder::new();
    read_into_with(text, profile, &mut builder)?;
    builder.finish()
}

/// Read PROV-N text under [`Profile::Default`], sending events to `sink` as
/// each statement is parsed.
///
/// # Errors
///
/// As [`read`], plus any error the sink returns.
pub fn read_into(text: &str, sink: &mut impl Sink) -> Result<()> {
    read_into_with(text, Profile::Default, sink)
}

/// Read PROV-N text under `profile`, sending events to `sink` as each
/// statement is parsed.
///
/// # Errors
///
/// As [`read`], plus any error the sink returns.
pub fn read_into_with(text: &str, profile: Profile, sink: &mut impl Sink) -> Result<()> {
    let tokens = lexer::tokenize(text)?;
    parser::Parser::new(&tokens, profile).document(sink)
}
