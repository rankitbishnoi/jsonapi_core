# Atomic Operations

The [JSON:API Atomic Operations extension](https://jsonapi.org/ext/atomic/)
lets a client batch multiple `add` / `update` / `remove` operations into a
single request, executed transactionally on the server. `jsonapi_core`
supports it behind the `atomic-ops` feature.

## Enabling the feature

```toml
[dependencies]
jsonapi_core = { version = "0.1", features = ["atomic-ops"] }
```

This unlocks the `jsonapi_core::atomic` module.

## The Content-Type

An Atomic Operations request **must** declare the extension URI in its
`Content-Type`:

```text
Content-Type: application/vnd.api+json; ext="https://jsonapi.org/ext/atomic"
```

`jsonapi_core` provides a constant and a constructor:

```rust
use jsonapi_core::{JsonApiMediaType, atomic::ATOMIC_EXT_URI};

let mt = JsonApiMediaType::with_ext([ATOMIC_EXT_URI]);
let header_value = mt.to_header_value();
// → application/vnd.api+json; ext="https://jsonapi.org/ext/atomic"
```

## The request envelope

```rust
pub struct AtomicRequest {
    pub operations: Vec<AtomicOperation>,
}
```

Wire form:

```json
{ "atomic:operations": [ /* ... */ ] }
```

The order of operations is significant — later ops can reference `lid` values
introduced by earlier `add` ops.

## The `AtomicOperation` enum

```rust
pub enum AtomicOperation {
    Add    { target: OperationTarget, data: PrimaryData<Resource> },
    Update { target: OperationTarget, data: PrimaryData<Resource> },
    Remove { target: OperationTarget },
}
```

Tagged on the wire by `op` (`"add"`, `"update"`, `"remove"`).

`OperationTarget` describes *where* the operation applies:

```rust
pub struct OperationTarget {
    pub r#ref: Option<OperationRef>,
    pub href: Option<String>,
}
```

| `ref` | `href` | Meaning |
|-------|--------|---------|
| `Some` | `None` | Structured pointer (`type` + `id`/`lid` + optional relationship) |
| `None` | `Some` | URL pointer |
| `None` | `None` | No target — typically `add` of a top-level resource |

Having both set is a spec violation. Use `OperationTarget::is_valid()` to check
for that case, or call `AtomicRequest::validate_lid_refs` (below) which catches
it along with `lid` errors.

## Building a request

```rust
use std::collections::BTreeMap;
use jsonapi_core::{
    Identity, PrimaryData, RelationshipData, Resource, ResourceIdentifier,
    atomic::{AtomicOperation, AtomicRequest, OperationRef, OperationTarget},
};

// 1. Add a person with lid "p1".
let person = Resource {
    type_: "people".into(),
    id: None,
    lid: Some("p1".into()),
    attributes: serde_json::json!({"name": "Dan Gebhardt"}),
    relationships: BTreeMap::new(),
    links: None,
    meta: None,
};

// 2. Add an article with lid "a1", whose author is the lid-referenced person.
let mut article_rels = BTreeMap::new();
article_rels.insert(
    "author".into(),
    RelationshipData::ToOne(Some(ResourceIdentifier {
        type_: "people".into(),
        identity: Identity::Lid("p1".into()),
        meta: None,
    })),
);
let article = Resource {
    type_: "articles".into(),
    id: None,
    lid: Some("a1".into()),
    attributes: serde_json::json!({"title": "Hello JSON:API"}),
    relationships: article_rels,
    links: None,
    meta: None,
};

// 3. Update the article's author relationship via lid reference.
let req = AtomicRequest {
    operations: vec![
        AtomicOperation::Add {
            target: OperationTarget::default(),
            data: PrimaryData::Single(Box::new(person)),
        },
        AtomicOperation::Add {
            target: OperationTarget::default(),
            data: PrimaryData::Single(Box::new(article)),
        },
        AtomicOperation::Update {
            target: OperationTarget {
                r#ref: Some(OperationRef {
                    type_: "articles".into(),
                    identity: Identity::Lid("a1".into()),
                    relationship: Some("author".into()),
                }),
                href: None,
            },
            data: PrimaryData::Single(Box::new(Resource {
                type_: "people".into(),
                id: None,
                lid: Some("p1".into()),
                attributes: serde_json::json!({}),
                relationships: BTreeMap::new(),
                links: None,
                meta: None,
            })),
        },
    ],
};
```

## Pre-flight validation

Before sending — or after receiving — call `validate_lid_refs`:

```rust
req.validate_lid_refs()?;
```

This single check catches:

- Any `ref.lid` that wasn't introduced by a strictly earlier `add` (forward
  reference).
- Any `lid` that's introduced **twice** in the same request.
- Any `OperationTarget` that has both `ref` and `href` set.

Failures return `Error::InvalidAtomicOperation { index, reason }`, where
`index` is the zero-based position of the offending operation.

Note: serialization and deserialization of `AtomicRequest` remain infallible
with respect to `lid` semantics. The wire format is permissive; validation
is opt-in and explicit.

## Serializing

```rust
let body = serde_json::to_string_pretty(&req)?;
```

The output uses the `atomic:operations` key (per spec):

```json
{
  "atomic:operations": [
    { "op": "add", "data": { "type": "people", "lid": "p1", ... } },
    { "op": "add", "data": { "type": "articles", "lid": "a1", ... } },
    { "op": "update",
      "ref": { "type": "articles", "lid": "a1", "relationship": "author" },
      "data": { "type": "people", "lid": "p1", "attributes": {} } }
  ]
}
```

## The response envelope

```rust
pub struct AtomicResponse {
    pub results: Vec<AtomicResult>,    // serialized as "atomic:results"
    pub jsonapi: Option<JsonApiObject>,
    pub meta: Option<Meta>,
    pub links: Option<Links>,
}

pub struct AtomicResult {
    pub data: Option<PrimaryData<Resource>>,
    pub meta: Option<Meta>,
    pub links: Option<Links>,
}
```

`results` aligns 1:1 with the request's `operations`. A `remove` typically
produces an empty `AtomicResult` (`{}`).

```rust
let resp: AtomicResponse = serde_json::from_str(&body)?;
assert_eq!(resp.results.len(), req.operations.len());
```

## Putting it together (request flow)

1. Build the `AtomicRequest`.
2. Call `validate_lid_refs()` to catch self-inflicted errors.
3. Serialize to JSON.
4. Set `Content-Type: application/vnd.api+json; ext="https://jsonapi.org/ext/atomic"`.
5. POST the body.
6. Parse the response into `AtomicResponse`.

The `examples/atomic_operations.rs` file in the workspace runs this exact flow:

```sh
cargo run --example atomic_operations --features atomic-ops
```
