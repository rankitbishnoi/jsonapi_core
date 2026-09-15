# Extractors and Responders

`jsonapi_axum` reads requests through **extractors** and writes responses through
**responders**. Both are thin bindings over `jsonapi_http`; every failure becomes
a spec-shaped [`JsonApiError`](./axum-error-handling.md).

## Extracting a request body — `JsonApi<T>`

`JsonApi<T>` validates the `Content-Type`, buffers the body, and deserializes a
`Document<T>` for a type deriving `JsonApi`:

```rust,ignore
use jsonapi_axum::{JsonApi, JsonApiError, JsonApiResponse, DocumentBuilder};

async fn create(JsonApi(document): JsonApi<Article>)
    -> Result<JsonApiResponse<Article>, JsonApiError>
{
    let article = document.into_single().map_err(|e| JsonApiError::from_core(&e))?;
    Ok(JsonApiResponse::new(DocumentBuilder::single(article).build()))
}
```

The rejection maps the failure to the right status automatically:

| Failure | Status |
|---------|--------|
| Wrong `Content-Type` | `415` |
| Body over the size limit (axum's `DefaultBodyLimit`) | `413` |
| Malformed JSON | `400` |
| Wire `type` ≠ the struct's `type` | `409`, `source.pointer` `/data/type` |
| A required attribute is missing | `422`, `source.pointer` `/data/attributes/<name>` |

### Client-supplied ids

On a **create**, JSON:API lets a client omit `id` (server assigns) or supply
one. Model the request body with an optional id so both parse:

```rust,ignore
#[derive(jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct NewArticle {
    #[jsonapi(id)]
    id: Option<String>,   // optional on input; the server owns identity
    title: String,
}
```

Two helpers enforce id policy:

```rust,ignore
use jsonapi_axum::ClientIdPolicy;

async fn create(body: JsonApi<NewArticle>) -> Result<_, JsonApiError> {
    // Reject a client-supplied id outright (403), or Accept / Assign it:
    body.check_client_id(ClientIdPolicy::Forbid)?;
    // ...
}

async fn update(
    axum::extract::Path(id): axum::extract::Path<String>,
    body: JsonApi<NewArticle>,
) -> Result<_, JsonApiError> {
    // On PATCH, a body id must match the URL id (409 on mismatch):
    body.require_id(&id)?;
    // ...
}
```

`ClientIdPolicy` is `Assign` (default — client id ignored), `Accept` (taken
as-is), or `Forbid` (rejected with `403`).

## Extracting query parameters

`JsonApiQuery` parses `sort` / `page` / `filter` / `fields` / `include` into a
typed [`Query`](./query-parsing.md). It needs no application state:

```rust,ignore
use jsonapi_axum::JsonApiQuery;

async fn list(JsonApiQuery(query): JsonApiQuery) -> JsonApiResponse<Article> {
    // query.sort, query.page, query.filter, query.fields, query.include
    # unimplemented!()
}
```

`JsonApiQueryValidated<T>` additionally validates the `include` paths against a
`TypeRegistry` in state, rooted at `T`'s resource type — an unknown path is
rejected with a `400` naming the bad segment. Provide the registry as
`Arc<TypeRegistry>` (via axum's `FromRef`):

```rust,ignore
use jsonapi_axum::JsonApiQueryValidated;

async fn list(q: JsonApiQueryValidated<Article>) -> JsonApiResponse<Article> {
    // q.query is parsed and its include paths are known-valid for `articles`.
    # unimplemented!()
}
// Router::new().route("/articles", get(list)).with_state(Arc::new(registry))
```

Use `JsonApiQuery` when you don't need include validation. See
[Include Path Validation](./include-validation.md) for how the registry works.

## Two infallible helper extractors

- **`NegotiatedMediaType`** reads the [`JsonApiMediaType`] that
  `JsonApiLayer` / `AcceptLayer` negotiated and stored in the request
  extensions, falling back to `JsonApiMediaType::plain()` when no layer ran — so
  it never fails. Pass it to `JsonApiResponse::media_type` to echo negotiated
  `ext` / `profile` back on the response.
- **`BaseUrl`** pulls your application's base URL from state (via `FromRef`), for
  building `self` / `related` links and the `Location` header.

```rust,ignore
use jsonapi_axum::{NegotiatedMediaType, JsonApiResponse};

async fn list(NegotiatedMediaType(media): NegotiatedMediaType)
    -> JsonApiResponse<Article>
{
    JsonApiResponse::new(/* ... */).media_type(media)
    # ; unimplemented!()
}
```

## Writing responses — `JsonApiResponse<P, I>`

`JsonApiResponse` wraps a `Document` and implements `IntoResponse`. It defaults
to `200 OK` with a plain JSON:API `Content-Type`, and degrades a serialization
failure to a JSON:API `500` rather than panicking at the response boundary. It
also implements `From<Document>`, so `.into()` works.

```rust,ignore
use axum::http::StatusCode;
use jsonapi_axum::{DocumentBuilder, JsonApiResponse};

// 200 OK
JsonApiResponse::new(DocumentBuilder::single(article).build());

// 201 Created with a Location header (shorthand for .status(201).location(..))
JsonApiResponse::new(doc).created(format!("/articles/{}", id));

// custom status, negotiated media type, and sparse fieldsets in one chain
JsonApiResponse::new(doc)
    .status(StatusCode::ACCEPTED)
    .media_type(media)
    .fields(query.fields);   // applies fields[type] filtering; empty is a no-op
```

`fields(..)` applies [sparse-fieldset](./sparse-fieldsets.md) filtering from the
request `Query`; an empty config skips the filtering round-trip entirely.

## Relationship endpoints

JSON:API defines dedicated relationship URLs
(`/articles/1/relationships/author`) whose payload is bare **linkage**, not a
full resource. `jsonapi_axum` has matching extractors and a responder:

- **`JsonApiToOne(pub Option<ResourceIdentifier>)`** — extracts a to-one
  payload; `null` clears the relationship. An array payload is rejected `400`.
- **`JsonApiToMany(pub Vec<ResourceIdentifier>)`** — extracts a to-many payload.
  A `null` or object payload is rejected `400`.
- **`RelationshipResponse`** — a responder for a linkage document, with the same
  fluent `status` / `media_type` plus `links` and `meta`:

```rust,ignore
use jsonapi_axum::{JsonApiToOne, RelationshipResponse};
use jsonapi_core::RelationshipData;

async fn replace_author(JsonApiToOne(identifier): JsonApiToOne)
    -> RelationshipResponse
{
    // persist the new linkage, then echo it back
    RelationshipResponse::new(RelationshipData::ToOne(identifier))
}
```

For the exhaustive method list, see the
[docs.rs API reference](https://docs.rs/jsonapi_axum).
