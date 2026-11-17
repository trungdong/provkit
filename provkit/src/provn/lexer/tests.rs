use super::*;
use crate::test_util::syntax;

fn kinds(src: &str) -> Vec<TokenKind> {
    tokenize(src).unwrap().into_iter().map(|t| t.kind).collect()
}

fn name(prefix: &str, local: &str) -> TokenKind {
    TokenKind::Name {
        prefix: prefix.into(),
        local: local.into(),
    }
}

/// The line and column of the syntax error `src` raises.
fn syntax_at(src: &str) -> (usize, usize) {
    syntax(tokenize(src).unwrap_err()).0
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
    assert_eq!(syntax_at(r#""open"#), (1, 1));
    syntax(tokenize(r#""\q""#).unwrap_err());
}

#[test]
fn language_tag_only_after_a_string() {
    assert_eq!(kinds("ex:a @en")[1], name("", "@en"));
    assert_eq!(kinds("\"x\", @en")[2], name("", "@en"));
    assert_eq!(syntax_at("\"x\" @1"), (1, 6));
    assert!(matches!(&kinds("ex:a@b")[0], TokenKind::Name { local, .. } if local == "a@b"));
}

#[test]
fn iri_typed_and_comments() {
    let t = tokenize("prefix ex <http://example.org/> // c\n/* multi\nline */ \"1\" %% xsd:int")
        .unwrap();
    assert_eq!(t[2].kind, TokenKind::Iri("http://example.org/".into()));
    assert_eq!(t[3].kind, TokenKind::Str("1".into()));
    assert_eq!(t[4].kind, TokenKind::Typed);
    assert_eq!((t[3].line, t[3].column), (3, 9));
    syntax(tokenize("/* open").unwrap_err());
    syntax(tokenize("<open").unwrap_err());
    syntax(tokenize("'ex:open").unwrap_err());
}

#[test]
fn unterminated_iri_is_reported_at_its_start() {
    for src in ["x <http://x", "x <http://x\n"] {
        assert_eq!(syntax_at(src), (1, 3), "{src:?}");
    }
}

#[test]
fn forbidden_iri_character_is_reported_at_the_character() {
    for (src, column) in [("<a{b>", 3), ("<ab|>", 4), ("<a\\b>", 3)] {
        assert_eq!(syntax_at(src), (1, column), "{src:?}");
    }
}

#[test]
fn columns_count_characters_not_bytes() {
    let t = tokenize("\"é\" ex:é ex:x").unwrap();
    assert_eq!(t[1].kind, name("ex", "é"));
    assert_eq!((t[2].line, t[2].column), (1, 10));
    assert_eq!(syntax_at("\"é\" \"\\q\""), (1, 6));
}

#[test]
fn columns_count_source_characters_after_a_decoded_escape() {
    let t = tokenize("\"\\u00E9\\U0001F600\" ex:x <\\u00E9> ex:y").unwrap();
    assert_eq!(t[0].kind, TokenKind::Str("é\u{1F600}".into()));
    assert_eq!((t[1].kind.clone(), t[1].column), (name("ex", "x"), 20));
    assert_eq!(t[2].kind, TokenKind::Iri("é".into()));
    assert_eq!((t[3].kind.clone(), t[3].column), (name("ex", "y"), 34));
}

#[test]
fn a_language_tag_stops_at_an_underscore() {
    let t = kinds("\"x\" @en_US");
    assert_eq!(
        t,
        [
            TokenKind::Str("x".into()),
            TokenKind::LangTag("en".into()),
            name("", "_US"),
            TokenKind::Eof
        ]
    );
}

#[test]
fn local_part_rules() {
    assert!(
        matches!(&kinds(r"ex:a\.b\=c%2Fd")[0], TokenKind::Name { local, .. } if local == "a.b=c%2Fd")
    );
    assert!(matches!(&kinds("ex:a.b")[0], TokenKind::Name { local, .. } if local == "a.b"));
    // "ex:a." is the name "ex:a" followed by an unexpected '.'.
    syntax(tokenize("ex:a.").unwrap_err());
    assert!(
        matches!(&kinds("bbc:")[0], TokenKind::Name { prefix, local } if prefix == "bbc" && local.is_empty())
    );
    syntax(tokenize("ex:a:b").unwrap_err());
}

#[test]
fn blank_node_label_is_a_syntax_error() {
    assert_eq!(syntax_at("_:x"), (1, 2));
}

#[test]
fn local_part_accepts_the_full_grammar() {
    // PN_CHARS_OTHERS and an escaped colon and brackets.
    assert_eq!(
        kinds("ex:a/b@c~d&e+f*g?h#i$j!k")[0],
        name("ex", "a/b@c~d&e+f*g?h#i$j!k")
    );
    assert_eq!(kinds(r"ex:a\:b\[c\]d")[0], name("ex", "a:b[c]d"));
    // `-` and `.` continue a local part bare, as the writer writes them.
    assert_eq!(kinds("ex:a-b.c")[0], name("ex", "a-b.c"));
    // An escaped dot may end a local part; a bare one may not.
    assert_eq!(kinds(r"ex:a\.")[0], name("ex", "a."));
    syntax(tokenize(r"ex:a\q").unwrap_err());
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
    assert_eq!(syntax_at("ex:a @\\q"), (1, 7));
    assert_eq!(syntax_at(", @\\"), (1, 4));
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

/// `/` and `*` are name characters ([53] and [54]), so `//` or `/*` glued to
/// a name joins it; a comment starts after whitespace or punctuation.
#[test]
fn a_comment_glued_to_a_name_is_part_of_the_name() {
    assert_eq!(kinds("ex:b//c"), [name("ex", "b//c"), TokenKind::Eof]);
    assert_eq!(
        kinds("endDocument//c"),
        [name("", "endDocument//c"), TokenKind::Eof]
    );
    assert_eq!(
        kinds("document/*c*/"),
        [name("", "document/*c*/"), TokenKind::Eof]
    );
    assert_eq!(
        kinds("entity(ex:e)//c"),
        [
            name("", "entity"),
            TokenKind::LParen,
            name("ex", "e"),
            TokenKind::RParen,
            TokenKind::Eof
        ]
    );
    assert_eq!(
        kinds("endDocument //c"),
        [name("", "endDocument"), TokenKind::Eof]
    );
}

#[test]
fn a_block_comment_needs_both_delimiters() {
    assert_eq!(syntax_at("/*/"), (1, 1));
    assert_eq!(
        syntax(tokenize("/*/").unwrap_err()).1,
        "unterminated comment"
    );
    assert_eq!(syntax_at("x /*/ y"), (1, 3));
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
    // `\U` takes any scalar value, including those `\u` also covers.
    assert_eq!(
        kinds("\"\\uFFFF\\U00010000\\U00000041\\U000000E9\\U0000FFFF\""),
        [
            TokenKind::Str("\u{FFFF}\u{10000}A\u{E9}\u{FFFF}".into()),
            TokenKind::Eof
        ]
    );
    assert_eq!(
        kinds("<a\\U00000041\\U000000E9\\U0000FFFF>"),
        [TokenKind::Iri("aA\u{E9}\u{FFFF}".into()), TokenKind::Eof]
    );
    // Names take no code point escapes.
    assert!(tokenize("ex:a\\u00E9").is_err());
}

#[test]
fn a_decoded_escape_is_content_and_is_not_lexed_again() {
    // A quote, a line feed in a short string, and a backslash before `n`.
    for (src, content) in [
        ("\"\\u0022\"", "\""),
        ("\"a\\u000Ab\"", "a\nb"),
        ("\"\\u005Cn\"", "\\n"),
    ] {
        assert_eq!(
            kinds(src),
            [TokenKind::Str(content.into()), TokenKind::Eof],
            "{src}"
        );
    }
}

#[test]
fn a_bad_code_point_escape_is_reported_at_its_backslash() {
    for (src, column) in [
        ("\"a\\u00e9\"", 3),
        ("\"a\\uD800\"", 3),
        ("\"a\\U0000D800\"", 3),
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
        assert_eq!(syntax_at(src), (1, column), "{src:?}");
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
    assert_eq!(syntax_at("\"\"\"p\r\nq\\x\"\"\""), (2, 2));
}

#[test]
fn a_qualified_name_literal_left_open_is_reported_at_its_quote() {
    for src in ["x 'ex:a", "x 'ex:a\ny'", "x 'ex:a\r", "x 'ex:a\\"] {
        assert_eq!(syntax_at(src), (1, 3), "{src:?}");
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
    assert_eq!(syntax_at("a\n\"\"\"x\n\\q\"\"\""), (3, 1));
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
    for space in [
        '\u{A0}', '\u{2028}', '\u{2003}', '\u{3000}', '\u{c}', '\u{b}', '\u{85}',
    ] {
        assert_eq!(syntax_at(&format!("ex:a{space}ex:b")), (1, 5), "{space:?}");
        syntax(tokenize(&format!("{space}ex:a")).unwrap_err());
    }
}

#[test]
fn iri_characters_follow_the_shared_rule() {
    // IRI_REF ::= '<' ([^<>"{}|^`\]-[#x00-#x20])* '>'
    const FORBIDDEN: [char; 8] = ['<', '"', '{', '}', '|', '^', '`', '\\'];
    let others = ['\u{7F}', 'é', '\u{A0}', '\u{2028}', '😀'];
    for c in (b'!'..=b'~').map(char::from).chain(others) {
        if c == '>' {
            continue;
        }
        let text = format!("<a{c}b>");
        if FORBIDDEN.contains(&c) {
            assert_eq!(syntax_at(&text), (1, 3), "{c:?}");
        } else {
            assert_eq!(kinds(&text)[0], TokenKind::Iri(format!("a{c}b")), "{c:?}");
        }
    }
}

#[test]
fn every_string_escape_decodes() {
    // ECHAR ::= '\' [tbnrf\"']
    let cases = [
        (r"\t", '\t'),
        (r"\b", '\u{8}'),
        (r"\n", '\n'),
        (r"\r", '\r'),
        (r"\f", '\u{c}'),
        (r"\\", '\\'),
        (r#"\""#, '"'),
        (r"\'", '\''),
    ];
    for (escape, decoded) in cases {
        assert_eq!(
            kinds(&format!("\"a{escape}b\""))[0],
            TokenKind::Str(format!("a{decoded}b")),
            "{escape}"
        );
    }
    assert_eq!(syntax_at(r#""a\qb""#), (1, 3));
}

#[test]
fn a_language_subtag_may_hold_digits() {
    assert_eq!(kinds("\"x\"@en-1")[1], TokenKind::LangTag("en-1".into()));
    assert_eq!(
        kinds("\"x\"@de-CH-1996")[1],
        TokenKind::LangTag("de-CH-1996".into())
    );
}

#[test]
fn a_raw_line_break_ends_a_short_string() {
    assert_eq!(syntax_at("\"a\nb\""), (1, 1));
    assert_eq!(syntax_at("\"a\rb\""), (1, 1));
}

#[test]
fn a_space_or_control_character_in_an_iri_is_reported_where_it_is() {
    for c in [' ', '\t', '\0', '\u{1}', '\u{1F}'] {
        let text = format!("<http://d{c}/x>");
        let (at, message) = syntax(tokenize(&text).unwrap_err());
        assert_eq!(at, (1, 10), "{c:?}");
        assert!(message.starts_with("invalid character"), "{message}");
    }
    // A line break ends the scan: the IRI never closed.
    for text in ["<http://d\n/x>", "<http://d\r/x>", "<http://d"] {
        let (at, message) = syntax(tokenize(text).unwrap_err());
        assert_eq!(at, (1, 1), "{text:?}");
        assert_eq!(message, "unterminated IRI");
    }
}
