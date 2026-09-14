# Building Responses

`DocumentBuilder` assembles JSON:API response documents fluently.

```rust
use jsonapi_core::{DocumentBuilder, Link, Resource};
# fn article() -> Resource { Resource { r#type: "articles".into(), id: Some("1".into()), lid: None, attributes: serde_json::json!({"title": "Hi"}), relationships: Default::default(), links: None, meta: None } }
# fn author() -> Resource { Resource { r#type: "people".into(), id: Some("9".into()), lid: None, attributes: serde_json::json!({"name": "Dan"}), relationships: Default::default(), links: None, meta: None } }

let doc = DocumentBuilder::single(article())
    .include(author())
    .link("self", Link::String("/articles/1".into()))
    .build();
```

- `single(p)` / `collection(ps)` / `DocumentBuilder::new().no_data()` set the primary data.
- `include(r)` / `include_many(rs)` add compound-document resources. Included resources are
  deduplicated by `(type, id)`, and any that duplicate a primary resource are dropped.
- `link`, `links`, `meta`, `jsonapi`, `profile`, `ext` set top-level members.

## Error and meta-only documents

```rust
use jsonapi_core::{ApiError, Document, Resource};

let doc: Document<Resource> = Document::errors([ApiError {
    status: Some("404".into()),
    title: Some("Not Found".into()),
    ..Default::default()
}]);
```

Use `Document::meta_only(meta)` for a meta-only response.
