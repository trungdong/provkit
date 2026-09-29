//! Recursive-descent parser for PROV-N (Recommendation section 3).
//!
//! One method per production. Names resolve through [`Scope`], records are
//! built by [`Record::from_parts`], and every event goes to a [`Sink`].

use super::Profile;
use super::lexer::{Token, TokenKind};
use crate::qname::well_known::{PROV_QUALIFIED_NAME, XSD_INT, XSD_QNAME};
use crate::{
    Attributes, DateTime, Error, Formal, FormalAttr, Kind, LangStr, Namespaces, QName, Record,
    Result, Scope, Sink, Value,
};

/// The statement keyword `prefix:local` as a kind and its allowed argument
/// counts after any `id;`, or `None` for anything else.
fn classify(prefix: &str, local: &str, profile: Profile) -> Option<(Kind, &'static [usize])> {
    if (prefix, local) == ("prov", "mentionOf")
        || (profile == Profile::Default && (prefix, local) == ("", "mentionOf"))
    {
        return Some((Kind::Mention, &[3]));
    }
    if !prefix.is_empty() {
        return None;
    }
    Some(match local {
        "entity" | "agent" => (Kind::from_json_key(local)?, &[0]),
        "activity" => (Kind::Activity, &[0, 2]),
        "wasGeneratedBy" | "used" | "wasInvalidatedBy" | "wasAssociatedWith" => {
            (Kind::from_json_key(local)?, &[1, 3])
        }
        "wasStartedBy" | "wasEndedBy" => (Kind::from_json_key(local)?, &[1, 4]),
        "wasInformedBy" | "wasAttributedTo" | "wasInfluencedBy" | "alternateOf"
        | "specializationOf" | "hadMember" => (Kind::from_json_key(local)?, &[2]),
        "actedOnBehalfOf" => (Kind::Delegation, &[2, 3]),
        "wasDerivedFrom" => (Kind::Derivation, &[2, 5]),
        _ => return None,
    })
}

/// A name token split into its parts, before resolution.
struct NameParts {
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
    tokens: &'t [Token],
    pos: usize,
    profile: Profile,
    document_ns: Namespaces,
    bundle_ns: Option<Namespaces>,
}

impl<'t> Parser<'t> {
    /// A parser over `tokens`, which end with `Eof`.
    pub(super) fn new(tokens: &'t [Token], profile: Profile) -> Self {
        Parser {
            tokens,
            pos: 0,
            profile,
            document_ns: Namespaces::new(),
            bundle_ns: None,
        }
    }

    // Token helpers

    fn peek(&self) -> &'t Token {
        &self.tokens[self.pos]
    }

    /// The current token, advancing unless it is `Eof`.
    fn bump(&mut self) -> &'t Token {
        let token = &self.tokens[self.pos];
        if token.kind != TokenKind::Eof {
            self.pos += 1;
        }
        token
    }

    fn error_at(token: &Token, message: String) -> Error {
        Error::Syntax {
            format: "PROV-N",
            line: token.line,
            column: token.column,
            message,
        }
    }

    /// "expected `what`, found ..." at the current token.
    fn expected(&self, what: &str) -> Error {
        let token = self.peek();
        Self::error_at(
            token,
            format!("expected {what}, found {}", describe(&token.kind)),
        )
    }

    fn expect(&mut self, kind: &TokenKind, what: &str) -> Result<()> {
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

    fn scope(&self) -> Scope<'_> {
        Scope::new(&self.document_ns, self.bundle_ns.as_ref())
    }

    fn resolve(&self, name: &NameParts) -> Result<QName> {
        self.scope().resolve_parts(&name.prefix, &name.local)
    }

    /// A name token.
    fn name(&mut self, what: &str) -> Result<NameParts> {
        let TokenKind::Name { prefix, local } = &self.peek().kind else {
            return Err(self.expected(what));
        };
        let parts = NameParts {
            prefix: prefix.clone(),
            local: local.clone(),
        };
        self.bump();
        Ok(parts)
    }

    /// A name token in an identifier or attribute-name position, where an
    /// unsigned integer is a bare local name made of digits. The lexer
    /// cannot tell that apart from `INT_LITERAL`.
    fn identifier(&mut self, what: &str) -> Result<NameParts> {
        if let TokenKind::Int(digits) = &self.peek().kind
            && !digits.starts_with('-')
        {
            let parts = NameParts {
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
        while !self.is_keyword("endDocument") {
            if self.peek().kind == TokenKind::Eof {
                return Err(self.expected("'endDocument'"));
            }
            if self.is_keyword("bundle") {
                self.bundle(sink)?;
            } else {
                let record = self.expression()?;
                sink.record(record)?;
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
                has_default = true;
                self.bump();
                let iri = self.iri()?;
                ns.set_default(iri);
            } else if self.is_keyword("prefix") {
                self.bump();
                let token = self.peek();
                let name = self.name("a prefix")?;
                if !name.prefix.is_empty() {
                    return Err(Self::error_at(
                        token,
                        format!("expected a prefix, found {}", describe(&token.kind)),
                    ));
                }
                if seen.contains(&name.local) {
                    return Err(Self::error_at(
                        token,
                        format!(
                            "expected a new prefix, found '{}' declared twice in one scope",
                            name.local
                        ),
                    ));
                }
                let iri = self.iri()?;
                ns.declare(&name.local, &iri)?;
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
        self.bundle_ns = Some(ns.clone());
        let id = self.resolve(&id)?;
        sink.start_bundle(id, ns)?;
        while !self.is_keyword("endBundle") {
            if self.peek().kind == TokenKind::Eof
                || self.is_keyword("bundle")
                || self.is_keyword("endDocument")
            {
                return Err(self.expected("a statement or 'endBundle'"));
            }
            let record = self.expression()?;
            sink.record(record)?;
        }
        self.bump();
        self.bundle_ns = None;
        sink.end_bundle()
    }

    // expression ::= keyword "(" optionalIdentifier arguments optionalAttributeValuePairs ")"

    fn expression(&mut self) -> Result<Record> {
        let start = self.peek();
        let TokenKind::Name { prefix, local } = &start.kind else {
            return Err(self.expected("a statement"));
        };
        let (kind, arities) = classify(prefix, local, self.profile).ok_or_else(|| {
            let message = if !prefix.is_empty() {
                format!(
                    "expected a PROV statement, found the extensibility expression \
                     '{prefix}:{local}(...)', which is not supported"
                )
            } else if local == "mentionOf" {
                "expected 'prov:mentionOf' under the strict profile, found 'mentionOf'".to_owned()
            } else {
                format!("expected a PROV statement, found unknown keyword '{local}'")
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
                    kind.provn_keyword(),
                    args.len()
                ),
            ));
        }
        let formals = self.formals(kind, arities, args)?;
        Record::from_parts(kind, id, formals, attributes)
            .map_err(|e| Self::error_at(start, e.to_string()))
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
                            kind.provn_keyword()
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
                Arg::Time(DateTime::parse(text)?)
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

    fn literal(&mut self) -> Result<Value> {
        let value = match &self.peek().kind {
            TokenKind::Str(s) => {
                self.bump();
                match &self.peek().kind {
                    TokenKind::LangTag(tag) => {
                        self.bump();
                        Value::LangStr(LangStr::new(s.as_str(), tag.as_str())?)
                    }
                    TokenKind::Typed => {
                        self.bump();
                        let datatype = self.name("a datatype")?;
                        let datatype = self.resolve(&datatype)?;
                        if datatype == *XSD_QNAME || datatype == *PROV_QUALIFIED_NAME {
                            Value::QName(self.scope().resolve(s)?)
                        } else {
                            Value::typed(s.as_str(), datatype)?
                        }
                    }
                    _ => Value::Str(s.clone()),
                }
            }
            TokenKind::Int(i) => {
                self.bump();
                Value::Typed {
                    lexical: i.clone(),
                    datatype: XSD_INT.clone(),
                }
            }
            TokenKind::QNameLiteral { prefix, local } => {
                self.bump();
                Value::QName(self.scope().resolve_parts(prefix, local)?)
            }
            _ => return Err(self.expected("a literal")),
        };
        Ok(value)
    }
}

/// A token as it reads in an error message.
fn describe(kind: &TokenKind) -> String {
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
        TokenKind::Eof => "end of input".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{Document, Error, Kind, Profile, QName, Record, Value, provn};

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

    fn ex(l: &str) -> QName {
        QName::new("http://example.org/", l)
    }

    fn syntax_message(e: Error) -> String {
        match e {
            Error::Syntax { message, .. } => message,
            other => panic!("expected a syntax error, got {other:?}"),
        }
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
            matches!(&d.records[2], Record::Generation(g) if g.id == Some(ex("g1")) && g.activity == Some(QName::new("http://default.org/", "a1")))
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
        assert!(matches!(
            err("ex:custom(ex:a)", Profile::Default),
            Error::Syntax { .. }
        ));
    }

    #[test]
    fn strict_enforces_three_rules_only() {
        // Extensibility expressions are rejected under both profiles.
        assert!(matches!(
            err("ex:custom(ex:a)", Profile::Strict),
            Error::Syntax { .. }
        ));
        // Attributes on alternateOf, specializationOf, hadMember and
        // mentionOf are accepted.
        for text in [
            "alternateOf(ex:a, ex:b)",
            "specializationOf(ex:a, ex:b)",
            "hadMember(ex:c, ex:e)",
            "prov:mentionOf(ex:a, ex:b, ex:c)",
        ] {
            let with_attrs = text.replace(')', ", [ex:k=\"v\"])");
            assert!(read(&with_attrs, Profile::Strict).is_ok(), "{with_attrs}");
        }
        // Identifiers on those relations are accepted.
        assert!(read("alternateOf(ex:i; ex:a, ex:b)", Profile::Strict).is_ok());
        // A bundle may come before a top-level record.
        let text = "document\nprefix ex <http://e/>\nbundle ex:b\nentity(ex:x)\nendBundle\nentity(ex:e)\nendDocument";
        assert!(provn::read_with(text, Profile::Strict).is_ok());
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
            err("entity(zz:e)", Profile::Default),
            Error::UndeclaredPrefix(p) if p == "zz"
        ));
        assert!(
            matches!(provn::read("entity(ex:e)"), Err(Error::Syntax { line: 1, column: 1, message, .. }) if message.starts_with("expected 'document'"))
        );
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
        match err("entity(ex:e]", Profile::Default) {
            Error::Syntax {
                line,
                column,
                message,
                ..
            } => {
                assert_eq!((line, column), (4, 12));
                assert_eq!(message, "expected ',' or ')', found ']'");
            }
            other => panic!("unexpected {other:?}"),
        }
        let message = syntax_message(
            provn::read("document\nprefix ex <http://e/>\nentity(ex:e)\n").unwrap_err(),
        );
        assert_eq!(message, "expected 'endDocument', found end of input");
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
    fn prefixes_and_datatypes_are_not_integers() {
        let text = "document\n prefix 123 <http://a/>\nendDocument";
        assert!(matches!(
            provn::read(text),
            Err(Error::Syntax { line: 2, .. })
        ));
        assert!(matches!(
            err(r#"entity(ex:e, [ex:a="1" %% 42])"#, Profile::Default),
            Error::Syntax { .. }
        ));
    }

    #[test]
    fn digit_names_and_empty_attributes() {
        let d = doc("entity(123, [])\nwasDerivedFrom(ex:a, 42)", Profile::Strict);
        assert_eq!(
            d.records[0].id(),
            Some(&QName::new("http://default.org/", "123"))
        );
        assert!(
            matches!(&d.records[1], Record::Derivation(r) if r.used_entity == Some(QName::new("http://default.org/", "42")))
        );
    }

    #[test]
    fn escaped_colon_in_a_bare_name_stays_local() {
        let d = doc(r"entity(ex\:e1)", Profile::Strict);
        assert_eq!(
            d.records[0].id(),
            Some(&QName::new("http://default.org/", "ex:e1"))
        );
    }

    #[test]
    fn qualified_name_datatypes_read_as_names() {
        let d = doc(
            r#"entity(ex:e, [ex:a="ex:v" %% prov:QUALIFIED_NAME, ex:b="ex:v" %% xsd:QName])"#,
            Profile::Strict,
        );
        let values: Vec<&Value> = d.records[0].attributes().iter().map(|(_, v)| v).collect();
        assert_eq!(values, [&Value::QName(ex("v")), &Value::QName(ex("v"))]);
        assert!(matches!(
            err(r#"entity(ex:e, [ex:a="zz:v" %% prov:QUALIFIED_NAME])"#, Profile::Default),
            Error::UndeclaredPrefix(p) if p == "zz"
        ));
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
            Some(&QName::new("http://b.org/", "e"))
        );
        assert_eq!(d.bundles[0].records[1].id(), Some(&ex("f")));
        assert_eq!(d.bundles[0].namespaces.iter().count(), 1);
    }

    #[test]
    fn bundle_identifier_resolves_in_its_own_scope() {
        let d = doc(
            "bundle ex:b\n  prefix ex <http://bundle.org/>\n  entity(ex:e)\nendBundle\nentity(ex:e)",
            Profile::Strict,
        );
        assert_eq!(d.bundles[0].id, QName::new("http://bundle.org/", "b"));
        assert_eq!(
            d.bundles[0].records[0].id(),
            Some(&QName::new("http://bundle.org/", "e"))
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
