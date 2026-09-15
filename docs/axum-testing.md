# Testing axum Handlers

The `testing` feature (off by default) provides in-process helpers that cut the
`oneshot` + build-request + collect-body + parse-JSON boilerplate out of handler
tests. Enable it in your `[dev-dependencies]`:

```toml
[dev-dependencies]
jsonapi_axum = { version = "0.4", features = ["testing"] }
```

It keeps the default build lean and pulls in `tower`'s `util` (for `oneshot`)
only for tests.

## The three pieces

- **`TestRequest`** — a request builder with JSON:API defaults. Body-bearing
  methods (`post` / `patch` / `put`) get a `Content-Type: application/vnd.api+json`
  automatically at `build()` unless you override it.
- **`RouterTestExt`** — adds `Router::send(request)`, which drives the router
  in-process and collects the response.
- **`JsonApiTestResponse`** — the collected response, with fluent `assert_*`
  methods (each panics with a clear message and returns `self` for chaining) and
  raw accessors.

```rust,ignore
use jsonapi_axum::testing::{RouterTestExt, TestRequest};
use http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn create_rejects_blank_title() {
    let body = json!({"data": {"type": "articles", "attributes": {"title": ""}}});
    let request = TestRequest::post("/articles").body_json(&body).build();

    app().send(request).await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY)
        .assert_error(422)
        .assert_error_pointer("/data/attributes/title");
}
```

## Building requests

```rust,ignore
TestRequest::get("/articles?sort=-created").accept_json_api().build();
TestRequest::post("/articles").body_document(&document).build();  // serialize a typed Document
TestRequest::post("/articles").body_json(&value).build();         // a serde_json::Value body
TestRequest::post("/articles").raw_body(b"not json".as_ref()).build(); // deliberately malformed
```

`body_document` / `body_json` set the JSON:API `Content-Type`; `raw_body` plus an
explicit `header(..)` lets you construct deliberately-malformed requests to test
rejections.

## Asserting on the response

The `assert_*` set covers the common JSON:API checks and chains:

```rust,ignore
app().send(request).await
    .assert_status(StatusCode::OK)
    .assert_json_api_content_type()
    .assert_error(409)                 // errors[0].status == "409"
    .assert_error_pointer("/data/id")  // errors[0].source.pointer
    .assert_error_parameter("sort")    // errors[0].source.parameter
    .assert_error_count(2);
```

For anything the assertions don't cover, drop to the raw accessors —
`.json()`, `.data()`, `.errors()`, `.header(name)`, or the public
`status` / `headers` / `body` fields (`body` is `None` for an empty or non-JSON
response, e.g. a `204`):

```rust,ignore
let response = app().send(request).await;
assert_eq!(response.data()["attributes"]["title"], "Hello");
assert_eq!(response.header("location"), Some("/articles/1"));
```

The helpers are async — bring your own runtime (`#[tokio::test]` here).
