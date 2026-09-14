# Cursor Pagination

`jsonapi_core` supports the [cursor-pagination profile][profile]
(`CURSOR_PAGINATION_PROFILE`).

[profile]: https://jsonapi.org/profiles/ethanresnick/cursor-pagination

## Reading cursor parameters

```rust
use jsonapi_core::{Query, CursorPage};

let q = Query::from_query_string("?page[size]=20&page[after]=abc")?;
let cursor = CursorPage::from_query(&q)?;
assert_eq!(cursor.size, Some(20));
assert_eq!(cursor.after.as_deref(), Some("abc"));
# Ok::<(), jsonapi_core::Error>(())
```

`page[size]` must parse as an integer or you get `Error::QueryParse`; `page[after]` and
`page[before]` are opaque cursor strings.

## Building pagination links

`CursorLinks` builds `first`/`prev`/`next`/`last` links, preserving the request's other
query parameters:

```rust
use jsonapi_core::CursorLinks;

let links = CursorLinks::new("/articles")
    .preserve(&[("filter[status]", "published")])
    .size(20)
    .build(/* first */ true, /* prev */ None, /* next */ Some("cursor99"), /* last */ None);

// next → /articles?filter[status]=published&page[size]=20&page[after]=cursor99
assert!(links.contains("next"));
```

Each of `first`/`prev`/`next`/`last` is opt-in — cursor pagination often cannot compute a
`last` link, so pass `None` when you can't.

## Advertising the profile

Attach the profile URI to a response with the builder:

```rust
# use jsonapi_core::{DocumentBuilder, Resource, CURSOR_PAGINATION_PROFILE};
# fn r() -> Resource { Resource { r#type: "articles".into(), id: Some("1".into()), lid: None, attributes: serde_json::json!({}), relationships: Default::default(), links: None, meta: None } }
let doc = DocumentBuilder::collection(vec![r()])
    .profile(CURSOR_PAGINATION_PROFILE)
    .build();
```
