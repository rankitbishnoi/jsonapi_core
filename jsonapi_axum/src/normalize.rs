//! JSON:API-shape framework-generated error responses.
//!
//! axum emits plain-text responses for the errors it generates itself —
//! unmatched routes (404), method-not-allowed (405), built-in extractor
//! rejections, and `DefaultBodyLimit` (413). A JSON:API client should never
//! receive a `text/plain` body with no error document. This module closes that
//! gap two ways:
//!
//! - [`not_found`] is a [`Router::fallback`](axum::Router::fallback) handler that
//!   turns an unmatched route into a `404` JSON:API error document.
//! - [`NormalizeErrorsLayer`] is a safety-net tower layer that rewrites *any*
//!   response with a `>= 400` status whose `Content-Type` is not
//!   `application/vnd.api+json` into a JSON:API error document carrying the same
//!   status (folding the original plain body into `detail`). Responses that are
//!   already JSON:API pass through untouched, so our own error docs are never
//!   double-wrapped.
//!
//! The reusable status→document synthesis lives in
//! [`jsonapi_http::error_response_for_status`]; this module only wires it into
//! axum, mirroring the body-unification approach of `jsonapi_http`'s guard
//! layers.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{HeaderMap, Request, Response, StatusCode, header};
use http_body::Body;
use http_body_util::{BodyExt, Full, combinators::UnsyncBoxBody};
use tower::{Layer, Service};

use jsonapi_http::{JSON_API_MEDIA_TYPE, error_response_for_status};

use crate::error::JsonApiError;

/// Boxed error type shared by every response body the layer produces (mirrors
/// `jsonapi_http::layer`).
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The unified response body type produced by a [`NormalizeErrorsService`].
type ResponseBody = UnsyncBoxBody<Bytes, BoxError>;

/// A [`Router::fallback`](axum::Router::fallback) handler responding `404 Not
/// Found` as a JSON:API error document for any unmatched route.
///
/// ```no_run
/// use axum::Router;
/// use jsonapi_axum::not_found;
///
/// let app: Router = Router::new().fallback(not_found);
/// ```
pub async fn not_found() -> JsonApiError {
    JsonApiError::from_status(
        StatusCode::NOT_FOUND,
        Some("no route matched the request URI".to_string()),
    )
}

/// Tower layer that rewrites non-JSON:API error responses into JSON:API error
/// documents. See the [module docs](self) for the full contract.
#[derive(Clone, Debug, Default)]
pub struct NormalizeErrorsLayer;

impl NormalizeErrorsLayer {
    /// Create a new normalize-errors layer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for NormalizeErrorsLayer {
    type Service = NormalizeErrorsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        NormalizeErrorsService { inner }
    }
}

/// The [`Service`] backing [`NormalizeErrorsLayer`]. Constructed by the layer's
/// [`Layer::layer`] impl; not created directly.
#[derive(Clone, Debug)]
pub struct NormalizeErrorsService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for NormalizeErrorsService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Body<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Response = Response<ResponseBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        // Take the ready inner service; leave a clone behind (the tower
        // clone-and-replace idiom that preserves readiness).
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let response = inner.call(req).await?;
            Ok(normalize(response).await)
        })
    }
}

/// Pass a response through unchanged, or — for a non-JSON:API `>= 400` response —
/// rewrite it into a JSON:API error document carrying the same status.
async fn normalize<B>(response: Response<B>) -> Response<ResponseBody>
where
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    let status = response.status();
    let is_error = status.is_client_error() || status.is_server_error();

    // Success responses and responses already shaped as JSON:API pass through
    // untouched (the latter guard prevents double-wrapping our own error docs).
    if !is_error || is_json_api(response.headers()) {
        return response.map(box_inner);
    }

    // Fold the original plain body into `detail` so the cause is not lost.
    let detail = collect_detail(response.into_body()).await;
    error_response_for_status(status, detail).map(box_bytes)
}

/// Whether a response is already a JSON:API document (its `Content-Type` starts
/// with `application/vnd.api+json`, tolerating any `ext`/`profile` parameters).
fn is_json_api(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|content_type| content_type.starts_with(JSON_API_MEDIA_TYPE))
}

/// Buffer a response body and return its text as an error `detail`, if any.
///
/// A failure to read the body, an empty body, or a non-UTF-8 body yields `None`
/// rather than a misleading detail — the synthesized error still carries the
/// correct status and title.
async fn collect_detail<B>(body: B) -> Option<String>
where
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    let bytes = body.collect().await.ok()?.to_bytes();
    let text = std::str::from_utf8(&bytes).ok()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Box a `Bytes` payload into the unified response body. `Full`'s error is
/// [`Infallible`](std::convert::Infallible), so the map closure is unreachable.
fn box_bytes(bytes: Bytes) -> ResponseBody {
    Full::new(bytes)
        .map_err(|never| match never {})
        .boxed_unsync()
}

/// Box an inner service's response body into the unified response body.
fn box_inner<B>(body: B) -> ResponseBody
where
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    body.map_err(Into::into).boxed_unsync()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body as AxumBody;
    use axum::routing::get;
    use serde_json::Value;
    use tower::ServiceExt;

    /// Build the router-under-test: one `GET /articles` route, a `not_found`
    /// fallback, and the normalize layer on top.
    fn app() -> Router {
        async fn list() -> &'static str {
            "ok"
        }
        async fn boom() -> (StatusCode, &'static str) {
            (StatusCode::INTERNAL_SERVER_ERROR, "kaboom")
        }
        async fn already_json_api() -> JsonApiError {
            JsonApiError::from_status(StatusCode::CONFLICT, Some("dup".into()))
        }
        Router::new()
            .route("/articles", get(list))
            .route("/boom", get(boom))
            .route("/conflict", get(already_json_api))
            .fallback(not_found)
            .layer(NormalizeErrorsLayer::new())
    }

    fn get_request(uri: &str) -> Request<AxumBody> {
        Request::builder().uri(uri).body(AxumBody::empty()).unwrap()
    }

    fn post_request(uri: &str) -> Request<AxumBody> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .body(AxumBody::empty())
            .unwrap()
    }

    async fn read(response: Response<AxumBody>) -> (StatusCode, String, Value) {
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, content_type, value)
    }

    #[test]
    fn unmatched_route_yields_404_json_api_document() {
        pollster::block_on(async {
            let response = app().oneshot(get_request("/nope")).await.unwrap();
            let (status, content_type, json) = read(response).await;
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(content_type, JSON_API_MEDIA_TYPE);
            assert_eq!(json["errors"][0]["status"], "404");
        });
    }

    #[test]
    fn wrong_method_yields_405_json_api_document() {
        pollster::block_on(async {
            // `/articles` exists for GET; a POST is 405, which axum emits as
            // plain text and the layer normalizes.
            let response = app().oneshot(post_request("/articles")).await.unwrap();
            let (status, content_type, json) = read(response).await;
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(content_type, JSON_API_MEDIA_TYPE);
            assert_eq!(json["errors"][0]["status"], "405");
        });
    }

    #[test]
    fn plain_text_500_is_normalized_and_folds_body_into_detail() {
        pollster::block_on(async {
            let response = app().oneshot(get_request("/boom")).await.unwrap();
            let (status, content_type, json) = read(response).await;
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(content_type, JSON_API_MEDIA_TYPE);
            assert_eq!(json["errors"][0]["status"], "500");
            assert_eq!(json["errors"][0]["detail"], "kaboom");
        });
    }

    #[test]
    fn existing_json_api_error_passes_through_unchanged() {
        pollster::block_on(async {
            // A handler-produced JSON:API error must not be re-wrapped: it keeps
            // its single error object (with the original detail), not a synthesized
            // one derived from the serialized body.
            let response = app().oneshot(get_request("/conflict")).await.unwrap();
            let (status, content_type, json) = read(response).await;
            assert_eq!(status, StatusCode::CONFLICT);
            assert_eq!(content_type, JSON_API_MEDIA_TYPE);
            assert_eq!(json["errors"].as_array().unwrap().len(), 1);
            assert_eq!(json["errors"][0]["status"], "409");
            assert_eq!(json["errors"][0]["detail"], "dup");
        });
    }

    #[test]
    fn success_response_passes_through_untouched() {
        pollster::block_on(async {
            let response = app().oneshot(get_request("/articles")).await.unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(status, StatusCode::OK);
            assert_eq!(&bytes[..], b"ok");
        });
    }
}
