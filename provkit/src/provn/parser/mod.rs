//! Recursive-descent parser for PROV-N (Recommendation section 3).
//!
//! One method per production. Names resolve through [`Namespaces`], records are
//! built by [`Record::from_parts`], and every event goes to a [`DocumentBuilder`].
//!
//! Every error carries a position. A grammar error is [`Error::Syntax`], and
//! an error from the name, value or record layer is wrapped by [`at`].
//!
//! Extensibility expressions (section 5) are parsed by the methods in the
//! `extension` module and recorded in [`Parser::ignored`] instead of the
//! document.

mod declarations;

use super::lexer::{Token, TokenKind};
use super::{Ignored, Profile, Relaxation};
use crate::builder::DocumentBuilder;
use crate::error::clip;
use crate::namespaces::{PROV_PREFIX, Scope};
use crate::{
    Attributes, DateTime, Error, Formal, FormalAttr, Kind, LangStr, Namespaces, QName, Record,
    Result, Value,
};

/// The kind that the statement keyword `prefix:local` names, or `None` for
/// anything else. `mentionOf` is written `prov:mentionOf`, and only
/// [`Profile::Default`] also reads it bare. Every other keyword is bare.
fn classify(prefix: &str, local: &str, profile: Profile) -> Option<Kind> {
    let kind = Kind::from_name(local)?;
    let accepted = if kind == Kind::Mention {
        prefix == PROV_PREFIX || (prefix.is_empty() && profile.allows(Relaxation::BareMentionOf))
    } else {
        prefix.is_empty()
    };
    accepted.then_some(kind)
}

/// Attach `token`'s position to an error that has none.
pub(super) fn at(token: &Token, error: Error) -> Error {
    match error {
        Error::Syntax { .. } | Error::At { .. } => error,
        other => Error::At {
            line: token.line,
            column: token.column,
            error: Box::new(other),
        },
    }
}

/// A name token split into its parts, before resolution.
pub(super) struct NameParts<'t> {
    token: &'t Token,
    prefix: &'t str,
    local: &'t str,
}

/// A positional argument, resolved but not yet matched to a formal.
enum Arg {
    Name(QName),
    Time(DateTime),
    Marker,
}

/// A positional argument with the token it was read from.
type Positional<'t> = (&'t Token, Arg);

/// The tokens of a literal, before any name in it is resolved.
pub(super) enum LiteralParts<'t> {
    /// A string, with what follows it.
    Str(&'t str, Suffix<'t>),
    /// An `INT_LITERAL`.
    Int(&'t str),
    /// A `QUALIFIED_NAME_LITERAL`.
    Name { prefix: &'t str, local: &'t str },
}

/// What follows a string literal.
pub(super) enum Suffix<'t> {
    /// Nothing.
    None,
    /// A language tag.
    Lang(&'t str),
    /// A datatype name.
    Datatype(NameParts<'t>),
}

// ANCHOR: parser
pub(super) struct Parser<'t> {
    source: &'t str,
    tokens: &'t [Token],
    pos: usize,
    profile: Profile,
    document_ns: Namespaces,
    /// Inside a bundle, the bundle's own table.
    bundle_ns: Option<Namespaces>,
    /// The outermost extensibility expressions, in source order.
    pub(super) ignored: Vec<Ignored>,
}
// ANCHOR_END: parser

impl<'t> Parser<'t> {
    /// A parser over `tokens`, which are lexed from `source` and end with
    /// `Eof`.
    pub(super) fn new(source: &'t str, tokens: &'t [Token], profile: Profile) -> Self {
        Parser {
            source,
            tokens,
            pos: 0,
            profile,
            document_ns: Namespaces::new(),
            bundle_ns: None,
            ignored: Vec::new(),
        }
    }

    // Token helpers

    pub(super) fn peek(&self) -> &'t Token {
        &self.tokens[self.pos]
    }

    /// The token after the current one, or `Eof` at the end.
    pub(super) fn peek_next(&self) -> &'t Token {
        self.tokens.get(self.pos + 1).unwrap_or_else(|| self.peek())
    }

    /// The current token, advancing unless it is `Eof`.
    pub(super) fn bump(&mut self) -> &'t Token {
        let token = &self.tokens[self.pos];
        if token.kind != TokenKind::Eof {
            self.pos += 1;
        }
        token
    }

    /// The source text `token` was lexed from.
    pub(super) fn text_of(&self, token: &Token) -> &'t str {
        &self.source[token.start..token.end]
    }

    /// "expected `what`, found `found`" at `token`.
    pub(super) fn expected_found(token: &Token, what: &str, found: &str) -> Error {
        Error::Syntax {
            line: token.line,
            column: token.column,
            message: format!("expected {what}, found {found}"),
        }
    }

    /// "expected `what`, found" the token itself, at `token`.
    pub(super) fn expected_at(token: &Token, what: &str) -> Error {
        Self::expected_found(token, what, &describe(&token.kind))
    }

    /// "expected `what`, found ..." at the current token.
    pub(super) fn expected(&self, what: &str) -> Error {
        Self::expected_at(self.peek(), what)
    }

    pub(super) fn expect(&mut self, kind: &TokenKind, what: &str) -> Result<()> {
        if self.peek().kind == *kind {
            self.bump();
            Ok(())
        } else {
            Err(self.expected(what))
        }
    }

    fn is_keyword(&self, word: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Name { prefix, local } if prefix.is_empty() && local == word)
    }

    fn keyword(&mut self, word: &str) -> Result<()> {
        if self.is_keyword(word) {
            self.bump();
            Ok(())
        } else {
            Err(self.expected(&format!("'{word}'")))
        }
    }

    /// The scope names resolve in: the open bundle's table in front of the
    /// document's, or the document's alone.
    fn scope(&self) -> Scope<'_> {
        match &self.bundle_ns {
            Some(own) => Scope::within(own, &self.document_ns),
            None => Scope::of(&self.document_ns),
        }
    }

    /// The name in the current scope. An error is placed at the name token.
    fn resolve(&self, name: &NameParts) -> Result<QName> {
        self.scope()
            .resolve_parts(name.prefix, name.local)
            .map_err(|e| at(name.token, e))
    }

    /// A name token.
    fn name(&mut self, what: &str) -> Result<NameParts<'t>> {
        let token = self.peek();
        let TokenKind::Name { prefix, local } = &token.kind else {
            return Err(self.expected(what));
        };
        let parts = NameParts {
            token,
            prefix,
            local,
        };
        self.bump();
        Ok(parts)
    }

    /// A name token in an identifier or attribute-name position, where an
    /// unsigned integer is a bare local name made of digits. The lexer
    /// cannot tell that apart from `INT_LITERAL`.
    pub(super) fn identifier(&mut self, what: &str) -> Result<NameParts<'t>> {
        let token = self.peek();
        if let Some(digits) = token.kind.digit_name() {
            let parts = NameParts {
                token,
                prefix: "",
                local: digits,
            };
            self.bump();
            return Ok(parts);
        }
        self.name(what)
    }

    fn iri(&mut self) -> Result<&'t str> {
        if let TokenKind::Iri(iri) = &self.peek().kind {
            self.bump();
            Ok(iri)
        } else {
            Err(self.expected("an IRI in angle brackets"))
        }
    }

    // ANCHOR: parse_document
    // [1] document ::= "document" (namespaceDeclarations)? (expression)* (bundle)* "endDocument"

    pub(super) fn document(&mut self, builder: &mut DocumentBuilder) -> Result<()> {
        self.keyword("document")?;
        self.document_ns = self.namespace_declarations()?;
        builder.start_document(self.document_ns.clone());
        let mut after_bundle = false;
        while !self.is_keyword("endDocument") {
            if self.peek().kind == TokenKind::Eof {
                return Err(self.expected("'endDocument'"));
            }
            if self.is_keyword("bundle") {
                self.bundle(builder)?;
                after_bundle = true;
            } else {
                // Expressions come before bundles: production [1].
                if after_bundle && !self.profile.allows(Relaxation::StatementAfterBundle) {
                    return Err(self.expected(
                        "'bundle' or 'endDocument' after a bundle under the strict profile",
                    ));
                }
                self.statement(builder)?;
            }
        }
        self.bump();
        if self.peek().kind != TokenKind::Eof {
            return Err(self.expected("end of input"));
        }
        builder.end_document();
        Ok(())
    }
    // ANCHOR_END: parse_document

    // bundle ::= "bundle" identifier namespaceDeclarations? expression* "endBundle"

    fn bundle(&mut self, builder: &mut DocumentBuilder) -> Result<()> {
        self.bump();
        let id = self.identifier("a bundle identifier")?;
        let ns = self.namespace_declarations()?;
        // The bundle's own name resolves in the bundle scope (PROV-N 3.7.4).
        self.bundle_ns = Some(ns.clone());
        let name = self.resolve(&id)?;
        builder
            .start_bundle(name, ns)
            .map_err(|e| at(id.token, e))?;
        while !self.is_keyword("endBundle") {
            if self.peek().kind == TokenKind::Eof
                || self.is_keyword("bundle")
                || self.is_keyword("endDocument")
            {
                return Err(self.expected("a statement or 'endBundle'"));
            }
            self.statement(builder)?;
        }
        self.bump();
        self.bundle_ns = None;
        builder.end_bundle();
        Ok(())
    }

    // ANCHOR: parse_statement
    /// One statement: a PROV expression, sent to `builder` as a record, or an
    /// extensibility expression, parsed and recorded in `ignored`. A keyword
    /// with a prefix starts an extensibility expression (section 5), except
    /// `prov:mentionOf` followed by the shape of a mention; a bare unknown
    /// keyword is a syntax error.
    fn statement(&mut self, builder: &mut DocumentBuilder) -> Result<()> {
        let is_extension = match &self.peek().kind {
            TokenKind::Name { prefix, .. } if prefix.is_empty() => false,
            TokenKind::Name { prefix, local } => {
                !(prefix == PROV_PREFIX
                    && local == Kind::Mention.name()
                    && self.mention_shape_follows())
            }
            _ => false,
        };
        if is_extension {
            self.extensibility_expression()
        } else {
            builder.record(self.expression()?);
            Ok(())
        }
    }
    // ANCHOR_END: parse_statement

    /// Whether the tokens after the current `prov:mentionOf` have the shape
    /// of a mention (PROV-LINKS): `(`, three names separated by `,`, then
    /// `)` or `, [`. [`Profile::Default`] also takes an `identifierOrMarker`
    /// and `;` before the names, and `-` for a name, as it does for bare
    /// `mentionOf`. Any other shape is an extensibility expression under
    /// production [49].
    fn mention_shape_follows(&self) -> bool {
        let kind = |i: usize| self.tokens.get(self.pos + i).map(|t| &t.kind);
        let markers = self.profile.allows(Relaxation::MarkerInRequiredPosition);
        let identifier = self
            .profile
            .allows(Relaxation::IdentifierOrAttributesOnBareKind);
        let is_name = |i: usize| {
            kind(i).is_some_and(|k| {
                matches!(k, TokenKind::Name { .. })
                    || k.digit_name().is_some()
                    || (markers && *k == TokenKind::Marker)
            })
        };
        if kind(1) != Some(&TokenKind::LParen) {
            return false;
        }
        let mut i = 2;
        if identifier
            && kind(i + 1) == Some(&TokenKind::Semicolon)
            && kind(i).is_some_and(TokenKind::is_identifier_or_marker)
        {
            i += 2;
        }
        for position in 0..3 {
            if position > 0 {
                if kind(i) != Some(&TokenKind::Comma) {
                    return false;
                }
                i += 1;
            }
            if !is_name(i) {
                return false;
            }
            i += 1;
        }
        match kind(i) {
            Some(TokenKind::RParen) => true,
            Some(TokenKind::Comma) => kind(i + 1) == Some(&TokenKind::LBracket),
            _ => false,
        }
    }

    /// Whether `kind` takes neither an identifier nor attributes here.
    /// Productions [24] to [26] give those relations neither, and only the
    /// default profile reads them anyway.
    fn takes_no_extras(&self, kind: Kind) -> bool {
        kind.is_bare()
            && !self
                .profile
                .allows(Relaxation::IdentifierOrAttributesOnBareKind)
    }

    // expression ::= keyword "(" optionalIdentifier arguments optionalAttributeValuePairs ")"

    fn expression(&mut self) -> Result<Record> {
        let (start, kind) = self.keyword_kind()?;
        self.expect(&TokenKind::LParen, "'('")?;
        let (id, mut args) = self.identifier_and_first_argument(kind)?;
        let attributes = self.remaining_arguments(kind, &mut args)?;
        Self::check_arity(start, kind, args.len())?;
        let formals = self.formals(kind, args)?;
        Record::from_parts(kind, id, formals, attributes).map_err(|e| at(start, e))
    }

    // keyword ::= the name of a PROV statement

    /// The keyword's token and kind, with the token consumed.
    fn keyword_kind(&mut self) -> Result<(&'t Token, Kind)> {
        let start = self.peek();
        let TokenKind::Name { prefix, local } = &start.kind else {
            return Err(self.expected("a statement"));
        };
        let kind = classify(prefix, local, self.profile).ok_or_else(|| {
            if prefix.is_empty() && local == Kind::Mention.name() {
                Self::expected_at(start, "'prov:mentionOf' under the strict profile")
            } else {
                Self::expected_found(
                    start,
                    "a PROV statement",
                    &format!("unknown keyword {}", describe(&start.kind)),
                )
            }
        })?;
        self.bump();
        Ok((start, kind))
    }

    // optionalIdentifier ::= (identifierOrMarker ";")?

    /// An element's mandatory identifier, or a relation's `id;` or `-;`
    /// before its first argument.
    fn identifier_and_first_argument(
        &mut self,
        kind: Kind,
    ) -> Result<(Option<QName>, Vec<Positional<'t>>)> {
        if kind.is_element() {
            let name = self.identifier("an identifier")?;
            return Ok((Some(self.resolve(&name)?), Vec::new()));
        }
        let first = self.argument()?;
        if self.peek().kind != TokenKind::Semicolon {
            return Ok((None, vec![first]));
        }
        let id = match first.1 {
            Arg::Name(q) => Some(q),
            Arg::Marker => None,
            Arg::Time(_) => {
                return Err(Self::expected_at(
                    first.0,
                    "an identifier or '-' before ';'",
                ));
            }
        };
        if self.takes_no_extras(kind) {
            return Err(Self::expected_at(
                first.0,
                &format!("no identifier in {} under the strict profile", kind.name()),
            ));
        }
        self.bump();
        Ok((id, vec![self.argument()?]))
    }

    /// The remaining arguments, pushed onto `args`, and the attribute block,
    /// up to the `)`. The argument list ends at `)` or at the `,` before `[`.
    fn remaining_arguments(
        &mut self,
        kind: Kind,
        args: &mut Vec<Positional<'t>>,
    ) -> Result<Attributes> {
        let closed = self.takes_no_extras(kind);
        let mut attributes = None;
        while self.peek().kind == TokenKind::Comma {
            self.bump();
            if self.peek().kind == TokenKind::LBracket {
                // Productions [24] to [26] give these relations no attribute
                // block. `prov:mentionOf` with one reads as a mention under
                // both profiles, a recorded departure.
                if closed && kind != Kind::Mention {
                    return Err(self.expected(&format!(
                        "no attributes in {} under the strict profile",
                        kind.name()
                    )));
                }
                attributes = Some(self.attribute_value_pairs()?);
                break;
            }
            args.push(self.argument()?);
        }
        // After the attribute block only ')' may follow.
        let closing = if attributes.is_some() {
            "')'"
        } else {
            "',' or ')'"
        };
        self.expect(&TokenKind::RParen, closing)?;
        Ok(attributes.unwrap_or_default())
    }

    /// An error at `start` unless `count` is one of the kind's arities.
    fn check_arity(start: &Token, kind: Kind, count: usize) -> Result<()> {
        let arities = kind.arities();
        if arities.contains(&count) {
            return Ok(());
        }
        let allowed = arities
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" or ");
        Err(Self::expected_found(
            start,
            &format!("{allowed} arguments to {}", kind.name()),
            &count.to_string(),
        ))
    }

    /// Match positional arguments to the kind's formals; `-` is absent.
    /// Under [`Profile::Strict`], `-` in the first `min(arities)` positions
    /// is an error, because the grammar has a plain identifier there.
    fn formals(&self, kind: Kind, args: Vec<Positional<'_>>) -> Result<Vec<(FormalAttr, Formal)>> {
        let required = kind.arities().iter().copied().min().unwrap_or(0);
        let mut out = Vec::new();
        for (i, (attr, (token, arg))) in kind.formals().iter().zip(args).enumerate() {
            let wanted = if attr.is_time() {
                "a dateTime or '-'"
            } else {
                "an identifier or '-'"
            };
            match arg {
                Arg::Marker
                    if !self.profile.allows(Relaxation::MarkerInRequiredPosition)
                        && i < required =>
                {
                    return Err(Self::expected_at(
                        token,
                        &format!(
                            "an identifier for argument {} of {} under the strict profile",
                            i + 1,
                            kind.name()
                        ),
                    ));
                }
                Arg::Marker => {}
                Arg::Time(t) if attr.is_time() => out.push((*attr, Formal::Time(t))),
                Arg::Name(q) if !attr.is_time() => out.push((*attr, Formal::Name(q))),
                Arg::Time(_) | Arg::Name(_) => {
                    return Err(Self::expected_at(token, &format!("{wanted} for {attr}")));
                }
            }
        }
        Ok(out)
    }

    // argument ::= identifier | time | "-"

    fn argument(&mut self) -> Result<Positional<'t>> {
        let token = self.peek();
        let arg = match &token.kind {
            TokenKind::Marker => {
                self.bump();
                Arg::Marker
            }
            TokenKind::DateTime(text) => {
                self.bump();
                Arg::Time(DateTime::parse(text).map_err(|e| at(token, e))?)
            }
            // A token that is not a name is refused by `identifier`.
            _ => {
                let name = self.identifier("an identifier, a dateTime or '-'")?;
                Arg::Name(self.resolve(&name)?)
            }
        };
        Ok((token, arg))
    }

    // optionalAttributeValuePairs ::= ("," "[" attributeValuePairs "]")?
    // The ',' is consumed by the argument loop.

    fn attribute_value_pairs(&mut self) -> Result<Attributes> {
        let mut attributes = Attributes::new();
        self.attribute_block(|parser, name| {
            let name = parser.resolve(&name)?;
            parser.expect(&TokenKind::Equals, "'='")?;
            attributes.insert(name, parser.literal()?);
            Ok(())
        })?;
        Ok(attributes)
    }

    // attributeValuePairs ::= ( | attributeValuePair ( "," attributeValuePair )* )
    // attributeValuePair ::= attribute "=" literal

    /// A bracketed attribute block, whose `[` is the current token. `pair`
    /// reads the rest of each pair after its name.
    pub(super) fn attribute_block(
        &mut self,
        mut pair: impl FnMut(&mut Self, NameParts<'t>) -> Result<()>,
    ) -> Result<()> {
        self.expect(&TokenKind::LBracket, "'['")?;
        if self.peek().kind == TokenKind::RBracket {
            self.bump();
            return Ok(());
        }
        loop {
            let name = self.identifier("an attribute name")?;
            pair(self, name)?;
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

    // literal ::= STRING_LITERAL ("@" LANGTAG | "%%" datatype)? | INT_LITERAL | QUALIFIED_NAME_LITERAL

    /// Consume a literal's tokens.
    pub(super) fn literal_parts(&mut self) -> Result<LiteralParts<'t>> {
        match &self.peek().kind {
            TokenKind::Str(s) => {
                self.bump();
                let suffix = match &self.peek().kind {
                    TokenKind::LangTag(tag) => {
                        self.bump();
                        Suffix::Lang(tag)
                    }
                    TokenKind::Typed => {
                        self.bump();
                        Suffix::Datatype(self.identifier("a datatype")?)
                    }
                    _ => Suffix::None,
                };
                Ok(LiteralParts::Str(s, suffix))
            }
            TokenKind::Int(i) => {
                self.bump();
                Ok(LiteralParts::Int(i))
            }
            TokenKind::QNameLiteral { prefix, local } => {
                self.bump();
                Ok(LiteralParts::Name { prefix, local })
            }
            _ => Err(self.expected("a literal")),
        }
    }

    /// A literal. A value error is placed at the literal's first token.
    fn literal(&mut self) -> Result<Value> {
        let start = self.peek();
        let value = match self.literal_parts()? {
            LiteralParts::Str(s, Suffix::None) => Ok(Value::Str(s.to_owned())),
            LiteralParts::Str(s, Suffix::Lang(tag)) => LangStr::new(s, tag).map(Value::LangStr),
            LiteralParts::Str(s, Suffix::Datatype(datatype)) => {
                let datatype = self.resolve(&datatype)?;
                Value::typed_in(self.scope(), s, datatype)
            }
            LiteralParts::Int(i) => Ok(Value::integer(i)),
            LiteralParts::Name { prefix, local } => {
                self.scope().resolve_parts(prefix, local).map(Value::QName)
            }
        };
        value.map_err(|e| at(start, e))
    }
}

/// A token as it reads in an error message.
pub(super) fn describe(kind: &TokenKind) -> String {
    let name = |prefix: &str, local: &str| {
        if prefix.is_empty() {
            local.to_owned()
        } else {
            format!("{prefix}:{local}")
        }
    };
    match kind {
        TokenKind::Name { prefix, local } => format!("'{}'", clip(&name(prefix, local))),
        TokenKind::QNameLiteral { prefix, local } => {
            format!(
                "the qualified-name literal '{}'",
                clip(&name(prefix, local))
            )
        }
        TokenKind::Iri(iri) => format!("the IRI <{}>", clip(iri)),
        TokenKind::Str(s) => format!("the string {:?}", clip(s)),
        TokenKind::Int(i) => format!("the integer {}", clip(i)),
        TokenKind::DateTime(d) => format!("the dateTime {}", clip(d)),
        TokenKind::LangTag(t) => format!("the language tag @{}", clip(t)),
        TokenKind::Typed => "'%%'".to_owned(),
        TokenKind::Marker => "'-'".to_owned(),
        TokenKind::LParen => "'('".to_owned(),
        TokenKind::RParen => "')'".to_owned(),
        TokenKind::LBracket => "'['".to_owned(),
        TokenKind::RBracket => "']'".to_owned(),
        TokenKind::Comma => "','".to_owned(),
        TokenKind::Semicolon => "';'".to_owned(),
        TokenKind::Equals => "'='".to_owned(),
        TokenKind::LBrace => "'{'".to_owned(),
        TokenKind::RBrace => "'}'".to_owned(),
        TokenKind::Eof => "end of input".to_owned(),
    }
}

#[cfg(test)]
mod tests;
