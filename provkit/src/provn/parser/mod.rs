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
    prefix: String,
    local: String,
}

/// A positional argument, resolved but not yet matched to a formal.
enum Arg {
    Name(QName),
    Time(DateTime),
    Marker,
}

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

    pub(super) fn error_at(token: &Token, message: String) -> Error {
        Error::Syntax {
            line: token.line,
            column: token.column,
            message,
        }
    }

    /// "expected `what`, found ..." at the current token.
    pub(super) fn expected(&self, what: &str) -> Error {
        let token = self.peek();
        Self::error_at(
            token,
            format!("expected {what}, found {}", describe(&token.kind)),
        )
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
            .resolve_parts(&name.prefix, &name.local)
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
            prefix: prefix.clone(),
            local: local.clone(),
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
                prefix: String::new(),
                local: digits.to_owned(),
            };
            self.bump();
            return Ok(parts);
        }
        self.name(what)
    }

    fn iri(&mut self) -> Result<String> {
        if let TokenKind::Iri(iri) = &self.peek().kind {
            self.bump();
            Ok(iri.clone())
        } else {
            Err(self.expected("an IRI in angle brackets"))
        }
    }

    // document ::= "document" namespaceDeclarations? (expression | bundle)* "endDocument"

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
                    return Err(Self::error_at(
                        self.peek(),
                        format!(
                            "expected 'bundle' or 'endDocument' after a bundle under the \
                             strict profile, found {}",
                            describe(&self.peek().kind)
                        ),
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
            && (is_name(i) || kind(i) == Some(&TokenKind::Marker))
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
        let start = self.peek();
        let TokenKind::Name { prefix, local } = &start.kind else {
            return Err(self.expected("a statement"));
        };
        let kind = classify(prefix, local, self.profile).ok_or_else(|| {
            let message = if prefix.is_empty() && local == Kind::Mention.name() {
                "expected 'prov:mentionOf' under the strict profile, found 'mentionOf'".to_owned()
            } else {
                format!(
                    "expected a PROV statement, found unknown keyword {}",
                    describe(&start.kind)
                )
            };
            Self::error_at(start, message)
        })?;
        self.bump();
        self.expect(&TokenKind::LParen, "'('")?;
        let closed = self.takes_no_extras(kind);

        // Elements take a mandatory identifier; relations an optional `id;` or `-;`.
        let mut id = None;
        let mut args: Vec<(&Token, Arg)> = Vec::new();
        if kind.is_element() {
            let name = self.identifier("an identifier")?;
            id = Some(self.resolve(&name)?);
        } else {
            let first = self.argument()?;
            if self.peek().kind == TokenKind::Semicolon {
                id = match first.1 {
                    Arg::Name(q) => Some(q),
                    Arg::Marker => None,
                    Arg::Time(_) => {
                        return Err(Self::error_at(
                            first.0,
                            format!(
                                "expected an identifier or '-' before ';', found {}",
                                describe(&first.0.kind)
                            ),
                        ));
                    }
                };
                if closed {
                    return Err(Self::error_at(
                        first.0,
                        format!(
                            "expected no identifier in {} under the strict profile, found {}",
                            kind.name(),
                            describe(&first.0.kind)
                        ),
                    ));
                }
                self.bump();
                args.push(self.argument()?);
            } else {
                args.push(first);
            }
        }

        // The argument list ends at ')' or at the ',' before '['.
        let mut attributes = None;
        while self.peek().kind == TokenKind::Comma {
            self.bump();
            if self.peek().kind == TokenKind::LBracket {
                // Productions [24] to [26] give these relations no attribute
                // block. `prov:mentionOf` with one reads as a mention under
                // both profiles, a recorded departure.
                if closed && kind != Kind::Mention {
                    return Err(Self::error_at(
                        self.peek(),
                        format!(
                            "expected no attributes in {} under the strict profile, found '['",
                            kind.name()
                        ),
                    ));
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
        let attributes = attributes.unwrap_or_default();

        let arities = kind.arities();
        if !arities.contains(&args.len()) {
            let allowed = arities
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" or ");
            return Err(Self::error_at(
                start,
                format!(
                    "expected {allowed} arguments to {}, found {}",
                    kind.name(),
                    args.len()
                ),
            ));
        }
        let formals = self.formals(kind, arities, args)?;
        Record::from_parts(kind, id, formals, attributes).map_err(|e| at(start, e))
    }

    /// Match positional arguments to the kind's formals; `-` is absent.
    /// Under [`Profile::Strict`], `-` in the first `min(arities)` positions
    /// is an error, because the grammar has a plain identifier there.
    fn formals(
        &self,
        kind: Kind,
        arities: &[usize],
        args: Vec<(&Token, Arg)>,
    ) -> Result<Vec<(FormalAttr, Formal)>> {
        let required = arities.iter().copied().min().unwrap_or(0);
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
                    return Err(Self::error_at(
                        token,
                        format!(
                            "expected an identifier for argument {} of {} under the strict \
                             profile, found '-'",
                            i + 1,
                            kind.name()
                        ),
                    ));
                }
                Arg::Marker => {}
                Arg::Time(t) if attr.is_time() => out.push((*attr, Formal::Time(t))),
                Arg::Name(q) if !attr.is_time() => out.push((*attr, Formal::Name(q))),
                Arg::Time(_) | Arg::Name(_) => {
                    return Err(Self::error_at(
                        token,
                        format!(
                            "expected {wanted} for prov:{}, found {}",
                            attr.local_name(),
                            describe(&token.kind)
                        ),
                    ));
                }
            }
        }
        Ok(out)
    }

    // argument ::= identifier | time | "-"

    fn argument(&mut self) -> Result<(&'t Token, Arg)> {
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
            TokenKind::Name { .. } | TokenKind::Int(_) => {
                let name = self.identifier("an identifier, a dateTime or '-'")?;
                Arg::Name(self.resolve(&name)?)
            }
            _ => return Err(self.expected("an identifier, a dateTime or '-'")),
        };
        Ok((token, arg))
    }

    // optionalAttributeValuePairs ::= ("," "[" attributeValuePairs "]")?
    // The ',' is consumed by the argument loop.

    fn attribute_value_pairs(&mut self) -> Result<Attributes> {
        self.expect(&TokenKind::LBracket, "'['")?;
        let mut attributes = Attributes::new();
        if self.peek().kind == TokenKind::RBracket {
            self.bump();
            return Ok(attributes);
        }
        loop {
            let name = self.identifier("an attribute name")?;
            let name = self.resolve(&name)?;
            self.expect(&TokenKind::Equals, "'='")?;
            let value = self.literal()?;
            attributes.insert(name, value);
            match self.peek().kind {
                TokenKind::Comma => {
                    self.bump();
                }
                TokenKind::RBracket => {
                    self.bump();
                    return Ok(attributes);
                }
                _ => return Err(self.expected("',' or ']'")),
            }
        }
    }

    // literal ::= STRING_LITERAL ("@" LANGTAG | "%%" datatype)? | INT_LITERAL | QUALIFIED_NAME_LITERAL

    /// A literal. A value error is placed at the literal's first token.
    fn literal(&mut self) -> Result<Value> {
        let start = self.peek();
        let value = match &start.kind {
            TokenKind::Str(s) => {
                self.bump();
                match &self.peek().kind {
                    TokenKind::LangTag(tag) => {
                        self.bump();
                        LangStr::new(s.as_str(), tag.as_str()).map(Value::LangStr)
                    }
                    TokenKind::Typed => {
                        self.bump();
                        let datatype = self.identifier("a datatype")?;
                        let datatype = self.resolve(&datatype)?;
                        Value::typed_in(self.scope(), s, datatype)
                    }
                    _ => Ok(Value::Str(s.clone())),
                }
            }
            TokenKind::Int(i) => {
                self.bump();
                Ok(Value::integer(i))
            }
            TokenKind::QNameLiteral { prefix, local } => {
                self.bump();
                self.scope().resolve_parts(prefix, local).map(Value::QName)
            }
            _ => return Err(self.expected("a literal")),
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
