//! Hot-path benchmarks for dynamic `Resource` (de)serialization. Run with
//! `cargo bench -p jsonapi_core`. Guards the Component 0 refactor against regressions.

use criterion::{Criterion, criterion_group, criterion_main};
use jsonapi_core::Document;
use jsonapi_core::Resource;
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

criterion_group!(benches, bench_resource_serde);
criterion_main!(benches);
