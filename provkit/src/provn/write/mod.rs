//! Document to PROV-N text.

use std::fmt::Write as _;
use std::io;

use super::chars;
use super::lexer::{Token, TokenKind, tokenize};
use crate::pn;
use crate::well_known::XSD_DATETIME;
use crate::{
    Bundle, Document, Error, FormalAttr, FormalRef, Namespaces, QName, Record, Result, Value,
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
/// [`Error::Unwritable`] for a name whose namespace IRI holds a character
/// `IRI_REF` forbids.
///
/// [`Error::UnwritableNamespace`] for a `prefix` or `default` declaration
/// with such an IRI.
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
    for bundle in document.merged_bundles().iter() {
        write_bundle(bundle, &names, &mut body)?;
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
/// `bundle.namespaces.with_fallback(document_namespaces)`. A namespace
/// missing from `namespaces` gets a minted `ns1`, `ns2`, ... prefix, which
/// stays in `namespaces` for the caller to declare.
///
/// # Errors
///
/// As [`to_string`].
pub fn statement(record: &Record, namespaces: &mut Namespaces) -> Result<String> {
    let mut s = String::new();
    expression(record, namespaces, &mut s)?;
    Ok(s)
}

/// A name as PROV-N writes it, escaped as the writer escapes it.
///
/// Use it for a bundle identifier, which lives in the bundle's own scope, so
/// pass `bundle.namespaces.with_fallback(document_namespaces)`. A namespace
/// missing from `namespaces` gets a minted `ns1`, `ns2`, ... prefix, which
/// stays in `namespaces`. A later call for the same namespace reuses it.
///
/// ```
/// use provkit::{Namespaces, QName, provn};
///
/// let mut names = Namespaces::new();
/// names.declare("ex", "http://example.org/").unwrap();
/// let id = QName::new("http://example.org/", "b1")?;
/// assert_eq!(provn::name(&id, &mut names)?, "ex:b1");
///
/// let other = QName::new("http://other.org/", "b2")?;
/// assert_eq!(provn::name(&other, &mut names)?, "ns1:b2");
/// assert_eq!(names.lookup("ns1"), Some("http://other.org/"));
/// # Ok::<(), provkit::Error>(())
/// ```
///
/// # Errors
///
/// [`Error::Unwritable`] if the name's namespace IRI holds a character
/// `IRI_REF` forbids. `namespaces` is then untouched.
pub fn name(name: &QName, namespaces: &mut Namespaces) -> Result<String> {
    if let Some(reason) = iri_problem(name.namespace()) {
        return Err(unwritable(name, reason));
    }
    let local = escape_local(name);
    let prefix = match namespaces.render_parts(name) {
        None if !lexes_as_bare_name(&local, name.local()) => {
            Some(namespaces.ensure_prefix(name.namespace()))
        }
        other => other,
    };
    Ok(match prefix {
        Some(prefix) => format!("{prefix}:{local}"),
        None => local,
    })
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
/// the default namespace when `prefix` is `None`. The prefix needs no check,
/// because `Namespaces` holds only `PN_PREFIX` prefixes.
fn check_declaration(prefix: Option<&str>, iri: &str) -> Result<()> {
    match iri_problem(iri) {
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
        // An element with any formal present writes all of them, so an
        // activity writes both times or neither.
        if !r.formals().is_empty() {
            for &attr in kind.formals() {
                parts.push(formal(r, attr, names)?);
            }
        }
    } else {
        // A relation without an identifier omits `id;` altogether. A kind
        // PROV-DM gives no identifier keeps one it carries (a PROV-JSON key
        // or a hand-built record), so the output reads back equal under
        // `Profile::Default`.
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
/// [`pn::local_char`] says whether a character is written bare or after `\`.
/// `QName::new` has already refused every character it does not cover.
fn escape_local(q: &QName) -> String {
    let local = q.local();
    let mut out = String::with_capacity(local.len());
    for (i, c) in local.char_indices() {
        if matches!(pn::local_char(local, i, c), Some(pn::LocalChar::Escaped)) {
            out.push('\\');
        }
        out.push(c);
    }
    out
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

/// A value is written bare when its text is an `INT_LITERAL`, an optional
/// minus sign and at least one digit, and reading that text back gives the
/// same datatype.
fn bare_int(lexical: &str, datatype: &QName) -> bool {
    let digits = lexical.strip_prefix('-').unwrap_or(lexical);
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && matches!(Value::integer(lexical), Value::Typed { datatype: read, .. } if read == *datatype)
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
mod tests;
