# Error Handling in axum

Every error a handler returns is a `JsonApiError`, which renders as a JSON:API
error document. This chapter covers building one, mapping your own error types,
aggregating field errors, and attaching correlation ids. It is the HTTP
counterpart of the core [Error Handling](./error-handling.md) chapter.

## `JsonApiError`

`JsonApiError` is used both as an extractor `Rejection` and as an `IntoResponse`
error returned from a handler. Convenience constructors cover the common cases:

```rust,ignore
use jsonapi_axum::JsonApiError;

JsonApiError::not_found("article `9` does not exist");   // 404
JsonApiError::forbidden("not your resource");            // 403
JsonApiError::conflict("id already exists");             // 409, source.pointer /data/id
JsonApiError::internal("db pool exhausted");             // 500 (detail scrubbed by default)
```

`internal(detail)` returns a `500` whose body **never** echoes the raw `detail`,
so internal messages don't leak to clients — log the raw text in your app. Enable
the `debug-errors` feature to include it in the response for local debugging.

It also converts from the lower-level types: `From<jsonapi_core::Error>`,
`From<Box<ApiError>>`, and `From<ApiErrors>`.

## Building errors fluently

Start from an HTTP status with `with_status`, then chain the `ApiErrorExt`
setters (`pointer`, `detail`, `code`, `title`, `id`, `meta`, `about_link`):

```rust,ignore
use jsonapi_axum::{with_status, ApiErrorExt, JsonApiError};

let err = JsonApiError::from_api_error(
    with_status(422)
        .pointer("/data/attributes/title")
        .detail("must not be empty"),
);
```

`with_status` sets the numeric `status` and the canonical HTTP `title`; the
setters fill in the rest.

## Aggregating field errors

Validation typically produces one error per bad field. Collect them into an
`ApiErrors` accumulator; converting it to a `JsonApiError` yields a single
document with one member per problem (the shared status becomes the top-level
HTTP status):

```rust,ignore
use jsonapi_axum::{with_status, ApiErrorExt, ApiErrors, JsonApiError};

let mut errors = ApiErrors::new();
errors.push(with_status(422).pointer("/data/attributes/title").detail("required"));
errors.push(with_status(422).pointer("/data/attributes/body").detail("required"));
let response: JsonApiError = errors.into();   // one 422 document, two members
```

## Mapping domain errors with `?`

Implement `IntoJsonApiError` for your own error type and use `.or_json_api()` so
`?` stays clean in handlers:

```rust,ignore
use jsonapi_axum::{IntoJsonApiError, JsonApiError, ResultExt};

struct NotAuthorized;
impl IntoJsonApiError for NotAuthorized {
    fn into_json_api_error(self) -> JsonApiError {
        JsonApiError::forbidden("not your resource")
    }
}

async fn handler() -> Result<(), JsonApiError> {
    authorize().or_json_api()?;   // NotAuthorized -> 403 JSON:API document
    Ok(())
}
# fn authorize() -> Result<(), NotAuthorized> { Ok(()) }
```

There is deliberately **no** blanket `From<E: IntoJsonApiError>` impl — it would
collide with the concrete `From<jsonapi_core::Error>` impl under Rust's coherence
rules. Use `.or_json_api()` / `.into_json_api_error()` instead.

## Optional integrations (off by default)

| Feature | Effect |
|---------|--------|
| `validator` | `from_validation_errors(&ValidationErrors) -> Vec<ApiError>` — one `422` per field, `source.pointer` `/data/attributes/<field>`, deterministic ordering. |
| `anyhow` | `From<anyhow::Error> for JsonApiError` = `500`. |
| `sqlx` | `From<sqlx::Error> for JsonApiError`: `RowNotFound` → `404`, else `500`. |
| `debug-errors` | Include the raw `detail` in an `internal` `500` (local debugging only). |

```toml
jsonapi_axum = { version = "0.4", features = ["validator", "anyhow", "sqlx"] }
```

With the `validator` feature, feed field errors straight into a document:

```rust,ignore
use jsonapi_axum::{from_validation_errors, JsonApiError};

article.validate()
    .map_err(|e| JsonApiError::from_api_errors(from_validation_errors(&e)))?;
```

## Correlation / request ids

`RequestIdLayer` makes every error response traceable: it resolves a correlation
id, echoes it as an `x-request-id` response header, and stamps it onto each
`errors[].id` of a JSON:API error document (never overwriting an id a handler
already set). Only error responses (`>= 400`) are buffered; success responses
pass through untouched.

Don't reinvent id generation — compose with `tower-http`'s `SetRequestIdLayer`.
Install `RequestIdLayer` **outermost** relative to `NormalizeErrorsLayer` so it
also stamps documents the normalize layer synthesizes (e.g. a `404`):

```rust,ignore
use jsonapi_axum::{JsonApiLayer, NormalizeErrorsLayer, RequestIdLayer};
use tower_http::request_id::{MakeRequestUuid, SetRequestIdLayer};

let app = router
    .layer(JsonApiLayer::new())
    .layer(NormalizeErrorsLayer::new())
    .layer(RequestIdLayer::new())                              // reads x-request-id, stamps
    .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));  // mints it (outermost)
```

The id is resolved from (1) the configured header (default `x-request-id`),
(2) a `RequestId` request extension, then (3) a generated UUID — the last **only**
with the `uuid` feature and `RequestIdLayer::new().generate()`. With no source
and generation off, the layer is a graceful no-op. A handler can read the
resolved id with the `RequestId` extractor.
