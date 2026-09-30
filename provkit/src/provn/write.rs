//! Document to PROV-N text.

use std::fmt::Write as _;
use std::io;

use super::chars;
use super::lexer::{Token, TokenKind, tokenize};
use crate::document::merged_bundles;
use crate::pn;
use crate::well_known::{XSD_DATETIME, XSD_INT};
use crate::{
    Bundle, Document, Error, FormalAttr, FormalRef, Kind, Namespaces, QName, Record, Result, Value,
};

/// Write `document` as PROV-N.
///
/// # Errors
///
/// As [`to_string`], plus [`Error::Io`] if `out` fails.
pub fn write(document: &Document, out: &mut impl io::Write) -> Result<()> {
    out.write_all(to_string(document)?.as_bytes())?;
    Ok(())
}

/// Write `document` as a PROV-N string.
///
/// A namespace with no declared prefix gets a minted `ns1`, `ns2`, ... and
/// the document is left unchanged. Records without an identifier are written
/// without one. Bundles that share an identifier are written as one.
///
/// Whatever this function accepts reads back with [`read`](super::read) as
/// an equal document.
///
/// # Errors
///
/// [`Error::Unwritable`] for a name PROV-N cannot carry:
///
/// - a local part with whitespace, a control character, one of
///   `{ } < > " | ^ \`, a `%` not followed by two hex digits, or a first
///   character that cannot start a name (a combining mark, `·`, `‿`, `⁀`);
/// - a prefix that is not a `PN_PREFIX`;
/// - a namespace IRI with a character `IRI_REF` forbids.
///
/// [`Error::UnwritableNamespace`] for a `prefix` or `default` declaration
/// with such a prefix or namespace IRI.
///
/// [`Error::InvalidRecord`] for an attribute that spells one of the record's
/// own formals.
pub fn to_string(document: &Document) -> Result<String> {
    // Names are rendered before the declarations are printed, so that every
    // prefix minted along the way is declared.
    let mut names = document.namespaces.clone();
    let mut body = String::new();
    for record in &document.records {
        body.push_str("  ");
        expression(record, &mut names, &mut body)?;
        body.push('\n');
    }
    for bundle in merged_bundles(&document.bundles) {
        write_bundle(&bundle, &names, &mut body)?;
    }

    let mut s = String::from("document\n");
    let declared = declarations(&names, None, "  ", &mut s)?;
    if declared {
        s.push('\n');
    }
    s.push_str(&body);
    s.push_str("endDocument\n");
    Ok(s)
}

/// One record as a single PROV-N expression, with no indentation.
///
/// Names are written against `namespaces`. For a record inside a bundle, pass
/// `bundle.namespaces.with_fallback(document_namespaces)`.
///
/// # Errors
///
/// As [`to_string`].
pub fn statement(record: &Record, namespaces: &Namespaces) -> Result<String> {
    let mut names = namespaces.clone();
    let mut s = String::new();
    expression(record, &mut names, &mut s)?;
    Ok(s)
}

/// A name as PROV-N writes it, escaped as the writer escapes it.
///
/// Use it for a bundle identifier, which lives in the bundle's own scope, so
/// pass `bundle.namespaces.with_fallback(document_namespaces)`. A namespace
/// missing from `namespaces` gets a minted `ns1`, `ns2`, ... prefix.
///
/// ```
/// use provkit::{Namespaces, QName, provn};
///
/// let mut names = Namespaces::new();
/// names.declare("ex", "http://example.org/").unwrap();
/// let id = QName::new("http://example.org/", "b1");
/// assert_eq!(provn::statement_name(&id, &names)?, "ex:b1");
/// # Ok::<(), provkit::Error>(())
/// ```
///
/// # Errors
///
/// As [`to_string`], for the name's local part, prefix and namespace IRI.
pub fn statement_name(name: &QName, namespaces: &Namespaces) -> Result<String> {
    let mut names = namespaces.clone();
    self::name(name, &mut names)
}

fn unwritable(name: &QName, reason: String) -> Error {
    Error::Unwritable {
        name: name.clone(),
        reason,
    }
}

/// `c` with its code point, so an invisible character shows in a message.
fn show(c: char) -> String {
    format!("{c:?} (U+{:04X})", u32::from(c))
}

/// Why PROV-N cannot write the namespace IRI `iri`, if it cannot.
fn iri_problem(iri: &str) -> Option<String> {
    chars::iri_forbidden(iri).map(|c| format!("the namespace IRI contains {}", show(c)))
}

/// Check that PROV-N can write a declaration of `iri`: under `prefix`, or as
/// the default namespace when `prefix` is `None`.
fn check_declaration(prefix: Option<&str>, iri: &str) -> Result<()> {
    let reason = match prefix {
        Some(p) if !pn::is_pn_prefix(p) => Some("the prefix is not a PN_PREFIX".to_owned()),
        _ => iri_problem(iri),
    };
    match reason {
        Some(reason) => Err(Error::UnwritableNamespace {
            prefix: prefix.unwrap_or_default().to_owned(),
            iri: iri.to_owned(),
            reason,
        }),
        None => Ok(()),
    }
}

/// Print `default` and `prefix` lines for the entries of `names` that `inherited`
/// does not already provide. Returns whether any line was printed.
fn declarations(
    names: &Namespaces,
    inherited: Option<&Namespaces>,
    indent: &str,
    s: &mut String,
) -> Result<bool> {
    let mut any = false;
    if let Some(default) = names.default_namespace()
        && inherited.and_then(Namespaces::default_namespace) != Some(default)
    {
        check_declaration(None, default)?;
        let _ = writeln!(s, "{indent}default <{default}>");
        any = true;
    }
    for (prefix, iri) in names.iter() {
        if inherited.is_some_and(|o| o.lookup(prefix) == Some(iri)) {
            continue;
        }
        check_declaration(Some(prefix), iri)?;
        let _ = writeln!(s, "{indent}prefix {prefix} <{iri}>");
        any = true;
    }
    Ok(any)
}

fn write_bundle(bundle: &Bundle, outer: &Namespaces, s: &mut String) -> Result<()> {
    let mut names = bundle.namespaces.with_fallback(outer);
    let mut body = String::new();
    for record in &bundle.records {
        body.push_str("    ");
        expression(record, &mut names, &mut body)?;
        body.push('\n');
    }
    let id = name(&bundle.id, &mut names)?;
    let _ = writeln!(s, "  bundle {id}");
    declarations(&names, Some(outer), "    ", s)?;
    s.push_str(&body);
    s.push_str("  endBundle\n");
    Ok(())
}

fn expression(r: &Record, names: &mut Namespaces, s: &mut String) -> Result<()> {
    // The reader would reject such an attribute, so the writer does too.
    r.check_attributes()?;
    let kind = r.kind();
    let mut parts: Vec<String> = Vec::new();

    s.push_str(kind.name());
    s.push('(');
    if kind.is_element() {
        // Elements never take `-` for the identifier.
        if let Some(id) = r.id() {
            parts.push(name(id, names)?);
        }
        // An activity writes both times or neither.
        if kind == Kind::Activity && !r.formals().is_empty() {
            parts.push(formal(r, FormalAttr::StartTime, names)?);
            parts.push(formal(r, FormalAttr::EndTime, names)?);
        }
    } else {
        // A relation without an identifier omits `id;` altogether.
        if let Some(id) = r.id() {
            let _ = write!(s, "{}; ", name(id, names)?);
        }
        for &attr in kind.formals() {
            parts.push(formal(r, attr, names)?);
        }
    }
    s.push_str(&parts.join(", "));

    let attrs = r.attributes();
    if !attrs.is_empty() {
        if !parts.is_empty() {
            s.push_str(", ");
        }
        s.push('[');
        for (i, (n, v)) in attrs.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            let _ = write!(s, "{}={}", name(n, names)?, value(v, names)?);
        }
        s.push(']');
    }
    s.push(')');
    Ok(())
}

/// The formal `attr` of `record`, or `-` when it is absent.
fn formal(record: &Record, attr: FormalAttr, names: &mut Namespaces) -> Result<String> {
    Ok(match record.formal(attr) {
        Some(FormalRef::Name(q)) => name(q, names)?,
        Some(FormalRef::Time(t)) => t.to_string(),
        None => "-".to_owned(),
    })
}

/// `prefix:local`, or a bare `local` in the default namespace, with the local
/// part escaped.
fn name(q: &QName, names: &mut Namespaces) -> Result<String> {
    if let Some(reason) = iri_problem(q.namespace()) {
        return Err(unwritable(q, reason));
    }
    let local = escape_local(q)?;
    let prefix = match names.render_parts(q) {
        None if !lexes_as_bare_name(&local, q.local()) => Some(names.ensure_prefix(q.namespace())),
        other => other,
    };
    Ok(match prefix {
        Some(prefix) => {
            if !pn::is_pn_prefix(&prefix) {
                let reason = format!("its prefix `{prefix}` is not a PN_PREFIX");
                return Err(unwritable(q, reason));
            }
            format!("{prefix}:{local}")
        }
        None => local,
    })
}

/// Whether `text`, written bare, lexes back as exactly one name with no
/// prefix and local part `local`. Anything else (an integer, a comment,
/// an error, nothing at all) takes a prefix instead.
///
/// This runs the lexer once per bare name written, which costs a small
/// allocation each time; names under a prefix skip it.
fn lexes_as_bare_name(text: &str, local: &str) -> bool {
    matches!(
        tokenize(text).as_deref(),
        Ok([Token { kind: TokenKind::Name { prefix, local: read }, .. }, Token { kind: TokenKind::Eof, .. }])
            if prefix.is_empty() && read == local
    )
}

/// The local part of `q` as a `PN_LOCAL` that lexes back to the same text.
///
/// [`pn::local_char`] says whether a character is written bare or after
/// `\`. A character no `PN_LOCAL` holds is an error.
fn escape_local(q: &QName) -> Result<String> {
    let local = q.local();
    let mut out = String::with_capacity(local.len());
    for (i, c) in local.char_indices() {
        match pn::local_char(local, i, c) {
            Some(pn::LocalChar::Bare) => {}
            Some(pn::LocalChar::Escaped) => out.push('\\'),
            None if c == '%' => {
                return Err(unwritable(
                    q,
                    "a '%' in a local part must be followed by two hex digits".to_owned(),
                ));
            }
            None => {
                let place = if i == 0 && pn::is_pn_chars(c) {
                    "at the start of a local part"
                } else {
                    "in a local part"
                };
                return Err(unwritable(q, format!("{} cannot appear {place}", show(c))));
            }
        }
        out.push(c);
    }
    Ok(out)
}

fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// An `xsd:int` is written bare only when it fits the grammar's `INT_LITERAL`:
/// an optional minus sign and at least one digit.
fn bare_int(lexical: &str, datatype: &QName) -> bool {
    let digits = lexical.strip_prefix('-').unwrap_or(lexical);
    *datatype == *XSD_INT && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn value(v: &Value, names: &mut Namespaces) -> Result<String> {
    debug_assert!(v.has_one_representation(), "{v:?} has a folded datatype");
    Ok(match v {
        Value::Str(s) => string(s),
        Value::LangStr(l) => format!("{}@{}", string(l.value()), l.lang()),
        Value::QName(q) => format!("'{}'", name(q, names)?),
        Value::DateTime(d) => format!(
            "{} %% {}",
            string(&d.to_string()),
            name(&XSD_DATETIME, names)?
        ),
        Value::Typed { lexical, datatype } if bare_int(lexical, datatype) => lexical.clone(),
        Value::Typed { lexical, datatype } => {
            format!("{} %% {}", string(lexical), name(datatype, names)?)
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::well_known::{PROV_LABEL, PROV_TYPE, XSD_INT, XSD_QNAME};
    use crate::{
        Attributes, Bundle, DateTime, Document, Error, Formal, FormalAttr as F, Kind, LangStr,
        Namespaces, QName, Record, Value, provn,
    };

    fn ex(l: &str) -> QName {
        QName::new("http://example.org/", l)
    }

    fn dt() -> DateTime {
        DateTime::parse("2011-11-16T16:05:33Z").unwrap()
    }

    fn element(kind: Kind, id: QName, attrs: Attributes) -> Record {
        Record::from_parts(kind, Some(id), vec![], attrs).unwrap()
    }

    fn relation(kind: Kind, id: Option<QName>, formals: Vec<(F, Formal)>) -> Record {
        Record::from_parts(kind, id, formals, Attributes::new()).unwrap()
    }

    /// A document in `http://example.org/` under `ex`, holding `records`.
    fn document(records: Vec<Record>) -> Document {
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.records = records;
        d
    }

    /// The single statement line of a one-record document.
    fn line(record: Record) -> String {
        let d = document(vec![record]);
        provn::statement(&d.records[0], &d.namespaces).unwrap()
    }

    #[test]
    fn prints_the_expected_text() {
        let mut d = Document::new();
        d.namespaces.set_default("http://default.org/");
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        let mut a = Attributes::new();
        a.insert(PROV_TYPE.clone(), ex("T").into());
        a.insert(ex("n"), 1i32.into());
        a.insert(ex("s"), "a\"b\n".into());
        a.insert(PROV_LABEL.clone(), LangStr::new("x", "en").unwrap().into());
        a.insert(ex("d"), dt().into());
        a.insert(ex("f"), 1.5f64.into());
        d.records.push(element(Kind::Entity, ex("e1"), a));
        d.records.push(
            Record::from_parts(
                Kind::Activity,
                Some(QName::new("http://default.org/", "a1")),
                vec![(F::StartTime, Formal::Time(dt()))],
                Attributes::new(),
            )
            .unwrap(),
        );
        let a1 = QName::new("http://default.org/", "a1");
        d.records.push(relation(
            Kind::Generation,
            Some(ex("g1")),
            vec![
                (F::Entity, Formal::Name(ex("e1"))),
                (F::Activity, Formal::Name(a1)),
            ],
        ));
        d.records.push(relation(
            Kind::Generation,
            None,
            vec![(F::Entity, Formal::Name(ex("e1")))],
        ));
        d.records.push(relation(
            Kind::Mention,
            None,
            vec![
                (F::SpecificEntity, Formal::Name(ex("e2"))),
                (F::GeneralEntity, Formal::Name(ex("e1"))),
                (F::Bundle, Formal::Name(ex("b"))),
            ],
        ));
        d.records
            .push(element(Kind::Entity, ex("e.2"), Attributes::new()));
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: Namespaces::new(),
            records: vec![element(Kind::Entity, ex("e2"), Attributes::new())],
        });
        let expected = "\
document
  default <http://default.org/>
  prefix ex <http://example.org/>

  entity(ex:e1, [prov:type='ex:T', ex:n=1, ex:s=\"a\\\"b\\n\", prov:label=\"x\"@en, ex:d=\"2011-11-16T16:05:33Z\" %% xsd:dateTime, ex:f=\"1.5\" %% xsd:double])
  activity(a1, 2011-11-16T16:05:33Z, -)
  wasGeneratedBy(ex:g1; ex:e1, a1, -)
  wasGeneratedBy(ex:e1, -, -)
  mentionOf(ex:e2, ex:e1, ex:b)
  entity(ex:e\\.2)
  bundle ex:b
    entity(ex:e2)
  endBundle
endDocument
";
        assert_eq!(provn::to_string(&d).unwrap(), expected);
        assert_eq!(
            provn::statement(&d.records[3], &d.namespaces).unwrap(),
            "wasGeneratedBy(ex:e1, -, -)"
        );
    }

    #[test]
    fn relations_have_full_arity() {
        let r = relation(
            Kind::Derivation,
            Some(ex("d1")),
            vec![(F::UsedEntity, Formal::Name(ex("e0")))],
        );
        assert_eq!(line(r), "wasDerivedFrom(ex:d1; -, ex:e0, -, -, -)");
        let r = relation(Kind::Start, None, vec![(F::Time, Formal::Time(dt()))]);
        assert_eq!(line(r), "wasStartedBy(-, -, -, 2011-11-16T16:05:33Z)");
    }

    #[test]
    fn activity_writes_both_times_or_neither() {
        let neither = element(Kind::Activity, ex("a"), Attributes::new());
        assert_eq!(line(neither), "activity(ex:a)");
        let end_only = Record::from_parts(
            Kind::Activity,
            Some(ex("a")),
            vec![(F::EndTime, Formal::Time(dt()))],
            Attributes::new(),
        )
        .unwrap();
        assert_eq!(line(end_only), "activity(ex:a, -, 2011-11-16T16:05:33Z)");
        let mut attrs = Attributes::new();
        attrs.insert(ex("k"), "v".into());
        assert_eq!(
            line(element(Kind::Activity, ex("a"), attrs)),
            "activity(ex:a, [ex:k=\"v\"])"
        );
    }

    #[test]
    fn identified_relation_leads_with_its_id_and_a_semicolon() {
        let r = relation(
            Kind::Usage,
            Some(ex("u")),
            vec![(F::Activity, Formal::Name(ex("a")))],
        );
        assert_eq!(line(r), "used(ex:u; ex:a, -, -)");
    }

    #[test]
    fn mention_is_written_bare() {
        let r = relation(
            Kind::Mention,
            None,
            vec![(F::SpecificEntity, Formal::Name(ex("s")))],
        );
        assert_eq!(line(r), "mentionOf(ex:s, -, -)");
    }

    #[test]
    fn values_follow_the_grammar() {
        let mut a = Attributes::new();
        a.insert(ex("q"), ex("name").into());
        a.insert(ex("i"), 1i32.into());
        a.insert(ex("neg"), Value::typed("-42", XSD_INT.clone()).unwrap());
        a.insert(ex("zeros"), Value::typed("005", XSD_INT.clone()).unwrap());
        a.insert(ex("l"), 7i64.into());
        a.insert(ex("c"), Value::typed("x", ex("dt")).unwrap());
        assert_eq!(
            line(element(Kind::Entity, ex("e"), a)),
            "entity(ex:e, [ex:q='ex:name', ex:i=1, ex:neg=-42, ex:zeros=005, \
             ex:l=\"7\" %% xsd:long, ex:c=\"x\" %% ex:dt])"
        );
    }

    #[test]
    fn a_lone_minus_or_empty_int_is_not_bare() {
        for text in ["-", "", "+1", "1.0", "--1", "1 ", " 5 ", "abc"] {
            let mut a = Attributes::new();
            a.insert(ex("i"), Value::typed(text, XSD_INT.clone()).unwrap());
            assert_eq!(
                line(element(Kind::Entity, ex("e"), a)),
                format!("entity(ex:e, [ex:i=\"{text}\" %% xsd:int])"),
                "lexical form {text:?}"
            );
        }
    }

    #[test]
    fn local_parts_and_strings_escape() {
        let all = ex("a=b'c(d)e,f:g-h[i]j.k;l");
        assert_eq!(
            line(element(Kind::Entity, all, Attributes::new())),
            "entity(ex:a\\=b\\'c\\(d\\)e\\,f\\:g\\-h\\[i\\]j\\.k\\;l)"
        );
        let mut a = Attributes::new();
        a.insert(ex("s"), "q\" b\\ n\n r\r t\t".into());
        assert_eq!(
            line(element(Kind::Entity, ex("e"), a)),
            "entity(ex:e, [ex:s=\"q\\\" b\\\\ n\\n r\\r t\\t\"])"
        );
    }

    #[test]
    fn undeclared_namespaces_get_minted_prefixes_and_the_document_is_untouched() {
        let mut d = Document::new();
        d.records.push(element(
            Kind::Entity,
            QName::new("http://new.org/", "e"),
            Attributes::new(),
        ));
        let before = d.namespaces.clone();
        assert_eq!(
            provn::to_string(&d).unwrap(),
            "document\n  prefix ns1 <http://new.org/>\n\n  entity(ns1:e)\nendDocument\n"
        );
        assert_eq!(d.namespaces, before);
    }

    #[test]
    fn default_namespace_names_mint_no_prefix() {
        let mut d = Document::new();
        d.namespaces.set_default("http://default.org/");
        d.records.push(element(
            Kind::Entity,
            QName::new("http://default.org/", "e"),
            Attributes::new(),
        ));
        assert_eq!(
            provn::to_string(&d).unwrap(),
            "document\n  default <http://default.org/>\n\n  entity(e)\nendDocument\n"
        );
    }

    #[test]
    fn empty_document_has_no_blank_line() {
        assert_eq!(
            provn::to_string(&Document::new()).unwrap(),
            "document\nendDocument\n"
        );
    }

    #[test]
    fn bundles_sharing_an_identifier_are_written_once() {
        let mut d = document(vec![]);
        for id in ["e1", "e2"] {
            d.bundles.push(Bundle {
                id: ex("b"),
                namespaces: Namespaces::new(),
                records: vec![element(Kind::Entity, ex(id), Attributes::new())],
            });
        }
        let text = provn::to_string(&d).unwrap();
        assert_eq!(text.matches("bundle ex:b").count(), 1);
        assert!(text.contains("entity(ex:e1)") && text.contains("entity(ex:e2)"));
    }

    #[test]
    fn bundle_prefix_overrides_the_documents_for_its_own_identifier() {
        let bundle_ns = "http://bundle.org/";
        let mut d = document(vec![]);
        let mut inner = Namespaces::new();
        inner.declare("ex", bundle_ns).unwrap();
        d.bundles.push(Bundle {
            id: QName::new(bundle_ns, "b"),
            namespaces: inner,
            records: vec![
                element(Kind::Entity, QName::new(bundle_ns, "in"), Attributes::new()),
                element(Kind::Entity, ex("out"), Attributes::new()),
            ],
        });
        let text = provn::to_string(&d).unwrap();
        // The identifier and the bundle's own names use the bundle's `ex`;
        // the document's `ex` namespace needs a fresh prefix inside.
        let expected = "\
document
  prefix ex <http://example.org/>

  bundle ex:b
    prefix ex <http://bundle.org/>
    prefix ns1 <http://example.org/>
    entity(ex:in)
    entity(ns1:out)
  endBundle
endDocument
";
        assert_eq!(text, expected);
    }

    #[test]
    fn bundle_inherits_and_does_not_repeat_document_declarations() {
        let mut d = document(vec![]);
        d.namespaces.set_default("http://default.org/");
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: Namespaces::new(),
            records: vec![element(
                Kind::Entity,
                QName::new("http://default.org/", "e"),
                Attributes::new(),
            )],
        });
        assert_eq!(
            provn::to_string(&d).unwrap(),
            "document\n  default <http://default.org/>\n  prefix ex <http://example.org/>\n\n  \
             bundle ex:b\n    entity(e)\n  endBundle\nendDocument\n"
        );
    }

    #[test]
    fn bundle_scope_prefers_the_bundle() {
        let mut outer = Namespaces::new();
        outer.declare("ex", "http://example.org/").unwrap();
        outer.declare("o", "http://o.org/").unwrap();
        let mut inner = Namespaces::new();
        inner.declare("ex", "http://bundle.org/").unwrap();
        let bundle = Bundle {
            id: ex("b"),
            namespaces: inner,
            records: vec![],
        };
        let table = bundle.namespaces.with_fallback(&outer);
        assert_eq!(table.lookup("ex").unwrap(), "http://bundle.org/");
        assert_eq!(table.lookup("o").unwrap(), "http://o.org/");
    }

    /// Write `d`; when the writer accepts it, it must read back equal.
    /// Returns whether the writer accepted it.
    fn written_reads_back(d: &Document) -> bool {
        match provn::to_string(d) {
            Ok(text) => {
                let back = provn::read(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
                assert_eq!(back, *d, "{text}");
                true
            }
            Err(Error::Unwritable { .. } | Error::UnwritableNamespace { .. }) => false,
            Err(e) => panic!("unexpected error {e:?} for {d:?}"),
        }
    }

    /// A document using `local` in every name position, under a declared
    /// prefix and bare in the default namespace.
    fn every_position(local: &str) -> Document {
        let default = "http://default.org/";
        let mut d = document(vec![]);
        d.namespaces.set_default(default);
        for ns in ["http://example.org/", default] {
            let n = QName::new(ns, local);
            let mut a = Attributes::new();
            a.insert(n.clone(), Value::QName(n.clone()));
            a.insert(ex("t"), Value::typed("x", n.clone()).unwrap());
            d.records.push(element(Kind::Entity, n.clone(), a));
            d.records.push(relation(
                Kind::Usage,
                Some(n.clone()),
                vec![(F::Entity, Formal::Name(n.clone()))],
            ));
            d.bundles.push(Bundle {
                id: n,
                namespaces: Namespaces::new(),
                records: vec![],
            });
        }
        d
    }

    /// Every ASCII character and a sample beyond it, including combining
    /// marks, `·`, `‿`, non-characters and separators.
    fn palette() -> Vec<char> {
        let mut chars: Vec<char> = (0u8..0x80).map(char::from).collect();
        chars.extend([
            'é', '\u{A0}', '\u{B7}', '\u{D7}', '\u{300}', '\u{36F}', '\u{1680}', '\u{203F}',
            '\u{2040}', '\u{2028}', '\u{FEFF}', '\u{FFFE}', '😀',
        ]);
        chars
    }

    /// Local parts and lexical forms: every palette character alone and in
    /// context, pseudo-random mixes, and hand-picked edge cases.
    fn sample_texts() -> Vec<String> {
        let chars = palette();
        let mut locals: Vec<String> = Vec::new();
        for &c in &chars {
            locals.extend([
                c.to_string(),
                format!("a{c}"),
                format!("a{c}b"),
                format!("{c}a"),
            ]);
        }
        // Pseudo-random strings of one to four palette characters, so pairs
        // such as `%4` followed by a letter, or `\.` mixes, are covered.
        let mut seed: u64 = 0x5eed;
        for _ in 0..3000 {
            let mut local = String::new();
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            for _ in 0..=(seed >> 60) % 4 {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                let i = usize::try_from(seed >> 33).unwrap() % chars.len();
                local.push(chars[i]);
            }
            locals.push(local);
        }
        locals.extend(
            [
                "",
                "12",
                "%41",
                "a%4",
                "%zz",
                "a%",
                "%%41",
                "a.",
                ".a",
                "-1",
                "a\\",
                "//x",
                "/*y",
                "a//x",
                "a/*y",
                "\u{1680}a",
                "a\u{1680}",
                "\u{A0}a",
                "a\u{A0}",
                "\u{2028}a",
                "a\u{2028}",
            ]
            .map(String::from),
        );
        locals
    }

    /// Values from every public constructor: the `From` conversions, and
    /// [`Value::typed`] over sample lexical forms and datatypes.
    fn sample_values() -> Vec<Value> {
        let mut values = vec![
            Value::from(0i32),
            Value::from(i32::MIN),
            Value::from(i64::MAX),
            Value::from(1.5f32),
            Value::from(f32::NAN),
            Value::from(f64::INFINITY),
            Value::from(f64::NEG_INFINITY),
            Value::from(-0.0f64),
            Value::from(true),
            Value::from(false),
            Value::from("a\"b\\c\nd\re\tf"),
            Value::from(String::from("é\u{2028}😀")),
            Value::from(ex("q")),
            Value::from(dt()),
            Value::from(LangStr::new("hi", "en-GB").unwrap()),
        ];
        let datatypes = [
            XSD_INT.clone(),
            crate::well_known::XSD_LONG.clone(),
            crate::well_known::XSD_STRING.clone(),
            crate::well_known::XSD_DATETIME.clone(),
            crate::well_known::XSD_DOUBLE.clone(),
            QName::new(crate::well_known::XSD, "anyURI"),
            ex("dt"),
        ];
        for lexical in sample_texts() {
            for datatype in &datatypes {
                values.push(Value::typed(lexical.as_str(), datatype.clone()).unwrap());
            }
        }
        for lexical in ["2011-11-16T16:05:33Z", "2011-11-16T16:05:33.25+01:00"] {
            values.push(Value::typed(lexical, crate::well_known::XSD_DATETIME.clone()).unwrap());
        }
        values
    }

    #[test]
    fn a_qualified_name_literal_is_a_name_exactly_when_provn_can_write_it() {
        let names = document(vec![]).namespaces;
        let (mut resolved, mut kept) = (0, 0);
        for local in sample_texts() {
            let text = format!("ex:{local}");
            let writable = provn::statement_name(&ex(&local), &names).is_ok();
            let value = Value::typed_in(&names, &text, XSD_QNAME.clone()).unwrap();
            match &value {
                Value::QName(q) => {
                    assert!(writable, "{text:?}");
                    assert_eq!(*q, ex(&local));
                    resolved += 1;
                }
                Value::Typed { lexical, .. } => {
                    assert!(!writable, "{text:?}");
                    assert_eq!(*lexical, text);
                    kept += 1;
                }
                other => panic!("{text:?}: {other:?}"),
            }
            // Either way it is written, and reads back equal from both formats.
            let mut a = Attributes::new();
            a.insert(ex("v"), value);
            let d = document(vec![element(Kind::Entity, ex("e"), a)]);
            assert!(written_reads_back(&d), "{text:?}");
            #[cfg(feature = "json")]
            {
                let json = crate::json::to_string(&d).unwrap();
                assert_eq!(crate::json::read(&json).unwrap(), d, "{json}");
            }
        }
        assert!(resolved > 1000 && kept > 100, "{resolved} {kept}");
    }

    #[test]
    fn whatever_the_writer_accepts_reads_back_equal() {
        let chars = palette();
        let locals = sample_texts();
        let (mut accepted, mut rejected) = (0, 0);
        for local in &locals {
            if written_reads_back(&every_position(local)) {
                accepted += 1;
            } else {
                rejected += 1;
            }
        }
        assert!(accepted > 1000 && rejected > 100, "{accepted} {rejected}");

        // Every value the public constructors build is accepted, and reads
        // back equal through PROV-JSON too.
        for v in sample_values() {
            let mut a = Attributes::new();
            a.insert(ex("v"), v);
            let d = document(vec![element(Kind::Entity, ex("e"), a)]);
            assert!(written_reads_back(&d));
            #[cfg(feature = "json")]
            {
                let text = crate::json::to_string(&d).unwrap();
                assert_eq!(crate::json::read(&text).unwrap(), d, "{text}");
            }
        }

        // Namespace IRIs, minted and default, and declared prefixes.
        for &c in &chars {
            let iri = format!("http://example.org/{c}/");
            let mut minted = Document::new();
            minted.records.push(element(
                Kind::Entity,
                QName::new(iri.as_str(), "e"),
                Attributes::new(),
            ));
            written_reads_back(&minted);
            let mut default = Document::new();
            default.namespaces.set_default(iri.as_str());
            written_reads_back(&default);
            for prefix in [format!("p{c}"), format!("{c}p"), format!("p{c}q")] {
                let mut declared = Document::new();
                if declared
                    .namespaces
                    .declare(&prefix, "http://p.org/")
                    .is_ok()
                {
                    declared.records.push(element(
                        Kind::Entity,
                        QName::new("http://p.org/", "e"),
                        Attributes::new(),
                    ));
                    written_reads_back(&declared);
                }
            }
        }
    }

    /// The name and reason of the `Unwritable` error from writing `d`.
    fn unwritable(d: &Document) -> (QName, String) {
        match provn::to_string(d) {
            Err(Error::Unwritable { name, reason }) => (name, reason),
            other => panic!("expected Error::Unwritable, got {other:?}"),
        }
    }

    #[test]
    fn unrepresentable_local_parts_are_errors() {
        let cases = [
            ("a b", "' ' (U+0020)"),
            ("a\tb", "'\\t' (U+0009)"),
            ("a\u{7}b", "'\\u{7}' (U+0007)"),
            ("a{b", "'{'"),
            ("a}b", "'}'"),
            ("a<b", "'<'"),
            ("a>b", "'>'"),
            ("a\"b", "'\"'"),
            ("a|b", "'|'"),
            ("a^b", "'^'"),
            ("a\\b", "'\\\\'"),
            ("a`b", "'`'"),
            ("a%", "'%'"),
            ("a%4", "'%'"),
            ("a%zz", "'%'"),
            ("\u{301}a", "at the start"),
            ("\u{B7}a", "at the start"),
        ];
        for (local, needle) in cases {
            let d = document(vec![element(Kind::Entity, ex(local), Attributes::new())]);
            let (name, reason) = unwritable(&d);
            assert_eq!(name, ex(local));
            // The reason names the character, with its code point.
            assert!(reason.contains(needle), "{local:?}: {reason}");
        }
        // The same local parts work in the middle where the grammar allows them.
        let d = document(vec![element(
            Kind::Entity,
            ex("a\u{301}\u{B7}"),
            Attributes::new(),
        )]);
        assert!(written_reads_back(&d));
    }

    #[test]
    fn a_prefix_that_is_not_a_pn_prefix_cannot_be_written() {
        // `Namespaces::declare` already refuses such prefixes, so the
        // writer's check is a guard and is exercised directly.
        for bad in ["", "a b", "1a", "a."] {
            let e = super::check_declaration(Some(bad), "http://x/").unwrap_err();
            assert!(
                matches!(&e, Error::UnwritableNamespace { prefix, iri, .. } if prefix == bad && iri == "http://x/"),
                "{bad:?}: {e:?}"
            );
        }
        assert!(super::check_declaration(Some("a.b"), "http://x/").is_ok());
    }

    #[test]
    fn unrepresentable_iris_are_errors() {
        let name_in = |iri: &str| element(Kind::Entity, QName::new(iri, "e"), Attributes::new());
        // Met while rendering a name.
        let mut minted = Document::new();
        minted.records.push(name_in("http://x.org/a b/"));
        let (name, reason) = unwritable(&minted);
        assert_eq!(name, QName::new("http://x.org/a b/", "e"));
        assert!(reason.contains("' ' (U+0020)"), "{reason}");

        // Met while writing a declaration. The default namespace has an
        // empty prefix.
        let mut default = Document::new();
        default.namespaces.set_default("http://x.org/>");
        assert!(matches!(
            provn::to_string(&default),
            Err(Error::UnwritableNamespace { prefix, iri, .. })
                if prefix.is_empty() && iri == "http://x.org/>"
        ));

        let mut declared = Document::new();
        declared.namespaces.declare("q", "http://x.org/\"").unwrap();
        assert!(matches!(
            provn::to_string(&declared),
            Err(Error::UnwritableNamespace { prefix, iri, reason })
                if prefix == "q" && iri == "http://x.org/\"" && reason.contains("'\"'")
        ));

        let names = Namespaces::new();
        let bad = QName::new("http://x.org/{", "e");
        assert!(matches!(
            provn::statement_name(&bad, &names),
            Err(Error::Unwritable { name, .. }) if name == bad
        ));
        assert!(matches!(
            provn::statement(&name_in("http://x.org/|"), &names),
            Err(Error::Unwritable { .. })
        ));
    }

    #[cfg(feature = "json")]
    #[test]
    fn a_json_key_with_a_trailing_space_cannot_be_written() {
        let d = crate::json::read(
            r#"{"prefix": {"ex": "http://example.org/"},
                "wasAttributedTo": {"_:a": {"prov:entity": "ex:e", "prov:agent ": "ex:ag"}}}"#,
        )
        .unwrap();
        let Record::Attribution(a) = &d.records[0] else {
            panic!()
        };
        assert_eq!(a.agent, None);
        let (name, reason) = unwritable(&d);
        assert_eq!(name, QName::new(crate::well_known::PROV, "agent "));
        assert!(reason.contains("' ' (U+0020)"), "{reason}");
    }

    #[test]
    fn other_escapable_characters_are_written_bare_and_read_back() {
        let local = "_a~b!c$d&e*f+g/h?i#j@k%41";
        let d = document(vec![element(Kind::Entity, ex(local), Attributes::new())]);
        assert_eq!(line(d.records[0].clone()), format!("entity(ex:{local})"));
        assert!(written_reads_back(&d));
    }

    #[test]
    fn bare_names_that_would_misread_take_a_prefix() {
        let mut d = Document::new();
        d.namespaces.set_default("http://default.org/");
        for local in ["12", "", "@a", "//x", "/*y"] {
            d.records.push(element(
                Kind::Entity,
                QName::new("http://default.org/", local),
                Attributes::new(),
            ));
        }
        let text = provn::to_string(&d).unwrap();
        for expected in [
            "entity(ns1:12)",
            "entity(ns1:)",
            "entity(ns1:@a)",
            "entity(ns1://x)",
            "entity(ns1:/*y)",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        assert!(written_reads_back(&d));

        // Names that lex back as themselves stay bare.
        for local in ["\u{1680}a", "a/b", "a@b"] {
            let mut d = Document::new();
            d.namespaces.set_default("http://default.org/");
            d.records.push(element(
                Kind::Entity,
                QName::new("http://default.org/", local),
                Attributes::new(),
            ));
            let text = provn::to_string(&d).unwrap();
            assert!(text.contains(&format!("entity({local})")), "{text}");
            assert!(!text.contains("ns1"), "{text}");
            assert!(written_reads_back(&d));
        }
        assert!(written_reads_back(&d));
    }

    #[test]
    fn an_attribute_spelling_an_own_formal_cannot_be_written() {
        let mut a = Attributes::new();
        a.insert(
            QName::new(crate::well_known::PROV, "entity"),
            ex("x").into(),
        );
        let r = Record::Usage(crate::Usage {
            id: None,
            activity: Some(ex("a")),
            entity: None,
            time: None,
            attributes: a,
        });
        let d = document(vec![r]);
        assert!(matches!(
            provn::to_string(&d),
            Err(Error::InvalidRecord {
                kind: Kind::Usage,
                ..
            })
        ));
    }

    #[test]
    fn io_failure_is_reported_as_io() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("disk full"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let err = provn::write(&Document::new(), &mut Broken).unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err:?}");
        let mut ok = Vec::new();
        provn::write(&Document::new(), &mut ok).unwrap();
        assert_eq!(ok, b"document\nendDocument\n");
    }
}
