# Building HTTP Servers

The core `jsonapi_core` crate is transport-agnostic — it models documents and
validates them, but knows nothing about HTTP. Two companion crates turn it into a
server stack:

| Crate | Role |
|-------|------|
| [`jsonapi_axum`](https://docs.rs/jsonapi_axum) | The [axum](https://docs.rs/axum) adapter — extractors, responders, and middleware. **Start here** if you're writing an axum service. |
| [`jsonapi_http`](https://docs.rs/jsonapi_http) | The framework-agnostic layer the adapter is built on — request parsing, response building, and `tower` middleware over the [`http`](https://docs.rs/http) crate's types. Depend on it directly only to write an adapter for another framework. |

`jsonapi_axum` is a **thin binding**: it provides axum's `FromRequest` /
`FromRequestParts` / `IntoResponse` glue and re-exports everything else. The
JSON:API logic all lives in `jsonapi_http` and `jsonapi_core`, so the same
building blocks can back an adapter for any `tower`-based framework.

## Install

```toml
[dependencies]
jsonapi_axum = "0.4"
# Deriving JsonApi on your resource types needs a direct jsonapi_core dependency:
# the derive macro expands to `::jsonapi_core` paths.
jsonapi_core = "0.4"
axum = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
```

The most-used `jsonapi_core` runtime types (`Document`, `DocumentBuilder`,
`Query`, `TypeRegistry`, `ApiError`, `Resource`, `Relationship`,
`JsonApiMediaType`, the cursor-pagination helpers) are **re-exported** from
`jsonapi_axum`, so handler code can import from one crate. The whole crate is
also available as `jsonapi_axum::jsonapi_core`.

## A minimal server

```rust,ignore
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
    let app = Router::new()
        .route("/articles", get(|| async { "…" }).post(create))
        // One layer enforces JSON:API content negotiation for the whole API.
        .layer(JsonApiLayer::new());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

Two runnable examples ship with the crate:

```sh
cargo run -p jsonapi_axum --example crud_server        # list / get / create / update / delete
cargo run -p jsonapi_axum --example compound_document  # ?include= resolution
```

## Content negotiation

One layer enforces the JSON:API media-type rules for an entire router:

- **`JsonApiLayer`** — the combined guard (content type **and** `Accept`).
- **`ContentTypeLayer`** — content type only.
- **`AcceptLayer`** — `Accept` negotiation only.

```rust,ignore
use jsonapi_axum::JsonApiLayer;

let app = router.layer(JsonApiLayer::new());
```

- `Content-Type` is enforced only on **body-bearing** methods (`POST` / `PUT` /
  `PATCH`). A `GET` or `DELETE` with no body is never rejected with a `415`.
- A request whose `Content-Type` isn't `application/vnd.api+json` gets a `415`
  error document; when checked together with `Accept`, content type wins first.
- An `Accept` header with nothing acceptable gets a `406`. A **missing** `Accept`
  means "anything", so a plain `application/vnd.api+json` response is served.

This is the HTTP enforcement of the primitives described in
[Content Negotiation](./content-negotiation.md).

### Advertising extensions and profiles

Advertise the `ext` / `profile` URIs your server supports; the layer negotiates
the request's `Accept` against them and stores the result in the request
extensions:

```rust,ignore
let app = router.layer(
    JsonApiLayer::new().ext(["https://jsonapi.org/ext/atomic"]),
);
```

The negotiated [`JsonApiMediaType`] is placed in the request extensions. A
responder cannot see the request, so to echo the negotiated `ext` / `profile`
back on the response `Content-Type` a handler reads it (via the
[`NegotiatedMediaType`](./axum-extractors-responders.md) extractor, or
`axum::Extension<JsonApiMediaType>`) and passes it to
`JsonApiResponse::media_type`. Requested URIs the server did not advertise are
dropped rather than rejected.

## Normalizing non-JSON:API errors

axum and `tower` can produce error responses that never reach your handler — an
unmatched route (`404`), a body-size-limit rejection (`413`), a timeout. Two
helpers make those spec-shaped too:

- **`not_found`** — a `Router::fallback` that returns a `404` JSON:API error
  document for any unmatched route.
- **`NormalizeErrorsLayer`** — rewrites any `>= 400` response whose
  `Content-Type` is not `application/vnd.api+json` into a JSON:API error
  document with the same status, folding the original body into `detail`.
  Responses that are already JSON:API pass through untouched.

```rust,ignore
use jsonapi_axum::{JsonApiLayer, NormalizeErrorsLayer, not_found};

let app = router
    .fallback(not_found)
    .layer(JsonApiLayer::new())
    .layer(NormalizeErrorsLayer::new());
```

With these in place, **every** response your API emits — from a handler, an
extractor rejection, a layer, or the framework itself — is a JSON:API document.

## Where to go next

- [Extractors and Responders](./axum-extractors-responders.md) — read requests
  and write responses.
- [Error Handling in axum](./axum-error-handling.md) — `JsonApiError`, the
  fluent builder, domain-error mapping, and correlation ids.
- [Compound Documents and Pagination](./axum-includes-pagination.md) — `include`
  resolution and pagination links.
- [Testing axum Handlers](./axum-testing.md) — the in-process test harness.
