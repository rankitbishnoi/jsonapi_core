# Sparse Fieldsets

JSON:API's [sparse fieldsets](https://jsonapi.org/format/#fetching-sparse-fieldsets)
let a client request only specific fields per resource type via
`?fields[type]=field1,field2`. `jsonapi_core` supports filtering at the
**server** side too — both for typed structs and for raw JSON values.

## `FieldsetConfig`

`FieldsetConfig` is the shared specification: a map from type name to the set
of allowed field names.

```rust
use jsonapi_core::FieldsetConfig;

let config = FieldsetConfig::new()
    .fields("articles", &["title", "body"])
    .fields("people", &["name"]);
```

Semantics:

- A type with **no** entry in the config passes through unfiltered.
- A type with an entry keeps only the listed fields. Both `attributes` and
  `relationships` are filtered against the same list.
- Empty `attributes` or `relationships` blocks are dropped from the output.

## Two filtering paths

| Path | Use when |
|------|----------|
| `SparseSerializer::new(&resource, &config)` | You have a typed `ResourceObject` (a derived struct or a `Resource`). |
| `sparse_filter(&value, &config)` | You have a raw `serde_json::Value` document (e.g. assembled by hand or fetched from elsewhere). |

### Typed path

`SparseSerializer` wraps a `ResourceObject` and serializes it through the
filter. Use it when you're producing a response from a typed value:

```rust
use jsonapi_core::{FieldsetConfig, SparseSerializer};

let config = FieldsetConfig::new().fields("articles", &["title"]);
let json = serde_json::to_value(SparseSerializer::new(&article, &config))?;

assert_eq!(json["attributes"]["title"], "Hello");
assert!(json["attributes"].get("body").is_none());
```

The `SparseSerializer` itself implements `Serialize`, so it composes with
anything that takes a `&impl Serialize` — including `serde_json::to_string`
and `serde_json::to_writer`.

### Dynamic path

`sparse_filter` takes a full `serde_json::Value` document and returns a filtered
clone. It walks `data` (single resource or array) and `included`, applying
`config` to each resource it finds:

```rust
use jsonapi_core::{FieldsetConfig, sparse_filter};

let doc: serde_json::Value = serde_json::json!({
    "data": {
        "type": "articles", "id": "1",
        "attributes": {"title": "Hello", "body": "World"},
        "relationships": {
            "author": {"data": {"type": "people", "id": "9"}}
        }
    },
    "included": [{
        "type": "people", "id": "9",
        "attributes": {"name": "Dan", "email": "dan@example.com"}
    }]
});

let config = FieldsetConfig::new()
    .fields("articles", &["title"])     // drop body, drop author
    .fields("people", &["name"]);       // drop email

let filtered = sparse_filter(&doc, &config);
assert!(filtered["data"]["attributes"].get("body").is_none());
assert!(filtered["data"].get("relationships").is_none());
assert!(filtered["included"][0]["attributes"].get("email").is_none());
```

## Translating from a query string

A typical web handler maps the request's query string into a `FieldsetConfig`.
You can do this manually from `?fields[articles]=title,body&fields[people]=name`:

```rust
use jsonapi_core::FieldsetConfig;
use std::collections::HashMap;

fn parse_fieldset(q: &HashMap<String, String>) -> FieldsetConfig {
    let mut config = FieldsetConfig::new();
    for (key, value) in q {
        if let Some(rest) = key.strip_prefix("fields[") {
            if let Some(type_) = rest.strip_suffix(']') {
                let fields: Vec<&str> = value.split(',').collect();
                config = config.fields(type_, &fields);
            }
        }
    }
    config
}
```

Pair this with the [Query Builder](./query-builder.md) on the client side, where
the same `?fields[type]=...` syntax is produced from `.fields(type, &[...])`.

## Behaviour notes

- Field names in the config that don't exist on the resource are silently
  ignored. There's no error path — over-specifying is allowed.
- The id and type fields are never filtered. Sparse fieldsets affect attributes
  and relationships only.
- The typed path is single-resource; for filtering an entire `Document<T>`,
  serialize first and then run `sparse_filter` over the resulting `Value`.
