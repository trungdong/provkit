//! Character classes of the PROV-N name grammar (Recommendation section
//! 3.8, with `PN_PREFIX`, `PN_CHARS_BASE`, `PN_CHARS_U` and `PN_CHARS` from
//! SPARQL).

/// Characters that may follow `\` in a local part: `PN_CHARS_ESC` [55] plus
/// the SPARQL escapes `_ ~ ! $ & * + / ? # @ %`.
#[cfg_attr(not(feature = "provn"), allow(dead_code))]
pub(crate) const PN_CHARS_ESC: &str = "_~.-!$&'()*+,;=/?#@%:[]";

/// `PN_CHARS_BASE`.
pub(crate) fn is_pn_chars_base(c: char) -> bool {
    matches!(c,
        'A'..='Z' | 'a'..='z' | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

/// `PN_CHARS_U`.
#[cfg_attr(not(feature = "provn"), allow(dead_code))]
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
#[cfg_attr(not(feature = "provn"), allow(dead_code))]
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
