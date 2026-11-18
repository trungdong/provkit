//! The crate's error type.

use std::borrow::Cow;
use std::fmt;

use crate::{Formal, FormalAttr, Kind, QName};

/// The most characters of rejected input that a message quotes.
pub(crate) const QUOTED: usize = 80;

// ANCHOR: clip
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
// ANCHOR_END: clip

/// Every failure the crate can report.
///
/// The variant and its fields say what failed. The text that `Display`
/// prints is for a person to read and can change between releases, so
/// match on the variant and not on the text.
///
/// A variant is feature-gated only when its payload type comes from an
/// optional dependency: [`Error::Json`] holds a [`JsonError`]. Every
/// other variant, including the ones only one reader or writer produces,
/// exists under every feature set.
///
/// The enum and each of its struct variants are `#[non_exhaustive]`, so a
/// `match` in another crate needs a wildcard arm and a pattern on a struct
/// variant ends in `..`:
///
/// ```
/// fn line(e: &provkit::Error) -> Option<usize> {
///     match e {
///         provkit::Error::Syntax { line, .. } | provkit::Error::At { line, .. } => Some(*line),
///         _ => None,
///     }
/// }
/// ```
///
/// Without the `..` the pattern does not compile:
///
/// ```compile_fail,E0638
/// fn line(e: &provkit::Error) -> Option<usize> {
///     match e {
///         provkit::Error::Syntax { line, column, message } => Some(*line),
///         _ => None,
///     }
/// }
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// PROV-N text the grammar rejects.
    #[error("PROV-N syntax error at line {line}, column {column}: {message}")]
    #[non_exhaustive]
    Syntax {
        /// 1-based line.
        line: usize,
        /// 1-based column, in characters.
        column: usize,
        /// What the reader expected and what it found.
        message: String,
    },
    // ANCHOR: error_at
    /// An error at a position in PROV-N text: a name, literal or record the
    /// reader could parse but not accept.
    #[error("line {line}, column {column}: {error}")]
    #[non_exhaustive]
    At {
        /// 1-based line.
        line: usize,
        /// 1-based column, in characters.
        column: usize,
        /// The error at that position. `Display` prints it, so it is not
        /// also the `source()` of this error.
        error: Box<Error>,
    },
    // ANCHOR_END: error_at
    /// PROV-JSON that is valid JSON but not a PROV document.
    #[error("PROV-JSON: {message}")]
    #[non_exhaustive]
    JsonStructure {
        /// What was wrong, ending ` at line L column C` when the error has a
        /// position; the column is in characters.
        message: String,
    },
    // ANCHOR: error_in_record
    /// An error in one record of a PROV-JSON document, or in a bundle's key.
    /// Inside a bundle, the record's error sits in a second `InRecord`
    /// naming the bundle.
    #[error("{section} `{}`: {error}", clip(.key))]
    #[non_exhaustive]
    InRecord {
        /// The section the record is in: a kind's name, or `bundle`.
        section: String,
        /// The record's key, as written.
        key: String,
        /// The error in that record. `Display` prints it, so it is not also
        /// the `source()` of this error.
        error: Box<Error>,
    },
    // ANCHOR_END: error_in_record
    /// Parts that do not make a record of their kind.
    #[error("{}: {message}", .kind.name())]
    #[non_exhaustive]
    InvalidRecord {
        /// The kind being built.
        kind: Kind,
        /// What was wrong.
        message: String,
    },
    /// A name PROV-N cannot write, because its namespace IRI holds a
    /// character `IRI_REF` forbids.
    #[error("PROV-N cannot write the name {}: {reason}", clip(&.name.to_string()))]
    #[non_exhaustive]
    Unwritable {
        /// The name, which displays as its IRI.
        name: QName,
        /// The character that prevents it.
        reason: String,
    },
    /// A namespace IRI is empty or all whitespace. `Namespaces::declare`,
    /// `Namespaces::set_default`, `Namespaces::ensure_prefix` and
    /// `QName::new` refuse one.
    #[error("{}a namespace IRI cannot be empty or whitespace", blank_namespace(.prefix.as_deref()))]
    #[non_exhaustive]
    EmptyNamespace {
        /// The prefix whose declaration is at fault. The empty string stands
        /// for the default namespace, and `None` for a namespace that no
        /// declaration names.
        prefix: Option<String>,
    },
    /// A namespace declaration PROV-N cannot write, because the IRI holds a
    /// character `IRI_REF` forbids.
    #[error("PROV-N cannot declare {}: {reason}", declaration(.prefix, .iri))]
    #[non_exhaustive]
    UnwritableNamespace {
        /// The prefix. The empty string stands for the default namespace.
        prefix: String,
        /// The namespace IRI.
        iri: String,
        /// Why PROV-N cannot write the IRI.
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
    #[non_exhaustive]
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
    #[error("invalid {} literal `{}`: {reason}", short_name(.datatype), clip(.lexical))]
    #[non_exhaustive]
    InvalidLiteral {
        /// The datatype. The message prints it as `xsd:local` or
        /// `prov:local` under the reserved prefixes and as its IRI otherwise.
        datatype: QName,
        /// The text, as written.
        lexical: String,
        /// What was wrong.
        reason: String,
    },
    /// Records sharing an identifier cannot be merged.
    #[error("cannot unify records `{}`: {cause}", clip(&.id.to_string()))]
    #[non_exhaustive]
    Unification {
        /// The shared identifier.
        id: QName,
        /// Which kinds or which formal attribute conflict.
        cause: UnificationCause,
    },
    /// Two bundles in one document carry the same identifier.
    #[error("duplicate bundle `{}`", clip(&.0.to_string()))]
    DuplicateBundle(QName),
    /// JSON could not be parsed or printed.
    #[cfg(feature = "json")]
    #[error(transparent)]
    Json(JsonError),
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

/// The declaration at fault in [`Error::EmptyNamespace`], with its separator,
/// or nothing when no declaration is at fault.
fn blank_namespace(prefix: Option<&str>) -> String {
    match prefix {
        None => String::new(),
        Some("") => "the default namespace: ".to_owned(),
        Some(p) => format!("prefix `{}`: ", clip(p)),
    }
}

/// A datatype as a message prints it: `xsd:local` or `prov:local` under the
/// two reserved prefixes, the clipped IRI otherwise.
pub(crate) fn short_name(name: &QName) -> Cow<'_, str> {
    match name.namespace() {
        crate::well_known::XSD => Cow::Owned(format!("xsd:{}", name.local())),
        crate::well_known::PROV => Cow::Owned(format!("prov:{}", name.local())),
        _ => Cow::Owned(clip(&name.to_string()).into_owned()),
    }
}

/// Why two records under one identifier cannot be merged, the payload of
/// [`Error::Unification`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnificationCause {
    /// The records are of two kinds that PROV-CONSTRAINTS 53 to 55 keep
    /// apart under one identifier.
    #[non_exhaustive]
    Kinds {
        /// The kind seen first.
        first: Kind,
        /// The kind seen second.
        second: Kind,
    },
    /// One formal attribute holds two values that do not unify. `None` is
    /// an absent formal.
    #[non_exhaustive]
    Formal {
        /// The formal attribute.
        attr: FormalAttr,
        /// The value in the record seen first.
        first: Option<Formal>,
        /// The value in the record seen second.
        second: Option<Formal>,
    },
}

impl fmt::Display for UnificationCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnificationCause::Kinds { first, second } => write!(
                f,
                "{} and {} cannot share an identifier",
                first.name(),
                second.name()
            ),
            UnificationCause::Formal {
                attr,
                first,
                second,
            } => write!(
                f,
                "{attr} is {} in one record and {} in another",
                formal_text(first.as_ref()),
                formal_text(second.as_ref())
            ),
        }
    }
}

/// A formal value as a message prints it, clipped; `absent` for `None`.
fn formal_text(formal: Option<&Formal>) -> Cow<'_, str> {
    match formal {
        None => Cow::Borrowed("absent"),
        Some(Formal::Name(q)) => Cow::Owned(clip(&q.to_string()).into_owned()),
        Some(Formal::Time(t)) => Cow::Owned(clip(&t.to_string()).into_owned()),
    }
}

/// Text that is not JSON, or JSON that could not be printed: the payload of
/// [`Error::Json`].
///
/// A reader error has a position, a 1-based line and column counted in
/// characters, and prints as `serde_json`'s message followed by
/// ` at line L column C`. A writer error has no position. The `serde_json`
/// error is the [`source`](std::error::Error::source).
///
/// The type has no `From<serde_json::Error>`, so a caller's own `serde_json`
/// call does not convert into [`Error`] by `?`:
///
/// ```compile_fail,E0277
/// fn parse(text: &str) -> provkit::Result<serde_json::Value> {
///     Ok(serde_json::from_str(text)?)
/// }
/// ```
#[cfg(feature = "json")]
#[derive(Debug, thiserror::Error)]
#[error("{}{}", self.message(), self.suffix())]
pub struct JsonError {
    #[source]
    source: serde_json::Error,
    /// 1-based line and column in characters; `None` for a writer error.
    position: Option<(usize, usize)>,
}

#[cfg(feature = "json")]
impl JsonError {
    /// A reader error at a position already counted in characters.
    pub(crate) fn positioned(source: serde_json::Error, line: usize, column: usize) -> Self {
        JsonError {
            source,
            position: Some((line, column)),
        }
    }

    /// A writer error, which has no position.
    pub(crate) fn unpositioned(source: serde_json::Error) -> Self {
        JsonError {
            source,
            position: None,
        }
    }

    /// 1-based line, counted from the start of the input; `None` for a
    /// writer error.
    #[must_use]
    pub fn line(&self) -> Option<usize> {
        self.position.map(|(line, _)| line)
    }

    /// 1-based column in characters, counted from the start of the line;
    /// `None` for a writer error.
    #[must_use]
    pub fn column(&self) -> Option<usize> {
        self.position.map(|(_, column)| column)
    }

    /// `serde_json`'s message without the byte position it ends with.
    fn message(&self) -> String {
        let text = self.source.to_string();
        let end = text.rfind(" at line ").unwrap_or(text.len());
        text[..end].to_owned()
    }

    fn suffix(&self) -> String {
        match self.position {
            Some((line, column)) => format!(" at line {line} column {column}"),
            None => String::new(),
        }
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
                    datatype: crate::well_known::XSD_DATETIME.clone(),
                    lexical: long.clone(),
                    reason: "missing 'T'".into(),
                },
                &text_count,
            ),
            (
                Error::Unification {
                    id: huge_name.clone(),
                    cause: UnificationCause::Kinds {
                        first: Kind::Usage,
                        second: Kind::Generation,
                    },
                },
                &name_count,
            ),
            (
                Error::Unification {
                    id: QName::new("http://example.org/", "g").unwrap(),
                    cause: UnificationCause::Formal {
                        attr: FormalAttr::Activity,
                        first: Some(Formal::Name(huge_name.clone())),
                        second: None,
                    },
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
