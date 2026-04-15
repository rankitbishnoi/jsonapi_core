# Defining Resources with the Derive Macro

`#[derive(JsonApi)]` is the bridge between idiomatic Rust structs and the JSON:API
envelope format. It generates `ResourceObject`, `Serialize`, and `Deserialize`
implementations from a small attribute language.

This chapter introduces the macro through worked examples. For the full grammar,
see the [Derive Macro Reference](./derive-macro-reference.md).

## A minimal resource

```rust
use jsonapi_core::JsonApi;

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
    email: String,
}
```

What the macro does:

- Adds `impl ResourceObject for Person` returning `"people"` from
  `resource_type()` and `Some(&id)` from `resource_id()`.
- Generates `Serialize` that produces:
  ```json
  { "type": "people", "id": "1",
    "attributes": { "name": "...", "email": "..." } }
  ```
- Generates `Deserialize` that accepts that shape — and, importantly, accepts
  fuzzy casing for member names (see below).

Unannotated fields are placed in `attributes`. Exactly one field must be marked
`#[jsonapi(id)]`; its type must be `String` or `Option<String>`.

## Fuzzy deserialization

The generated `Deserialize` impl accepts every common case variant of each field:

- `firstName` (camelCase)
- `first_name` (snake\_case)
- `first-name` (kebab-case)
- `FirstName` (PascalCase)

So a server that emits `first-name` and one that emits `firstName` both round-trip
into the same Rust field. This is independent of your **output** casing — that's
controlled separately by `#[jsonapi(case = "...")]`.

## Output casing

```rust
#[derive(JsonApi)]
#[jsonapi(type = "people", case = "camelCase")]
struct Person {
    #[jsonapi(id)]
    id: String,
    first_name: String,   // serializes as "firstName"
    word_count: u32,      // serializes as "wordCount"
}
```

Supported values: `"camelCase"`, `"snake_case"`, `"kebab-case"`, `"PascalCase"`,
`"none"` (default — pass-through). The same setting drives both the generated
`Serialize` impl and the canonical alias used by `Deserialize`. The fuzzy aliases
for the other casings stay enabled either way.

## Relationships

Mark a field with `#[jsonapi(relationship)]` to put it in the `relationships`
member instead of `attributes`. The field's type must be `Relationship<T>` (to-one)
or `Vec<Relationship<T>>` (to-many).

```rust
use jsonapi_core::{JsonApi, Relationship};

#[derive(JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
    #[jsonapi(relationship, type = "comments")]
    comments: Vec<Relationship<Comment>>,
}
```

The `type = "people"` attribute on the relationship feeds the [`TypeRegistry`] so
include-path validation can walk the relationship graph. See
[Include Path Validation](./include-validation.md) for how to use it.

## Resource-level `meta` and `links`

Two optional struct-level members are exposed via dedicated attributes:

```rust
use jsonapi_core::{JsonApi, Links, Meta};

#[derive(JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(meta)]
    extra: Option<Meta>,
    #[jsonapi(links)]
    resource_links: Option<Links>,
}
```

At most one field per struct may be tagged `#[jsonapi(meta)]` or
`#[jsonapi(links)]`. The wire location is the resource object's `meta` /
`links` member, not the document-level one.

## Local identifiers (`lid`)

JSON:API 1.1 introduces `lid` for client-generated identifiers used during
operations like create-with-relationships. Mark an `Option<String>` field
with `#[jsonapi(lid)]`:

```rust
#[derive(JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: Option<String>,
    #[jsonapi(lid)]
    lid: Option<String>,
    title: String,
}
```

When `id` is omitted but `lid` is present, the resource serializes with `lid`
only — the convention for "I haven't been persisted yet, but I have a stable
local handle other operations can refer to."

## Renaming and skipping

```rust
#[derive(JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(rename = "headline")]
    title: String,
    #[jsonapi(skip)]
    cached_at: Option<std::time::Instant>,
}
```

- `rename` overrides the wire name for that field.
- `skip` excludes the field from both serialization and deserialization.

## Compile-time checks

The macro rejects invalid configurations at compile time:

- Missing `#[jsonapi(id)]` field
- Duplicate `id`, `lid`, `meta`, or `links` annotations
- `type = "..."` on a field that isn't a relationship
- A struct-level `type` string that violates JSON:API member-name rules
- A `rename` value that violates JSON:API member-name rules

A failing `try_compile` test in the crate's `tests/compile_fail/` directory
documents each of these.

## Cross-references

- For how relationship cardinality maps to the `Relationship<T>` /
  `Vec<Relationship<T>>` distinction, see
  [Relationships and Identifiers](./relationships.md).
- For typed lookups against the deserialized graph, see
  [The Registry and Resolver](./registry-and-resolver.md).
