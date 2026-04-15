# jsonapi_core

A typed [JSON:API v1.1](https://jsonapi.org/format/) serialization library for Rust.

## Features

- **Full type model** — `Document`, `Resource`, `Relationship`, `Link`, `ApiError`, and all JSON:API 1.1 types with custom serde implementations
- **Derive macro** — `#[derive(JsonApi)]` maps Rust structs to JSON:API resource envelopes
- **Fuzzy deserialization** — accepts camelCase, snake_case, kebab-case, and PascalCase variants of field names
- **Registry** — typed lookups from `included` arrays via `Relationship<T>` references
- **Recursive resolver** — kitsu-core-style flattened output with cycle detection
- **Query builder** — JSON:API-aware query strings with bracket encoding and RFC 3986 percent-encoding
- **Content negotiation** — `ext`/`profile` media-type parsing, `Content-Type` validation, `Accept` negotiation
- **Sparse fieldsets** — typed and dynamic filtering paths
- **Include path validation** — relationship graph walking with static type metadata
- **Atomic Operations extension** — feature-gated `atomic` module implementing the JSON:API v1.1 Atomic Operations extension (add/update/remove, lid cross-refs).

## Install

```sh
cargo add jsonapi_core
```

## Quick Example

```rust
use jsonapi_core::{
    Document, JsonApi, PrimaryData, Relationship, Resource, ResourceObject,
};

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
}

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
}

// Deserialize a JSON:API response
let json = r#"{
    "data": {
        "type": "articles", "id": "1",
        "attributes": {"title": "Hello JSON:API"},
        "relationships": {
            "author": {"data": {"type": "people", "id": "9"}}
        }
    },
    "included": [{
        "type": "people", "id": "9",
        "attributes": {"name": "Dan Gebhardt"}
    }]
}"#;

// Use Document<Resource> to handle mixed types in `included`
let doc: Document<Resource> = serde_json::from_str(json).unwrap();
let registry = doc.registry();

// Typed lookup — deserializes the stored Value into a Person
let author: Person = registry.get_by_id("people", "9").unwrap();
assert_eq!(author.name, "Dan Gebhardt");
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `derive` | yes | Re-exports `#[derive(JsonApi)]` from `jsonapi_core_derive` |
| `atomic-ops` | off | Atomic Operations extension types (`atomic` module). |

## Documentation

See the [API docs on docs.rs](https://docs.rs/jsonapi_core) for a full tutorial and API reference.

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](http://opensource.org/licenses/MIT) at your option.
