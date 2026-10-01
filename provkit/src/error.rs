//! The crate's error type.

use std::borrow::Cow;

use crate::{Kind, QName};

/// The most characters of rejected input that a message quotes.
pub(crate) const QUOTED: usize = 80;

/// `text` as a message quotes it: whole when short, otherwise its first
/// characters and its length. The error's payload keeps the whole text.
pub(crate) fn clip(text: &str) -> Cow<'_, str> {
    match text.char_indices().nth(QUOTED) {
        None => Cow::Borrowed(text),
        Some((end, _)) => Cow::Owned(format!(
            "{}... ({} characters)",
            &text[..end],
            text.chars().count()
        )),
    }
}

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
    /// An error in one record of a PROV-JSON document, or in a bundle's key.
    #[error("{section} `{}`: {error}", clip(.key))]
    InRecord {
        /// The section the record is in: a kind's name, or `bundle`.
        section: String,
        /// The record's key, as written.
        key: String,
        /// The error in that record. `Display` prints it, so it is not also
        /// the `source()` of this error.
        error: Box<Error>,
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
    #[error("PROV-N cannot write the name {}: {reason}", clip(&.name.to_string()))]
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
    #[error("undeclared prefix `{}`", clip(.0))]
    UndeclaredPrefix(String),
    /// A name without a prefix was used and no default namespace is declared.
    #[error("`{}` has no prefix and no default namespace is declared", clip(.0))]
    NoDefaultNamespace(String),
    /// `prov` or `xsd` was declared to a different IRI.
    #[error(
        "prefix `{prefix}` is reserved for <{expected}> and cannot be declared as <{}>",
        clip(.found)
    )]
    ReservedPrefix {
        /// The reserved prefix.
        prefix: String,
        /// Its fixed IRI.
        expected: String,
        /// The IRI the input tried to bind.
        found: String,
    },
    /// The text is not a `PN_PREFIX`.
    #[error("`{}` is not a valid prefix", clip(.0))]
    InvalidPrefix(String),
    /// The text is not a qualified name. The payload is the whole text as
    /// given, for example `ex:a b`, not the local part alone.
    #[error("`{}` is not a valid qualified name", clip(.0))]
    InvalidQName(String),
    /// The text is not a language tag.
    #[error("`{}` is not a language tag", clip(.0))]
    InvalidLanguageTag(String),
    /// A literal's text is not valid for its datatype.
    #[error("invalid {datatype} literal `{}`: {reason}", clip(.lexical))]
    InvalidLiteral {
        /// The datatype, as `prefix:local` under its usual prefix (`xsd:dateTime`).
        datatype: String,
        /// The text, as written.
        lexical: String,
        /// What was wrong.
        reason: String,
    },
    /// Records sharing an identifier cannot be merged.
    #[error("cannot unify records `{}`: {reason}", clip(&.id.to_string()))]
    Unification {
        /// The shared identifier.
        id: QName,
        /// Which attribute or kinds conflict.
        reason: String,
    },
    /// Two bundles in one document carry the same identifier.
    #[error("duplicate bundle `{}`", clip(&.0.to_string()))]
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
    let iri = clip(iri);
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
    fn clip_keeps_short_text_and_cuts_long_text_on_a_character() {
        assert_eq!(clip("short"), "short");
        let eighty = "é".repeat(80);
        assert_eq!(clip(&eighty), eighty);
        let long = "é".repeat(81);
        assert_eq!(clip(&long), format!("{eighty}... (81 characters)"));
    }

    #[test]
    fn messages_quote_at_most_eighty_characters() {
        let long = "x".repeat(1 << 20);
        let huge_name = QName::new("http://example.org/", &long).unwrap();
        // A name displays as its whole IRI, which is longer than its local part.
        let name_count = format!("({} characters)", huge_name.to_string().chars().count());
        let text_count = "(1048576 characters)".to_owned();
        // Each error with the length its message must report.
        let errors = [
            (Error::InvalidQName(long.clone()), &text_count),
            (Error::InvalidPrefix(long.clone()), &text_count),
            (Error::InvalidLanguageTag(long.clone()), &text_count),
            (Error::UndeclaredPrefix(long.clone()), &text_count),
            (Error::NoDefaultNamespace(long.clone()), &text_count),
            (
                Error::InvalidLiteral {
                    datatype: "xsd:dateTime".into(),
                    lexical: long.clone(),
                    reason: "missing 'T'".into(),
                },
                &text_count,
            ),
            (
                Error::Unification {
                    id: huge_name.clone(),
                    reason: "kinds differ".into(),
                },
                &name_count,
            ),
            (Error::DuplicateBundle(huge_name.clone()), &name_count),
            (
                Error::Unwritable {
                    name: huge_name,
                    reason: "the namespace IRI contains '>' (U+003E)".into(),
                },
                &name_count,
            ),
            (
                Error::InRecord {
                    section: "entity".into(),
                    key: long.clone(),
                    error: Box::new(Error::InvalidQName("a b".into())),
                },
                &text_count,
            ),
            (
                Error::ReservedPrefix {
                    prefix: "prov".into(),
                    expected: "http://www.w3.org/ns/prov#".into(),
                    found: long.clone(),
                },
                &text_count,
            ),
            (
                Error::UnwritableNamespace {
                    prefix: "p".into(),
                    iri: long.clone(),
                    reason: "the namespace IRI contains '>' (U+003E)".into(),
                },
                &text_count,
            ),
            (
                Error::UnwritableNamespace {
                    prefix: String::new(),
                    iri: long.clone(),
                    reason: "the namespace IRI contains '>' (U+003E)".into(),
                },
                &text_count,
            ),
        ];
        for (e, count) in errors {
            let text = e.to_string();
            assert!(text.len() < 300, "{} bytes", text.len());
            assert!(text.contains(count.as_str()), "{text}");
        }
    }

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
    fn in_record_prints_its_error_and_has_no_source() {
        let e = Error::InRecord {
            section: "entity".into(),
            key: "zz:e".into(),
            error: Box::new(Error::UndeclaredPrefix("zz".into())),
        };
        assert_eq!(e.to_string(), "entity `zz:e`: undeclared prefix `zz`");
        assert!(std::error::Error::source(&e).is_none());
    }

    #[test]
    fn the_remaining_messages_read_as_declared() {
        let name = QName::new("http://example.org/", "a").unwrap();
        assert_eq!(
            Error::DuplicateBundle(name.clone()).to_string(),
            "duplicate bundle `http://example.org/a`"
        );
        assert_eq!(
            Error::InvalidPrefix("1x".into()).to_string(),
            "`1x` is not a valid prefix"
        );
        assert_eq!(
            Error::ReservedPrefix {
                prefix: "prov".into(),
                expected: "http://www.w3.org/ns/prov#".into(),
                found: "http://x.org/".into(),
            }
            .to_string(),
            "prefix `prov` is reserved for <http://www.w3.org/ns/prov#> \
             and cannot be declared as <http://x.org/>"
        );
        assert_eq!(
            Error::InvalidLanguageTag("e n".into()).to_string(),
            "`e n` is not a language tag"
        );
        assert_eq!(
            Error::Unwritable {
                name,
                reason: "the namespace IRI contains '>' (U+003E)".into(),
            }
            .to_string(),
            "PROV-N cannot write the name http://example.org/a: \
             the namespace IRI contains '>' (U+003E)"
        );
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
