//! Hot-path benchmarks for dynamic `Resource` (de)serialization. Run with
//! `cargo bench -p jsonapi_core`. Guards the Component 0 refactor against regressions.

use criterion::{Criterion, criterion_group, criterion_main};
use jsonapi_core::{Document, ResolveConfig, Resource};
use std::hint::black_box;

// A compound document whose relationships carry data + relationship-level links/meta,
// so the ResourceRelationship path is exercised.
const COMPOUND: &str = r#"{
  "data": [
    {
      "type": "articles", "id": "1",
      "attributes": {"title": "A", "body": "hello world"},
      "relationships": {
        "author": {
          "data": {"type": "people", "id": "9"},
          "links": {"related": "/articles/1/author"},
          "meta": {"count": 1}
        },
        "comments": {
          "data": [{"type": "comments", "id": "5"}, {"type": "comments", "id": "12"}]
        }
      }
    }
  ],
  "included": [
    {"type": "people", "id": "9", "attributes": {"name": "Dan"}},
    {"type": "comments", "id": "5", "attributes": {"body": "first"}},
    {"type": "comments", "id": "12", "attributes": {"body": "second"}}
  ]
}"#;

fn bench_resource_serde(c: &mut Criterion) {
    c.bench_function("resource_deserialize_compound", |b| {
        b.iter(|| {
            let doc: Document<Resource> = serde_json::from_str(black_box(COMPOUND)).unwrap();
            black_box(doc);
        });
    });

    let doc: Document<Resource> = serde_json::from_str(COMPOUND).unwrap();
    c.bench_function("resource_serialize_compound", |b| {
        b.iter(|| {
            let s = serde_json::to_string(black_box(&doc)).unwrap();
            black_box(s);
        });
    });
}

// Exercises the typed-parse pre-pass (build_included_set / check_relationship /
// check_included_ref), which plain `serde_json::from_str` does not touch.
fn bench_parse_prepass(c: &mut Criterion) {
    c.bench_function("document_parse_compound", |b| {
        b.iter(|| {
            let doc = Document::<Resource>::parse(black_box(COMPOUND)).unwrap();
            black_box(doc);
        });
    });
}

// Exercises Registry typed lookups (get_by_id) and the recursive resolver.
fn bench_registry(c: &mut Criterion) {
    let doc: Document<Resource> = serde_json::from_str(COMPOUND).unwrap();
    let registry = doc.registry().unwrap();

    c.bench_function("registry_get_by_id", |b| {
        b.iter(|| {
            let r: Resource = registry
                .get_by_id(black_box("people"), black_box("9"))
                .unwrap();
            black_box(r);
        });
    });

    let primary = serde_json::to_value(&doc).unwrap();
    let data = primary["data"][0].clone();
    c.bench_function("registry_resolve_compound", |b| {
        b.iter(|| {
            let flat = registry.resolve(black_box(&data), &ResolveConfig::default());
            black_box(flat);
        });
    });
}

criterion_group!(
    benches,
    bench_resource_serde,
    bench_parse_prepass,
    bench_registry
);
criterion_main!(benches);
