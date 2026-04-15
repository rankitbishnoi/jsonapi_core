#![warn(missing_docs)]
//! Derive macro implementation for [`jsonapi_core`](https://docs.rs/jsonapi_core).
//!
//! This crate provides the `#[derive(JsonApi)]` procedural macro that generates
//! [`ResourceObject`], [`serde::Serialize`], and [`serde::Deserialize`] impls
//! for user-defined JSON:API resource types. It is re-exported by `jsonapi_core`
//! under the `derive` feature (on by default) and is not intended to be used
//! directly.

mod codegen;
mod parse;
mod validate;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derive macro for JSON:API resource types.
///
/// Generates implementations of `ResourceObject`, `Serialize`, and `Deserialize` that
/// produce and consume the JSON:API resource envelope format.
///
/// # Struct-Level Attributes
///
/// | Attribute | Required | Description |
/// |-----------|----------|-------------|
/// | `#[jsonapi(type = "...")]` | yes | The JSON:API type string (e.g. `"articles"`) |
/// | `#[jsonapi(case = "...")]` | no | Output case convention: `"camelCase"`, `"snake_case"`, `"kebab-case"`, `"PascalCase"`, `"none"` |
///
/// # Field-Level Attributes
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `#[jsonapi(id)]` | Marks the resource ID field. Required, exactly one per struct. Type: `String` or `Option<String>`. |
/// | `#[jsonapi(lid)]` | Marks the local identifier field (JSON:API 1.1). At most one. Type: `Option<String>`. |
/// | `#[jsonapi(relationship)]` | Field appears in `relationships`, not `attributes`. Must be `Relationship<T>` or `Vec<Relationship<T>>`. |
/// | `#[jsonapi(relationship, type = "...")]` | Relationship with explicit target type for `TypeInfo`. |
/// | `#[jsonapi(meta)]` | Maps to resource-level `meta`. At most one. Type: `Option<Meta>`. |
/// | `#[jsonapi(links)]` | Maps to resource-level `links`. At most one. Type: `Option<Links>`. |
/// | `#[jsonapi(rename = "...")]` | Override the wire name for this field. |
/// | `#[jsonapi(skip)]` | Exclude from serialization and deserialization. |
///
/// Unannotated fields are serialized as attributes.
///
/// # Fuzzy Deserialization
///
/// The generated `Deserialize` impl accepts all common case variants of each field
/// name (camelCase, snake_case, kebab-case, PascalCase). This handles servers with
/// inconsistent casing. Output casing is controlled by `#[jsonapi(case = "...")]`.
///
/// # Compile-Time Validation
///
/// The macro rejects invalid usage at compile time:
/// - Missing `#[jsonapi(id)]` field
/// - Duplicate `id`, `lid`, `meta`, or `links` annotations
/// - Invalid type string or rename value (per JSON:API member-name rules)
/// - `type = "..."` on non-relationship fields
///
/// # Example
///
/// ```ignore
/// use jsonapi_core::{JsonApi, Relationship, Meta, Links};
///
/// #[derive(Debug, Clone, PartialEq, JsonApi)]
/// #[jsonapi(type = "articles", case = "camelCase")]
/// struct Article {
///     #[jsonapi(id)]
///     id: String,
///     title: String,
///     word_count: u32,
///     #[jsonapi(relationship, type = "people")]
///     author: Relationship<Person>,
///     #[jsonapi(meta)]
///     extra: Option<Meta>,
///     #[jsonapi(links)]
///     resource_links: Option<Links>,
///     #[jsonapi(skip)]
///     cached: Option<String>,
/// }
/// ```
///
/// This generates `impl ResourceObject for Article` providing `resource_type()` → `"articles"`,
/// a `Serialize` impl producing the JSON:API envelope with camelCase member names
/// (`wordCount`), and a `Deserialize` impl accepting any common casing.
#[proc_macro_derive(JsonApi, attributes(jsonapi))]
pub fn derive_json_api(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match parse::parse(&input) {
        Ok((struct_attrs, fields)) => {
            codegen::generate(&input.ident, &struct_attrs, &fields).into()
        }
        Err(err) => err.to_compile_error().into(),
    }
}
