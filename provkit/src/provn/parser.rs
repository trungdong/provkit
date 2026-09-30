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
mod tests {
    use crate::provn::Profile;
    use crate::test_util::ex;
    use crate::{Document, Error, Kind, QName, Record, Value, provn};

    const HEAD: &str =
        "document\n  default <http://default.org/>\n  prefix ex <http://example.org/>\n";

    fn read(body: &str, profile: Profile) -> crate::Result<Document> {
        provn::read_with(&format!("{HEAD}{body}\nendDocument"), profile)
    }

    fn doc(body: &str, profile: Profile) -> Document {
        read(body, profile).unwrap()
    }

    fn err(body: &str, profile: Profile) -> Error {
        read(body, profile).unwrap_err()
    }

    fn syntax_message(e: Error) -> String {
        match e {
            Error::Syntax { message, .. } => message,
            other => panic!("expected a syntax error, got {other:?}"),
        }
    }

    /// The position and the inner error of an [`Error::At`].
    fn located(e: Error) -> ((usize, usize), Error) {
        match e {
            Error::At {
                line,
                column,
                error,
            } => ((line, column), *error),
            other => panic!("expected Error::At, got {other:?}"),
        }
    }

    /// The values of the first record's attributes, in order.
    fn values(d: &Document) -> Vec<&Value> {
        d.records[0].attributes().iter().map(|(_, v)| v).collect()
    }

    #[test]
    fn attribute_named_after_an_own_formal_is_rejected() {
        let text = "document\nprefix ex <http://example.org/>\n\
                    wasGeneratedBy(ex:e, ex:a, -, [prov:activity='ex:b'])\nendDocument\n";
        let e = provn::read(text).unwrap_err();
        // The message names the position and the kind, once each.
        assert_eq!(
            e.to_string(),
            "line 3, column 1: wasGeneratedBy: prov:activity is given as an attribute; \
             its value belongs in the positional argument"
        );
        let (position, inner) = located(e);
        assert_eq!(position, (3, 1));
        assert!(
            matches!(
                inner,
                Error::InvalidRecord {
                    kind: Kind::Generation,
                    ..
                }
            ),
            "{inner:?}"
        );
        // prov:type and prov:label on a relation stay attributes.
        let d = doc(
            "wasGeneratedBy(ex:e, ex:a, -, [prov:type='ex:T', prov:label=\"l\"])",
            Profile::Strict,
        );
        assert_eq!(d.records[0].attributes().len(), 2);
    }

    #[test]
    fn elements_relations_and_ids() {
        let d = doc(
            "entity(ex:e1)\nactivity(a1, 2011-11-16T16:05:33Z, -)\n\
             wasGeneratedBy(ex:g1; ex:e1, a1, -)\nwasGeneratedBy(ex:e1)\n\
             wasGeneratedBy(-; ex:e1, -, -)",
            Profile::Strict,
        );
        assert_eq!(d.records.len(), 5);
        assert!(
            matches!(&d.records[1], Record::Activity(a) if a.start_time.is_some() && a.end_time.is_none())
        );
        assert!(
            matches!(&d.records[2], Record::Generation(g) if g.id == Some(ex("g1")) && g.activity == Some(QName::new("http://default.org/", "a1").unwrap()))
        );
        assert!(
            matches!(&d.records[3], Record::Generation(g) if g.id.is_none() && g.activity.is_none())
        );
        assert!(
            matches!(&d.records[4], Record::Generation(g) if g.id.is_none() && g.entity == Some(ex("e1")))
        );
    }

    #[test]
    fn attribute_values() {
        let d = doc(
            r#"entity(ex:e, [prov:type='ex:T', ex:n=1, ex:s="a\"b", prov:label="x"@en, ex:d="2011-11-16T16:05:33Z" %% xsd:dateTime, ex:f="1.5" %% xsd:double, ex:t="t" %% xsd:string, ex:q="ex:v" %% xsd:QName, ex:l="ex:w" %% prov:QUALIFIED_NAME])"#,
            Profile::Strict,
        );
        let Record::Entity(e) = &d.records[0] else {
            panic!("expected an entity")
        };
        let vals: Vec<&Value> = e.attributes.iter().map(|(_, v)| v).collect();
        assert!(matches!(vals[0], Value::QName(q) if *q == ex("T")));
        assert!(matches!(vals[1], Value::Typed { lexical, .. } if lexical == "1"));
        assert_eq!(*vals[2], Value::Str("a\"b".into()));
        assert!(matches!(vals[3], Value::LangStr(_)));
        assert!(matches!(vals[4], Value::DateTime(_)));
        assert!(matches!(vals[5], Value::Typed { lexical, .. } if lexical == "1.5"));
        assert_eq!(*vals[6], Value::Str("t".into()));
        assert_eq!(*vals[7], Value::QName(ex("v")));
        assert_eq!(*vals[8], Value::QName(ex("w")));
        // An untyped dateTime is not a literal.
        assert!(matches!(
            err(
                "entity(ex:e, [ex:d=2011-11-16T16:05:33Z])",
                Profile::Default
            ),
            Error::Syntax { .. }
        ));
    }

    #[test]
    fn profiles() {
        assert!(matches!(
            err("mentionOf(ex:a, ex:b, ex:c)", Profile::Strict),
            Error::Syntax { .. }
        ));
        assert_eq!(
            doc("mentionOf(ex:a, ex:b, ex:c)", Profile::Default).records[0].kind(),
            Kind::Mention
        );
        assert_eq!(
            doc("prov:mentionOf(ex:a, ex:b, ex:c)", Profile::Strict).records[0].kind(),
            Kind::Mention
        );
        assert!(matches!(
            err("wasGeneratedBy(-, ex:a, -)", Profile::Strict),
            Error::Syntax { .. }
        ));
        assert!(
            matches!(&doc("wasGeneratedBy(-, ex:a, -)", Profile::Default).records[0], Record::Generation(g) if g.entity.is_none())
        );
        // Beyond the leading required positions, `-` is fine under Strict.
        assert!(read("wasDerivedFrom(ex:a, ex:b, -, -, -)", Profile::Strict).is_ok());
    }

    #[test]
    fn mention_takes_attributes() {
        assert!(
            read(
                "prov:mentionOf(ex:a, ex:b, ex:c, [ex:k=\"v\"])",
                Profile::Strict
            )
            .is_ok()
        );
    }

    #[test]
    fn strict_is_the_whole_grammar() {
        let p = "prefix ex <http://example.org/>\n";
        let cases = [
            (
                format!("document\n{p}default <http://d/>\nentity(e)\nendDocument\n"),
                (3, 1),
            ),
            (
                format!("document\n{p}bundle ex:b\nendBundle\nentity(ex:f)\nendDocument\n"),
                (5, 1),
            ),
            (
                format!(
                    "document\n{p}bundle ex:b\nprefix ey <http://e/>\ndefault <http://d/>\nendBundle\nendDocument\n"
                ),
                (5, 1),
            ),
            (
                format!("document\n{p}hadMember(ex:m; ex:c, ex:e)\nendDocument\n"),
                (3, 11),
            ),
            (
                format!("document\n{p}alternateOf(ex:a; ex:b, ex:c)\nendDocument\n"),
                (3, 13),
            ),
            (
                format!("document\n{p}specializationOf(ex:a, ex:b, [ex:k=\"x\"])\nendDocument\n"),
                (3, 30),
            ),
            (
                format!(
                    "document\n{p}prefix prov <http://www.w3.org/ns/prov#>\nentity(ex:e)\nendDocument\n"
                ),
                (3, 8),
            ),
            (
                format!(
                    "document\n{p}prefix xsd <http://www.w3.org/2001/XMLSchema#>\nentity(ex:e)\nendDocument\n"
                ),
                (3, 8),
            ),
        ];
        for (text, position) in cases {
            assert!(provn::read_with(&text, Profile::Default).is_ok(), "{text}");
            match provn::read_with(&text, Profile::Strict) {
                Err(Error::Syntax { line, column, .. }) => {
                    assert_eq!((line, column), position, "{text}");
                }
                other => panic!("{text}: expected a syntax error, got {other:?}"),
            }
        }
    }

    #[test]
    fn strict_reads_an_empty_bundle() {
        let text =
            "document\nprefix ex <http://example.org/>\nbundle ex:b\nendBundle\nendDocument\n";
        for profile in [Profile::Strict, Profile::Default] {
            let d = provn::read_with(text, profile).unwrap();
            assert_eq!(d.bundles.len(), 1);
            assert!(d.bundles[0].records.is_empty());
        }
    }

    #[test]
    fn a_second_default_and_a_moved_reserved_prefix_fail_under_both_profiles() {
        let twice = "document\ndefault <http://a/>\ndefault <http://b/>\nendDocument\n";
        let moved = "document\nprefix prov <http://other/>\nendDocument\n";
        for profile in [Profile::Strict, Profile::Default] {
            assert!(matches!(
                provn::read_with(twice, profile),
                Err(Error::Syntax {
                    line: 3,
                    column: 1,
                    ..
                })
            ));
            let (position, inner) = located(provn::read_with(moved, profile).unwrap_err());
            assert_eq!(position, (2, 8));
            assert!(matches!(inner, Error::ReservedPrefix { .. }), "{inner:?}");
        }
    }

    #[test]
    fn integer_literals_type_by_range() {
        let d = doc(
            "entity(ex:e, [ex:a=42, ex:b=3000000000, ex:c=99999999999999999999, \
             ex:d=-2147483649, ex:e=007])",
            Profile::Strict,
        );
        let datatypes: Vec<_> = values(&d)
            .iter()
            .map(|v| v.datatype().unwrap().local().to_owned())
            .collect();
        assert_eq!(datatypes, ["int", "long", "integer", "long", "int"]);
        let texts: Vec<_> = values(&d)
            .iter()
            .map(|v| match v {
                Value::Typed { lexical, .. } => lexical.as_str(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            texts,
            [
                "42",
                "3000000000",
                "99999999999999999999",
                "-2147483649",
                "007"
            ]
        );
    }

    #[test]
    fn a_bare_minus_zero_is_an_int_and_a_plus_sign_is_a_syntax_error() {
        let d = doc("entity(ex:e, [ex:a=-0])", Profile::Strict);
        assert!(matches!(
            values(&d)[0],
            Value::Typed { lexical, datatype } if lexical == "-0" && datatype.local() == "int"
        ));
        for profile in [Profile::Strict, Profile::Default] {
            assert!(matches!(
                err("entity(ex:e, [ex:a=+5])", profile),
                Error::Syntax { .. }
            ));
        }
    }

    #[test]
    fn a_language_tag_ends_at_an_underscore_and_the_rest_is_a_syntax_error() {
        for profile in [Profile::Strict, Profile::Default] {
            assert!(matches!(
                err(r#"entity(ex:e, [ex:a="x" @en_US])"#, profile),
                Error::Syntax { .. }
            ));
        }
    }

    #[test]
    fn a_brace_outside_a_dictionary_is_a_syntax_error_at_the_brace() {
        match provn::read_with("document { endDocument", Profile::Default) {
            Err(Error::Syntax { line, column, .. }) => assert_eq!((line, column), (1, 10)),
            other => panic!("expected a syntax error, got {other:?}"),
        }
    }

    #[test]
    fn a_datatype_may_be_all_digits() {
        let d = doc(r#"entity(ex:e, [ex:k="x" %% 123])"#, Profile::Strict);
        assert_eq!(
            values(&d)[0].datatype().unwrap().iri(),
            "http://default.org/123"
        );
    }

    #[test]
    fn arity_and_structure_errors() {
        let message = syntax_message(err("wasGeneratedBy(ex:e, ex:a)", Profile::Default));
        assert!(message.contains("1 or 3"), "{message}");
        assert!(message.starts_with("expected"), "{message}");
        assert!(matches!(
            err("entity()", Profile::Default),
            Error::Syntax { .. }
        ));
        assert!(matches!(
            err(
                "bundle ex:b\n bundle ex:c\n endBundle\nendBundle",
                Profile::Default
            ),
            Error::Syntax { .. }
        ));
        assert!(matches!(
            located(err("entity(zz:e)", Profile::Default)),
            (_, Error::UndeclaredPrefix(p)) if p == "zz"
        ));
        assert!(matches!(
            provn::read("entity(ex:e)"),
            Err(Error::Syntax {
                line: 1,
                column: 1,
                ..
            })
        ));
        // A time where a name belongs, and a name where a time belongs.
        assert!(matches!(
            err(
                "wasGeneratedBy(ex:e, 2011-11-16T16:05:33Z, -)",
                Profile::Default
            ),
            Error::Syntax { .. }
        ));
        assert!(matches!(
            err("wasGeneratedBy(ex:e, ex:a, ex:t)", Profile::Default),
            Error::Syntax { .. }
        ));
    }

    #[test]
    fn errors_carry_positions() {
        assert!(matches!(
            err("entity(ex:e]", Profile::Default),
            Error::Syntax {
                line: 4,
                column: 12,
                ..
            }
        ));
        assert!(matches!(
            provn::read("document\nprefix ex <http://e/>\nentity(ex:e)\n"),
            Err(Error::Syntax {
                line: 4,
                column: 1,
                ..
            })
        ));
        // A name the grammar accepts and the namespace table does not.
        let text = "document\nprefix ex <http://example.org/>\nentity(zz:e)\nendDocument\n";
        assert!(matches!(
            located(provn::read(text).unwrap_err()),
            ((3, 8), Error::UndeclaredPrefix(p)) if p == "zz"
        ));
    }

    #[test]
    fn each_layer_error_sits_at_its_token() {
        let read = |body: &str| {
            let text = format!("document\nprefix ex <http://example.org/>\n{body}\nendDocument\n");
            located(provn::read(&text).unwrap_err())
        };
        // An undeclared prefix, at the token that carries it: an attribute
        // name, a datatype, a name argument, a bundle identifier, and a
        // literal's first token for a qualified-name value.
        for (body, position) in [
            ("entity(ex:e, [zz:k=1])", (3, 15)),
            ("entity(ex:e, [ex:k=\"1\" %% zz:t])", (3, 27)),
            ("used(ex:a, zz:e, -)", (3, 12)),
            ("bundle zz:b\nendBundle", (3, 8)),
            ("entity(ex:e, [ex:k=\"zz:v\" %% xsd:QName])", (3, 20)),
        ] {
            let (found, inner) = read(body);
            assert_eq!(found, position, "{body}");
            assert!(
                matches!(&inner, Error::UndeclaredPrefix(p) if p == "zz"),
                "{body}: {inner:?}"
            );
        }
        assert!(matches!(
            read("entity(e)"),
            ((3, 8), Error::NoDefaultNamespace(l)) if l == "e"
        ));
        // A bare qualified-name value, at the literal's first token.
        assert!(matches!(
            read("entity(ex:e, [ex:k=\"v\" %% xsd:QName])"),
            ((3, 20), Error::NoDefaultNamespace(l)) if l == "v"
        ));
        // A dateTime argument, at its own token.
        assert!(matches!(
            read("activity(ex:a, -, 2012-13-01T00:00:00)"),
            ((3, 19), Error::InvalidLiteral { .. })
        ));
        // A reserved prefix, at the prefix token.
        let text = "document\nprefix prov <http://example.org/>\nendDocument\n";
        assert!(matches!(
            located(provn::read(text).unwrap_err()),
            ((2, 8), Error::ReservedPrefix { .. })
        ));
    }

    #[test]
    fn every_reader_error_has_a_position() {
        let cases = [
            ("entity(zz:e)", 3, 8),                            // undeclared prefix
            ("entity(e)", 3, 8),                               // no default namespace
            ("entity(ex:e, [ex:k='zz:v'])", 3, 20),            // undeclared prefix in a value
            ("activity(ex:a, 2012-13-01T00:00:00, -)", 3, 16), // bad dateTime argument
            (
                "wasGeneratedBy(ex:e, ex:a, -, [prov:activity='ex:b'])",
                3,
                1,
            ),
            ("bundle ex:b\nendBundle\nbundle ex:b\nendBundle", 5, 8), // duplicate bundle
        ];
        for (body, line, column) in cases {
            let text = format!("document\nprefix ex <http://example.org/>\n{body}\nendDocument\n");
            match provn::read(&text) {
                Err(Error::At {
                    line: l, column: c, ..
                }) => assert_eq!((l, c), (line, column), "{body}"),
                other => panic!("{body}: expected Error::At, got {other:?}"),
            }
        }
    }

    #[test]
    fn only_a_closing_paren_follows_attributes() {
        let message = syntax_message(err("entity(ex:e, [ex:a=1], ex:b)", Profile::Default));
        assert!(message.starts_with("expected ')'"), "{message}");
    }

    #[test]
    fn nothing_follows_end_document() {
        match provn::read("document\nendDocument\nentity(ex:e)") {
            Err(Error::Syntax {
                line,
                column,
                message,
                ..
            }) => {
                assert_eq!((line, column), (3, 1));
                assert!(message.starts_with("expected end of input"), "{message}");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn second_default_in_one_scope_is_rejected() {
        let text = "document\n default <http://a/>\n default <http://b/>\nendDocument";
        assert!(matches!(
            provn::read(text),
            Err(Error::Syntax { line: 3, .. })
        ));
    }

    #[test]
    fn a_prefix_is_not_an_integer() {
        let text = "document\n prefix 123 <http://a/>\nendDocument";
        assert!(matches!(
            provn::read(text),
            Err(Error::Syntax { line: 2, .. })
        ));
    }

    #[test]
    fn digit_names_and_empty_attributes() {
        let d = doc("entity(123, [])\nwasDerivedFrom(ex:a, 42)", Profile::Strict);
        assert_eq!(
            d.records[0].id(),
            Some(&QName::new("http://default.org/", "123").unwrap())
        );
        assert!(
            matches!(&d.records[1], Record::Derivation(r) if r.used_entity == Some(QName::new("http://default.org/", "42").unwrap()))
        );
    }

    #[test]
    fn escaped_colon_in_a_bare_name_stays_local() {
        let d = doc(r"entity(ex\:e1)", Profile::Strict);
        assert_eq!(
            d.records[0].id(),
            Some(&QName::new("http://default.org/", "ex:e1").unwrap())
        );
    }

    #[test]
    fn qualified_name_datatypes_read_as_names() {
        let d = doc(
            r#"entity(ex:e, [ex:a="ex:v" %% prov:QUALIFIED_NAME, ex:b="ex:v" %% xsd:QName])"#,
            Profile::Strict,
        );
        assert_eq!(values(&d), [&Value::QName(ex("v")), &Value::QName(ex("v"))]);
        assert!(matches!(
            located(err(r#"entity(ex:e, [ex:a="zz:v" %% prov:QUALIFIED_NAME])"#, Profile::Default)),
            (_, Error::UndeclaredPrefix(p)) if p == "zz"
        ));
        // A name the grammar writes with an escape is spelled here without it.
        let d = doc(
            r#"entity(ex:e, [ex:a="ex:a=b" %% xsd:QName, ex:b='ex:a\=b'])"#,
            Profile::Strict,
        );
        assert_eq!(values(&d), [&Value::QName(ex("a=b")); 2]);
    }

    #[test]
    fn a_qualified_name_literal_whose_text_is_not_a_name_is_an_error() {
        for (literal, text) in [
            (r#""not a name" %% prov:QUALIFIED_NAME"#, "not a name"),
            (r#""ex:a b" %% xsd:QName"#, "ex:a b"),
            (r#""ex:a%4" %% xsd:QName"#, "ex:a%4"),
            (r#""" %% xsd:QName"#, ""),
        ] {
            let (position, inner) = located(err(
                &format!("entity(ex:e, [ex:a={literal}])"),
                Profile::Strict,
            ));
            assert_eq!(position, (4, 20), "{literal}");
            assert!(
                matches!(&inner, Error::InvalidQName(t) if t == text),
                "{literal}: {inner:?}"
            );
        }
    }

    #[test]
    fn a_local_part_provn_cannot_write_is_a_syntax_error() {
        // The lexer ends a name at a `%` without two hex digits and takes
        // no `\u` escape in a name, so neither reaches the resolver.
        for body in [r"entity(ex:a%4)", r"entity(ex:a\u00E9)", "entity(ex:a b)"] {
            assert!(
                matches!(err(body, Profile::Strict), Error::Syntax { .. }),
                "{body}"
            );
        }
    }

    #[test]
    fn ill_formed_literals_read_and_round_trip() {
        let literals = [
            (
                r#""2012-13-01T00:00:00" %% xsd:dateTime"#,
                "2012-13-01T00:00:00",
            ),
            (r#""x" %% xsd:dateTime"#, "x"),
            (r#""abc" %% xsd:int"#, "abc"),
            (r#""maybe" %% xsd:boolean"#, "maybe"),
            (r#""1,5" %% xsd:decimal"#, "1,5"),
        ];
        let pairs: Vec<String> = literals
            .iter()
            .enumerate()
            .map(|(i, (literal, _))| format!("ex:k{i}={literal}"))
            .collect();
        let d = doc(
            &format!("entity(ex:e, [{}])", pairs.join(", ")),
            Profile::Strict,
        );
        let read = values(&d);
        assert_eq!(read.len(), literals.len());
        for (value, (literal, text)) in read.iter().zip(literals) {
            assert!(
                matches!(value, Value::Typed { lexical, .. } if lexical == text),
                "{literal}: {value:?}"
            );
        }
        // Each literal is written with the text it was read with.
        let written = provn::to_string(&d).unwrap();
        for (literal, _) in literals {
            assert!(written.contains(literal), "{literal}\n{written}");
        }
        assert_eq!(provn::read(&written).unwrap(), d);
        #[cfg(feature = "json")]
        {
            let json = crate::json::to_string(&d).unwrap();
            assert_eq!(crate::json::read(&json).unwrap(), d, "{json}");
        }
    }

    #[test]
    fn long_language_subtags_read() {
        let d = doc(
            r#"entity(ex:e, [ex:k="x"@abcdefghi, ex:l="x"@en-GB-x-averylongsubtag])"#,
            Profile::Strict,
        );
        let tags: Vec<&str> = values(&d)
            .into_iter()
            .map(|v| match v {
                Value::LangStr(l) => l.lang(),
                other => panic!("expected a tagged string, got {other:?}"),
            })
            .collect();
        assert_eq!(tags, ["abcdefghi", "en-GB-x-averylongsubtag"]);
    }

    #[test]
    fn an_ill_formed_time_argument_is_an_error() {
        for body in [
            "activity(ex:a, 2012-13-01T00:00:00, -)",
            "wasGeneratedBy(ex:e, ex:a, 2012-02-30T00:00:00)",
        ] {
            let (_, inner) = located(err(body, Profile::Default));
            assert!(
                matches!(&inner, Error::InvalidLiteral { datatype, .. } if datatype == "xsd:dateTime"),
                "{body}: {inner:?}"
            );
        }
    }

    #[test]
    fn literals_compare_by_value_and_keep_their_text() {
        let body = r#"entity(ex:e, [ex:i=005, ex:j="+5" %% xsd:int, ex:d="1.0E0" %% xsd:double, ex:b="1" %% xsd:boolean, ex:c="1.50" %% xsd:decimal])"#;
        let other = r#"entity(ex:e, [ex:i=5, ex:j=" 5 " %% xsd:int, ex:d="1" %% xsd:double, ex:b="true" %% xsd:boolean, ex:c="1.5" %% xsd:decimal])"#;
        let d = doc(body, Profile::Strict);
        assert_eq!(d, doc(other, Profile::Strict));
        assert_ne!(d, doc(&other.replace("ex:i=5", "ex:i=6"), Profile::Strict));
        assert_ne!(
            d,
            doc(
                &other.replace("ex:i=5", r#"ex:i="5" %% xsd:long"#),
                Profile::Strict
            )
        );
        let written = provn::to_string(&d).unwrap();
        for literal in [
            "ex:i=005",
            r#"ex:j="+5" %% xsd:int"#,
            r#"ex:d="1.0E0" %% xsd:double"#,
            r#"ex:b="1" %% xsd:boolean"#,
            r#"ex:c="1.50" %% xsd:decimal"#,
        ] {
            assert!(written.contains(literal), "{literal}\n{written}");
        }
        let lexicals = |d: &Document| -> Vec<String> {
            values(d)
                .into_iter()
                .map(|v| match v {
                    Value::Typed { lexical, .. } => lexical.clone(),
                    other => panic!("expected a typed literal, got {other:?}"),
                })
                .collect()
        };
        assert_eq!(lexicals(&provn::read(&written).unwrap()), lexicals(&d));
        #[cfg(feature = "json")]
        {
            let json = crate::json::to_string(&d).unwrap();
            assert_eq!(lexicals(&crate::json::read(&json).unwrap()), lexicals(&d));
        }
    }

    #[test]
    fn equal_values_under_one_name_are_one_pair() {
        let d = doc(
            r#"entity(ex:e, [ex:v=" 5 " %% xsd:int, ex:v="5" %% xsd:int, ex:v=5, ex:v="5" %% xsd:long])"#,
            Profile::Strict,
        );
        // The first spelling stays; the `xsd:long` is another value.
        let read = values(&d);
        assert_eq!(read.len(), 2);
        assert!(matches!(read[0], Value::Typed { lexical, .. } if lexical == " 5 "));
        assert_eq!(
            read[1].datatype(),
            Some(crate::well_known::XSD_LONG.clone())
        );
    }

    #[test]
    fn bundles_scope_prefixes() {
        let d = doc(
            "bundle ex:b\n  prefix b <http://b.org/>\n  entity(b:e)\n  entity(ex:f)\nendBundle",
            Profile::Strict,
        );
        assert_eq!(d.bundles[0].id, ex("b"));
        assert_eq!(
            d.bundles[0].records[0].id(),
            Some(&QName::new("http://b.org/", "e").unwrap())
        );
        assert_eq!(d.bundles[0].records[1].id(), Some(&ex("f")));
        assert_eq!(d.bundles[0].namespaces.iter().count(), 1);
    }

    #[test]
    fn bundle_identifier_resolves_in_its_own_scope() {
        let d = doc(
            "bundle ex:b\n  prefix ex <http://bundle.org/>\n  entity(ex:e)\nendBundle\nentity(ex:e)",
            Profile::Default,
        );
        assert_eq!(
            d.bundles[0].id,
            QName::new("http://bundle.org/", "b").unwrap()
        );
        assert_eq!(
            d.bundles[0].records[0].id(),
            Some(&QName::new("http://bundle.org/", "e").unwrap())
        );
        // The bundle's declarations end with the bundle.
        assert_eq!(d.records[0].id(), Some(&ex("e")));
    }

    #[test]
    fn duplicate_prefix_in_one_scope_is_rejected() {
        let text = "document\n prefix ex <http://a/>\n prefix ex <http://b/>\nendDocument";
        assert!(matches!(
            provn::read(text),
            Err(Error::Syntax { line: 3, .. })
        ));
    }
}
