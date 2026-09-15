# jsonapi_axum

[axum](https://docs.rs/axum) adapter for [`jsonapi_core`](https://docs.rs/jsonapi_core) —
build [JSON:API v1.1](https://jsonapi.org/format/) servers in Rust with typed extractors,
responders, and content-negotiation middleware.

It is a **thin binding**: all JSON:API logic lives in the framework-agnostic
[`jsonapi_http`](https://docs.rs/jsonapi_http) crate, and `jsonapi_axum` only provides
axum's `FromRequest` / `FromRequestParts` / `IntoResponse` glue. The same building blocks
can back adapters for other frameworks.

## What you get

| Type | Role |
|------|------|
| `JsonApi<T>` | Extract and deserialize a typed request document (`415` / `400` / `409` / `422` on failure, with `source.pointer`). |
| `JsonApiQuery` | Parse `sort` / `page` / `filter` / `fields` / `include` into a typed `Query`. Needs no application state. |
| `JsonApiQueryValidated<T>` | Like `JsonApiQuery`, but also validates `include` paths against a `TypeRegistry` in state. Opt-in. |
| `JsonApiResponse<P, I>` | An `IntoResponse` that serializes a document with the JSON:API media type. Supports custom status and `ext`/`profile`. |
| `JsonApiError` | The shared rejection + error responder — every failure renders as a spec-shaped error document. |
| `ContentTypeLayer` / `AcceptLayer` / `JsonApiLayer` | tower layers that enforce `Content-Type` (415) and `Accept` (406) for a whole router. |

## Install

```toml
[dependencies]
jsonapi_axum = "0.1"
# Required to derive JsonApi on your resource types (the derive macro expands to
# ::jsonapi_core paths, so the crate must be a direct dependency):
jsonapi_core = "0.4"
axum = "0.8"
```

The most-used `jsonapi_core` runtime types (`Document`, `DocumentBuilder`, `Query`,
`TypeRegistry`, `ApiError`, `Resource`, `Relationship`, `JsonApiMediaType`, cursor-pagination
helpers) are **re-exported** from `jsonapi_axum`, so handler code can import from one crate.
The whole crate is also available as `jsonapi_axum::jsonapi_core`.

## Quick start

```rust,no_run
use axum::routing::get;
use axum::Router;
use jsonapi_axum::{DocumentBuilder, JsonApi, JsonApiLayer, JsonApiResponse};

#[derive(Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
}

// POST /articles — deserialize a typed JSON:API document from the request body.
async fn create(JsonApi(document): JsonApi<Article>) -> JsonApiResponse<Article> {
    let article = document.into_single().expect("single resource");
    JsonApiResponse::new(DocumentBuilder::single(article).build())
}

#[tokio::main]
async fn main() {
    let app: Router = Router::new()
        .route("/articles", get(|| async { "…" }).post(create))
        // One layer enforces JSON:API content negotiation for the whole API:
        // 415 for a bad request Content-Type, 406 for an unacceptable Accept.
        .layer(JsonApiLayer::new());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

A complete, runnable CRUD server (list / get / create / update / delete with an in-memory
store) lives in [`examples/crud_server.rs`](examples/crud_server.rs):

```sh
cargo run -p jsonapi_axum --example crud_server
```

## Content negotiation

`ContentTypeLayer` / `JsonApiLayer` only enforce `Content-Type` on body-bearing methods
(`POST` / `PUT` / `PATCH`) — a `GET` or `DELETE` without a body is never rejected with a
`415`. `AcceptLayer` / `JsonApiLayer` negotiate the `Accept` header (returning `406` when
nothing is acceptable) and store the negotiated `JsonApiMediaType` in the request
extensions. A handler that wants the response `Content-Type` to carry negotiated
`ext`/`profile` reads it (e.g. `axum::Extension<JsonApiMediaType>`) and passes it to
`JsonApiResponse::media_type` — a responder's `IntoResponse` cannot see the request, so it
cannot apply it automatically.

## Validating includes

`JsonApiQueryValidated<T>` validates `include` paths against a `TypeRegistry`, rooted at
`T`'s resource type. Provide the registry through state as `Arc<TypeRegistry>` (via axum's
`FromRef`):

```rust,ignore
async fn list(query: JsonApiQueryValidated<Article>) -> JsonApiResponse<Article> {
    // `query.query` is parsed and its include paths are known-valid for `articles`.
    # unimplemented!()
}

// Router::new().route("/articles", get(list)).with_state(Arc::new(registry))
```

Apps that don't need include validation use `JsonApiQuery`, which requires no state.

## Errors

Every error a handler returns is a `JsonApiError`, which renders as a JSON:API error
document. Build one-off errors fluently with `with_status` + the `ApiErrorExt` setters, or
use the convenience constructors (`not_found`, `forbidden`, `conflict`, `internal`):

```rust,ignore
use jsonapi_axum::{with_status, ApiErrorExt, JsonApiError};

let err = JsonApiError::from_api_error(
    with_status(422)
        .pointer("/data/attributes/title")
        .detail("must not be empty"),
);
```

`internal(detail)` returns a `500` whose body never echoes the raw `detail` (so internal
messages don't leak); log the raw text in your app. Enable the `debug-errors` feature to
include it in the response for local debugging.

### Mapping domain errors with `?`

Implement `IntoJsonApiError` for your own error type and use `.or_json_api()` so `?` stays
clean in handlers:

```rust,ignore
use jsonapi_axum::{IntoJsonApiError, JsonApiError, ResultExt};

struct NotAuthorized;
impl IntoJsonApiError for NotAuthorized {
    fn into_json_api_error(self) -> JsonApiError {
        JsonApiError::forbidden("not your resource")
    }
}

async fn handler() -> Result<(), JsonApiError> {
    authorize().or_json_api()?; // NotAuthorized -> 403 JSON:API document
    Ok(())
}
# fn authorize() -> Result<(), NotAuthorized> { Ok(()) }
```

There is deliberately **no** blanket `From<E: IntoJsonApiError>` impl — it would collide with
the concrete `From<jsonapi_core::Error>` impl under coherence. Use `.or_json_api()` /
`.into_json_api_error()` instead.

### Aggregating field errors

Collect one error per invalid attribute into an `ApiErrors` accumulator; converting it to a
`JsonApiError` yields a single document with one member per problem (the shared status
becomes the top-level HTTP status):

```rust,ignore
use jsonapi_axum::{with_status, ApiErrorExt, ApiErrors, JsonApiError};

let mut errors = ApiErrors::new();
errors.push(with_status(422).pointer("/data/attributes/title").detail("required"));
errors.push(with_status(422).pointer("/data/attributes/body").detail("required"));
let response: JsonApiError = errors.into(); // one 422 document, two members
```

### Correlation / request ids

`RequestIdLayer` makes every error response traceable: it resolves a correlation id, echoes it
as an `x-request-id` response header, and stamps it onto each `errors[].id` of a JSON:API error
document (never overwriting an id a handler already set). Only error responses (`>= 400`) are
buffered; success responses pass through untouched.

Don't reinvent id generation — compose with `tower-http`'s `SetRequestIdLayer` (which mints /
propagates `x-request-id`). Install `RequestIdLayer` **outermost** relative to
`NormalizeErrorsLayer` so it also stamps documents the normalize layer synthesizes (e.g. a
404):

```rust,ignore
use jsonapi_axum::{JsonApiLayer, NormalizeErrorsLayer, RequestIdLayer};
use tower_http::request_id::{MakeRequestUuid, SetRequestIdLayer};

let app = router
    .layer(JsonApiLayer::new())
    .layer(NormalizeErrorsLayer::new())
    .layer(RequestIdLayer::new())                                   // reads x-request-id, stamps
    .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));       // mints x-request-id (outermost)
```

The id is resolved from (1) the configured header (default `x-request-id`), (2) a `RequestId`
request extension, then (3) a generated UUID — the last **only** with the `uuid` feature and
`RequestIdLayer::new().generate()`. With no source and generation off, the layer is a no-op.
A handler can read the resolved id with the `RequestId` extractor.

### Optional integrations (off by default)

| Feature | Effect |
|---|---|
| `validator` | `from_validation_errors(&ValidationErrors) -> Vec<ApiError>` — one `422` per field, `source.pointer` = `/data/attributes/<field>`. |
| `anyhow` | `From<anyhow::Error> for JsonApiError` = `500`. |
| `sqlx` | `From<sqlx::Error> for JsonApiError`: `RowNotFound` -> `404`, else `500`. |
| `debug-errors` | Include the raw `detail` in an `internal` `500` (for local debugging only). |
| `uuid` | Enable `RequestIdLayer::generate()` to mint a UUID when no upstream request id is present. |

```toml
jsonapi_axum = { version = "0.2", features = ["validator", "anyhow", "sqlx", "uuid"] }
```

## Scope

This crate parses and represents `sort` / `filter` / `page`; **applying** them to a
datastore is the consumer's job (it is ORM-specific and a deliberate non-goal). Request
body-size limits are the adapter's responsibility — apply axum's `DefaultBodyLimit` /
`RequestBodyLimitLayer` as usual.

## License

MIT OR Apache-2.0, matching the workspace.
