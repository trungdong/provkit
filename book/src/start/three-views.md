# One document, three views

The same document, three times. A draft entity is used by an editing
activity, which generates a report. PROV-N is the notation people read,
PROV-JSON is the one programs exchange, and the Rust is what provkit
holds in memory for both.

## PROV-N

```provn
document
  prefix ex <http://example.org/>
  entity(ex:draft)
  entity(ex:report, [prov:label="Report"])
  activity(ex:edit, 2026-10-16T09:00:00, 2026-10-16T10:00:00)
  used(ex:edit, ex:draft, -)
  wasGeneratedBy(ex:report, ex:edit, -)
endDocument
```

Every statement is a record. `entity`, `activity` and the relations
`used` and `wasGeneratedBy` are four of the eighteen kinds provkit
holds, seventeen from PROV-DM and `mentionOf` from
[PROV-Links](https://www.w3.org/TR/2013/NOTE-prov-links-20130430/#term-mention).
`-` is an absent argument; the writer writes every position of a
relation, so a reader never has to count. `ex:draft` is a qualified
name. The prefix `ex` stands for the namespace declared at the top.

## PROV-JSON

```json
{
  "prefix": {
    "ex": "http://example.org/"
  },
  "entity": {
    "ex:draft": {},
    "ex:report": {
      "prov:label": "Report"
    }
  },
  "activity": {
    "ex:edit": {
      "prov:startTime": "2026-10-16T09:00:00",
      "prov:endTime": "2026-10-16T10:00:00"
    }
  },
  "wasGeneratedBy": {
    "_:id2": {
      "prov:entity": "ex:report",
      "prov:activity": "ex:edit"
    }
  },
  "used": {
    "_:id1": {
      "prov:activity": "ex:edit",
      "prov:entity": "ex:draft"
    }
  }
}
```

The same records, keyed by kind and then by identifier. A relation
without an identifier gets a blank-node label such as `_:id1` for the
key, which the reader throws away again. Attributes become key-value
pairs inside the record object.

## Rust

```rust
# extern crate provkit;
use provkit::{Activity, Attributes, DateTime, Document, Entity, Generation, QName, Record, Usage, json, provn};

fn main() -> Result<(), provkit::Error> {
    let ex = "http://example.org/";
    let mut doc = Document::new();
    doc.namespaces.declare("ex", ex)?;

    let mut label = Attributes::new();
    label.insert(QName::new("http://www.w3.org/ns/prov#", "label")?, "Report");

    doc.records.push(Record::Entity(Entity {
        id: QName::new(ex, "draft")?,
        attributes: Attributes::new(),
    }));
    doc.records.push(Record::Entity(Entity {
        id: QName::new(ex, "report")?,
        attributes: label,
    }));
    doc.records.push(Record::Activity(Activity {
        id: QName::new(ex, "edit")?,
        start_time: Some(DateTime::parse("2026-10-16T09:00:00")?),
        end_time: Some(DateTime::parse("2026-10-16T10:00:00")?),
        attributes: Attributes::new(),
    }));
    doc.records.push(Record::Usage(Usage {
        id: None,
        activity: Some(QName::new(ex, "edit")?),
        entity: Some(QName::new(ex, "draft")?),
        time: None,
        attributes: Attributes::new(),
    }));
    doc.records.push(Record::Generation(Generation {
        id: None,
        entity: Some(QName::new(ex, "report")?),
        activity: Some(QName::new(ex, "edit")?),
        time: None,
        attributes: Attributes::new(),
    }));

    let from_provn = provn::read(r#"document
  prefix ex <http://example.org/>
  entity(ex:draft)
  entity(ex:report, [prov:label="Report"])
  activity(ex:edit, 2026-10-16T09:00:00, 2026-10-16T10:00:00)
  used(ex:edit, ex:draft, -)
  wasGeneratedBy(ex:report, ex:edit, -)
endDocument
"#)?;
    let from_json = json::read(r#"{
  "prefix": {
    "ex": "http://example.org/"
  },
  "entity": {
    "ex:draft": {},
    "ex:report": {
      "prov:label": "Report"
    }
  },
  "activity": {
    "ex:edit": {
      "prov:startTime": "2026-10-16T09:00:00",
      "prov:endTime": "2026-10-16T10:00:00"
    }
  },
  "wasGeneratedBy": {
    "_:id2": {
      "prov:entity": "ex:report",
      "prov:activity": "ex:edit"
    }
  },
  "used": {
    "_:id1": {
      "prov:activity": "ex:edit",
      "prov:entity": "ex:draft"
    }
  }
}
"#)?;
    assert!(doc == from_provn);
    assert!(doc == from_json);
    Ok(())
}
```

A `Document` holds a namespace table, a list of records and a list of
bundles. Each record is one variant of the `Record` enum, wrapping a
struct with public fields, so a document can be built by hand as above
or read with `json::read` and `provn::read`. The two `assert!` lines
pass because `Document` equality is set-based, so record order, attribute
order, the namespace table and blank-node labels do not count.

## Check it

```sh
provkit compare book/examples/edit.provn book/examples/edit.json
echo $?
```

```text
0
```

`compare` reads both files, whichever format each is in, and exits 0
when the documents are equal, 1 when they differ, and 2 on a read
error. Change a label in one file and run it again to see the record
printed under the side that has it.

Next: [New to Rust](new-to-rust.md) or [New to PROV](new-to-prov.md),
or straight to [Chapter 1](../chapters/1-orientation.md).
