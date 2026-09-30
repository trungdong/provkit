//! The crate's error type.

use crate::{Kind, QName};

/// Every failure the crate can report.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// PROV-N text the grammar rejects.
    #[error("PROV-N syntax error at line {line}, column {column}: {message}")]
    Syntax {
        /// 1-based line.
        line: usize,
        /// 1-based column, in characters.
        column: usize,
        /// What the reader expected and what it found.
        message: String,
    },
    /// An error at a position in PROV-N text: a name, literal or record the
    /// reader could parse but not accept.
    #[error("line {line}, column {column}: {source}")]
    At {
        /// 1-based line.
        line: usize,
        /// 1-based column, in characters.
        column: usize,
        /// The error at that position.
        #[source]
        source: Box<Error>,
    },
    /// PROV-JSON that is valid JSON but not a PROV document.
    #[cfg(feature = "json")]
    #[error("PROV-JSON: {message}")]
    JsonStructure {
        /// What was wrong.
        message: String,
    },
    /// Parts that do not make a record of their kind.
    #[error("{}: {message}", .kind.json_key())]
    InvalidRecord {
        /// The kind being built.
        kind: Kind,
        /// What was wrong.
        message: String,
    },
    /// A name PROV-N cannot write.
    #[error("PROV-N cannot write the name {name}: {reason}")]
    Unwritable {
        /// The name, which displays as its IRI.
        name: QName,
        /// The character or rule that prevents it.
        reason: String,
    },
    /// A namespace declaration PROV-N cannot write.
    #[error("PROV-N cannot declare prefix `{prefix}` for <{iri}>: {reason}")]
    UnwritableNamespace {
        /// The prefix. The empty string stands for the default namespace.
        prefix: String,
        /// The namespace IRI.
        iri: String,
        /// The character or rule that prevents it.
        reason: String,
    },
    /// A prefix was used without a declaration.
    #[error("undeclared prefix `{0}`")]
    UndeclaredPrefix(String),
    /// A name without a prefix was used and no default namespace is declared.
    #[error("`{0}` has no prefix and no default namespace is declared")]
    NoDefaultNamespace(String),
    /// `prov` or `xsd` was declared to a different IRI.
    #[error("prefix `{prefix}` is reserved for <{expected}> and cannot be declared as <{found}>")]
    ReservedPrefix {
        /// The reserved prefix.
        prefix: String,
        /// Its fixed IRI.
        expected: String,
        /// The IRI the input tried to bind.
        found: String,
    },
    /// The text is not a `PN_PREFIX`.
    #[error("`{0}` is not a valid prefix")]
    InvalidPrefix(String),
    /// The text is not a qualified name.
    #[error("`{0}` is not a valid qualified name")]
    InvalidQName(String),
    /// The text is not a language tag.
    #[error("`{0}` is not a language tag")]
    InvalidLanguageTag(String),
    /// A literal's text is not valid for its datatype.
    #[error("invalid {datatype} literal `{lexical}`: {reason}")]
    InvalidLiteral {
        /// The datatype, as written.
        datatype: String,
        /// The text, as written.
        lexical: String,
        /// What was wrong.
        reason: String,
    },
    /// Records sharing an identifier cannot be merged.
    #[error("cannot unify records `{id}`: {reason}")]
    Unification {
        /// The shared identifier.
        id: QName,
        /// Which attribute or kinds conflict.
        reason: String,
    },
    /// Two bundles in one document carry the same identifier.
    #[error("duplicate bundle `{0}`")]
    DuplicateBundle(QName),
    /// JSON could not be parsed or printed.
    #[cfg(feature = "json")]
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// A writer's output stream failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// `Result` with the crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
