//! Character classes of the PROV-N name grammar (Recommendation section
//! 3.7.1, productions [52] to [56], with `PN_PREFIX`, `PN_CHARS_BASE`,
//! `PN_CHARS_U` and `PN_CHARS` from SPARQL 1.0, productions [95], [96],
//! [98] and [99]).

/// `PN_CHARS_ESC` [55]: the characters that may follow `\` in a local part.
/// A local part holds most of them only after one; `-` and `.` also stand
/// bare where `PN_LOCAL` [53] admits them.
pub(crate) const PN_CHARS_ESC: &str = "='(),-:;[].";

/// `PN_CHARS_BASE`.
fn is_pn_chars_base(c: char) -> bool {
    matches!(c,
        'A'..='Z' | 'a'..='z' | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

/// `PN_CHARS_U`.
fn is_pn_chars_u(c: char) -> bool {
    is_pn_chars_base(c) || c == '_'
}

/// `PN_CHARS`.
fn is_pn_chars(c: char) -> bool {
    is_pn_chars_u(c)
        || c == '-'
        || c.is_ascii_digit()
        || c == '\u{B7}'
        || matches!(c, '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
}

/// The plain characters of `PN_CHARS_OTHERS` [54]; `%` and `\` escapes are
/// handled separately.
fn is_pn_local_other(c: char) -> bool {
    matches!(c, '/' | '@' | '~' | '&' | '+' | '*' | '?' | '#' | '$' | '!')
}

// ANCHOR: prefix_len
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
// ANCHOR_END: prefix_len

/// Whether all of `s` is one `PN_PREFIX`.
pub(crate) fn is_pn_prefix(s: &str) -> bool {
    prefix_len(s) == Some(s.len())
}

// ANCHOR: local_char
/// How PROV-N writes one character of a local part.
pub(crate) enum LocalChar {
    /// As it is.
    Bare,
    /// After a `\`.
    Escaped,
}

/// Whether `PN_LOCAL` [53] holds `c` bare: `PN_CHARS_U`, a digit or a plain
/// `PN_CHARS_OTHERS` character anywhere, and the rest of `PN_CHARS` after
/// the first character. `%` and `.` have their own rules.
pub(crate) fn is_bare_local_char(c: char, first: bool) -> bool {
    is_pn_chars_u(c) || c.is_ascii_digit() || is_pn_local_other(c) || (!first && is_pn_chars(c))
}

/// Whether the `%` at byte `at` of `local` starts a `PERCENT` [56]: two hex
/// digits follow it. The reader keeps `%XX` verbatim and the writer writes
/// it bare; any other `%` has no place in a local part.
pub(crate) fn hex_pair_follows(local: &str, at: usize) -> bool {
    local
        .as_bytes()
        .get(at + 1..at + 3)
        .is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit))
}

/// How PROV-N writes the character `c` at byte `at` of `local`, or `None`
/// when no `PN_LOCAL` [53] holds it there. The writer escapes every
/// `PN_CHARS_ESC` character except `-` after the first character and `.`
/// between the first and the last, which `PN_LOCAL` admits bare and `ProvPy`
/// 3.3.0 writes bare.
pub(crate) fn local_char(local: &str, at: usize, c: char) -> Option<LocalChar> {
    if c == '.' {
        let ends_the_part = at == 0 || at + 1 == local.len();
        Some(if ends_the_part {
            LocalChar::Escaped
        } else {
            LocalChar::Bare
        })
    } else if c == '-' {
        Some(if at == 0 {
            LocalChar::Escaped
        } else {
            LocalChar::Bare
        })
    } else if PN_CHARS_ESC.contains(c) {
        Some(LocalChar::Escaped)
    } else if c == '%' {
        hex_pair_follows(local, at).then_some(LocalChar::Bare)
    } else if is_bare_local_char(c, at == 0) {
        Some(LocalChar::Bare)
    } else {
        None
    }
}
// ANCHOR_END: local_char
