//! The crate's error type.

use crate::QName;

/// Every failure the crate can report.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The input text does not follow the format's grammar.
    #[error("{format} syntax error at line {line}, column {column}: {message}")]
    Syntax {
        /// `"PROV-N"` or `"PROV-JSON"`.
        format: &'static str,
        /// 1-based line.
        line: usize,
        /// 1-based column.
        column: usize,
        /// "expected X, found Y".
        message: String,
    },
    /// The input is well-formed but not a PROV document (unknown record kind,
    /// an element without an identifier, ...).
    #[error("{format}: {message}")]
    Structure {
        /// `"PROV-N"` or `"PROV-JSON"`.
        format: &'static str,
        /// What was wrong.
        message: String,
    },
    /// A prefix was used without a declaration. The empty string means a bare
    /// name with no default namespace.
    #[error("undeclared prefix `{0}`")]
    UndeclaredPrefix(String),
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
    /// The text is not a qualified name.
    #[error("`{0}` is not a valid qualified name")]
    InvalidQName(String),
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
