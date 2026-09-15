# Compound Documents and Pagination

Two handler-level concerns get dedicated helpers in `jsonapi_axum`: assembling
the `included` array for an `?include=` request, and generating pagination
`links`.

## Resolving includes

`resolve_includes` builds the deduped `included` array of a compound document
from the requested `include` paths. You implement `IncludeResolver` — a **batch
loader keyed by identity** — and the library walks the linkage (including
transitive paths like `comments.author`), dedups by `(type, id)`, and calls your
loader **once per type per level** (no N+1). It never touches a datastore and
never applies sort / filter / page — fetching and those concerns stay yours.

```rust,ignore
use std::future::Future;
use jsonapi_axum::{resolve_includes, IncludeResolver, DocumentBuilder,
                   JsonApiQuery, JsonApiResponse, JsonApiError, ResultExt};
use jsonapi_core::Resource;
use axum::extract::State;
use axum::response::IntoResponse;

struct Store { /* your data */ }

impl IncludeResolver for Store {
    type Error = MyError;   // your own error type, mapped at the edge

    fn load(&self, type_name: &str, ids: &[String])
        -> impl Future<Output = Result<Vec<Resource>, Self::Error>> + Send
    {
        // Fetch the resources of `type_name` with these ids and return them as
        // dynamic `Resource`s (e.g. via `Resource::from_typed`).
        # async { unimplemented!() }
    }
}

async fn list(State(store): State<Store>, JsonApiQuery(query): JsonApiQuery)
    -> Result<impl IntoResponse, JsonApiError>
{
    let primary: Vec<Resource> = /* your primaries as dynamic resources */;
    let paths: Vec<&str> = query.include.iter().map(String::as_str).collect();
    let included = resolve_includes(&primary, &paths, &store).await.or_json_api()?;
    let doc = DocumentBuilder::collection(primary).include_many(included).build();
    Ok(JsonApiResponse::new(doc))
}
```

A few properties worth knowing:

- **`IncludeResolver::Error` is your own type**, not `JsonApiError` — the
  resolver lives in the framework-agnostic `jsonapi_http`. Map it at the handler
  edge with `.or_json_api()` (see
  [Error Handling in axum](./axum-error-handling.md)).
- Primary resources are never re-emitted as included; lid-only refs and cyclic
  linkage terminate safely; a loader returning fewer rows than requested simply
  drops the missing refs.
- `fields[type]` filtering of the included resources happens later, at
  serialization (`JsonApiResponse::fields`), so the two compose.

Pair this with `JsonApiQueryValidated<T>` (see
[Extractors and Responders](./axum-extractors-responders.md)) to reject unknown
include paths before doing any loading.

See `examples/compound_document.rs` for a runnable server.

## Pagination links

`pagination_links` builds the top-level pagination `links` object from the
request URI. You pass the `PageStrategy` you actually applied (from
`jsonapi_core`) and the optional total count:

```rust,ignore
use jsonapi_axum::pagination_links;
use jsonapi_core::PageStrategy;

let links = pagination_links(
    &uri,
    PageStrategy::PageNumber { number: 2, size: 10 },
    Some(35),  // total item count, if known — enables the `last` link
);
let doc = DocumentBuilder::collection(page).links(links).build();
```

It uses the request path as the base and **preserves every non-`page[...]`
query parameter** — `sort`, `filter`, `fields`, `include`, and any custom
params — across every generated link (`first` / `prev` / `next` / `last` /
`self`), so a client walking the pages keeps the rest of its query intact. The
pagination math itself is delegated to `jsonapi_core`'s pagination support (see
[Cursor Pagination](./pagination.md)).
