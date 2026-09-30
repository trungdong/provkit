//! Character classes that only the PROV-N reader and writer use.

/// Whether `IRI_REF` forbids `c` between its angle brackets: a control
/// character, a space, or one of `< > " { } | ^ ` \`.
pub(crate) fn is_iri_forbidden(c: char) -> bool {
    c <= ' ' || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '^' | '`' | '\\')
}

/// The first character of `iri` that [`is_iri_forbidden`].
pub(crate) fn iri_forbidden(iri: &str) -> Option<char> {
    iri.chars().find(|&c| is_iri_forbidden(c))
}
