//! Document to wire structs to JSON text.

use std::io::Write;

use indexmap::IndexMap;

use super::wire::{JsonBundle, JsonDocument, JsonRecord, JsonSection, JsonValue, OneOrMany};
use crate::namespaces::WriteScope;
use crate::well_known::{XSD_DATETIME, XSD_QNAME};
use crate::{Bundle, Document, Error, FormalRef, Kind, Namespaces, QName, Record, Result, Value};

/// Write `document` as PROV-JSON, pretty-printed with two-space indentation.
///
/// Records without an identifier get `_:id1`, `_:id2`, ... in document order.
/// A namespace with no declared prefix gets a minted `ns1`, `ns2`, ...
///
/// # Errors
///
/// - [`Error::Io`](crate::Error::Io) if `out` fails.
/// - [`Error::InvalidRecord`](crate::Error::InvalidRecord) for an attribute
///   that spells one of its record's own formals.
pub fn write(document: &Document, out: &mut impl Write) -> Result<()> {
    let wire = to_wire(document)?;
    serde_json::to_writer_pretty(&mut *out, &wire).map_err(|e| {
        // serde_json wraps a failing writer's error; report it as IO.
        if e.io_error_kind().is_some() {
            Error::Io(e.into())
        } else {
            Error::Json(e)
        }
    })?;
    out.write_all(b"\n")?;
    Ok(())
}

/// Write `document` as a PROV-JSON string.
///
/// # Errors
///
/// As [`write()`], except that [`Error::Io`](crate::Error::Io) cannot occur.
///
/// # Panics
///
/// Never in practice. `serde_json` writes valid UTF-8 and the trailing
/// newline is ASCII, so the buffer always converts to a `String`.
pub fn to_string(document: &Document) -> Result<String> {
    let mut buf = Vec::new();
    write(document, &mut buf)?;
    Ok(String::from_utf8(buf).expect("serde_json writes UTF-8"))
}

/// The `prefix` key that holds the default namespace.
const DEFAULT: &str = "default";

fn to_wire(document: &Document) -> Result<JsonDocument> {
    // A copy of the document's table, so minting a prefix never touches the
    // caller's document. It is written last, after every name has been seen.
    let mut table = working_table(&document.namespaces)?;
    let mut next_blank = 0;
    let sections = sections(
        &document.records,
        &mut WriteScope::of(&mut table),
        &mut next_blank,
    )?;
    let mut bundles = IndexMap::new();
    for bundle in document.merged_bundles().iter() {
        let (bundle_key, wire) = bundle_wire(bundle, &table, &bundles, &mut next_blank)?;
        bundles.insert(bundle_key, wire);
    }
    Ok(JsonDocument {
        prefix: prefix_table(&table, |_| true, document.namespaces.default_namespace()),
        sections,
        bundle: bundles,
    })
}

/// `namespaces` without a prefix named `default`: PROV-JSON reads that key
/// as the default namespace, so names under such a prefix get a minted `nsN`.
fn working_table(namespaces: &Namespaces) -> Result<Namespaces> {
    let mut table = Namespaces::new();
    if let Some(default) = namespaces.default_namespace() {
        table.set_default(default);
    }
    for (prefix, iri) in namespaces.iter().filter(|(p, _)| *p != DEFAULT) {
        table.declare(prefix, iri)?;
    }
    Ok(table)
}

/// The key or value text that writes `name`.
fn key(name: &QName, names: &mut WriteScope<'_>) -> String {
    match names.render_parts(name) {
        Some(prefix) => format!("{prefix}:{}", name.local()),
        None => name.local().to_owned(),
    }
}

/// A bundle and its key, both written against the bundle's table over the
/// document's: bundle declarations win, undeclared prefixes fall back.
///
/// Bundles with different identifiers can render to one key when each
/// bundle binds the prefix differently. The later bundle then gets a fresh
/// `nsN` prefix, declared in its own `prefix` block, for its key.
fn bundle_wire(
    bundle: &Bundle,
    outer: &Namespaces,
    written: &IndexMap<String, JsonBundle>,
    next_blank: &mut usize,
) -> Result<(String, JsonBundle)> {
    // The bundle's own table, which also takes the prefixes minted for it.
    let mut own = working_table(&bundle.namespaces)?;
    let (bundle_key, sections) = {
        let mut names = WriteScope::within(&mut own, outer);
        let mut bundle_key = key(&bundle.id, &mut names);
        if written.contains_key(&bundle_key) {
            let local = bundle.id.local();
            let prefix = names.mint_prefix(bundle.id.namespace(), |p| {
                !written.contains_key(&format!("{p}:{local}"))
            });
            bundle_key = format!("{prefix}:{local}");
        }
        (
            bundle_key,
            sections(&bundle.records, &mut names, next_blank)?,
        )
    };
    // Entries the document already declares are inherited, not repeated.
    let inherited = |prefix: &str, iri: &str| outer.lookup(prefix) == Some(iri);
    let prefix = prefix_table(
        &own,
        |(p, iri)| !inherited(p, iri),
        bundle.namespaces.default_namespace(),
    );
    Ok((bundle_key, JsonBundle { prefix, sections }))
}

/// The `prefix` object: declared prefixes that pass `keep`, then `default`.
fn prefix_table(
    table: &Namespaces,
    keep: impl Fn((&str, &str)) -> bool,
    default: Option<&str>,
) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for (prefix, iri) in table.iter() {
        if keep((prefix, iri)) {
            out.insert(prefix.to_owned(), iri.to_string());
        }
    }
    if let Some(default) = default {
        out.insert(DEFAULT.to_owned(), default.to_string());
    }
    out
}

/// Records grouped by kind in `Kind::ALL` order, then by identifier. Blank
/// labels follow record order, whatever the grouping.
fn sections(
    records: &[Record],
    names: &mut WriteScope<'_>,
    next_blank: &mut usize,
) -> Result<IndexMap<Kind, JsonSection>> {
    let blanks: Vec<Option<String>> = records
        .iter()
        .map(|r| {
            r.id().is_none().then(|| {
                *next_blank += 1;
                format!("_:id{next_blank}")
            })
        })
        .collect();
    let mut out = IndexMap::new();
    for &kind in Kind::ALL {
        let mut section: IndexMap<String, Vec<JsonRecord>> = IndexMap::new();
        for (record, blank) in records
            .iter()
            .zip(&blanks)
            .filter(|(r, _)| r.kind() == kind)
        {
            let id = match (record.id(), blank) {
                (Some(id), _) => key(id, names),
                (None, Some(label)) => label.clone(),
                (None, None) => unreachable!("a record without an id has a label"),
            };
            section
                .entry(id)
                .or_default()
                .push(record_wire(record, names)?);
        }
        if section.is_empty() {
            continue;
        }
        let section: JsonSection = section
            .into_iter()
            .map(|(id, mut bodies)| {
                let group = if bodies.len() == 1 {
                    OneOrMany::One(bodies.remove(0))
                } else {
                    OneOrMany::Many(bodies)
                };
                (id, group)
            })
            .collect();
        out.insert(kind, section);
    }
    Ok(out)
}

/// A record's body. An attribute spelling one of the record's own formals
/// would overwrite or pose as that formal, so it is an error.
fn record_wire(record: &Record, names: &mut WriteScope<'_>) -> Result<JsonRecord> {
    record.check_attributes()?;
    let mut body = JsonRecord::new();
    for (attr, value) in record.formals() {
        let text = match value {
            FormalRef::Name(q) => key(q, names),
            FormalRef::Time(t) => t.to_string(),
        };
        body.insert(attr.to_string(), JsonValue::Str(text));
    }
    // Several values under one name become an array, in insertion order.
    let mut grouped: IndexMap<String, Vec<JsonValue>> = IndexMap::new();
    for (name, value) in record.attributes() {
        grouped
            .entry(key(name, names))
            .or_default()
            .push(value_wire(value, names));
    }
    for (name, mut values) in grouped {
        let value = if values.len() == 1 {
            values.remove(0)
        } else {
            JsonValue::Many(values)
        };
        body.insert(name, value);
    }
    Ok(body)
}

/// Only plain strings are written bare. Everything else is a `$` object, so
/// the reader never sees a raw JSON number or boolean.
fn value_wire(value: &Value, names: &mut WriteScope<'_>) -> JsonValue {
    let mut typed = |value: String, datatype: &QName| JsonValue::Typed {
        value,
        datatype: Some(key(datatype, names)),
        lang: None,
    };
    match value {
        Value::Str(s) => JsonValue::Str(s.clone()),
        // The note requires `lang` alone: section 2.2.
        Value::LangStr(l) => JsonValue::Typed {
            value: l.value().to_owned(),
            datatype: None,
            lang: Some(l.lang().to_owned()),
        },
        Value::QName(q) => {
            let text = key(q, names);
            JsonValue::Typed {
                value: text,
                datatype: Some(key(&XSD_QNAME, names)),
                lang: None,
            }
        }
        Value::DateTime(d) => typed(d.to_string(), &XSD_DATETIME),
        Value::Typed(t) => typed(t.lexical().to_owned(), t.datatype()),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::{Broken, bundle, ex};
    use crate::{
        Attributes, Bundle, Document, Entity, Generation, Kind, LangStr, Namespaces, QName, Record,
        json,
    };

    fn entity_in(id: QName) -> Record {
        Record::Entity(Entity {
            id,
            attributes: Attributes::new(),
        })
    }

    #[test]
    fn a_bundle_writes_bare_names_against_its_own_default() {
        let one = QName::new("http://one.org/", "a").unwrap();
        let two = QName::new("http://two.org/", "b").unwrap();
        let mut d = Document::new();
        d.namespaces.set_default("http://one.org/");
        let mut inner = bundle("b1", vec![entity_in(one), entity_in(two)]);
        inner.namespaces.set_default("http://two.org/");
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.bundles.push(inner);

        let text = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entities = v["bundle"]["ex:b1"]["entity"].as_object().unwrap();
        // The name in the bundle's default namespace is bare; the other is not.
        assert!(entities.contains_key("b"), "{text}");
        assert!(!entities.contains_key("a"), "{text}");
        assert_eq!(json::read(&text).unwrap(), d);
    }

    #[test]
    fn a_bundle_inherits_a_document_prefix_and_declares_a_minted_one() {
        let minted = QName::new("http://new.org/", "m").unwrap();
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.bundles
            .push(bundle("b1", vec![entity_in(ex("e")), entity_in(minted)]));

        let text = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            v["prefix"],
            serde_json::json!({"ex": "http://example.org/"})
        );
        let inner = &v["bundle"]["ex:b1"];
        assert_eq!(
            inner["prefix"],
            serde_json::json!({"ns1": "http://new.org/"})
        );
        let entities = inner["entity"].as_object().unwrap();
        assert!(entities.contains_key("ex:e"), "{text}");
        assert!(entities.contains_key("ns1:m"), "{text}");
        assert_eq!(json::read(&text).unwrap(), d);
    }

    #[test]
    fn output_is_indented_by_two_spaces_and_ends_with_a_newline() {
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.records.push(entity_in(ex("e")));
        assert_eq!(
            json::to_string(&d).unwrap(),
            "{\n  \"prefix\": {\n    \"ex\": \"http://example.org/\"\n  },\n  \"entity\": {\n    \"ex:e\": {}\n  }\n}\n"
        );
    }

    #[test]
    fn shape_and_blank_ids() {
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.records.push(Record::Generation(Generation {
            id: None,
            entity: Some(ex("e")),
            activity: None,
            time: None,
            attributes: Attributes::new(),
        }));
        d.records.push(entity_in(ex("e")));
        d.records
            .push(entity_in(QName::new("http://other/", "o").unwrap()));
        let text = json::to_string(&d).unwrap();
        let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("no {needle}"));
        assert!(at("\"prefix\"") < at("\"entity\""));
        assert!(at("\"entity\"") < at("\"wasGeneratedBy\""));
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v.as_object().unwrap().len(), 3);
        assert_eq!(v["prefix"]["ns1"], "http://other/");
        assert_eq!(v["entity"]["ns1:o"], serde_json::json!({}));
        assert_eq!(v["wasGeneratedBy"]["_:id1"]["prov:entity"], "ex:e");
        // The caller's document is untouched.
        assert_eq!(d.namespaces.iter().count(), 1);
    }

    #[test]
    fn blank_ids_continue_through_bundles() {
        let blank = || {
            Record::Generation(Generation {
                id: None,
                entity: Some(ex("e")),
                activity: None,
                time: None,
                attributes: Attributes::new(),
            })
        };
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.records.push(blank());
        d.bundles.push(bundle("b", vec![blank(), blank()]));
        let v: serde_json::Value = serde_json::from_str(&json::to_string(&d).unwrap()).unwrap();
        assert!(v["wasGeneratedBy"]["_:id1"].is_object());
        let inner = &v["bundle"]["ex:b"]["wasGeneratedBy"];
        assert!(inner["_:id2"].is_object() && inner["_:id3"].is_object());
    }

    #[test]
    fn blank_ids_follow_record_order() {
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.records
            .push(Record::from_parts(Kind::Usage, None, vec![], Attributes::new()).unwrap());
        d.records
            .push(Record::from_parts(Kind::Generation, None, vec![], Attributes::new()).unwrap());
        let v: serde_json::Value = serde_json::from_str(&json::to_string(&d).unwrap()).unwrap();
        assert!(v["used"]["_:id1"].is_object());
        assert!(v["wasGeneratedBy"]["_:id2"].is_object());
    }

    #[test]
    fn failing_writer_reports_io() {
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.records.push(entity_in(ex("e")));
        assert!(matches!(
            json::write(&d, &mut Broken),
            Err(crate::Error::Io(_))
        ));
    }

    #[test]
    fn merged_bundles_with_clashing_prefixes_round_trip() {
        let bundle = |iri: &str, local: &str| {
            let mut ns = Namespaces::new();
            ns.declare("o", iri).unwrap();
            Bundle {
                id: ex("b"),
                namespaces: ns,
                records: vec![entity_in(QName::new(iri, local).unwrap())],
            }
        };
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.bundles.push(bundle("http://one.org/", "x"));
        d.bundles.push(bundle("http://two.org/", "y"));
        let text = json::to_string(&d).unwrap();
        assert_eq!(json::read(&text).unwrap(), d);
    }

    #[test]
    fn default_namespace_never_mints_a_prefix() {
        let mut d = Document::new();
        d.namespaces.set_default("http://example.org/");
        d.records.push(entity_in(ex("e")));
        let text = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            v["prefix"],
            serde_json::json!({"default": "http://example.org/"})
        );
        assert!(v["entity"]["e"].is_object());
        assert_eq!(json::read(&text).unwrap(), d);
    }

    #[test]
    fn every_value_variant_round_trips() {
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        let mut a = Attributes::new();
        a.insert(ex("s"), "plain".into());
        a.insert(ex("l"), LangStr::new("bonjour", "fr").unwrap().into());
        a.insert(ex("q"), ex("q").into());
        a.insert(
            ex("d"),
            crate::DateTime::parse("2011-11-16T16:05:33.5+01:00")
                .unwrap()
                .into(),
        );
        a.insert(ex("i"), 7i32.into());
        a.insert(ex("f"), 1.5f64.into());
        a.insert(ex("b"), true.into());
        a.insert(ex("s"), "second".into());
        d.records.push(Record::Entity(Entity {
            id: ex("e"),
            attributes: a,
        }));
        for &kind in Kind::ALL {
            let id = Some(ex(kind.name()));
            d.records
                .push(Record::from_parts(kind, id, vec![], Attributes::new()).unwrap());
        }
        let text = json::to_string(&d).unwrap();
        assert_eq!(json::read(&text).unwrap(), d);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let body = &v["entity"]["ex:e"];
        for key in ["ex:i", "ex:f", "ex:b", "ex:q", "ex:d", "ex:l"] {
            assert!(body[key]["$"].is_string(), "{key} must be a `$` string");
        }
        assert_eq!(body["ex:l"]["lang"], "fr");
        assert!(body["ex:l"].get("type").is_none(), "{text}");
        assert_eq!(body["ex:q"]["type"], "xsd:QName");
        assert_eq!(body["ex:s"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn bundle_redeclaring_its_own_prefix_round_trips() {
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.records.push(entity_in(ex("top")));
        let mut inner = Namespaces::new();
        inner.declare("ex", "http://shadow.org/").unwrap();
        d.bundles.push(Bundle {
            id: QName::new("http://shadow.org/", "b").unwrap(),
            namespaces: inner,
            records: vec![
                entity_in(QName::new("http://shadow.org/", "e").unwrap()),
                entity_in(ex("f")),
            ],
        });
        let text = json::to_string(&d).unwrap();
        assert!(text.contains("\"ex:b\""), "key uses the bundle's table");
        assert_eq!(json::read(&text).unwrap(), d);
    }

    #[test]
    fn attribute_spelling_an_own_formal_fails_to_write() {
        let mut attributes = Attributes::new();
        attributes.insert(
            QName::new(crate::well_known::PROV, "activity").unwrap(),
            ex("a2").into(),
        );
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.records.push(Record::Generation(Generation {
            id: None,
            entity: Some(ex("e")),
            activity: Some(ex("a")),
            time: None,
            attributes,
        }));
        assert!(matches!(
            json::to_string(&d),
            Err(crate::Error::InvalidRecord {
                kind: Kind::Generation,
                ..
            })
        ));
    }

    #[test]
    fn a_prefix_named_default_is_minted_afresh() {
        let mut d = Document::new();
        d.namespaces.declare("default", "http://d.org/").unwrap();
        d.namespaces.set_default("http://example.org/");
        d.records
            .push(entity_in(QName::new("http://d.org/", "e").unwrap()));
        let mut inner = Namespaces::new();
        inner.declare("default", "http://b.org/").unwrap();
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: inner,
            records: vec![entity_in(QName::new("http://b.org/", "f").unwrap())],
        });
        let text = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            v["prefix"],
            serde_json::json!({"ns1": "http://d.org/", "default": "http://example.org/"})
        );
        assert!(v["entity"]["ns1:e"].is_object());
        assert_eq!(
            v["bundle"]["b"]["prefix"],
            serde_json::json!({"ns2": "http://b.org/"})
        );
        assert_eq!(json::read(&text).unwrap(), d);
    }

    #[test]
    fn bundles_sharing_an_identifier_merge_and_round_trip() {
        let mut d = Document::new();
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        let mut other = Namespaces::new();
        other.declare("o", "http://o.org/").unwrap();
        d.bundles.push(bundle("b", vec![entity_in(ex("one"))]));
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: other,
            records: vec![entity_in(QName::new("http://o.org/", "two").unwrap())],
        });
        let text = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["bundle"].as_object().unwrap().len(), 1);
        assert_eq!(json::read(&text).unwrap(), d);
    }

    /// Two bundles whose identifiers differ but whose own tables spell both
    /// as `ex:b`.
    fn colliding_bundles() -> Document {
        let bundle = |iri: &str| {
            let mut ns = Namespaces::new();
            ns.declare("ex", iri).unwrap();
            Bundle {
                id: QName::new(iri, "b").unwrap(),
                namespaces: ns,
                records: vec![entity_in(QName::new(iri, "e").unwrap())],
            }
        };
        let mut d = Document::new();
        d.bundles.push(bundle("http://a.org/"));
        d.bundles.push(bundle("http://c.org/"));
        d
    }

    #[test]
    fn colliding_bundle_keys_get_a_fresh_prefix_and_round_trip() {
        let d = colliding_bundles();
        let text = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let bundles = v["bundle"].as_object().unwrap();
        assert_eq!(bundles.len(), 2, "{text}");
        assert!(bundles.contains_key("ex:b"), "{text}");
        assert_eq!(
            v["bundle"]["ns1:b"]["prefix"],
            serde_json::json!({"ex": "http://c.org/", "ns1": "http://c.org/"}),
            "{text}"
        );
        assert_eq!(json::read(&text).unwrap(), d);

        // A minted key never lands on a key an earlier bundle already owns.
        let mut d = colliding_bundles();
        let mut third = Namespaces::new();
        third.declare("ns1", "http://x.org/").unwrap();
        d.bundles.insert(
            1,
            Bundle {
                id: QName::new("http://x.org/", "b").unwrap(),
                namespaces: third,
                records: vec![],
            },
        );
        let text = json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let keys: Vec<_> = v["bundle"].as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["ex:b", "ns1:b", "ns2:b"], "{text}");
        assert_eq!(json::read(&text).unwrap(), d);
    }

    #[cfg(feature = "provn")]
    #[test]
    fn colliding_bundles_read_from_provn_survive_json() {
        let text = "document
  bundle ex:b
    prefix ex <http://a.org/>
    entity(ex:e)
  endBundle
  bundle ex:b
    prefix ex <http://c.org/>
    entity(ex:e)
  endBundle
endDocument
";
        let d = crate::provn::read(text).unwrap();
        assert_eq!(d, colliding_bundles());
        let json = json::to_string(&d).unwrap();
        assert_eq!(json::read(&json).unwrap(), d, "{json}");
    }
}
