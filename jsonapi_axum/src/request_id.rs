//! Request-id / error-id correlation.
//!
//! When a client reports "I got an error", an operator must be able to find it
//! in the logs. [`RequestIdLayer`] resolves a correlation id per request, echoes
//! it back as a response header, and stamps it onto every `errors[].id` of a
//! JSON:API error document — so every error response is traceable with zero
//! handler boilerplate.
//!
//! # Id source
//!
//! The layer does **not** reinvent request-id generation. It resolves the id in
//! order:
//!
//! 1. the configured request header (default `x-request-id`) — this is what
//!    `tower-http`'s `SetRequestIdLayer` / `PropagateRequestIdLayer` set, and
//!    what a client or proxy sends;
//! 2. a [`RequestId`] request extension (e.g. set by an outer copy of this
//!    layer);
//! 3. a freshly generated UUID — **only** when the `uuid` feature is enabled and
//!    [`RequestIdLayer::generate`] was called.
//!
//! If none of these yield an id, the layer is a graceful no-op: it never
//! fabricates a non-unique id, sets no header, and leaves the body untouched.
//!
//! # Layer order
//!
//! Install [`RequestIdLayer`] **outermost** relative to
//! [`NormalizeErrorsLayer`](crate::NormalizeErrorsLayer) so it stamps both
//! handler-produced and normalized/synthesized error documents:
//!
//! ```no_run
//! use axum::Router;
//! use jsonapi_axum::{JsonApiLayer, NormalizeErrorsLayer, RequestIdLayer};
//!
//! let app: Router = Router::new()
//!     // ... routes ...
//!     .layer(JsonApiLayer::new())
//!     .layer(NormalizeErrorsLayer::new())
//!     .layer(RequestIdLayer::new()); // outermost
//! ```
//!
//! # Cost
//!
//! Only **error** responses (`status >= 400`) whose `Content-Type` is
//! `application/vnd.api+json` are buffered and re-serialized to inject the id.
//! Success responses (`< 400`) pass through untouched and are never buffered —
//! they may be large or streaming. The response header is always set (cheap).

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::extract::FromRequestParts;
use bytes::Bytes;
use http::request::Parts;
use http::{HeaderMap, HeaderName, HeaderValue, Request, Response, header};
use http_body::Body;
use http_body_util::{BodyExt, Full, combinators::UnsyncBoxBody};
use tower::{Layer, Service};

use jsonapi_http::{JSON_API_MEDIA_TYPE, stamp_error_ids_in_bytes};

/// The default request/response header carrying the correlation id.
const DEFAULT_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Boxed error type shared by every response body the layer produces (mirrors
/// `jsonapi_http::layer` and [`NormalizeErrorsLayer`](crate::NormalizeErrorsLayer)).
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The unified response body type produced by a [`RequestIdService`].
type ResponseBody = UnsyncBoxBody<Bytes, BoxError>;

/// The resolved correlation id.
///
/// [`RequestIdLayer`] inserts this into the request's extensions once resolved,
/// and it doubles as a [`FromRequestParts`] extractor so a handler can read the
/// id — to log it, or to set it explicitly on an [`ApiError`](jsonapi_core::ApiError)
/// via the `.id(..)` setter.
///
/// ```no_run
/// use jsonapi_axum::RequestId;
///
/// async fn handler(RequestId(id): RequestId) -> String {
///     format!("handling {id}")
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // The layer stores the resolved id here; prefer it.
        if let Some(id) = parts.extensions.get::<RequestId>() {
            return Ok(id.clone());
        }
        // Fallback so the extractor still works without the layer: read the
        // default `x-request-id` header directly.
        let from_header = header_id(&parts.headers, &DEFAULT_HEADER).unwrap_or_default();
        Ok(RequestId(from_header))
    }
}

/// Tower layer that resolves a correlation id, echoes it as a response header,
/// and stamps it onto JSON:API error documents. See the [module docs](self).
#[derive(Clone, Debug)]
pub struct RequestIdLayer {
    header_name: HeaderName,
    generate: bool,
}

impl RequestIdLayer {
    /// A layer reading/echoing the default `x-request-id` header, with id
    /// generation off (a no-op when no upstream id is present).
    #[must_use]
    pub fn new() -> Self {
        Self {
            header_name: DEFAULT_HEADER,
            generate: false,
        }
    }

    /// Use a custom header name for both reading the incoming id and echoing it
    /// on the response (instead of `x-request-id`).
    #[must_use]
    pub fn header_name(mut self, name: HeaderName) -> Self {
        self.header_name = name;
        self
    }

    /// Generate a fresh UUID when no upstream id is present (header/extension).
    ///
    /// Requires the `uuid` feature. Without it, a missing id leaves the layer a
    /// no-op.
    #[cfg(feature = "uuid")]
    #[cfg_attr(docsrs, doc(cfg(feature = "uuid")))]
    #[must_use]
    pub fn generate(mut self) -> Self {
        self.generate = true;
        self
    }
}

impl Default for RequestIdLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdService {
            inner,
            header_name: self.header_name.clone(),
            generate: self.generate,
        }
    }
}

/// The [`Service`] backing [`RequestIdLayer`]. Constructed by the layer's
/// [`Layer::layer`] impl; not created directly.
#[derive(Clone, Debug)]
pub struct RequestIdService<S> {
    inner: S,
    header_name: HeaderName,
    generate: bool,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for RequestIdService<S>
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

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        // Take the ready inner service; leave a clone behind (the tower
        // clone-and-replace idiom that preserves readiness).
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let header_name = self.header_name.clone();
        let generate = self.generate;

        Box::pin(async move {
            let id = resolve_id(&req, &header_name, generate);
            // Expose the resolved id to extractors/handlers.
            if let Some(ref id) = id {
                req.extensions_mut().insert(RequestId(id.clone()));
            }

            let response = inner.call(req).await?;
            Ok(apply_id(response, id, &header_name).await)
        })
    }
}

/// Resolve the correlation id: request header → [`RequestId`] extension →
/// optional generated UUID. `None` when no source yields one.
fn resolve_id<B>(req: &Request<B>, header_name: &HeaderName, generate: bool) -> Option<String> {
    if let Some(id) = header_id(req.headers(), header_name) {
        return Some(id);
    }
    if let Some(RequestId(id)) = req.extensions().get::<RequestId>()
        && !id.is_empty()
    {
        return Some(id.clone());
    }
    generate.then(generated_id).flatten()
}

/// Read a non-empty, UTF-8 header value as a trimmed id.
fn header_id(headers: &HeaderMap, header_name: &HeaderName) -> Option<String> {
    headers
        .get(header_name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Generate a fresh id, or `None` when the `uuid` feature is off.
fn generated_id() -> Option<String> {
    #[cfg(feature = "uuid")]
    {
        Some(uuid::Uuid::new_v4().to_string())
    }
    #[cfg(not(feature = "uuid"))]
    {
        None
    }
}

/// Echo the id as a response header and — only for JSON:API error responses —
/// stamp it onto `errors[].id`. A `None` id is a graceful no-op.
async fn apply_id<B>(
    mut response: Response<B>,
    id: Option<String>,
    header_name: &HeaderName,
) -> Response<ResponseBody>
where
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    let Some(id) = id else {
        return response.map(box_inner);
    };

    // Always echo the id (cheap; header only).
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert(header_name.clone(), value);
    }

    // Only buffer/re-serialize JSON:API error responses. Success responses
    // (< 400) pass through untouched — never buffered.
    let status = response.status();
    let is_error = status.is_client_error() || status.is_server_error();
    if !is_error || !is_json_api(response.headers()) {
        return response.map(box_inner);
    }

    let (parts, body) = response.into_parts();
    let buffered = body
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .unwrap_or_default();
    let stamped = stamp_error_ids_in_bytes(&buffered, &id).into_owned();
    Response::from_parts(parts, box_bytes(Bytes::from(stamped)))
}

/// Whether a response is a JSON:API document (its `Content-Type` starts with
/// `application/vnd.api+json`, tolerating any `ext`/`profile` parameters).
fn is_json_api(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|content_type| content_type.starts_with(JSON_API_MEDIA_TYPE))
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
    use crate::error::JsonApiError;
    use axum::Router;
    use axum::body::Body as AxumBody;
    use axum::routing::get;
    use http::StatusCode;
    use serde_json::Value;
    use tower::ServiceExt;

    fn app() -> Router {
        async fn ok() -> &'static str {
            "ok-body"
        }
        async fn boom() -> JsonApiError {
            JsonApiError::not_found("missing")
        }
        Router::new()
            .route("/ok", get(ok))
            .route("/boom", get(boom))
            .layer(RequestIdLayer::new())
    }

    #[test]
    fn error_response_gets_id_in_header_and_error_document() {
        pollster::block_on(async {
            let request = Request::builder()
                .uri("/boom")
                .header("x-request-id", "req-abc")
                .body(AxumBody::empty())
                .unwrap();
            let response = app().oneshot(request).await.unwrap();

            assert_eq!(
                response
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok()),
                Some("req-abc")
            );
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let json: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(json["errors"][0]["id"], "req-abc");
        });
    }

    #[test]
    fn success_response_is_unchanged_but_carries_the_header() {
        pollster::block_on(async {
            let request = Request::builder()
                .uri("/ok")
                .header("x-request-id", "req-ok")
                .body(AxumBody::empty())
                .unwrap();
            let response = app().oneshot(request).await.unwrap();

            assert_eq!(
                response
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok()),
                Some("req-ok")
            );
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            // Byte-for-byte unchanged: not buffered/re-serialized.
            assert_eq!(status, StatusCode::OK);
            assert_eq!(&bytes[..], b"ok-body");
        });
    }

    #[test]
    fn no_id_source_with_generation_off_is_a_noop() {
        pollster::block_on(async {
            let request = Request::builder()
                .uri("/boom")
                .body(AxumBody::empty())
                .unwrap();
            let response = app().oneshot(request).await.unwrap();

            assert!(response.headers().get("x-request-id").is_none());
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let json: Value = serde_json::from_slice(&bytes).unwrap();
            // No id was fabricated onto the error document.
            assert!(json["errors"][0].get("id").is_none());
        });
    }

    #[test]
    fn custom_header_name_is_read_and_echoed() {
        pollster::block_on(async {
            let router = Router::new()
                .route(
                    "/boom",
                    get(|| async { JsonApiError::not_found("missing") }),
                )
                .layer(
                    RequestIdLayer::new().header_name(HeaderName::from_static("x-correlation-id")),
                );
            let request = Request::builder()
                .uri("/boom")
                .header("x-correlation-id", "corr-1")
                .body(AxumBody::empty())
                .unwrap();
            let response = router.oneshot(request).await.unwrap();

            assert_eq!(
                response
                    .headers()
                    .get("x-correlation-id")
                    .and_then(|v| v.to_str().ok()),
                Some("corr-1")
            );
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let json: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(json["errors"][0]["id"], "corr-1");
        });
    }

    #[test]
    fn request_id_extractor_reads_the_resolved_id() {
        pollster::block_on(async {
            async fn echo(RequestId(id): RequestId) -> String {
                id
            }
            let router = Router::new()
                .route("/whoami", get(echo))
                .layer(RequestIdLayer::new());
            let request = Request::builder()
                .uri("/whoami")
                .header("x-request-id", "req-xyz")
                .body(AxumBody::empty())
                .unwrap();
            let response = router.oneshot(request).await.unwrap();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(&bytes[..], b"req-xyz");
        });
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn generation_stamps_a_uuid_when_no_upstream_id() {
        pollster::block_on(async {
            let router = Router::new()
                .route(
                    "/boom",
                    get(|| async { JsonApiError::not_found("missing") }),
                )
                .layer(RequestIdLayer::new().generate());
            let request = Request::builder()
                .uri("/boom")
                .body(AxumBody::empty())
                .unwrap();
            let response = router.oneshot(request).await.unwrap();

            let header_id = response
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            assert!(header_id.is_some());
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let json: Value = serde_json::from_slice(&bytes).unwrap();
            // The generated id is echoed in the header and stamped on the error.
            assert_eq!(json["errors"][0]["id"], header_id.unwrap());
        });
    }
}
