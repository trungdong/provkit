//! Document to wire structs to JSON text.

use std::io::Write;
use std::mem;

use indexmap::IndexMap;
use indexmap::map::Entry;

use super::wire::{
    DEFAULT, JsonBundle, JsonDocument, JsonRecord, JsonSection, JsonValue, OneOrMany,
};
use crate::namespaces::WriteScope;
use crate::well_known::{XSD_DATETIME, XSD_QNAME};
use crate::{
    Bundle, Document, Error, FormalRef, JsonError, Kind, Namespaces, QName, Record, Result, Value,
};

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
            Error::Json(JsonError::unpositioned(e))
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
        prefix: prefix_table(&table, |_| true),
        sections,
        bundle: bundles.into_iter().collect(),
        repeated_bundle: None,
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
    let prefix = prefix_table(&own, |(p, iri)| !inherited(p, iri));
    Ok((bundle_key, JsonBundle { prefix, sections }))
}

/// The `prefix` object: declared prefixes that pass `keep`, then `default`.
fn prefix_table(
    table: &Namespaces,
    keep: impl Fn((&str, &str)) -> bool,
) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for (prefix, iri) in table.iter() {
        if keep((prefix, iri)) {
            out.insert(prefix.to_owned(), iri.to_string());
        }
    }
    if let Some(default) = table.default_namespace() {
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
    // Each record's identifier, or the blank label it is written under.
    let ids: Vec<std::result::Result<&QName, String>> = records
        .iter()
        .map(|r| {
            r.id().ok_or_else(|| {
                *next_blank += 1;
                format!("_:id{next_blank}")
            })
        })
        .collect();
    let mut out = IndexMap::new();
    for &kind in Kind::ALL {
        let mut section: IndexMap<String, Vec<JsonRecord>> = IndexMap::new();
        for (record, id) in records.iter().zip(&ids).filter(|(r, _)| r.kind() == kind) {
            let id = match id {
                Ok(id) => key(id, names),
                Err(label) => label.clone(),
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
    // No attribute key equals a formal's, because `check_attributes` has
    // refused an attribute named after one.
    for (name, value) in record.attributes() {
        let name = key(name, names);
        let value = value_wire(value, names);
        match body.entry(name) {
            Entry::Vacant(slot) => {
                slot.insert(value);
            }
            Entry::Occupied(mut slot) => match slot.get_mut() {
                JsonValue::Many(values) => values.push(value),
                first => {
                    let first_value = mem::replace(first, JsonValue::Many(Vec::new()));
                    *first = JsonValue::Many(vec![first_value, value]);
                }
            },
        }
    }
    Ok(body)
}

/// Only plain strings are written bare. Everything else is a `$` object, so
/// the reader never sees a raw JSON number or boolean.
fn value_wire(value: &Value, names: &mut WriteScope<'_>) -> JsonValue {
    match value {
        Value::Str(s) => JsonValue::Str(s.clone()),
        // The PROV-JSON note, section 2.2, gives a language-tagged string
        // `lang` and no `type`.
        Value::LangStr(l) => JsonValue::Typed {
            value: l.value().to_owned(),
            datatype: None,
            lang: Some(l.lang().to_owned()),
        },
        Value::QName(q) => {
            let text = key(q, names);
            typed(text, &XSD_QNAME, names)
        }
        Value::DateTime(d) => typed(d.to_string(), &XSD_DATETIME, names),
        Value::Typed(t) => typed(t.lexical().to_owned(), t.datatype(), names),
    }
}

/// A `$` object with `type`.
fn typed(value: String, datatype: &QName, names: &mut WriteScope<'_>) -> JsonValue {
    JsonValue::Typed {
        value,
        datatype: Some(key(datatype, names)),
        lang: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::{
        Broken, bundle, document, element, entity, entity_in, ex, relation, round_trip, written,
    };
    use crate::{
        Attributes, Bundle, Document, Formal, FormalAttr, Generation, Kind, LangStr, Namespaces,
        QName, Record, json,
    };

    /// A `wasGeneratedBy` of `ex:e` with no identifier.
    fn blank_generation() -> Record {
        relation(
            Kind::Generation,
            None,
            vec![(FormalAttr::Entity, Formal::Name(ex("e")))],
        )
    }

    #[test]
    fn a_bundle_writes_bare_names_against_its_own_default() {
        let mut d = Document::new();
        d.namespaces.set_default("http://one.org/");
        let mut inner = bundle(
            "b1",
            vec![
                entity_in("http://one.org/", "a"),
                entity_in("http://two.org/", "b"),
            ],
        );
        inner.namespaces.set_default("http://two.org/");
        d.namespaces.declare("ex", "http://example.org/").unwrap();
        d.bundles.push(inner);

        let (text, v) = round_trip(&d);
        let entities = v["bundle"]["ex:b1"]["entity"].as_object().unwrap();
        // The name in the bundle's default namespace is bare; the other is not.
        assert!(entities.contains_key("b"), "{text}");
        assert!(!entities.contains_key("a"), "{text}");
    }

    #[test]
    fn a_bundle_inherits_a_document_prefix_and_declares_a_minted_one() {
        let mut d = document(vec![]);
        d.bundles.push(bundle(
            "b1",
            vec![entity("e"), entity_in("http://new.org/", "m")],
        ));

        let (text, v) = round_trip(&d);
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
    }

    #[test]
    fn a_bundle_redeclaring_a_document_prefix_writes_it_only_at_another_iri() {
        // The same IRI: inherited, so no prefix block.
        let mut d = document(vec![]);
        let mut same = bundle("b1", vec![entity("e")]);
        same.namespaces
            .declare("ex", "http://example.org/")
            .unwrap();
        d.bundles.push(same);
        let (text, v) = round_trip(&d);
        assert!(v["bundle"]["ex:b1"].is_object(), "{text}");
        assert!(v["bundle"]["ex:b1"].get("prefix").is_none(), "{text}");

        // Another IRI: the bundle's own binding wins and is declared. In the
        // bundle's scope `ex` names other.org, so the bundle's own IRI is
        // written under a minted `ns1`, declared beside `ex`.
        let mut d = document(vec![]);
        let mut other = bundle("b2", vec![entity_in("http://other.org/", "o")]);
        other.namespaces.declare("ex", "http://other.org/").unwrap();
        d.bundles.push(other);
        let (text, v) = round_trip(&d);
        assert_eq!(
            v["bundle"]["ns1:b2"]["prefix"],
            serde_json::json!({"ex": "http://other.org/", "ns1": "http://example.org/"}),
            "{text}"
        );
        assert!(
            v["bundle"]["ns1:b2"]["entity"].get("ex:o").is_some(),
            "{text}"
        );
    }

    #[test]
    fn output_is_indented_by_two_spaces_and_ends_with_a_newline() {
        let d = document(vec![entity("e")]);
        assert_eq!(
            json::to_string(&d).unwrap(),
            "{\n  \"prefix\": {\n    \"ex\": \"http://example.org/\"\n  },\n  \"entity\": {\n    \"ex:e\": {}\n  }\n}\n"
        );
    }

    #[test]
    fn shape_and_blank_ids() {
        let d = document(vec![
            blank_generation(),
            entity("e"),
            entity_in("http://other/", "o"),
        ]);
        let (text, v) = written(&d);
        let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("no {needle}"));
        assert!(at("\"prefix\"") < at("\"entity\""));
        assert!(at("\"entity\"") < at("\"wasGeneratedBy\""));
        assert_eq!(v.as_object().unwrap().len(), 3);
        assert_eq!(v["prefix"]["ns1"], "http://other/");
        assert_eq!(v["entity"]["ns1:o"], serde_json::json!({}));
        assert_eq!(v["wasGeneratedBy"]["_:id1"]["prov:entity"], "ex:e");
        // The caller's document is untouched.
        assert_eq!(d.namespaces.iter().count(), 1);
    }

    #[test]
    fn blank_ids_continue_through_bundles() {
        let mut d = document(vec![blank_generation()]);
        d.bundles
            .push(bundle("b", vec![blank_generation(), blank_generation()]));
        let (_, v) = written(&d);
        assert!(v["wasGeneratedBy"]["_:id1"].is_object());
        let inner = &v["bundle"]["ex:b"]["wasGeneratedBy"];
        assert!(inner["_:id2"].is_object() && inner["_:id3"].is_object());
    }

    #[test]
    fn blank_ids_follow_record_order() {
        let d = document(vec![
            relation(Kind::Usage, None, vec![]),
            relation(Kind::Generation, None, vec![]),
        ]);
        let (_, v) = written(&d);
        assert!(v["used"]["_:id1"].is_object());
        assert!(v["wasGeneratedBy"]["_:id2"].is_object());
    }

    #[test]
    fn failing_writer_reports_io() {
        let d = document(vec![entity("e")]);
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
                records: vec![entity_in(iri, local)],
            }
        };
        let mut d = document(vec![]);
        d.bundles.push(bundle("http://one.org/", "x"));
        d.bundles.push(bundle("http://two.org/", "y"));
        round_trip(&d);
    }

    #[test]
    fn default_namespace_never_mints_a_prefix() {
        let mut d = Document::new();
        d.namespaces.set_default("http://example.org/");
        d.records.push(entity("e"));
        let (_, v) = round_trip(&d);
        assert_eq!(
            v["prefix"],
            serde_json::json!({"default": "http://example.org/"})
        );
        assert!(v["entity"]["e"].is_object());
    }

    #[test]
    fn every_value_variant_round_trips() {
        let mut a = Attributes::new();
        a.insert(ex("s"), "plain");
        a.insert(ex("l"), LangStr::new("bonjour", "fr").unwrap());
        a.insert(ex("q"), ex("q"));
        a.insert(
            ex("d"),
            crate::DateTime::parse("2011-11-16T16:05:33.5+01:00").unwrap(),
        );
        a.insert(ex("i"), 7i32);
        a.insert(ex("f"), 1.5f64);
        a.insert(ex("b"), true);
        a.insert(ex("s"), "second");
        let mut d = document(vec![element(Kind::Entity, ex("e"), a)]);
        for &kind in Kind::ALL {
            d.records
                .push(relation(kind, Some(ex(kind.name())), vec![]));
        }
        let (text, v) = round_trip(&d);
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
        let mut d = document(vec![entity("top")]);
        let mut inner = Namespaces::new();
        inner.declare("ex", "http://shadow.org/").unwrap();
        d.bundles.push(Bundle {
            id: QName::new("http://shadow.org/", "b").unwrap(),
            namespaces: inner,
            records: vec![entity_in("http://shadow.org/", "e"), entity("f")],
        });
        let (text, _) = round_trip(&d);
        assert!(text.contains("\"ex:b\""), "key uses the bundle's table");
    }

    #[test]
    fn records_sharing_an_identifier_are_written_as_an_array() {
        let tagged = |tag: &str| {
            let mut attributes = Attributes::new();
            attributes.insert(ex("tag"), tag);
            element(Kind::Entity, ex("e"), attributes)
        };
        let d = document(vec![tagged("one"), tagged("two"), entity("single")]);
        let (_, v) = round_trip(&d);
        assert_eq!(v["entity"]["ex:e"].as_array().map(Vec::len), Some(2));
        assert!(v["entity"]["ex:single"].is_object());
    }

    #[test]
    fn attribute_spelling_an_own_formal_fails_to_write() {
        let mut attributes = Attributes::new();
        attributes.insert(
            QName::new(crate::well_known::PROV, "activity").unwrap(),
            ex("a2"),
        );
        let d = document(vec![Record::Generation(Generation {
            id: None,
            entity: Some(ex("e")),
            activity: Some(ex("a")),
            time: None,
            attributes,
        })]);
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
        d.records.push(entity_in("http://d.org/", "e"));
        let mut inner = Namespaces::new();
        inner.declare("default", "http://b.org/").unwrap();
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: inner,
            records: vec![entity_in("http://b.org/", "f")],
        });
        let (_, v) = round_trip(&d);
        assert_eq!(
            v["prefix"],
            serde_json::json!({"ns1": "http://d.org/", "default": "http://example.org/"})
        );
        assert!(v["entity"]["ns1:e"].is_object());
        assert_eq!(
            v["bundle"]["b"]["prefix"],
            serde_json::json!({"ns2": "http://b.org/"})
        );
    }

    #[test]
    fn bundles_sharing_an_identifier_merge_and_round_trip() {
        let mut d = document(vec![]);
        let mut other = Namespaces::new();
        other.declare("o", "http://o.org/").unwrap();
        d.bundles.push(bundle("b", vec![entity("one")]));
        d.bundles.push(Bundle {
            id: ex("b"),
            namespaces: other,
            records: vec![entity_in("http://o.org/", "two")],
        });
        let (_, v) = round_trip(&d);
        assert_eq!(v["bundle"].as_object().unwrap().len(), 1);
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
                records: vec![entity_in(iri, "e")],
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
        let (text, v) = round_trip(&d);
        let bundles = v["bundle"].as_object().unwrap();
        assert_eq!(bundles.len(), 2, "{text}");
        assert!(bundles.contains_key("ex:b"), "{text}");
        assert_eq!(
            v["bundle"]["ns1:b"]["prefix"],
            serde_json::json!({"ex": "http://c.org/", "ns1": "http://c.org/"}),
            "{text}"
        );

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
        let (text, v) = round_trip(&d);
        let keys: Vec<_> = v["bundle"].as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["ex:b", "ns1:b", "ns2:b"], "{text}");
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
        round_trip(&d);
    }
}
