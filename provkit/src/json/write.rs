//! Document to wire structs to JSON text.

use std::io::Write;

use indexmap::IndexMap;

use super::wire::{JsonBundle, JsonDocument, JsonRecord, JsonSection, JsonValue, OneOrMany};
use crate::document::merged_bundles;
use crate::well_known::{PROV_INTERNATIONALIZED_STRING, XSD_DATETIME, XSD_QNAME};
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
    let mut table = document.namespaces.clone();
    // `default` is the default-namespace key in PROV-JSON, so a prefix of that
    // name is dropped and its names get a minted `nsN`.
    table.forget(DEFAULT);
    let mut next_blank = 0;
    let sections = sections(&document.records, &mut table, &mut next_blank)?;
    let mut bundles = IndexMap::new();
    for bundle in merged_bundles(&document.bundles) {
        let (key, wire) = bundle_wire(&bundle, &table, &bundles, &mut next_blank)?;
        bundles.insert(key, wire);
    }
    Ok(JsonDocument {
        prefix: prefix_table(&table, |_| true, document.namespaces.default_namespace()),
        sections,
        bundle: bundles,
    })
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
    let mut table = outer.clone();
    for (prefix, iri) in bundle.namespaces.iter().filter(|(p, _)| *p != DEFAULT) {
        table.declare(prefix, iri)?;
    }
    if let Some(default) = bundle.namespaces.default_namespace() {
        table.set_default(default);
    }
    let mut key = table.render(&bundle.id);
    if written.contains_key(&key) {
        let local = bundle.id.local();
        let prefix = table.mint_prefix(bundle.id.namespace(), |p| {
            !written.contains_key(&format!("{p}:{local}"))
        });
        key = format!("{prefix}:{local}");
    }
    let sections = sections(&bundle.records, &mut table, next_blank)?;
    // Entries the document already declares are inherited, not repeated.
    let inherited = |prefix: &str, iri: &str| outer.lookup(prefix) == Some(iri);
    let prefix = prefix_table(
        &table,
        |(p, iri)| !inherited(p, iri),
        bundle.namespaces.default_namespace(),
    );
    Ok((key, JsonBundle { prefix, sections }))
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
    table: &mut Namespaces,
    next_blank: &mut usize,
) -> Result<IndexMap<String, JsonSection>> {
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
            let key = match (record.id(), blank) {
                (Some(id), _) => table.render(id),
                (None, Some(label)) => label.clone(),
                (None, None) => unreachable!("a record without an id has a label"),
            };
            section
                .entry(key)
                .or_default()
                .push(record_wire(record, table)?);
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
        out.insert(kind.name().to_owned(), section);
    }
    Ok(out)
}

/// A record's body. An attribute spelling one of the record's own formals
/// would overwrite or pose as that formal, so it is an error.
fn record_wire(record: &Record, table: &mut Namespaces) -> Result<JsonRecord> {
    record.check_attributes()?;
    let mut body = JsonRecord::new();
    for (attr, value) in record.formals() {
        let text = match value {
            FormalRef::Name(q) => table.render(q),
            FormalRef::Time(t) => t.to_string(),
        };
        body.insert(format!("prov:{}", attr.local_name()), JsonValue::Str(text));
    }
    // Several values under one name become an array, in insertion order.
    let mut grouped: IndexMap<String, Vec<JsonValue>> = IndexMap::new();
    for (name, value) in record.attributes().iter() {
        let key = table.render(name);
        grouped
            .entry(key)
            .or_default()
            .push(value_wire(value, table));
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
fn value_wire(value: &Value, table: &mut Namespaces) -> JsonValue {
    debug_assert!(
        value.has_one_representation(),
        "{value:?} has a folded datatype"
    );
    let mut typed = |value: String, datatype: &QName| JsonValue::Typed {
        value,
        datatype: Some(table.render(datatype)),
        lang: None,
    };
    match value {
        Value::Str(s) => JsonValue::Str(s.clone()),
        Value::LangStr(l) => JsonValue::Typed {
            value: l.value().to_owned(),
            datatype: Some(table.render(&PROV_INTERNATIONALIZED_STRING)),
            lang: Some(l.lang().to_owned()),
        },
        Value::QName(q) => {
            let text = table.render(q);
            JsonValue::Typed {
                value: text,
                datatype: Some(table.render(&XSD_QNAME)),
                lang: None,
            }
        }
        Value::DateTime(d) => typed(d.to_string(), &XSD_DATETIME),
        Value::Typed { lexical, datatype } => typed(lexical.clone(), datatype),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::q as ex;
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
        d.records.push(entity_in(QName::new("http://other/", "o")));
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
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: Namespaces::new(),
            records: vec![blank(), blank()],
        });
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
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("broken"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
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
                records: vec![entity_in(QName::new(iri, local))],
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
        assert_eq!(body["ex:l"]["type"], "prov:InternationalizedString");
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
            id: QName::new("http://shadow.org/", "b"),
            namespaces: inner,
            records: vec![
                entity_in(QName::new("http://shadow.org/", "e")),
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
            QName::new(crate::well_known::PROV, "activity"),
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
        d.records.push(entity_in(QName::new("http://d.org/", "e")));
        let mut inner = Namespaces::new();
        inner.declare("default", "http://b.org/").unwrap();
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: inner,
            records: vec![entity_in(QName::new("http://b.org/", "f"))],
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
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: Namespaces::new(),
            records: vec![entity_in(ex("one"))],
        });
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: other,
            records: vec![entity_in(QName::new("http://o.org/", "two"))],
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
                id: QName::new(iri, "b"),
                namespaces: ns,
                records: vec![entity_in(QName::new(iri, "e"))],
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
                id: QName::new("http://x.org/", "b"),
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
