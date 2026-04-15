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
- **Atomic Operations extension** — feature-gated `atomic` module implementing the JSON:API v1.1 Atomic Operations extension (add/update/remove, lid cross-refs)

## Install

```sh
cargo add jsonapi_core
```

Requires Rust **1.88+** and the **2024 edition**.

## Quick Example

```rust
use jsonapi_core::{Document, JsonApi, Relationship, Resource};

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
let registry = doc.registry().unwrap();

// Typed lookup — deserializes the stored Value into a Person
let author: Person = registry.get_by_id("people", "9").unwrap();
assert_eq!(author.name, "Dan Gebhardt");
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `derive` | yes | Re-exports `#[derive(JsonApi)]` from `jsonapi_core_derive` |
| `atomic-ops` | off | Atomic Operations extension types (`atomic` module) |

See [`docs/feature-flags.md`](docs/feature-flags.md) for details.

## Examples

Runnable examples ship with the crate:

```sh
cargo run --example basic_serialize       -p jsonapi_core
cargo run --example basic_deserialize     -p jsonapi_core
cargo run --example dynamic_resource      -p jsonapi_core
cargo run --example query_builder         -p jsonapi_core
cargo run --example content_negotiation   -p jsonapi_core
cargo run --example atomic_operations     -p jsonapi_core --features atomic-ops
```

## Documentation

- **[The jsonapi_core Guide](docs/SUMMARY.md)** — chapter-by-chapter walkthrough
  covering documents, resources, relationships, the registry, the query builder,
  sparse fieldsets, content negotiation, atomic operations, and a cookbook of
  common recipes.
- **[API docs on docs.rs](https://docs.rs/jsonapi_core)** — type-level reference
  for every public item.

The guide is laid out as an [mdbook](https://rust-lang.github.io/mdBook/) under
`docs/`. To build it locally:

```sh
cargo install mdbook
mdbook serve docs
```

Or browse the markdown directly starting at [`docs/introduction.md`](docs/introduction.md).

## Repository layout

| Path | Contents |
|------|----------|
| `jsonapi_core/` | The library crate. |
| `jsonapi_core_derive/` | The proc-macro crate (re-exported via the `derive` feature). |
| `acceptance/` | Spec-conformance integration tests. |
| `docs/` | The guide book (this is what you're reading). |

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](http://opensource.org/licenses/MIT) at your option.
