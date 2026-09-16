//! In-process test utilities, behind the `testing` feature.
//!
//! These cut the `oneshot` + build-request + collect-body + parse-JSON
//! boilerplate that every JSON:API server test repeats, without hiding intent:
//!
//! 1. [`TestRequest`] builds an `axum` [`Request`] with JSON:API defaults
//!    (POST/PATCH/PUT get the `application/vnd.api+json` `Content-Type` unless
//!    you override it);
//! 2. [`RouterTestExt::send`] drives a [`Router`] with `oneshot` and collects the
//!    response into a [`JsonApiTestResponse`];
//! 3. [`JsonApiTestResponse`]'s `assert_*` methods chain, each panicking with a
//!    clear message, and its accessors (`json`/`data`/`errors`/`header`) drop you
//!    down to raw values for anything bespoke.
//!
//! Helpers are **async** — bring your own runtime (`#[tokio::test]`); nothing
//! blocks internally.
//!
//! ```no_run
//! use axum::Router;
//! use jsonapi_axum::testing::{RouterTestExt, TestRequest};
//! use http::StatusCode;
//!
//! # async fn run(app: Router) {
//! app.send(TestRequest::get("/articles/404").build())
//!     .await
//!     .assert_status(StatusCode::NOT_FOUND)
//!     .assert_json_api_content_type()
//!     .assert_error(404);
//! # }
//! ```

use std::future::Future;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::response::Response;
use bytes::Bytes;
use http::header::{self, HeaderName, HeaderValue};
use http::{HeaderMap, Method, Request, StatusCode};
use serde::Serialize;
use serde_json::Value;
use tower::ServiceExt;

use jsonapi_http::JSON_API_MEDIA_TYPE;

/// Builder for an `axum` request with JSON:API defaults. Start from a method
/// constructor ([`get`](Self::get) / [`post`](Self::post) / [`patch`](Self::patch)
/// / [`delete`](Self::delete)), set body/headers, then [`build`](Self::build).
///
/// A body-bearing method (POST/PATCH/PUT) is given the JSON:API `Content-Type`
/// automatically — unless you set one yourself (e.g. to construct a deliberately
/// wrong request alongside [`raw_body`](Self::raw_body)).
#[must_use = "call `.build()` to produce the request"]
pub struct TestRequest {
    method: Method,
    uri: String,
    headers: HeaderMap,
    body: Body,
}

impl TestRequest {
    fn new(method: Method, uri: impl Into<String>) -> Self {
        Self {
            method,
            uri: uri.into(),
            headers: HeaderMap::new(),
            body: Body::empty(),
        }
    }

    /// A `GET` request.
    pub fn get(uri: impl Into<String>) -> Self {
        Self::new(Method::GET, uri)
    }

    /// A `POST` request (JSON:API `Content-Type` defaulted at build time).
    pub fn post(uri: impl Into<String>) -> Self {
        Self::new(Method::POST, uri)
    }

    /// A `PATCH` request (JSON:API `Content-Type` defaulted at build time).
    pub fn patch(uri: impl Into<String>) -> Self {
        Self::new(Method::PATCH, uri)
    }

    /// A `DELETE` request.
    pub fn delete(uri: impl Into<String>) -> Self {
        Self::new(Method::DELETE, uri)
    }

    /// Set an arbitrary header (overriding any default for that name).
    pub fn header(mut self, name: HeaderName, value: &str) -> Self {
        self.headers.insert(
            name,
            HeaderValue::from_str(value).expect("valid header value"),
        );
        self
    }

    /// Set `Content-Type: application/vnd.api+json` explicitly. Rarely needed —
    /// body-bearing methods default to it — but handy on a `GET`.
    pub fn content_type_json_api(self) -> Self {
        self.header(header::CONTENT_TYPE, JSON_API_MEDIA_TYPE)
    }

    /// Set `Accept: application/vnd.api+json`.
    pub fn accept_json_api(self) -> Self {
        self.header(header::ACCEPT, JSON_API_MEDIA_TYPE)
    }

    /// Serialize a JSON:API document (typed or dynamic) as the request body.
    pub fn body_document<D: Serialize>(mut self, document: &D) -> Self {
        let bytes = serde_json::to_vec(document).expect("document serializes to JSON");
        self.body = Body::from(bytes);
        self
    }

    /// Set the body from a `serde_json::Value`.
    pub fn body_json(mut self, value: &Value) -> Self {
        self.body = Body::from(value.to_string());
        self
    }

    /// Set the body from raw bytes — for malformed-input tests (invalid JSON,
    /// wrong shape). Pair with [`header`](Self::header) to override the
    /// `Content-Type` when you need a deliberately non-conforming request.
    pub fn raw_body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Body::from(body.into());
        self
    }

    /// Finish building. Body-bearing methods without an explicit `Content-Type`
    /// get the JSON:API media type.
    #[must_use]
    pub fn build(self) -> Request<Body> {
        let mut headers = self.headers;
        let is_body_method = matches!(self.method, Method::POST | Method::PATCH | Method::PUT);
        if is_body_method && !headers.contains_key(header::CONTENT_TYPE) {
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(JSON_API_MEDIA_TYPE),
            );
        }

        let mut builder = Request::builder().method(self.method).uri(self.uri);
        for (name, value) in &headers {
            builder = builder.header(name, value);
        }
        builder.body(self.body).expect("valid request")
    }
}

/// Drive a [`Router`] in-process and collect the response. Implemented for
/// [`Router`]; call [`send`](Self::send) with a built [`TestRequest`].
pub trait RouterTestExt {
    /// Run `request` against this router via `oneshot` and collect the response.
    fn send(self, request: Request<Body>) -> impl Future<Output = JsonApiTestResponse> + Send;
}

impl RouterTestExt for Router {
    async fn send(self, request: Request<Body>) -> JsonApiTestResponse {
        let response = self
            .oneshot(request)
            .await
            .expect("router response is infallible");
        JsonApiTestResponse::collect(response).await
    }
}

/// A collected response: status, headers, and the parsed JSON body (`None` for
/// an empty body such as a `204`, or a non-JSON body).
#[derive(Debug, Clone)]
pub struct JsonApiTestResponse {
    /// The response status.
    pub status: StatusCode,
    /// The response headers.
    pub headers: HeaderMap,
    /// The parsed JSON body, or `None` when the body was empty / not JSON.
    pub body: Option<Value>,
}

impl JsonApiTestResponse {
    async fn collect(response: Response) -> Self {
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect response body");
        let body = if bytes.is_empty() {
            None
        } else {
            serde_json::from_slice(&bytes).ok()
        };
        Self {
            status,
            headers,
            body,
        }
    }

    /// Assert the response status.
    pub fn assert_status(self, expected: StatusCode) -> Self {
        assert_eq!(
            self.status, expected,
            "expected status {expected}, got {} (body: {:?})",
            self.status, self.body
        );
        self
    }

    /// Assert the response `Content-Type` is `application/vnd.api+json`
    /// (ignoring any `ext`/`profile` parameters).
    pub fn assert_json_api_content_type(self) -> Self {
        let content_type = self
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.starts_with(JSON_API_MEDIA_TYPE),
            "expected `Content-Type` {JSON_API_MEDIA_TYPE}, got {content_type:?}"
        );
        self
    }

    /// Assert `errors[0].status` equals `status` (compared as the JSON:API
    /// string it is serialized as).
    pub fn assert_error(self, status: u16) -> Self {
        let got = self.first_error().get("status").and_then(Value::as_str);
        assert_eq!(
            got,
            Some(status.to_string().as_str()),
            "expected errors[0].status {status:?}, got {got:?}"
        );
        self
    }

    /// Assert `errors[0].source.pointer` equals `pointer`.
    pub fn assert_error_pointer(self, pointer: &str) -> Self {
        let got = self
            .first_error()
            .pointer("/source/pointer")
            .and_then(Value::as_str);
        assert_eq!(
            got,
            Some(pointer),
            "expected errors[0].source.pointer {pointer:?}, got {got:?}"
        );
        self
    }

    /// Assert `errors[0].source.parameter` equals `parameter`.
    pub fn assert_error_parameter(self, parameter: &str) -> Self {
        let got = self
            .first_error()
            .pointer("/source/parameter")
            .and_then(Value::as_str);
        assert_eq!(
            got,
            Some(parameter),
            "expected errors[0].source.parameter {parameter:?}, got {got:?}"
        );
        self
    }

    /// Assert the number of members in the `errors` array.
    pub fn assert_error_count(self, count: usize) -> Self {
        let got = self.errors().as_array().map_or(0, std::vec::Vec::len);
        assert_eq!(got, count, "expected {count} error(s), got {got}");
        self
    }

    /// The parsed JSON body. Panics if the response had no JSON body.
    #[must_use]
    pub fn json(&self) -> &Value {
        self.body
            .as_ref()
            .expect("response had no JSON body to inspect")
    }

    /// The document's `data` member. Panics if absent.
    #[must_use]
    pub fn data(&self) -> &Value {
        self.json()
            .get("data")
            .expect("response document has no `data` member")
    }

    /// The document's `errors` member. Panics if absent.
    #[must_use]
    pub fn errors(&self) -> &Value {
        self.json()
            .get("errors")
            .expect("response document has no `errors` member")
    }

    /// A response header value as a string, if present and valid UTF-8.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// The first member of `errors`, panicking if the array is absent or empty.
    fn first_error(&self) -> &Value {
        self.errors()
            .as_array()
            .expect("`errors` is an array")
            .first()
            .expect("`errors` array is empty")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiErrorExt;
    use axum::routing::{get, post};
    use serde_json::json;

    /// A tiny router: `GET /ok` → a data document; `POST /boom` → a JSON:API
    /// error via our own error responder; `GET /empty` → `204`.
    fn app() -> Router {
        async fn ok() -> crate::JsonApiResponse<jsonapi_core::Resource> {
            let resource = jsonapi_core::Resource {
                r#type: "articles".into(),
                id: Some("1".into()),
                lid: None,
                attributes: json!({"title": "Hi"}),
                relationships: Default::default(),
                links: None,
                meta: None,
            };
            crate::JsonApiResponse::new(jsonapi_core::DocumentBuilder::single(resource).build())
        }
        async fn boom() -> crate::JsonApiError {
            crate::JsonApiError::from_api_error(
                crate::with_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .pointer("/data/attributes/title")
                    .detail("must not be empty"),
            )
        }
        async fn empty() -> StatusCode {
            StatusCode::NO_CONTENT
        }
        Router::new()
            .route("/ok", get(ok))
            .route("/boom", post(boom))
            .route("/empty", get(empty))
    }

    #[test]
    fn build_defaults_content_type_for_body_methods_only() {
        let post = TestRequest::post("/x").body_json(&json!({"a": 1})).build();
        assert_eq!(
            post.headers().get(header::CONTENT_TYPE).unwrap(),
            JSON_API_MEDIA_TYPE
        );

        let get = TestRequest::get("/x").build();
        assert!(get.headers().get(header::CONTENT_TYPE).is_none());
    }

    #[test]
    fn explicit_header_overrides_the_content_type_default() {
        let request = TestRequest::post("/x")
            .header(header::CONTENT_TYPE, "text/plain")
            .raw_body("not json")
            .build();
        assert_eq!(
            request.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/plain"
        );
    }

    #[test]
    fn send_collects_a_data_document() {
        pollster::block_on(async {
            let response = app().send(TestRequest::get("/ok").build()).await;
            response
                .clone()
                .assert_status(StatusCode::OK)
                .assert_json_api_content_type();
            assert_eq!(response.data()["attributes"]["title"], "Hi");
        });
    }

    #[test]
    fn send_collects_and_asserts_an_error_document() {
        pollster::block_on(async {
            app()
                .send(TestRequest::post("/boom").body_json(&json!({})).build())
                .await
                .assert_status(StatusCode::UNPROCESSABLE_ENTITY)
                .assert_json_api_content_type()
                .assert_error_count(1)
                .assert_error(422)
                .assert_error_pointer("/data/attributes/title");
        });
    }

    #[test]
    fn empty_body_parses_to_none() {
        pollster::block_on(async {
            let response = app().send(TestRequest::get("/empty").build()).await;
            response.clone().assert_status(StatusCode::NO_CONTENT);
            assert!(response.body.is_none());
        });
    }

    #[test]
    #[should_panic(expected = "expected status")]
    fn assert_status_panics_with_a_clear_message_on_mismatch() {
        pollster::block_on(async {
            app()
                .send(TestRequest::get("/ok").build())
                .await
                .assert_status(StatusCode::IM_A_TEAPOT);
        });
    }

    #[test]
    #[should_panic(expected = "expected errors[0].source.pointer")]
    fn assert_error_pointer_panics_on_mismatch() {
        pollster::block_on(async {
            app()
                .send(TestRequest::post("/boom").body_json(&json!({})).build())
                .await
                .assert_error_pointer("/data/attributes/body");
        });
    }
}
