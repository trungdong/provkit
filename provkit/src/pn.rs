//! Character classes of the PROV-N name grammar (Recommendation section
//! 3.8, with `PN_PREFIX`, `PN_CHARS_BASE`, `PN_CHARS_U` and `PN_CHARS` from
//! SPARQL).

/// Characters that may follow `\` in a local part: `PN_CHARS_ESC` [55] plus
/// the SPARQL escapes `_ ~ ! $ & * + / ? # @ %`.
pub(crate) const PN_CHARS_ESC: &str = "_~.-!$&'()*+,;=/?#@%:[]";

/// The Recommendation's `PN_CHARS_ESC` [55], which a local part holds only
/// after `\`.
const ALWAYS_ESCAPED: &str = "='(),-:;[].";

/// `PN_CHARS_BASE`.
pub(crate) fn is_pn_chars_base(c: char) -> bool {
    matches!(c,
        'A'..='Z' | 'a'..='z' | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

/// `PN_CHARS_U`.
pub(crate) fn is_pn_chars_u(c: char) -> bool {
    is_pn_chars_base(c) || c == '_'
}

/// `PN_CHARS`.
pub(crate) fn is_pn_chars(c: char) -> bool {
    is_pn_chars_u(c)
        || c == '-'
        || c.is_ascii_digit()
        || c == '\u{B7}'
        || matches!(c, '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
}

/// The plain characters of `PN_CHARS_OTHERS` [54]; `%` and `\` escapes are
/// handled separately.
pub(crate) fn is_pn_local_other(c: char) -> bool {
    matches!(c, '/' | '@' | '~' | '&' | '+' | '*' | '?' | '#' | '$' | '!')
}

/// Length of a `PN_PREFIX` at the start of `s`, which cannot end in `.`.
pub(crate) fn prefix_len(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    let (_, first) = chars.next()?;
    if !is_pn_chars_base(first) {
        return None;
    }
    let mut end = first.len_utf8();
    for (i, c) in chars {
        if is_pn_chars(c) {
            end = i + c.len_utf8();
        } else if c != '.' {
            break;
        }
    }
    Some(end)
}

/// Whether all of `s` is one `PN_PREFIX`.
pub(crate) fn is_pn_prefix(s: &str) -> bool {
    prefix_len(s) == Some(s.len())
}

/// How PROV-N writes one character of a local part.
pub(crate) enum LocalChar {
    /// As it is.
    Bare,
    /// After a `\`.
    Escaped,
}

/// How PROV-N writes the character `c` at byte `at` of `local`, or `None`
/// when no `PN_LOCAL` [54] holds it there. A `%` is held only before two hex
/// digits, because the reader keeps `%XX` verbatim.
pub(crate) fn local_char(local: &str, at: usize, c: char) -> Option<LocalChar> {
    let first = at == 0;
    if ALWAYS_ESCAPED.contains(c) {
        Some(LocalChar::Escaped)
    } else if c == '%' {
        let hex = local.as_bytes().get(at + 1..at + 3);
        hex.is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit))
            .then_some(LocalChar::Bare)
    } else if is_pn_chars_u(c)
        || c.is_ascii_digit()
        || is_pn_local_other(c)
        || (!first && is_pn_chars(c))
    {
        Some(LocalChar::Bare)
    } else if PN_CHARS_ESC.contains(c) {
        Some(LocalChar::Escaped)
    } else {
        None
    }
}

/// Whether `text` spells a name that a `QUALIFIED_NAME` [51] denotes:
/// `prefix:local`, `prefix:` or a bare `local`, with the local part as the
/// model holds it, without escapes. The first `:` ends the prefix.
pub(crate) fn is_qualified_name(text: &str) -> bool {
    let local = match text.split_once(':') {
        Some((prefix, local)) if is_pn_prefix(prefix) => local,
        Some(_) => return false,
        None if text.is_empty() => return false,
        None => text,
    };
    local
        .char_indices()
        .all(|(at, c)| local_char(local, at, c).is_some())
}

/// Whether `IRI_REF` forbids `c` between its angle brackets: a control
/// character, a space, or one of `< > " { } | ^ ` \`.
#[cfg_attr(not(feature = "provn"), allow(dead_code))]
pub(crate) fn is_iri_forbidden(c: char) -> bool {
    c <= ' ' || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '^' | '`' | '\\')
}

/// The first character of `iri` that [`is_iri_forbidden`].
#[cfg_attr(not(feature = "provn"), allow(dead_code))]
pub(crate) fn iri_forbidden(iri: &str) -> Option<char> {
    iri.chars().find(|&c| is_iri_forbidden(c))
}
