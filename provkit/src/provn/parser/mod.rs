//! Recursive-descent parser for PROV-N (Recommendation section 3).
//!
//! One method per production. Names resolve through [`Namespaces`], records are
//! built by [`Record::from_parts`], and every event goes to a [`Sink`].
//!
//! Every error carries a position. A grammar error is [`Error::Syntax`], and
//! an error from the name, value or record layer is wrapped by [`at`].
//!
//! Extensibility expressions (section 5) are parsed by the methods in
//! `extension.rs` and recorded in [`Parser::ignored`] instead of the sink.

use super::lexer::{Token, TokenKind};
use super::{Ignored, Profile};
use crate::sink::Sink;
use crate::well_known::{PROV, XSD};
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
        prefix == "prov" || (prefix.is_empty() && profile == Profile::Default)
    } else {
        prefix.is_empty()
    };
    accepted.then_some(kind)
}

/// Whether the grammar gives `kind` neither an identifier nor attributes:
/// productions [24] to [26].
fn is_bare_relation(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Specialization | Kind::Alternate | Kind::Membership
    )
}

/// Attach `token`'s position to an error that has none.
fn at(token: &Token, error: Error) -> Error {
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
    /// Inside a bundle, its table with the document's behind it.
    bundle_names: Option<Namespaces>,
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
            bundle_names: None,
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

    /// The table names resolve against.
    fn names(&self) -> &Namespaces {
        self.bundle_names.as_ref().unwrap_or(&self.document_ns)
    }

    /// The name in the current scope. An error is placed at the name token.
    fn resolve(&self, name: &NameParts) -> Result<QName> {
        self.names()
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
        if let TokenKind::Int(digits) = &token.kind
            && !digits.starts_with('-')
        {
            let parts = NameParts {
                token,
                prefix: String::new(),
                local: digits.clone(),
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

    pub(super) fn document(&mut self, sink: &mut impl Sink) -> Result<()> {
        self.keyword("document")?;
        self.document_ns = self.namespace_declarations()?;
        sink.start_document(self.document_ns.clone())?;
        let mut after_bundle = false;
        while !self.is_keyword("endDocument") {
            if self.peek().kind == TokenKind::Eof {
                return Err(self.expected("'endDocument'"));
            }
            if self.is_keyword("bundle") {
                self.bundle(sink)?;
                after_bundle = true;
            } else {
                // Expressions come before bundles: production [1].
                if after_bundle && self.profile == Profile::Strict {
                    return Err(Self::error_at(
                        self.peek(),
                        format!(
                            "expected 'bundle' or 'endDocument' after a bundle under the \
                             strict profile, found {}",
                            describe(&self.peek().kind)
                        ),
                    ));
                }
                self.statement(sink)?;
            }
        }
        self.bump();
        if self.peek().kind != TokenKind::Eof {
            return Err(self.expected("end of input"));
        }
        sink.end_document()
    }

    // namespaceDeclarations ::= (defaultNamespaceDeclaration | namespaceDeclaration)*

    fn namespace_declarations(&mut self) -> Result<Namespaces> {
        let mut ns = Namespaces::new();
        let mut seen: Vec<String> = Vec::new();
        let mut has_default = false;
        loop {
            if self.is_keyword("default") {
                if has_default {
                    return Err(Self::error_at(
                        self.peek(),
                        "expected a namespace declaration or a statement, found a second \
                         'default' in one scope"
                            .to_owned(),
                    ));
                }
                // `default` comes before every `prefix`: production [45].
                if !seen.is_empty() && self.profile == Profile::Strict {
                    return Err(Self::error_at(
                        self.peek(),
                        "expected 'default' before every 'prefix' under the strict profile, \
                         found 'default' after a prefix"
                            .to_owned(),
                    ));
                }
                has_default = true;
                self.bump();
                let iri = self.iri()?;
                ns.set_default(&iri);
            } else if self.is_keyword("prefix") {
                self.bump();
                let name = self.name("a prefix")?;
                if !name.prefix.is_empty() {
                    return Err(Self::error_at(
                        name.token,
                        format!("expected a prefix, found {}", describe(&name.token.kind)),
                    ));
                }
                if seen.contains(&name.local) {
                    return Err(Self::error_at(
                        name.token,
                        format!(
                            "expected a new prefix, found '{}' declared twice in one scope",
                            name.local
                        ),
                    ));
                }
                let iri = self.iri()?;
                // `prov` and `xsd` are predeclared: section 3.7.4.
                let predeclared =
                    (name.local == "prov" && iri == PROV) || (name.local == "xsd" && iri == XSD);
                if predeclared && self.profile == Profile::Strict {
                    return Err(Self::error_at(
                        name.token,
                        format!(
                            "expected a prefix other than the predeclared '{}' under the \
                             strict profile, found 'prefix {} <{iri}>'",
                            name.local, name.local
                        ),
                    ));
                }
                ns.declare(&name.local, &iri)
                    .map_err(|e| at(name.token, e))?;
                seen.push(name.local);
            } else {
                return Ok(ns);
            }
        }
    }

    // bundle ::= "bundle" identifier namespaceDeclarations? expression* "endBundle"

    fn bundle(&mut self, sink: &mut impl Sink) -> Result<()> {
        self.bump();
        let id = self.identifier("a bundle identifier")?;
        let ns = self.namespace_declarations()?;
        // The bundle's own name resolves in the bundle scope (PROV-N 3.7.4).
        self.bundle_names = Some(ns.with_fallback(&self.document_ns));
        let name = self.resolve(&id)?;
        sink.start_bundle(name, ns).map_err(|e| at(id.token, e))?;
        while !self.is_keyword("endBundle") {
            if self.peek().kind == TokenKind::Eof
                || self.is_keyword("bundle")
                || self.is_keyword("endDocument")
            {
                return Err(self.expected("a statement or 'endBundle'"));
            }
            self.statement(sink)?;
        }
        self.bump();
        self.bundle_names = None;
        sink.end_bundle()
    }

    /// One statement: a PROV expression, sent to `sink` as a record, or an
    /// extensibility expression, parsed and recorded in `ignored`. A keyword
    /// with a prefix that names no PROV statement starts an extensibility
    /// expression (section 5); a bare unknown keyword is a syntax error.
    fn statement(&mut self, sink: &mut impl Sink) -> Result<()> {
        let is_extension = matches!(
            &self.peek().kind,
            TokenKind::Name { prefix, local }
                if !prefix.is_empty() && classify(prefix, local, self.profile).is_none()
        );
        if is_extension {
            self.extensibility_expression()
        } else {
            let record = self.expression()?;
            sink.record(record)
        }
    }

    // expression ::= keyword "(" optionalIdentifier arguments optionalAttributeValuePairs ")"

    fn expression(&mut self) -> Result<Record> {
        let start = self.peek();
        let TokenKind::Name { prefix, local } = &start.kind else {
            return Err(self.expected("a statement"));
        };
        let kind = classify(prefix, local, self.profile).ok_or_else(|| {
            let message = if prefix.is_empty() && local == "mentionOf" {
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
                if self.profile == Profile::Strict && is_bare_relation(kind) {
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
                if self.profile == Profile::Strict && is_bare_relation(kind) {
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
                Arg::Marker if self.profile == Profile::Strict && i < required => {
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
                        Value::typed_in(self.names(), s, datatype)
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
                self.names().resolve_parts(prefix, local).map(Value::QName)
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
        TokenKind::Name { prefix, local } => format!("'{}'", name(prefix, local)),
        TokenKind::QNameLiteral { prefix, local } => {
            format!("the qualified-name literal '{}'", name(prefix, local))
        }
        TokenKind::Iri(iri) => format!("the IRI <{iri}>"),
        TokenKind::Str(s) => format!("the string {s:?}"),
        TokenKind::Int(i) => format!("the integer {i}"),
        TokenKind::DateTime(d) => format!("the dateTime {d}"),
        TokenKind::LangTag(t) => format!("the language tag @{t}"),
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
