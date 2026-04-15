# Cookbook

Short, runnable recipes for tasks that don't fit into one of the conceptual
chapters.

## Round-trip a typed resource

```rust
use jsonapi_core::{
    Document, Identity, JsonApi, PrimaryData, Relationship, RelationshipData,
    ResourceIdentifier,
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
struct Person { #[jsonapi(id)] id: String, name: String }

let article = Article {
    id: "1".into(),
    title: "Hello".into(),
    author: Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
        type_: "people".into(),
        identity: Identity::Id("9".into()),
        meta: None,
    }))),
};

let doc: Document<Article> = Document::Data {
    data: PrimaryData::Single(Box::new(article)),
    included: vec![], meta: None, jsonapi: None, links: None,
};
let json = serde_json::to_string(&doc)?;
```

## Walk a heterogeneous `included`

```rust
use jsonapi_core::{Document, PrimaryData, Resource, ResourceObject};

let doc: Document<Resource> = serde_json::from_str(json)?;
if let Document::Data { included, .. } = &doc {
    for resource in included {
        match resource.resource_type() {
            "people"   => { /* handle person */ }
            "comments" => { /* handle comment */ }
            other      => eprintln!("ignoring {other}"),
        }
    }
}
```

## Look up a typed value by relationship

```rust
let registry = doc.registry()?;
let author: Person = registry.get(&article.author)?;
```

## Build a query string for an article fetch

```rust
use jsonapi_core::QueryBuilder;

let qs = QueryBuilder::new()
    .include(&["author", "comments.author"])
    .fields("articles", &["title", "body"])
    .fields("people", &["name"])
    .build();

let url = format!("https://api.example.com/articles/1?{qs}");
```

## Validate include paths against a registered graph

```rust
use jsonapi_core::TypeRegistry;

let mut registry = TypeRegistry::new();
registry.register::<Article>().register::<Person>().register::<Comment>();

registry.validate_include_paths("articles", &["author", "comments.author"])?;
```

## Serialize a typed resource through a sparse fieldset

```rust
use jsonapi_core::{FieldsetConfig, SparseSerializer};

let config = FieldsetConfig::new().fields("articles", &["title"]);
let json = serde_json::to_string(&SparseSerializer::new(&article, &config))?;
```

## Filter a raw document

```rust
use jsonapi_core::{FieldsetConfig, sparse_filter};

let config = FieldsetConfig::new()
    .fields("articles", &["title"])
    .fields("people", &["name"]);
let filtered = sparse_filter(&doc_value, &config);
```

## Negotiate `Accept` and validate `Content-Type`

```rust
use jsonapi_core::{negotiate_accept, validate_content_type};

validate_content_type(req_content_type)?;        // 415 on Err
let mt = negotiate_accept(req_accept, &[], &[])?; // 406 on Err
println!("Content-Type: {}", mt.to_header_value());
```

## Build an error response

```rust
use jsonapi_core::{ApiError, Document, ErrorSource, Resource};

let doc: Document<Resource> = Document::Errors {
    errors: vec![ApiError {
        status: Some("422".into()),
        title: Some("Validation failed".into()),
        source: Some(ErrorSource {
            pointer: Some("/data/attributes/title".into()),
            ..Default::default()
        }),
        ..Default::default()
    }],
    meta: None, jsonapi: None, links: None,
};
```

## Resolve an entire response into kitsu-core-style flat output

```rust
use jsonapi_core::{Document, ResolveConfig, Resource};

let doc: Document<Resource> = serde_json::from_str(json)?;
let registry = doc.registry()?;
let value = serde_json::to_value(&doc)?;
let flat = registry.resolve(&value["data"], &ResolveConfig::default());
```

## Issue an Atomic Operations request (feature `atomic-ops`)

```rust
use jsonapi_core::{
    JsonApiMediaType,
    atomic::{ATOMIC_EXT_URI, AtomicRequest},
};

let req = AtomicRequest::default(); // populate operations…
req.validate_lid_refs()?;            // pre-flight
let body = serde_json::to_string(&req)?;
let content_type = JsonApiMediaType::with_ext([ATOMIC_EXT_URI]).to_header_value();
```

## Run the workspace examples

The crate ships runnable examples for each major feature:

```sh
cargo run --example basic_serialize       -p jsonapi_core
cargo run --example basic_deserialize     -p jsonapi_core
cargo run --example dynamic_resource      -p jsonapi_core
cargo run --example query_builder         -p jsonapi_core
cargo run --example content_negotiation   -p jsonapi_core
cargo run --example atomic_operations     -p jsonapi_core --features atomic-ops
```
