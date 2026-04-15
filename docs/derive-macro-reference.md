# Derive Macro Reference

This is the full reference for `#[derive(JsonApi)]`. The conceptual introduction
lives in [Defining Resources](./defining-resources.md); this chapter is the
flat lookup.

## Struct-level attributes

| Attribute | Required | Description |
|-----------|----------|-------------|
| `#[jsonapi(type = "...")]` | yes | The JSON:API type string (e.g. `"articles"`). Validated as a JSON:API member name at compile time. |
| `#[jsonapi(case = "...")]` | no | Output case convention. Values: `"camelCase"`, `"snake_case"`, `"kebab-case"`, `"PascalCase"`, `"none"` (default). |

## Field-level attributes

| Attribute | Description |
|-----------|-------------|
| `#[jsonapi(id)]` | Marks the resource ID field. **Required**, exactly one per struct. Type: `String` or `Option<String>`. |
| `#[jsonapi(lid)]` | Marks the local-identifier field (JSON:API 1.1). At most one. Type: `Option<String>`. |
| `#[jsonapi(relationship)]` | Field appears in `relationships`, not `attributes`. Type must be `Relationship<T>` or `Vec<Relationship<T>>`. |
| `#[jsonapi(relationship, type = "...")]` | Relationship plus an explicit target type for `TypeRegistry`. |
| `#[jsonapi(meta)]` | Maps to the resource-level `meta` member. At most one. Type: `Option<Meta>`. |
| `#[jsonapi(links)]` | Maps to the resource-level `links` member. At most one. Type: `Option<Links>`. |
| `#[jsonapi(rename = "...")]` | Override the wire name for this field. Validated as a JSON:API member name at compile time. |
| `#[jsonapi(skip)]` | Exclude from both serialization and deserialization. |

Unannotated fields are serialized as **attributes**.

## Generated code

For a derived struct, the macro produces:

1. **`impl ResourceObject for T`** with `resource_type`, `resource_id`,
   `resource_lid`, `field_names`, and `type_info`.
2. **`impl Serialize for T`** that writes the JSON:API envelope.
3. **`impl<'de> Deserialize<'de> for T`** with fuzzy member-name aliases.

The `type_info()` impl returns a `TypeInfo` populated with:

- `type_name` — the struct's `type` string.
- `field_names` — wire names of all attribute and relationship fields, in
  declaration order.
- `relationships` — `(field_name, target_type)` pairs for every relationship
  field that has `type = "..."` set.

## Output casing rules

When `case = "..."` is set:

- The struct's `type` string is **not** transformed — it's used verbatim.
- Field names (attributes and relationships) are converted using the chosen
  convention.
- A field-level `rename` overrides the conversion for that one field.
- The chosen convention's variant is the **canonical** alias for deserialization.
  The other case variants are still accepted for fuzzy parsing.

| Convention | `published_at` becomes | `firstName` becomes |
|------------|------------------------|---------------------|
| `camelCase` | `publishedAt` | `firstName` |
| `snake_case` | `published_at` | `first_name` |
| `kebab-case` | `published-at` | `first-name` |
| `PascalCase` | `PublishedAt` | `FirstName` |
| `none` (default) | `published_at` | `firstName` |

## Compile-time errors

The macro emits a compile error when:

- The `#[jsonapi(id)]` attribute is missing or applied to more than one field.
- `#[jsonapi(lid)]`, `#[jsonapi(meta)]`, or `#[jsonapi(links)]` is applied to
  more than one field.
- The struct-level `type = "..."` value violates JSON:API member-name rules.
- A field-level `rename = "..."` value violates JSON:API member-name rules.
- `type = "..."` is used on a non-relationship field.
- `case = "..."` is set to an unknown value.

Each of these has a `tests/compile_fail/` companion test in the crate.

## Hand-rolling `ResourceObject`

When you need behaviour the macro can't express, implement `ResourceObject`
yourself:

```rust
use jsonapi_core::model::ResourceObject;
use jsonapi_core::TypeInfo;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct My { /* ... */ }

impl ResourceObject for My {
    fn resource_type(&self) -> &str { "my-things" }
    fn resource_id(&self) -> Option<&str> { /* ... */ None }
    fn field_names() -> &'static [&'static str] { &["foo", "bar"] }
    fn type_info() -> TypeInfo {
        TypeInfo::new(
            "my-things",
            &["foo", "bar"],
            &[],
        )
    }
}
```

You're then responsible for producing the correct JSON:API envelope from your
`Serialize` / `Deserialize` impls. Look at `Resource`'s impl in
`jsonapi_core/src/model/resource.rs` for a worked example.
