//! Extensibility expressions (Recommendation section 5, productions [49] to
//! [51]). Each is parsed in full and left out of the document: names are not
//! resolved, literals are not built, a time is checked, and the outermost
//! expression is recorded as an [`Ignored`].

use super::Ignored;
use super::lexer::{Token, TokenKind};
use super::parser::{Parser, at, describe};
use crate::{DateTime, Result};

/// Levels of nested expressions and tuples an expression may hold.
const MAX_DEPTH: usize = 64;

impl Parser<'_> {
    /// An outermost extensibility expression, whose name is the current
    /// token. It is recorded in `ignored` once it has parsed.
    pub(super) fn extensibility_expression(&mut self) -> Result<()> {
        let name = self.bump();
        self.extensibility_body(0)?;
        self.ignored.push(Ignored {
            name: self.text_of(name).to_owned(),
            line: name.line,
            column: name.column,
        });
        Ok(())
    }

    // extensibilityExpression ::= QUALIFIED_NAME "(" optionalIdentifier extensibilityArgument
    //                             ( "," extensibilityArgument )* optionalAttributeValuePairs ")"

    /// The rest of an expression after its name, `depth` levels inside the
    /// outermost one.
    fn extensibility_body(&mut self, depth: usize) -> Result<()> {
        self.expect(&TokenKind::LParen, "'('")?;
        // optionalIdentifier ::= ( identifierOrMarker ";" )?
        if self.peek_next().kind == TokenKind::Semicolon && is_identifier_or_marker(self.peek()) {
            self.bump();
            self.bump();
        }
        self.extensibility_argument(depth)?;
        while self.peek().kind == TokenKind::Comma {
            self.bump();
            if self.peek().kind == TokenKind::LBracket {
                self.skip_attribute_value_pairs()?;
                return self.expect(&TokenKind::RParen, "')'");
            }
            self.extensibility_argument(depth)?;
        }
        self.expect(&TokenKind::RParen, "',' or ')'")
    }

    // extensibilityArgument ::= identifierOrMarker | literal | time
    //                         | extensibilityExpression | extensibilityTuple

    /// A nested expression's name has a non-empty prefix, as section 5
    /// requires of every extensibility expression. A bare name is an
    /// identifier, so a `(` after it is an error at the `(`.
    fn extensibility_argument(&mut self, depth: usize) -> Result<()> {
        let token = self.peek();
        match &token.kind {
            TokenKind::Name { prefix, .. }
                if !prefix.is_empty() && self.peek_next().kind == TokenKind::LParen =>
            {
                enter(depth, token)?;
                self.bump();
                self.extensibility_body(depth + 1)
            }
            TokenKind::LBrace | TokenKind::LParen => {
                enter(depth, token)?;
                self.extensibility_tuple(depth + 1)
            }
            TokenKind::Name { .. } | TokenKind::Marker => {
                self.bump();
                Ok(())
            }
            // Production [44]: a time is a `DATETIME`, so its text must be one.
            TokenKind::DateTime(text) => {
                DateTime::parse(text).map_err(|e| at(token, e))?;
                self.bump();
                Ok(())
            }
            TokenKind::Str(_) | TokenKind::Int(_) | TokenKind::QNameLiteral { .. } => {
                self.literal_parts().map(drop)
            }
            _ => Err(self.expected("an identifier, '-', a literal, a dateTime, '(' or '{'")),
        }
    }

    // extensibilityTuple ::= "{" extensibilityArgument ( "," extensibilityArgument )* "}"
    //                      | "(" extensibilityArgument ( "," extensibilityArgument )* ")"

    /// A tuple, whose opening brace or parenthesis is the current token.
    fn extensibility_tuple(&mut self, depth: usize) -> Result<()> {
        let (close, what) = if self.bump().kind == TokenKind::LBrace {
            (TokenKind::RBrace, "',' or '}'")
        } else {
            (TokenKind::RParen, "',' or ')'")
        };
        self.extensibility_argument(depth)?;
        while self.peek().kind == TokenKind::Comma {
            self.bump();
            self.extensibility_argument(depth)?;
        }
        self.expect(&close, what)
    }

    // attributeValuePairs ::= ( | attributeValuePair ( "," attributeValuePair )* )
    // attributeValuePair ::= attribute "=" literal

    /// An attribute block, unresolved. The current token is `[`.
    fn skip_attribute_value_pairs(&mut self) -> Result<()> {
        self.expect(&TokenKind::LBracket, "'['")?;
        if self.peek().kind == TokenKind::RBracket {
            self.bump();
            return Ok(());
        }
        loop {
            self.identifier("an attribute name")?;
            self.expect(&TokenKind::Equals, "'='")?;
            self.literal_parts()?;
            match self.peek().kind {
                TokenKind::Comma => {
                    self.bump();
                }
                TokenKind::RBracket => {
                    self.bump();
                    return Ok(());
                }
                _ => return Err(self.expected("',' or ']'")),
            }
        }
    }
}

/// Whether `token` is an `identifierOrMarker`: a name, a bare name made of
/// digits, or `-`.
fn is_identifier_or_marker(token: &Token) -> bool {
    matches!(token.kind, TokenKind::Name { .. } | TokenKind::Marker)
        || token.kind.digit_name().is_some()
}

/// Check that a level below `depth` fits, before parsing it at `token`.
fn enter(depth: usize, token: &Token) -> Result<()> {
    if depth < MAX_DEPTH {
        Ok(())
    } else {
        Err(Parser::error_at(
            token,
            format!(
                "expected at most {MAX_DEPTH} levels of nesting in an extensibility expression, \
                 found {} nested deeper",
                describe(&token.kind)
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::provn::Profile;
    use crate::{Error, provn};

    const P: &str = "document\nprefix ex <http://example.org/>\nprefix dictExt <http://example.org/dictionaries#>\n";

    #[test]
    fn example_46_reads_in_both_forms() {
        for body in [
            r#"dictExt:hadMembers(mId; d, {("k1",e1), ("k2",e2), ("k3",e3)}, [])"#,
            r#"dictExt:hadMembers(mid; d, dictExt:set(dictExt:pair("k1",e1), dictExt:pair("k2",e2)), [dictExt:uniqueKeys="true"])"#,
        ] {
            for profile in [Profile::Strict, Profile::Default] {
                let (doc, ignored) = report(&format!("entity(ex:e)\n{body}"), profile).unwrap();
                assert_eq!(doc.records.len(), 1, "{body}");
                assert_eq!(ignored.len(), 1, "{body}");
                assert_eq!(
                    (ignored[0].name.as_str(), ignored[0].line, ignored[0].column),
                    ("dictExt:hadMembers", 5, 1)
                );
            }
        }
    }

    #[test]
    fn only_a_prefixed_name_starts_an_extension() {
        for body in [
            "foo(ex:e)",
            "entty(ex:e)",
            "hadDictionaryMember(ex:d, ex:e, \"k\")",
        ] {
            assert!(
                matches!(
                    provn::read(&format!("{P}{body}\nendDocument\n")),
                    Err(Error::Syntax { .. })
                ),
                "{body}"
            );
        }
        assert!(
            provn::read_with(
                &format!("{P}mentionOf(ex:a, ex:b, ex:c)\nendDocument\n"),
                Profile::Strict
            )
            .is_err()
        );
        let (doc, ignored) = report("prov:mentionOf(ex:a, ex:b, ex:c)", Profile::Strict).unwrap();
        assert_eq!((doc.records.len(), ignored.len()), (1, 0));
    }

    #[test]
    fn malformed_extensions_are_syntax_errors() {
        for body in [
            "ex:foo(ex:e",
            "ex:foo()",
            "ex:foo(ex:e,)",
            "ex:foo({})",
            "ex:foo(ex:e, [ex:k=1], ex:f)",
        ] {
            assert!(
                matches!(
                    provn::read(&format!("{P}{body}\nendDocument\n")),
                    Err(Error::Syntax { .. })
                ),
                "{body}"
            );
        }
    }

    #[test]
    fn nesting_is_limited() {
        let deep = format!(
            "{P}ex:f({}ex:e{})\nendDocument\n",
            "(".repeat(100_000),
            ")".repeat(100_000)
        );
        assert!(matches!(provn::read(&deep), Err(Error::Syntax { .. })));
        let ok = format!(
            "{P}ex:f({}ex:e{})\nendDocument\n",
            "(".repeat(60),
            ")".repeat(60)
        );
        assert!(provn::read(&ok).is_ok());
    }

    /// The document with `body` on line 4, under `profile`.
    fn report(
        body: &str,
        profile: Profile,
    ) -> crate::Result<(crate::Document, Vec<provn::Ignored>)> {
        provn::read_report(&format!("{P}{body}\nendDocument\n"), profile)
            .map(|r| (r.document, r.ignored))
    }

    fn syntax_position(
        result: crate::Result<(crate::Document, Vec<provn::Ignored>)>,
    ) -> (usize, usize) {
        match result {
            Err(Error::Syntax { line, column, .. }) => (line, column),
            other => panic!("expected a syntax error, got {other:?}"),
        }
    }

    #[test]
    fn a_time_in_an_extension_must_be_a_datetime() {
        for profile in [Profile::Strict, Profile::Default] {
            for time in [
                "2011-99-99T99:99:99",
                "2011-02-30T00:00:00",
                "2011-11-16T25:00:00",
            ] {
                let text = format!("{P}ex:f({time})\nendDocument");
                match provn::read_with(&text, profile) {
                    Err(Error::At {
                        line,
                        column,
                        error,
                    }) => {
                        assert_eq!((line, column), (4, 6), "{time}");
                        assert!(matches!(*error, Error::InvalidLiteral { .. }), "{time}");
                    }
                    other => panic!("{time}: expected Error::At, got {other:?}"),
                }
            }
            let text =
                format!("{P}ex:f(2011-11-16T16:05:33, {{2012-01-01T00:00:00Z}})\nendDocument");
            let report = provn::read_report(&text, profile).unwrap();
            assert_eq!(report.ignored.len(), 1);
        }
    }

    #[test]
    fn the_limit_is_64_levels_below_the_outermost_expression() {
        let nested = |n: usize| format!("ex:f({}ex:e{})", "(".repeat(n), ")".repeat(n));
        assert!(report(&nested(64), Profile::Strict).is_ok());
        // The 65th opening parenthesis follows `ex:f(` and 64 others.
        assert_eq!(
            syntax_position(report(&nested(65), Profile::Strict)),
            (4, 70)
        );
        match report(&nested(65), Profile::Strict) {
            Err(Error::Syntax { message, .. }) => assert!(message.contains("64"), "{message}"),
            other => panic!("expected a syntax error, got {other:?}"),
        }
        let expressions = |n: usize| format!("ex:f({}ex:e{})", "ex:g(".repeat(n), ")".repeat(n));
        assert!(report(&expressions(64), Profile::Strict).is_ok());
        assert_eq!(
            syntax_position(report(&expressions(65), Profile::Strict)),
            (4, 5 + 64 * 5 + 1)
        );
    }

    #[test]
    fn every_argument_form_reads_unresolved() {
        // Undeclared prefixes, an ill-formed literal and every argument
        // production, none of which is checked or built.
        let body = r#"ex:f(zz:id; zz:a, -, 2011-11-16T16:05:33Z, "s", "s"@en, "x" %% xsd:dateTime, "s" %% 12, 5, -5, 'zz:q', ex:set(1), zz:g(-; 1), (1, {2}), {"k"}, [zz:k="v" %% zz:t, 7=1])"#;
        for profile in [Profile::Strict, Profile::Default] {
            let (doc, ignored) = report(body, profile).unwrap();
            assert!(doc.records.is_empty());
            assert_eq!(ignored.len(), 1);
            assert_eq!(ignored[0].name, "ex:f");
        }
        for body in [
            "ex:f(-; a)",
            "ex:f(123; a)",
            "ex:f(ex:id; -)",
            "ex:f((ex:e), [])",
            "ex:f({(1)})",
            "ex:f (a)",
        ] {
            assert!(report(body, Profile::Strict).is_ok(), "{body}");
        }
    }

    #[test]
    fn a_token_outside_the_productions_is_a_syntax_error_at_that_token() {
        for (body, column) in [
            ("ex:f(-5; a)", 8),
            ("ex:f(\"s\"; a)", 9),
            ("ex:f(<http://x>)", 6),
            ("ex:f(ex:e, [ex:k=ex:v])", 18),
            ("ex:f(ex:e, [ex:k])", 17),
            ("ex:f([ex:k=1])", 6),
            ("ex:f(ex:e;)", 11),
            ("ex:f(ex:e, {1)", 14),
            ("ex:f(ex:e, (1})", 14),
            ("ex:f(ex:e) ex:g)", 16),
            ("ex:f entity(ex:e)", 6),
            // A nested expression's name has a prefix; a bare name is an
            // identifier, which no '(' may follow.
            ("ex:f(set(1))", 9),
            ("ex:f({set(1)})", 10),
        ] {
            assert_eq!(
                syntax_position(report(body, Profile::Default)),
                (4, column),
                "{body}"
            );
        }
    }

    #[test]
    fn an_identifier_and_a_semicolon_precede_the_first_argument() {
        for profile in [Profile::Strict, Profile::Default] {
            let (doc, ignored) = report("ex:f(ex:e; ex:g)", profile).unwrap();
            assert!(doc.records.is_empty());
            assert_eq!(ignored.len(), 1);
        }
    }

    #[test]
    fn extensions_read_inside_bundles_and_before_bundles_under_strict() {
        let body = "ex:f(1)\nbundle ex:b\n  ex:g(2)\n  entity(ex:e)\n  ex:h(3)\nendBundle";
        for profile in [Profile::Strict, Profile::Default] {
            let (doc, ignored) = report(body, profile).unwrap();
            assert!(doc.records.is_empty());
            assert_eq!(doc.bundles[0].records.len(), 1);
            let names: Vec<(&str, usize, usize)> = ignored
                .iter()
                .map(|i| (i.name.as_str(), i.line, i.column))
                .collect();
            assert_eq!(names, [("ex:f", 4, 1), ("ex:g", 6, 3), ("ex:h", 8, 3)]);
        }
        // After a bundle, an extension is an expression out of place: production [1].
        let after = "bundle ex:b\nendBundle\nex:f(1)";
        assert_eq!(syntax_position(report(after, Profile::Strict)), (6, 1));
        assert_eq!(report(after, Profile::Default).unwrap().1.len(), 1);
    }

    #[test]
    fn a_prefixed_prov_keyword_is_an_extension() {
        let (doc, ignored) = report("prov:entity(ex:e)", Profile::Strict).unwrap();
        assert!(doc.records.is_empty());
        assert_eq!(ignored[0].name, "prov:entity");
    }

    #[test]
    fn the_name_is_reported_as_written() {
        let (_, ignored) = report(r"ex:a\,b\-c(1)", Profile::Strict).unwrap();
        assert_eq!(ignored[0].name, r"ex:a\,b\-c");
    }

    #[test]
    fn read_and_read_with_drop_the_list() {
        let text = format!("{P}entity(ex:e)\nex:f(1)\nendDocument\n");
        let provn::ReadReport {
            document: doc,
            ignored,
            ..
        } = provn::read_report(&text, Profile::Default).unwrap();
        assert_eq!(ignored.len(), 1);
        assert_eq!(provn::read(&text).unwrap(), doc);
        assert_eq!(provn::read_with(&text, Profile::Strict).unwrap(), doc);
        // A byte-order mark is stripped by every entry point.
        assert_eq!(
            provn::read_report(&format!("\u{FEFF}{text}"), Profile::Strict)
                .unwrap()
                .ignored,
            ignored
        );
    }
}
