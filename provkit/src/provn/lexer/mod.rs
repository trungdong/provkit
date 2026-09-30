//! PROV-N tokeniser (Recommendation section 3.8).
//!
//! Keywords are not distinguished here: every bare or prefixed name is a
//! `Name`, and the parser decides whether a name at statement start is a
//! keyword. The one context-sensitive rule is the language tag, which only
//! follows a string.

use super::chars::is_iri_forbidden;
use crate::pn::{PN_CHARS_ESC, hex_pair_follows, is_bare_local_char, prefix_len};
use crate::{Error, Result};

/// A token's class and payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// `QUALIFIED_NAME` [52], unescaped; `prefix` is empty for a bare name.
    Name { prefix: String, local: String },
    /// `QUALIFIED_NAME_LITERAL` [61], `'prefix:local'`.
    QNameLiteral { prefix: String, local: String },
    /// `IRI_REF` (from SPARQL), without the angle brackets.
    Iri(String),
    /// `STRING_LITERAL` [58], unescaped.
    Str(String),
    /// `INT_LITERAL` [60], as written.
    Int(String),
    /// `DATETIME`, as written.
    DateTime(String),
    /// `LANGTAG` (from SPARQL), without the `@`.
    LangTag(String),
    /// `%%`.
    Typed,
    /// `-`.
    Marker,
    /// `(`.
    LParen,
    /// `)`.
    RParen,
    /// `[`.
    LBracket,
    /// `]`.
    RBracket,
    /// `,`.
    Comma,
    /// `;`.
    Semicolon,
    /// `=`.
    Equals,
    /// `{`.
    LBrace,
    /// `}`.
    RBrace,
    /// End of input.
    Eof,
}

impl TokenKind {
    /// The digits of an unsigned `Int`, which is a bare name made of digits
    /// wherever the grammar wants an identifier. The lexer cannot tell that
    /// apart from `INT_LITERAL`, so the parser asks in identifier positions.
    pub(crate) fn digit_name(&self) -> Option<&str> {
        match self {
            TokenKind::Int(digits) if !digits.starts_with('-') => Some(digits),
            _ => None,
        }
    }
}

/// A token with its 1-based position and its byte span in the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) line: usize,
    pub(crate) column: usize,
    /// Byte offset of the first character.
    pub(crate) start: usize,
    /// Byte offset just past the last character.
    pub(crate) end: usize,
}

/// Tokenise the whole input eagerly. The last token is always `Eof`.
pub(crate) fn tokenize(text: &str) -> Result<Vec<Token>> {
    let mut lexer = Lexer {
        text,
        pos: 0,
        line: 1,
        line_start: 0,
        column_at: 0,
        column: 1,
        after_string: false,
    };
    let mut tokens = Vec::new();
    loop {
        lexer.skip_trivia()?;
        let token = lexer.next_token()?;
        let done = token.kind == TokenKind::Eof;
        tokens.push(token);
        if done {
            return Ok(tokens);
        }
    }
}

struct Lexer<'a> {
    text: &'a str,
    /// Byte offset of the next unread byte.
    pos: usize,
    line: usize,
    /// Byte offset of the current line's first byte.
    line_start: usize,
    /// Byte offset at which `column` was last computed.
    column_at: usize,
    /// 1-based column, in characters, of `column_at`.
    column: usize,
    /// Whether the previous token was a string, the only place `@` may start a language tag.
    after_string: bool,
}

impl<'a> Lexer<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    /// The 1-based column of `pos`. Counts only the characters since the
    /// last call, so a long single line is scanned once, not once per token.
    fn column(&mut self) -> usize {
        if self.column_at < self.line_start {
            self.column_at = self.line_start;
            self.column = 1;
        }
        self.column += self.text[self.column_at..self.pos].chars().count();
        self.column_at = self.pos;
        self.column
    }

    fn error(&self, message: impl Into<String>) -> Error {
        self.error_at(self.pos, message)
    }

    /// An error at byte `offset`, which lies at or after the current position.
    fn error_at(&self, offset: usize, message: impl Into<String>) -> Error {
        let (mut line, mut line_start) = (self.line, self.line_start);
        track_breaks(self.text, self.pos, offset, &mut line, &mut line_start);
        Error::Syntax {
            line,
            column: self.text[line_start..offset].chars().count() + 1,
            message: message.into(),
        }
    }

    /// Move past `len` bytes, counting line breaks (CRLF once).
    fn advance(&mut self, len: usize) {
        let end = self.pos + len;
        track_breaks(
            self.text,
            self.pos,
            end,
            &mut self.line,
            &mut self.line_start,
        );
        self.pos = end;
    }

    /// Advance past `len` bytes and return `kind`.
    fn emit(&mut self, len: usize, kind: TokenKind) -> TokenKind {
        self.advance(len);
        kind
    }

    /// Skip whitespace and comments. Whitespace is the grammar's `WS`:
    /// space, tab, CR and LF. Other Unicode spaces are not separators.
    fn skip_trivia(&mut self) -> Result<()> {
        loop {
            let rest = self.rest();
            let ws = rest
                .bytes()
                .take_while(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
                .count();
            if ws > 0 {
                self.advance(ws);
            } else if rest.starts_with("//") {
                let len = rest.find(['\n', '\r']).unwrap_or(rest.len());
                self.advance(len);
            } else if let Some(body) = rest.strip_prefix("/*") {
                // The search starts after the opening `/*`, so `/*/` stays open.
                let len = body
                    .find("*/")
                    .ok_or_else(|| self.error("unterminated comment"))?
                    + 4;
                self.advance(len);
            } else {
                return Ok(());
            }
        }
    }

    fn next_token(&mut self) -> Result<Token> {
        let (line, column, start) = (self.line, self.column(), self.pos);
        let rest = self.rest();
        let Some(c) = rest.chars().next() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                line,
                column,
                start,
                end: start,
            });
        };
        let kind = match c {
            '(' => self.emit(1, TokenKind::LParen),
            ')' => self.emit(1, TokenKind::RParen),
            '[' => self.emit(1, TokenKind::LBracket),
            ']' => self.emit(1, TokenKind::RBracket),
            ',' => self.emit(1, TokenKind::Comma),
            ';' => self.emit(1, TokenKind::Semicolon),
            '=' => self.emit(1, TokenKind::Equals),
            '{' => self.emit(1, TokenKind::LBrace),
            '}' => self.emit(1, TokenKind::RBrace),
            '<' => self.iri()?,
            '"' => self.string()?,
            '\'' => self.qname_literal()?,
            '@' if self.after_string => self.lang_tag()?,
            _ if rest.starts_with("%%") => self.emit(2, TokenKind::Typed),
            _ => self.literal_or_name()?,
        };
        self.after_string = matches!(kind, TokenKind::Str(_));
        Ok(Token {
            kind,
            line,
            column,
            start,
            end: self.pos,
        })
    }

    /// A dateTime, integer, name or marker, in that order of preference.
    fn literal_or_name(&mut self) -> Result<TokenKind> {
        let rest = self.rest();
        if let Some(len) = datetime_len(rest) {
            return Ok(self.emit(len, TokenKind::DateTime(rest[..len].to_owned())));
        }
        let int = int_len(rest);
        let name = scan_name(rest).map_err(|e| self.error_at(self.pos + e.offset, e.message))?;
        match name {
            Some(m) if m.len > int => {
                self.advance(m.len);
                if self.rest().starts_with(':') {
                    return Err(self.error("unexpected ':' inside a qualified name"));
                }
                Ok(TokenKind::Name {
                    prefix: m.prefix,
                    local: m.local,
                })
            }
            _ if int > 0 => Ok(self.emit(int, TokenKind::Int(rest[..int].to_owned()))),
            _ if rest.starts_with('-') => Ok(self.emit(1, TokenKind::Marker)),
            _ => {
                let c = rest.chars().next().unwrap_or_default();
                Err(self.error(format!("unexpected character {c:?}")))
            }
        }
    }

    /// `STRING_LITERAL`, short or `"""long"""`, unescaped.
    fn string(&mut self) -> Result<TokenKind> {
        let rest = self.rest();
        let long = rest.starts_with("\"\"\"");
        let quote = if long { "\"\"\"" } else { "\"" };
        let open = quote.len();
        let mut out = String::new();
        let mut chars = rest[open..].char_indices();
        while let Some((i, c)) = chars.next() {
            let at = open + i;
            if rest[at..].starts_with(quote) {
                self.advance(at + open);
                return Ok(TokenKind::Str(out));
            }
            match c {
                '\\' => {
                    let Some((_, e)) = chars.next() else { break };
                    out.push(match e {
                        'u' | 'U' => {
                            let (decoded, digits) = decode_code_point(&rest[at + 1..])
                                .map_err(|m| self.error_at(self.pos + at, m))?;
                            // The digits are ASCII, one byte each.
                            chars.nth(digits - 1);
                            decoded
                        }
                        't' => '\t',
                        'b' => '\u{8}',
                        'n' => '\n',
                        'r' => '\r',
                        'f' => '\u{c}',
                        '"' | '\'' | '\\' => e,
                        _ => {
                            return Err(
                                self.error_at(self.pos + at, format!("invalid escape '\\{e}'"))
                            );
                        }
                    });
                }
                '\n' | '\r' if !long => break,
                _ => out.push(c),
            }
        }
        Err(self.error("unterminated string"))
    }

    /// `IRI_REF`, without the angle brackets.
    fn iri(&mut self) -> Result<TokenKind> {
        let rest = self.rest();
        let mut out = String::new();
        let mut chars = rest.char_indices().skip(1);
        while let Some((i, c)) = chars.next() {
            match c {
                '>' => {
                    self.advance(i + 1);
                    return Ok(TokenKind::Iri(out));
                }
                '\\' if matches!(rest[i + 1..].chars().next(), Some('u' | 'U')) => {
                    let at = self.pos + i;
                    let (decoded, digits) =
                        decode_code_point(&rest[i + 1..]).map_err(|m| self.error_at(at, m))?;
                    if is_iri_forbidden(decoded) {
                        return Err(
                            self.error_at(at, format!("invalid character {decoded:?} in IRI"))
                        );
                    }
                    // The letter and the digits are ASCII, one byte each.
                    chars.nth(digits);
                    out.push(decoded);
                }
                // Whitespace and control characters end the scan: the IRI never closed.
                '\0'..=' ' => return Err(self.error("unterminated IRI")),
                _ if is_iri_forbidden(c) => {
                    return Err(
                        self.error_at(self.pos + i, format!("invalid character {c:?} in IRI"))
                    );
                }
                _ => out.push(c),
            }
        }
        Err(self.error("unterminated IRI"))
    }

    /// `'prefix:local'`, split like a name.
    fn qname_literal(&mut self) -> Result<TokenKind> {
        let rest = self.rest();
        let mut close = None;
        let mut chars = rest.char_indices().skip(1);
        while let Some((i, c)) = chars.next() {
            match c {
                '\'' => {
                    close = Some(i);
                    break;
                }
                '\\' => {
                    chars.next();
                }
                '\n' | '\r' => break,
                _ => {}
            }
        }
        let close = close.ok_or_else(|| self.error("unterminated qualified-name literal"))?;
        let content = &rest[1..close];
        let m = scan_name(content)
            .map_err(|e| self.error_at(self.pos + 1 + e.offset, e.message))?
            .filter(|m| m.len == content.len())
            .ok_or_else(|| self.error(format!("invalid qualified-name literal '{content}'")))?;
        self.advance(close + 1);
        Ok(TokenKind::QNameLiteral {
            prefix: m.prefix,
            local: m.local,
        })
    }

    /// `LANGTAG`: `@` `[A-Za-z]+` (`-` `[A-Za-z0-9]+)*`, without the `@`.
    fn lang_tag(&mut self) -> Result<TokenKind> {
        let rest = self.rest();
        let bytes = rest.as_bytes();
        let mut end = 1 + count_while(&bytes[1..], |b| b.is_ascii_alphabetic());
        if end == 1 {
            return Err(self.error_at(self.pos + 1, "expected a language tag after '@'"));
        }
        while bytes.get(end) == Some(&b'-') {
            let n = count_while(&bytes[end + 1..], |b| b.is_ascii_alphanumeric());
            if n == 0 {
                break;
            }
            end += 1 + n;
        }
        Ok(self.emit(end, TokenKind::LangTag(rest[1..end].to_owned())))
    }
}

/// Decode the `uXXXX` or `UXXXXXXXX` after a backslash in a string or IRI.
/// The digits are upper-case hexadecimal and name a Unicode scalar value:
/// `\u` one up to U+FFFF and `\U` one from U+10000, as section 6 assigns
/// them. Returns the character and the number of hex digits.
fn decode_code_point(s: &str) -> std::result::Result<(char, usize), String> {
    let letter = s.chars().next().unwrap_or_default();
    let (digits, lowest) = if letter == 'u' { (4, 0) } else { (8, 0x10000) };
    let invalid = || {
        format!(
            "invalid escape '\\{letter}': expected {digits} upper-case hex digits naming a Unicode \
             scalar value{}",
            if lowest > 0 { " from U+10000" } else { "" }
        )
    };
    let hex = s
        .get(1..=digits)
        .filter(|h| h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'F')))
        .ok_or_else(invalid)?;
    u32::from_str_radix(hex, 16)
        .ok()
        .filter(|&value| value >= lowest)
        .and_then(char::from_u32)
        .map(|c| (c, digits))
        .ok_or_else(invalid)
}

/// Update `line` and `line_start` for the bytes in `from..to`. A `\n` after
/// a `\r` continues the same break.
fn track_breaks(text: &str, from: usize, to: usize, line: &mut usize, line_start: &mut usize) {
    let bytes = text.as_bytes();
    for (k, &b) in bytes[from..to].iter().enumerate() {
        let i = from + k;
        match b {
            b'\r' => {
                *line += 1;
                *line_start = i + 1;
            }
            b'\n' => {
                if i == 0 || bytes[i - 1] != b'\r' {
                    *line += 1;
                }
                *line_start = i + 1;
            }
            _ => {}
        }
    }
}

fn count_while(bytes: &[u8], pred: impl Fn(u8) -> bool) -> usize {
    bytes.iter().take_while(|&&b| pred(b)).count()
}

/// Length of `-?[0-9]+` at the start of `s`, or 0.
fn int_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    let sign = usize::from(bytes.first() == Some(&b'-'));
    match count_while(&bytes[sign..], |b| b.is_ascii_digit()) {
        0 => 0,
        n => sign + n,
    }
}

/// Length of a `DATETIME` at the start of `s`:
/// `-?[0-9]{4,}-MM-DDThh:mm:ss(.[0-9]+)?(Z|[+-]hh:mm)?`.
fn datetime_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let digits =
        |from: usize| count_while(b.get(from..).unwrap_or_default(), |c| c.is_ascii_digit());
    let at = |from: usize, want: u8| b.get(from) == Some(&want);
    let mut i = usize::from(at(0, b'-'));
    let year = digits(i);
    if year < 4 {
        return None;
    }
    i += year;
    for sep in *b"--T::" {
        if !at(i, sep) || digits(i + 1) != 2 {
            return None;
        }
        i += 3;
    }
    if at(i, b'.') && digits(i + 1) > 0 {
        i += 1 + digits(i + 1);
    }
    if at(i, b'Z') {
        i += 1;
    } else if (at(i, b'+') || at(i, b'-'))
        && digits(i + 1) == 2
        && at(i + 3, b':')
        && digits(i + 4) == 2
    {
        i += 6;
    }
    Some(i)
}

/// A name found by [`scan_name`].
struct NameMatch {
    prefix: String,
    local: String,
    /// Bytes consumed.
    len: usize,
}

/// A malformed escape inside a name, at a byte offset from the scanned text.
struct NameError {
    offset: usize,
    message: String,
}

/// `QUALIFIED_NAME` [52] at the start of `s`: `PN_PREFIX ':' PN_LOCAL?` or
/// `PN_LOCAL`. A `:` after the match is left for the caller to reject.
fn scan_name(s: &str) -> std::result::Result<Option<NameMatch>, NameError> {
    if let Some(p) = prefix_len(s).filter(|&p| s[p..].starts_with(':')) {
        let (local, n) = scan_local(&s[p + 1..], p + 1)?;
        return Ok(Some(NameMatch {
            prefix: s[..p].to_owned(),
            local,
            len: p + 1 + n,
        }));
    }
    let (local, len) = scan_local(s, 0)?;
    Ok((len > 0).then(|| NameMatch {
        prefix: String::new(),
        local,
        len,
    }))
}

/// `PN_LOCAL` [53] at the start of `s`, unescaped, and the bytes it spans.
/// The character classes are [`pn`](crate::pn)'s; this function adds escape
/// decoding and error positions. `base` is `s`'s offset in the scanned text.
/// Trailing bare dots are left unconsumed.
fn scan_local(s: &str, base: usize) -> std::result::Result<(String, usize), NameError> {
    let mut out = String::new();
    // Length of `out` and offset in `s` after the last non-dot character.
    let (mut kept_len, mut kept_end) = (0, 0);
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let first = i == 0;
        let width = match c {
            '\\' => {
                let Some(&(_, e)) = chars.peek() else {
                    return Err(NameError {
                        offset: base + i,
                        message: "incomplete escape at end of input".to_owned(),
                    });
                };
                if !PN_CHARS_ESC.contains(e) {
                    return Err(NameError {
                        offset: base + i,
                        message: format!("invalid escape '\\{e}' in a name"),
                    });
                }
                chars.next();
                out.push(e);
                1 + e.len_utf8()
            }
            '%' if hex_pair_follows(s, i) => {
                chars.next();
                chars.next();
                out.push_str(&s[i..i + 3]);
                3
            }
            // A bare dot may continue a local part but not end it.
            '.' if !first => {
                out.push('.');
                continue;
            }
            _ if is_bare_local_char(c, first) => {
                out.push(c);
                c.len_utf8()
            }
            _ => break,
        };
        kept_len = out.len();
        kept_end = i + width;
    }
    out.truncate(kept_len);
    Ok((out, kept_end))
}

#[cfg(test)]
mod tests;
