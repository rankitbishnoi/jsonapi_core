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

## Versioning policy

`jsonapi_core` follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

### Lockstep workspace versions

The `jsonapi_core` and `jsonapi_core_derive` crates are versioned in lockstep
via `workspace.package.version`. They are always released together. Pin only
`jsonapi_core` in your `Cargo.toml`; the derive crate is re-exported via the
`derive` feature.

### What is public API

The following are **public API** and changes to them are governed by SemVer:

- All items re-exported at the `jsonapi_core` crate root (`Document`,
  `PrimaryData`, `Resource`, `ResourceObject`, `ResourceIdentifier`,
  `Identity`, `Relationship`, `RelationshipData`, `Links`, `Link`,
  `LinkObject`, `Hreflang`, `Meta`, `JsonApiObject`, `ApiError`, `ErrorLinks`,
  `ErrorSource`, `Registry`, `ResolveConfig`, `TypeRegistry`, `TypeInfo`,
  `QueryBuilder`, `FieldsetConfig`, `SparseSerializer`, `sparse_filter`,
  `CaseConfig`, `CaseConvention`, `Error`, `Result`, `JsonApiMediaType`,
  `validate_content_type`, `negotiate_accept`, `validate_member_name`,
  `MemberNameKind`).
- All items re-exported under the `atomic-ops` feature (`AtomicRequest`,
  `AtomicResponse`, `AtomicResult`, `AtomicOperation`, `OperationTarget`,
  `OperationRef`, `ATOMIC_EXT_URI`).
- The `#[derive(JsonApi)]` attribute set: `type`, `case` on the struct;
  `id`, `lid`, `relationship`, `meta`, `links`, `rename`, `skip`, and
  relationship `type` on fields.
- Default behaviours documented in the crate-level rustdoc and the
  [guide](docs/SUMMARY.md): the fuzzy-deserialization alias set, the
  `Option::None` → omitted-on-serialize rule, the `null` → `None` deserialize
  fall-through, the registry's silent skip on shape mismatch, the resolver's
  cycle detection.
- The minimum supported Rust version (MSRV).

### What is *not* public API

- Anything inside a `pub(crate)` or private module path. Items not re-exported
  at the crate root may be relocated or removed in any release.
- The exact text of error `Display` messages (the `Error` *enum variants* are
  public; the formatted strings are not).
- The exact alias ordering inside the fuzzy-deserialization fall-through chain
  (the *set* of accepted aliases is public; ties resolve in implementation
  order).
- Internals of the `jsonapi_core_derive` proc-macro crate. Use the derive only
  through `jsonapi_core`'s `derive` feature; do not depend on
  `jsonapi_core_derive` directly.
- Unreleased items behind unstable feature flags (none currently exist; this
  applies to any future `unstable_*` flags).

### `#[non_exhaustive]` guarantees

All public enums (`PrimaryData`, `Document`, `RelationshipData`, `Identity`,
`Hreflang`, `Link`, `CaseConvention`, `MemberNameKind`, `Error`, …) carry
`#[non_exhaustive]`. New variants may be added in **minor** releases. Match
arms in consumer code must include a `_ =>` fall-through.

### MSRV policy

The minimum supported Rust version is currently **1.88**. MSRV bumps require
a minor-version release (≥ `0.x.0` while pre-1.0; ≥ `x.0.0` post-1.0) and
will be called out in the [changelog](./CHANGELOG.md).

### Pre-1.0 caveat

While at `0.x`, breaking changes follow the SemVer pre-1.0 convention: a bump
to `0.(x+1).0` may include breaking changes. We will continue to maintain a
detailed changelog so each upgrade has a clear migration path. A 1.0 release
is planned once the typed parse-error story is resolved (see the project's
improvement-tracking notes).

### Changelog

See [`CHANGELOG.md`](./CHANGELOG.md) for a release-by-release record.

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](http://opensource.org/licenses/MIT) at your option.
