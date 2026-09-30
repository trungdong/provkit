//! PROV-N tokeniser (Recommendation section 3.8).
//!
//! Keywords are not distinguished here: every bare or prefixed name is a
//! `Name`, and the parser decides whether a name at statement start is a
//! keyword. The one context-sensitive rule is the language tag, which only
//! follows a string.

use super::chars::is_iri_forbidden;
use crate::pn::{PN_CHARS_ESC, is_pn_chars, is_pn_chars_u, is_pn_local_other, prefix_len};
use crate::{Error, Result};

/// A token's class and payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// `QUALIFIED_NAME` [51], unescaped; `prefix` is empty for a bare name.
    Name { prefix: String, local: String },
    /// `'prefix:local'`, a qualified-name literal.
    QNameLiteral { prefix: String, local: String },
    /// `IRI_REF` [56], without the angle brackets.
    Iri(String),
    /// `STRING_LITERAL` [60], unescaped.
    Str(String),
    /// `INT_LITERAL` [61], as written.
    Int(String),
    /// `DATETIME` [62], as written.
    DateTime(String),
    /// `LANGTAG` [63], without the `@`.
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
/// The digits are upper-case hexadecimal and name a Unicode scalar value.
/// Returns the character and the number of hex digits.
fn decode_code_point(s: &str) -> std::result::Result<(char, usize), String> {
    let letter = s.chars().next().unwrap_or_default();
    let digits = if letter == 'u' { 4 } else { 8 };
    let invalid = || {
        format!(
            "invalid escape '\\{letter}': expected {digits} upper-case hex digits naming a Unicode scalar value"
        )
    };
    let hex = s
        .get(1..=digits)
        .filter(|h| h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'F')))
        .ok_or_else(invalid)?;
    u32::from_str_radix(hex, 16)
        .ok()
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

/// `QUALIFIED_NAME` [51] at the start of `s`: `PN_PREFIX ':' PN_LOCAL?` or
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

/// `PN_LOCAL` [54] at the start of `s`, unescaped, and the bytes it spans.
/// `base` is `s`'s offset in the scanned text, for error positions. Trailing
/// bare dots are left unconsumed.
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
            '%' => {
                let hex = s.as_bytes().get(i + 1..i + 3);
                if !hex.is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) {
                    break;
                }
                chars.next();
                chars.next();
                out.push_str(&s[i..i + 3]);
                3
            }
            '.' if !first => {
                out.push('.');
                continue;
            }
            _ if is_pn_chars_u(c) || c.is_ascii_digit() || is_pn_local_other(c) => {
                out.push(c);
                c.len_utf8()
            }
            _ if !first && is_pn_chars(c) => {
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
mod tests {
    use super::*;
    use crate::Error;

    fn kinds(src: &str) -> Vec<TokenKind> {
        tokenize(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    fn name(prefix: &str, local: &str) -> TokenKind {
        TokenKind::Name {
            prefix: prefix.into(),
            local: local.into(),
        }
    }

    fn syntax_message(src: &str) -> String {
        match tokenize(src) {
            Err(Error::Syntax { message, .. }) => message,
            other => panic!("expected a syntax error for {src:?}, got {other:?}"),
        }
    }

    #[test]
    fn names_and_punctuation() {
        assert_eq!(
            kinds("entity(ex:e1, [prov:type='ex:T'])"),
            vec![
                name("", "entity"),
                TokenKind::LParen,
                name("ex", "e1"),
                TokenKind::Comma,
                TokenKind::LBracket,
                name("prov", "type"),
                TokenKind::Equals,
                TokenKind::QNameLiteral {
                    prefix: "ex".into(),
                    local: "T".into()
                },
                TokenKind::RBracket,
                TokenKind::RParen,
                TokenKind::Eof,
            ]
        );
        assert_eq!(kinds(";"), vec![TokenKind::Semicolon, TokenKind::Eof]);
    }

    #[test]
    fn datetime_beats_int_beats_name() {
        assert_eq!(
            kinds("2011-11-16T16:05:33Z 42 -7 e2 -")[..5],
            [
                TokenKind::DateTime("2011-11-16T16:05:33Z".into()),
                TokenKind::Int("42".into()),
                TokenKind::Int("-7".into()),
                name("", "e2"),
                TokenKind::Marker,
            ]
        );
        // A name that starts with digits is a name when it is longer than the integer prefix.
        assert!(matches!(&kinds("123abc")[0], TokenKind::Name { local, .. } if local == "123abc"));
        assert_eq!(
            kinds("2011-11-16T16:05:33.25+01:00")[0],
            TokenKind::DateTime("2011-11-16T16:05:33.25+01:00".into())
        );
    }

    #[test]
    fn strings_and_language_tags() {
        let t = tokenize(
            r#""a\"b\n" @en "x"@fr-CA """multi
line""" "#,
        )
        .unwrap();
        assert_eq!(t[0].kind, TokenKind::Str("a\"b\n".into()));
        assert_eq!(t[1].kind, TokenKind::LangTag("en".into()));
        assert_eq!(t[3].kind, TokenKind::LangTag("fr-CA".into()));
        assert_eq!(t[4].kind, TokenKind::Str("multi\nline".into()));
        assert!(matches!(
            tokenize(r#""open"#),
            Err(Error::Syntax {
                line: 1,
                column: 1,
                ..
            })
        ));
        assert!(matches!(tokenize(r#""\q""#), Err(Error::Syntax { .. })));
    }

    #[test]
    fn language_tag_only_after_a_string() {
        assert_eq!(kinds("ex:a @en")[1], name("", "@en"));
        assert_eq!(kinds("\"x\", @en")[2], name("", "@en"));
        assert!(syntax_message("\"x\" @1").contains("language tag"));
        assert!(matches!(&kinds("ex:a@b")[0], TokenKind::Name { local, .. } if local == "a@b"));
    }

    #[test]
    fn iri_typed_and_comments() {
        let t =
            tokenize("prefix ex <http://example.org/> // c\n/* multi\nline */ \"1\" %% xsd:int")
                .unwrap();
        assert_eq!(t[2].kind, TokenKind::Iri("http://example.org/".into()));
        assert_eq!(t[3].kind, TokenKind::Str("1".into()));
        assert_eq!(t[4].kind, TokenKind::Typed);
        assert_eq!((t[3].line, t[3].column), (3, 9));
        assert!(matches!(tokenize("/* open"), Err(Error::Syntax { .. })));
        assert!(matches!(tokenize("<open"), Err(Error::Syntax { .. })));
        assert!(matches!(tokenize("'ex:open"), Err(Error::Syntax { .. })));
    }

    #[test]
    fn unterminated_iri_is_reported_at_its_start() {
        for src in [
            "x <http://x",
            "x <http://x ",
            "x <http://x\n",
            "x <http://x\t>",
        ] {
            match tokenize(src) {
                Err(Error::Syntax {
                    line,
                    column,
                    message,
                    ..
                }) => {
                    assert_eq!((line, column), (1, 3), "{src:?}");
                    assert!(message.contains("unterminated IRI"), "{src:?}");
                }
                other => panic!("expected an error for {src:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn forbidden_iri_character_is_reported_at_the_character() {
        for (src, column) in [("<a{b>", 3), ("<ab|>", 4), ("<a\\b>", 3)] {
            match tokenize(src) {
                Err(Error::Syntax {
                    line,
                    column: c,
                    message,
                    ..
                }) => {
                    assert_eq!((line, c), (1, column), "{src:?}");
                    assert!(message.contains("in IRI"), "{src:?}");
                }
                other => panic!("expected an error for {src:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        let t = tokenize("\"é\" ex:é ex:x").unwrap();
        assert_eq!(t[1].kind, name("ex", "é"));
        assert_eq!((t[2].line, t[2].column), (1, 10));
        match tokenize("\"é\" \"\\q\"") {
            Err(Error::Syntax { line, column, .. }) => assert_eq!((line, column), (1, 6)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn local_part_rules() {
        assert!(
            matches!(&kinds(r"ex:a\.b\=c%2Fd")[0], TokenKind::Name { local, .. } if local == "a.b=c%2Fd")
        );
        assert!(matches!(&kinds("ex:a.b")[0], TokenKind::Name { local, .. } if local == "a.b"));
        // "ex:a." is the name "ex:a" followed by an unexpected '.'.
        assert!(matches!(tokenize("ex:a."), Err(Error::Syntax { .. })));
        assert!(
            matches!(&kinds("bbc:")[0], TokenKind::Name { prefix, local } if prefix == "bbc" && local.is_empty())
        );
        assert!(matches!(tokenize("ex:a:b"), Err(Error::Syntax { .. })));
    }

    #[test]
    fn blank_node_label_is_a_syntax_error() {
        assert!(syntax_message("_:x").contains("unexpected ':' inside a qualified name"));
    }

    #[test]
    fn local_part_accepts_the_full_grammar() {
        // PN_CHARS_OTHERS and an escaped colon and brackets.
        assert_eq!(
            kinds("ex:a/b@c~d&e+f*g?h#i$j!k")[0],
            name("ex", "a/b@c~d&e+f*g?h#i$j!k")
        );
        assert_eq!(kinds(r"ex:a\:b\[c\]d")[0], name("ex", "a:b[c]d"));
        // An escaped dot may end a local part; a bare one may not.
        assert_eq!(kinds(r"ex:a\.")[0], name("ex", "a."));
        assert!(matches!(tokenize(r"ex:a\q"), Err(Error::Syntax { .. })));
        assert_eq!(kinds("ex:a%2Fb")[0], name("ex", "a%2Fb"));
    }

    #[test]
    fn a_bare_name_may_start_with_an_at_sign() {
        assert_eq!(kinds("@foo"), [name("", "@foo"), TokenKind::Eof]);
        assert_eq!(
            kinds("\"x\"@en"),
            [
                TokenKind::Str("x".into()),
                TokenKind::LangTag("en".into()),
                TokenKind::Eof
            ]
        );
        assert_eq!(
            kinds("\"x\" @en"),
            [
                TokenKind::Str("x".into()),
                TokenKind::LangTag("en".into()),
                TokenKind::Eof
            ]
        );
        assert_eq!(kinds("ex:@foo"), [name("ex", "@foo"), TokenKind::Eof]);
    }

    #[test]
    fn an_at_sign_before_a_bad_escape_is_reported_at_the_escape() {
        match tokenize("ex:a @\\q") {
            Err(Error::Syntax { line, column, .. }) => assert_eq!((line, column), (1, 7)),
            other => panic!("unexpected {other:?}"),
        }
        match tokenize(", @\\") {
            Err(Error::Syntax { line, column, .. }) => assert_eq!((line, column), (1, 4)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn escapes_are_the_eleven_of_production_55() {
        assert_eq!(
            kinds(r"ex:a\=\'\(\)\,\-\:\;\[\]\."),
            [name("ex", "a='(),-:;[]."), TokenKind::Eof]
        );
        for c in "_~!$&*+/?#@%".chars() {
            assert!(tokenize(&format!("ex:a\\{c}b")).is_err(), "\\{c}");
        }
    }

    #[test]
    fn a_block_comment_needs_both_delimiters() {
        assert_eq!(syntax_message("/*/"), "unterminated comment");
        assert_eq!(syntax_message("x /*/ y"), "unterminated comment");
        assert_eq!(kinds("/**/ x"), [name("", "x"), TokenKind::Eof]);
        assert_eq!(kinds("/***/ x"), [name("", "x"), TokenKind::Eof]);
    }

    #[test]
    fn braces_are_tokens() {
        assert_eq!(
            kinds("{a}"),
            [
                TokenKind::LBrace,
                name("", "a"),
                TokenKind::RBrace,
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn code_point_escapes_decode_in_strings_and_iris() {
        assert_eq!(
            kinds("\"caf\\u00E9\""),
            [TokenKind::Str("café".into()), TokenKind::Eof]
        );
        assert_eq!(
            kinds("\"\\U0001F600!\""),
            [TokenKind::Str("\u{1F600}!".into()), TokenKind::Eof]
        );
        assert_eq!(
            kinds("\"\"\"a\\u000Ab\"\"\""),
            [TokenKind::Str("a\nb".into()), TokenKind::Eof]
        );
        assert_eq!(
            kinds("<http://e/\\u00E9\\U0001F600x>"),
            [
                TokenKind::Iri("http://e/é\u{1F600}x".into()),
                TokenKind::Eof
            ]
        );
        // Names take no code point escapes.
        assert!(tokenize("ex:a\\u00E9").is_err());
    }

    #[test]
    fn a_bad_code_point_escape_is_reported_at_its_backslash() {
        for (src, column) in [
            ("\"a\\u00e9\"", 3),
            ("\"a\\uD800\"", 3),
            ("\"a\\UDFFF0000\"", 3),
            ("\"a\\U00110000\"", 3),
            ("\"a\\u12\"", 3),
            ("\"a\\u12G4\"", 3),
            ("\"a\\U0001F60\"", 3),
            ("<a\\u00e9>", 3),
            ("<a\\uD800>", 3),
            ("<a\\u0020>", 3),
            ("<a\\u003E>", 3),
            ("<a\\u005C>", 3),
            ("<a\\u007B>", 3),
        ] {
            match tokenize(src) {
                Err(Error::Syntax {
                    line, column: c, ..
                }) => {
                    assert_eq!((line, c), (1, column), "{src:?}");
                }
                other => panic!("expected an error for {src:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_bare_carriage_return_ends_a_line() {
        let t = tokenize("a\rb // c\rd /* e\r*/ f\r\rg").unwrap();
        let at = |i: usize| (t[i].line, t[i].column);
        assert_eq!(at(1), (2, 1));
        assert_eq!(at(2), (3, 1));
        assert_eq!(at(3), (4, 4));
        assert_eq!(at(4), (6, 1));
        assert!(tokenize("\"a\rb\"").is_err());
    }

    #[test]
    fn crlf_inside_a_comment_and_a_long_string_counts_once() {
        let t = tokenize("a // x\r\nb /* y\r\nz */ c \"\"\"p\r\nq\"\"\" d").unwrap();
        assert_eq!((t[1].line, t[1].column), (2, 1));
        assert_eq!((t[2].line, t[2].column), (3, 6));
        assert_eq!(t[3].kind, TokenKind::Str("p\r\nq".into()));
        assert_eq!((t[4].line, t[4].column), (4, 6));
        match tokenize("\"\"\"p\r\nq\\x\"\"\"") {
            Err(Error::Syntax { line, column, .. }) => assert_eq!((line, column), (2, 2)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_qualified_name_literal_left_open_is_reported_at_its_quote() {
        for src in ["x 'ex:a", "x 'ex:a\ny'", "x 'ex:a\r", "x 'ex:a\\"] {
            match tokenize(src) {
                Err(Error::Syntax {
                    line,
                    column,
                    message,
                }) => {
                    assert_eq!((line, column), (1, 3), "{src:?}");
                    assert!(message.contains("unterminated"), "{src:?}");
                }
                other => panic!("expected an error for {src:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn percent_typed_marker_is_not_taken_by_a_name() {
        assert_eq!(
            kinds("ex:a%%xsd:int")[..3],
            [name("ex", "a"), TokenKind::Typed, name("xsd", "int")]
        );
    }

    #[test]
    fn qname_literal_with_escapes() {
        assert_eq!(
            kinds(r"'ex:it\'s'")[0],
            TokenKind::QNameLiteral {
                prefix: "ex".into(),
                local: "it's".into()
            }
        );
    }

    #[test]
    fn columns_stay_right_across_many_tokens() {
        let t = tokenize("é, é,\n  é , x").unwrap();
        let columns = |line| {
            t.iter()
                .filter(|t| t.line == line)
                .map(|t| t.column)
                .collect::<Vec<_>>()
        };
        assert_eq!(columns(1), [1, 2, 4, 5]);
        assert_eq!(columns(2), [3, 5, 7, 8]);
    }

    #[test]
    fn positions_count_crlf_once() {
        let t = tokenize("a\r\nb\nc").unwrap();
        assert_eq!((t[1].line, t[1].column), (2, 1));
        assert_eq!((t[2].line, t[2].column), (3, 1));
    }

    #[test]
    fn error_position_inside_a_multiline_string() {
        match tokenize("a\n\"\"\"x\n\\q\"\"\"") {
            Err(Error::Syntax { line, column, .. }) => assert_eq!((line, column), (3, 1)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn only_grammar_whitespace_separates_tokens() {
        assert_eq!(
            kinds(" \t\r\nex:a \t\r\n")[..2],
            [name("ex", "a"), TokenKind::Eof]
        );
        // U+1680 is in PN_CHARS_BASE, so it starts or continues a name.
        assert_eq!(kinds("\u{1680}a")[0], name("", "\u{1680}a"));
        assert_eq!(kinds("ex:a\u{1680}")[0], name("ex", "a\u{1680}"));
        // Other Unicode spaces are unexpected characters, not separators.
        for space in ['\u{A0}', '\u{2028}', '\u{2003}', '\u{3000}'] {
            let message = syntax_message(&format!("ex:a{space}ex:b"));
            assert!(
                message.contains("unexpected character"),
                "{space:?}: {message}"
            );
            assert!(matches!(
                tokenize(&format!("{space}ex:a")),
                Err(Error::Syntax { .. })
            ));
        }
    }

    #[test]
    fn iri_characters_follow_the_shared_rule() {
        let others = ['\u{7F}', 'é', '\u{A0}', '\u{2028}', '😀'];
        for c in (b'!'..=b'~').map(char::from).chain(others) {
            if c == '>' {
                continue;
            }
            let lexed = tokenize(&format!("<a{c}b>"));
            if is_iri_forbidden(c) {
                assert!(
                    syntax_message(&format!("<a{c}b>")).contains("in IRI"),
                    "{c:?}"
                );
            } else {
                assert_eq!(
                    lexed.unwrap()[0].kind,
                    TokenKind::Iri(format!("a{c}b")),
                    "{c:?}"
                );
            }
        }
    }
}
