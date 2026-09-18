# Building Responses

`DocumentBuilder` assembles JSON:API response documents fluently.

```rust
use jsonapi_core::{DocumentBuilder, Link, Resource};
# fn article() -> Resource { Resource::new("articles").with_id("1").with_attributes(serde_json::json!({"title": "Hi"})) }
# fn author() -> Resource { Resource::new("people").with_id("9").with_attributes(serde_json::json!({"name": "Dan"})) }

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

let doc: Document<Resource> = Document::errors([ApiError::new()
    .with_status("404")
    .with_title("Not Found")]);
```

Use `Document::meta_only(meta)` for a meta-only response.
