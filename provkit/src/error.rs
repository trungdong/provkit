//! The crate's error type.

use crate::{Kind, QName};

/// Every failure the crate can report.
///
/// A variant is feature-gated only when its payload type comes from an
/// optional dependency: [`Error::Json`] holds a `serde_json::Error`. Every
/// other variant, including the ones only one reader or writer produces,
/// exists under every feature set.
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
    #[error("line {line}, column {column}: {error}")]
    At {
        /// 1-based line.
        line: usize,
        /// 1-based column, in characters.
        column: usize,
        /// The error at that position. `Display` prints it, so it is not
        /// also the `source()` of this error.
        error: Box<Error>,
    },
    /// PROV-JSON that is valid JSON but not a PROV document.
    #[error("PROV-JSON: {message}")]
    JsonStructure {
        /// What was wrong.
        message: String,
    },
    /// Parts that do not make a record of their kind.
    #[error("{}: {message}", .kind.name())]
    InvalidRecord {
        /// The kind being built.
        kind: Kind,
        /// What was wrong.
        message: String,
    },
    /// A name PROV-N cannot write, because its namespace IRI holds a
    /// character `IRI_REF` forbids.
    #[error("PROV-N cannot write the name {name}: {reason}")]
    Unwritable {
        /// The name, which displays as its IRI.
        name: QName,
        /// The character that prevents it.
        reason: String,
    },
    /// A namespace declaration PROV-N cannot write, because the IRI holds a
    /// character `IRI_REF` forbids.
    #[error("PROV-N cannot declare {}: {reason}", declaration(.prefix, .iri))]
    UnwritableNamespace {
        /// The prefix. The empty string stands for the default namespace.
        prefix: String,
        /// The namespace IRI.
        iri: String,
        /// The character that prevents it.
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
    /// The text is not a qualified name. The payload is the whole text as
    /// given, for example `ex:a b`, not the local part alone.
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

/// A namespace declaration as [`Error::UnwritableNamespace`] names it. An
/// empty `prefix` is the default namespace.
fn declaration(prefix: &str, iri: &str) -> String {
    if prefix.is_empty() {
        format!("the default namespace <{iri}>")
    } else {
        format!("prefix `{prefix}` for <{iri}>")
    }
}

/// `Result` with the crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_prints_its_error_and_has_no_source() {
        let e = Error::At {
            line: 3,
            column: 8,
            error: Box::new(Error::UndeclaredPrefix("zz".into())),
        };
        assert_eq!(e.to_string(), "line 3, column 8: undeclared prefix `zz`");
        assert!(std::error::Error::source(&e).is_none());
    }

    #[test]
    fn unwritable_namespace_names_the_prefix_or_the_default_namespace() {
        let error = |prefix: &str| Error::UnwritableNamespace {
            prefix: prefix.into(),
            iri: "http://x.org/>".into(),
            reason: "the namespace IRI contains '>' (U+003E)".into(),
        };
        assert_eq!(
            error("p").to_string(),
            "PROV-N cannot declare prefix `p` for <http://x.org/>>: \
             the namespace IRI contains '>' (U+003E)"
        );
        assert_eq!(
            error("").to_string(),
            "PROV-N cannot declare the default namespace <http://x.org/>>: \
             the namespace IRI contains '>' (U+003E)"
        );
    }
}
