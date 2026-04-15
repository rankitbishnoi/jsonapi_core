#![warn(missing_docs)]
//! # jsonapi_core
//!
//! A typed [JSON:API v1.1](https://jsonapi.org/format/) serialization library for Rust.
//!
//! `jsonapi_core` gives you a complete type model for JSON:API documents — resources,
//! relationships, links, errors — with a derive macro that handles the envelope format
//! so you work with plain Rust structs. It also provides a query builder, content
//! negotiation, sparse fieldset filtering, and a registry for resolving `included`
//! resources.
//!
//! # Defining a Resource
//!
//! Use `#[derive(JsonApi)]` to map a Rust struct to a JSON:API resource. Fields become
//! attributes by default. Annotate relationships, meta, and links explicitly.
//!
//! ```
//! use jsonapi_core::{JsonApi, Relationship};
//!
//! #[derive(Debug, Clone, PartialEq, JsonApi)]
//! #[jsonapi(type = "articles")]
//! struct Article {
//!     #[jsonapi(id)]
//!     id: String,
//!     title: String,
//!     body: String,
//!     #[jsonapi(relationship, type = "people")]
//!     author: Relationship<Person>,
//! }
//!
//! #[derive(Debug, Clone, PartialEq, JsonApi)]
//! #[jsonapi(type = "people")]
//! struct Person {
//!     #[jsonapi(id)]
//!     id: String,
//!     name: String,
//! }
//! ```
//!
//! # Serializing
//!
//! Wrap a resource in a [`Document`] and serialize with serde. The derive macro produces
//! the JSON:API envelope (`type`, `id`, `attributes`, `relationships`).
//!
//! ```
//! # use jsonapi_core::{JsonApi, Relationship, Document, PrimaryData, Identity,
//! #     RelationshipData, ResourceIdentifier, ResourceObject};
//! # #[derive(Debug, Clone, PartialEq, JsonApi)]
//! # #[jsonapi(type = "articles")]
//! # struct Article {
//! #     #[jsonapi(id)]
//! #     id: String,
//! #     title: String,
//! #     body: String,
//! #     #[jsonapi(relationship, type = "people")]
//! #     author: Relationship<Person>,
//! # }
//! # #[derive(Debug, Clone, PartialEq, JsonApi)]
//! # #[jsonapi(type = "people")]
//! # struct Person {
//! #     #[jsonapi(id)]
//! #     id: String,
//! #     name: String,
//! # }
//! let article = Article {
//!     id: "1".into(),
//!     title: "JSON:API paints my bikeshed!".into(),
//!     body: "The shortest article. Ever.".into(),
//!     author: Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
//!         type_: "people".into(),
//!         identity: Identity::Id("9".into()),
//!         meta: None,
//!     }))),
//! };
//!
//! let doc: Document<Article> = Document::Data {
//!     data: PrimaryData::Single(Box::new(article)),
//!     included: vec![],
//!     meta: None,
//!     jsonapi: None,
//!     links: None,
//! };
//!
//! let json = serde_json::to_string_pretty(&doc).unwrap();
//! assert!(json.contains("\"type\": \"articles\""));
//! ```
//!
//! # Deserializing and the Registry
//!
//! Parse a JSON:API response and use the [`Registry`] to look up included resources.
//! Deserialize with [`Document<Resource>`] to handle mixed types in `included`, then
//! use typed lookups to get concrete structs back.
//!
//! ```
//! use jsonapi_core::{JsonApi, Document, PrimaryData, Resource, ResourceObject};
//!
//! #[derive(Debug, Clone, PartialEq, JsonApi)]
//! #[jsonapi(type = "people")]
//! struct Person {
//!     #[jsonapi(id)]
//!     id: String,
//!     name: String,
//! }
//!
//! let json = r#"{
//!     "data": {
//!         "type": "articles", "id": "1",
//!         "attributes": {"title": "Hello JSON:API"},
//!         "relationships": {
//!             "author": {"data": {"type": "people", "id": "9"}}
//!         }
//!     },
//!     "included": [{
//!         "type": "people", "id": "9",
//!         "attributes": {"name": "Dan Gebhardt"}
//!     }]
//! }"#;
//!
//! let doc: Document<Resource> = serde_json::from_str(json).unwrap();
//! let registry = doc.registry().unwrap();
//!
//! // Typed lookup — deserializes the stored Value into a Person
//! let author: Person = registry.get_by_id("people", "9").unwrap();
//! assert_eq!(author.name, "Dan Gebhardt");
//! ```
//!
//! # Dynamic Resources
//!
//! When you don't know the schema at compile time, use [`Resource`] as an open-set
//! fallback. It stores attributes as `serde_json::Value` and relationships as a
//! `HashMap`.
//!
//! ```
//! use jsonapi_core::{Document, PrimaryData, Resource, ResourceObject};
//!
//! let json = r#"{"data": {"type": "widgets", "id": "42", "attributes": {"color": "red"}}}"#;
//! let doc: Document<Resource> = serde_json::from_str(json).unwrap();
//!
//! if let Document::Data { data: PrimaryData::Single(widget), .. } = &doc {
//!     assert_eq!(widget.resource_type(), "widgets");
//!     assert_eq!(widget.attributes["color"], "red");
//! }
//! ```
//!
//! # Recursive Resolver
//!
//! The [`Registry::resolve()`] method produces kitsu-core-style flattened output:
//! attributes are hoisted onto the resource, relationships are resolved and inlined
//! recursively, and the JSON:API envelope is stripped.
//!
//! ```
//! use jsonapi_core::{Document, Registry, ResolveConfig, Resource};
//!
//! let json = r#"{
//!     "data": {
//!         "type": "articles", "id": "1",
//!         "attributes": {"title": "Hello"},
//!         "relationships": {
//!             "author": {"data": {"type": "people", "id": "9"}}
//!         }
//!     },
//!     "included": [{
//!         "type": "people", "id": "9",
//!         "attributes": {"name": "Dan"}
//!     }]
//! }"#;
//!
//! let doc: Document<Resource> = serde_json::from_str(json).unwrap();
//! let registry = doc.registry().unwrap();
//! let value: serde_json::Value = serde_json::to_value(&doc).unwrap();
//! let data = &value["data"];
//!
//! let flat = registry.resolve(data, &ResolveConfig::default());
//! assert_eq!(flat["title"], "Hello");
//! assert_eq!(flat["author"]["name"], "Dan");
//! ```
//!
//! # Sparse Fieldsets
//!
//! Use [`FieldsetConfig`] to filter which fields appear in serialized output.
//! Two paths are available: [`SparseSerializer`] wraps a typed resource, and
//! [`sparse_filter()`] operates on a raw `serde_json::Value` document.
//!
//! ```
//! # use jsonapi_core::{JsonApi, Relationship, Identity, RelationshipData,
//! #     ResourceIdentifier, FieldsetConfig, SparseSerializer, ResourceObject};
//! # #[derive(Debug, Clone, PartialEq, JsonApi)]
//! # #[jsonapi(type = "articles")]
//! # struct Article {
//! #     #[jsonapi(id)]
//! #     id: String,
//! #     title: String,
//! #     body: String,
//! #     #[jsonapi(relationship, type = "people")]
//! #     author: Relationship<Person>,
//! # }
//! # #[derive(Debug, Clone, PartialEq, JsonApi)]
//! # #[jsonapi(type = "people")]
//! # struct Person {
//! #     #[jsonapi(id)]
//! #     id: String,
//! #     name: String,
//! # }
//! # let article = Article {
//! #     id: "1".into(), title: "Hello".into(), body: "World".into(),
//! #     author: Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
//! #         type_: "people".into(), identity: Identity::Id("9".into()), meta: None,
//! #     }))),
//! # };
//! // Only include the "title" field for articles
//! let config = FieldsetConfig::new().fields("articles", &["title"]);
//! let json = serde_json::to_value(SparseSerializer::new(&article, &config)).unwrap();
//!
//! assert_eq!(json["attributes"]["title"], "Hello");
//! assert!(json["attributes"].get("body").is_none());
//! ```
//!
//! # Include Path Validation
//!
//! [`TypeRegistry`] stores static type metadata and validates that include paths
//! are traversable through the relationship graph.
//!
//! ```
//! # use jsonapi_core::{JsonApi, Relationship, TypeRegistry, ResourceObject};
//! # #[derive(Debug, Clone, PartialEq, JsonApi)]
//! # #[jsonapi(type = "articles")]
//! # struct Article {
//! #     #[jsonapi(id)]
//! #     id: String,
//! #     title: String,
//! #     #[jsonapi(relationship, type = "people")]
//! #     author: Relationship<Person>,
//! # }
//! # #[derive(Debug, Clone, PartialEq, JsonApi)]
//! # #[jsonapi(type = "people")]
//! # struct Person {
//! #     #[jsonapi(id)]
//! #     id: String,
//! #     name: String,
//! # }
//! let mut registry = TypeRegistry::new();
//! registry.register::<Article>();
//!
//! // "author" is a valid include path from articles
//! assert!(registry.validate_include_paths("articles", &["author"]).is_ok());
//!
//! // "editor" is not a relationship on articles
//! assert!(registry.validate_include_paths("articles", &["editor"]).is_err());
//! ```
//!
//! # Query Builder
//!
//! [`QueryBuilder`] produces JSON:API-compliant query strings with correct bracket
//! encoding and RFC 3986 percent-encoding.
//!
//! ```
//! use jsonapi_core::QueryBuilder;
//!
//! let qs = QueryBuilder::new()
//!     .include(&["author", "comments"])
//!     .fields("articles", &["title", "body"])
//!     .filter("published", "true")
//!     .sort(&["-created", "title"])
//!     .page("number", "1")
//!     .build();
//!
//! assert!(qs.contains("include=author,comments"));
//! assert!(qs.contains("fields[articles]=title,body"));
//! assert!(qs.contains("filter[published]=true"));
//! ```
//!
//! # Case Conversion
//!
//! The derive macro generates fuzzy deserialization aliases for all common case
//! variants of each field name. Output casing is controlled by [`CaseConvention`]
//! via the `#[jsonapi(case = "...")]` attribute.
//!
//! ```
//! use jsonapi_core::{CaseConvention, CaseConfig};
//!
//! let config = CaseConfig { member_case: CaseConvention::CamelCase };
//! assert_eq!(config.member_case.convert("first_name"), "firstName");
//! assert_eq!(CaseConvention::KebabCase.convert("firstName"), "first-name");
//! assert_eq!(CaseConvention::None.convert("first_name"), "first_name");
//! ```
//!
//! # Content Negotiation
//!
//! Validate incoming `Content-Type` headers and negotiate `Accept` headers per the
//! JSON:API 1.1 protocol.
//!
//! ```
//! use jsonapi_core::{validate_content_type, negotiate_accept, JsonApiMediaType};
//!
//! // Validate a Content-Type header (rejects unknown parameters)
//! let mt = validate_content_type("application/vnd.api+json").unwrap();
//! assert!(mt.ext.is_empty());
//!
//! // Negotiate an Accept header (returns server capabilities)
//! let response = negotiate_accept(
//!     "application/vnd.api+json, application/json",
//!     &[],  // server extensions
//!     &[],  // server profiles
//! ).unwrap();
//! assert_eq!(response.to_header_value(), "application/vnd.api+json");
//! ```
//!
//! # Atomic Operations
//!
//! The [`atomic`] module (feature `atomic-ops`) implements the JSON:API
//! [Atomic Operations extension](https://jsonapi.org/ext/atomic/) for bundling
//! add/update/remove operations into a single request.
//!
//! ```
//! # #[cfg(feature = "atomic-ops")] {
//! use std::collections::BTreeMap;
//! use jsonapi_core::{
//!     atomic::{AtomicOperation, AtomicRequest, OperationTarget, ATOMIC_EXT_URI},
//!     PrimaryData, Resource,
//! };
//!
//! let req = AtomicRequest {
//!     operations: vec![AtomicOperation::Add {
//!         target: OperationTarget::default(),
//!         data: PrimaryData::Single(Box::new(Resource {
//!             type_: "articles".into(),
//!             id: None,
//!             lid: Some("a1".into()),
//!             attributes: serde_json::json!({"title": "Hello"}),
//!             relationships: BTreeMap::new(),
//!             links: None,
//!             meta: None,
//!         })),
//!     }],
//! };
//!
//! assert_eq!(ATOMIC_EXT_URI, "https://jsonapi.org/ext/atomic");
//! let json = serde_json::to_string(&req).unwrap();
//! assert!(json.contains("\"op\":\"add\""));
//! req.validate_lid_refs().unwrap();
//! # }
//! ```
//!
//! # Member Name Validation
//!
//! Validate member names at runtime per JSON:API 1.1 rules. The derive macro also
//! performs compile-time validation of type strings and `#[jsonapi(rename)]` values.
//!
//! ```
//! use jsonapi_core::{validate_member_name, MemberNameKind};
//!
//! // Standard member name
//! assert!(matches!(validate_member_name("first-name"), Ok(MemberNameKind::Standard)));
//!
//! // @-member (extension namespaced)
//! match validate_member_name("@ext:comments").unwrap() {
//!     MemberNameKind::AtMember { namespace, member } => {
//!         assert_eq!(namespace, "ext");
//!         assert_eq!(member, "comments");
//!     }
//!     _ => unreachable!(),
//! }
//!
//! // Invalid: empty string
//! assert!(validate_member_name("").is_err());
//! ```
//!
//! # Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `derive` | yes | Re-exports `#[derive(JsonApi)]` from `jsonapi_core_derive` |
//! | `atomic-ops` | no | Atomic Operations extension types (`atomic` module) |

pub mod case;
pub mod error;
pub mod fieldset;
pub mod media_type;
pub mod model;
pub mod query;
pub mod registry;
pub mod type_registry;
pub mod validation;

#[cfg(feature = "atomic-ops")]
pub mod atomic;

pub use case::{CaseConfig, CaseConvention};
pub use error::{Error, Result};
pub use fieldset::{FieldsetConfig, SparseSerializer, sparse_filter};
pub use media_type::{JsonApiMediaType, negotiate_accept, validate_content_type};
pub use model::{
    ApiError, Document, ErrorLinks, ErrorSource, Hreflang, Identity, JsonApiObject, Link,
    LinkObject, Links, Meta, PrimaryData, Relationship, RelationshipData, Resource,
    ResourceIdentifier, ResourceObject,
};
pub use query::QueryBuilder;
pub use registry::{Registry, ResolveConfig};
pub use type_registry::{TypeInfo, TypeRegistry};
pub use validation::{MemberNameKind, validate_member_name};

#[cfg(feature = "atomic-ops")]
pub use atomic::{
    ATOMIC_EXT_URI, AtomicOperation, AtomicRequest, AtomicResponse, AtomicResult, OperationRef,
    OperationTarget,
};

#[cfg(feature = "derive")]
pub use jsonapi_core_derive::JsonApi;
