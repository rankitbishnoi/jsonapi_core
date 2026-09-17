# jsonapi_http

Framework-agnostic [JSON:API v1.1](https://jsonapi.org/format/) HTTP integration for
[`jsonapi_core`](https://docs.rs/jsonapi_core) — request parsing, response building, and
[tower](https://docs.rs/tower) middleware over the [`http`](https://docs.rs/http) crate's
types, with **no** web-framework dependency.

**If you are building an axum service, use
[`jsonapi_axum`](https://docs.rs/jsonapi_axum) instead** — it provides the
`FromRequest` / `IntoResponse` glue on top of this crate. Depend on `jsonapi_http`
directly only when writing an adapter for another framework (actix, warp, …): the
same building blocks back every adapter.

## What you get

Plain functions and tower layers over `http::Request` / `http::Response`:

| Module | Item | Role |
|--------|------|------|
| `request` | `check_content_type` | Validate the request `Content-Type` is `application/vnd.api+json` (→ 415 / 400). |
| `request` | `negotiate` | Negotiate a response media type from `Accept`, intersecting requested `ext`/`profile` with the server's (→ 406). |
| `request` | `parse_query` | Parse the URI query into a typed `jsonapi_core::Query`. |
| `request` | `deserialize_body` | Deserialize collected body `Bytes` into a typed `Document<T>`, mapping failures to an `ApiError` with `source.pointer`. |
| `response` | `document_response` / `json_api_response` | Serialize a `Document` into a JSON:API `http::Response` (200, or any status). |
| `response` | `content_type_value` | Build a `Content-Type` header value carrying negotiated `ext`/`profile`. |
| `error` | `status_for` / `to_api_error` | Map a `jsonapi_core::Error` to an HTTP status and a JSON:API `ApiError`. |
| `error` | `error_response` / `error_response_for` / `error_response_for_status` | Build a spec-shaped JSON:API error-document response (from `ApiError`s, an `Error`, or a bare status). |
| `error` | `with_status` + `ApiErrorExt` / `ApiErrors` | Fluently build an `ApiError` from a status and accumulate several into one document. |
| `id` | `ClientIdPolicy` / `check_client_id` / `check_id_matches` / `id_conflict` | Enforce client-supplied `id` policy on create (`Assign`/`Accept`/`Forbid`) and PATCH id-matching. |
| `include` | `IncludeResolver` / `resolve_includes` | Assemble a deduped compound-document `included` array from `include` paths via a batch loader (one load per type per level — no N+1). |
| `layer` | `ContentTypeLayer` / `AcceptLayer` / `JsonApiLayer` | tower layers that reject non-conforming requests (415 / 406) before they reach a handler. |

Every failure — from an extractor, a layer, or a handler — funnels through
`error_response`, so the wire format of an error is identical no matter where it was raised.

## Install

```toml
[dependencies]
jsonapi_http = "1.0.0-rc.1"
jsonapi_core = "1.0.0-rc.1"
```

## Building an adapter

An adapter reads the relevant part of a request, calls into `jsonapi_http`, and turns
any `Error` into an error response. For example, a body extractor:

```rust,ignore
use jsonapi_http::{check_content_type, deserialize_body, error_response_for};

fn extract_document<T>(req: &http::Request<bytes::Bytes>) -> http::Response<bytes::Bytes>
where
    T: jsonapi_core::ResourceObject + serde::de::DeserializeOwned,
{
    if let Err(err) = check_content_type(req.headers()) {
        return error_response_for(&err);
    }
    match deserialize_body::<T>(req.body()) {
        Ok(_document) => todo!("hand the typed document to the handler"),
        Err(err) => error_response(std::iter::once(*err)),
    }
}
```

The tower layers drop into any tower-based router unchanged:

```rust,ignore
use jsonapi_http::JsonApiLayer;
// service.layer(JsonApiLayer::new().ext(["https://jsonapi.org/ext/atomic"]))
```

## Scope

This crate parses and represents `sort` / `filter` / `page`; **applying** them to a
datastore is the consumer's job (a deliberate non-goal). It never buffers the request
body — the adapter drains the body into `Bytes` and enforces any body-size limit before
calling `deserialize_body`.

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](http://opensource.org/licenses/MIT) at your option.
