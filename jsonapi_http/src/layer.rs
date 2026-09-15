//! `tower` middleware that rejects non-conforming requests before they reach a
//! handler, responding with a JSON:API error document.
//!
//! Because `axum` (and other adapters) are built on `tower`, these layers drop
//! into an adapter's router unchanged — the adapter adds no logic, it just
//! re-exports them.
//!
//! # Body unification
//!
//! A guard either forwards to the inner service (returning its response body) or
//! short-circuits with a synthesized JSON:API error body. Both are unified into
//! a single [`UnsyncBoxBody<Bytes, BoxError>`](UnsyncBoxBody), so the layers
//! compose with any inner service whose response body has `Data = Bytes` and an
//! error convertible to a boxed error (which includes `axum::body::Body`).
//!
//! All three public layers ([`ContentTypeLayer`], [`AcceptLayer`],
//! [`JsonApiLayer`]) are backed by one [`GuardService`] configured differently,
//! so there is a single guard implementation to reason about.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{Request, Response};
use http_body::Body;
use http_body_util::{BodyExt, Full, combinators::UnsyncBoxBody};
use tower::{Layer, Service};

use jsonapi_core::Error;

use crate::error::error_response_for;
use crate::request::{check_content_type, negotiate};

/// Boxed error type shared by every response body a guard produces.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The unified response body type produced by a [`GuardService`].
type ResponseBody = UnsyncBoxBody<Bytes, BoxError>;

// --- Public layers -------------------------------------------------------

/// Rejects requests whose `Content-Type` is not `application/vnd.api+json` with
/// a `415` JSON:API error document (via [`crate::request::check_content_type`]).
#[derive(Clone, Debug, Default)]
pub struct ContentTypeLayer;

impl ContentTypeLayer {
    /// Create a new content-type guard layer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for ContentTypeLayer {
    type Service = GuardService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GuardService {
            inner,
            guard: Guard {
                check_content_type: true,
                accept: None,
            },
        }
    }
}

/// Rejects requests with no acceptable `Accept` media type with a `406` JSON:API
/// error document (via [`crate::request::negotiate`]).
///
/// On success the negotiated [`JsonApiMediaType`](jsonapi_core::JsonApiMediaType)
/// is inserted into the request's extensions. A handler that wants the response
/// `Content-Type` to reflect negotiated `ext`/`profile` must read it from there
/// (e.g. `axum::Extension<JsonApiMediaType>`) and pass it on — a responder's
/// `IntoResponse` cannot see the request, so it cannot apply it automatically.
/// The layer is configured with the server's advertised extensions/profiles via
/// [`AcceptLayer::ext`] / [`AcceptLayer::profile`].
#[derive(Clone, Debug, Default)]
pub struct AcceptLayer {
    ext: Vec<String>,
    profile: Vec<String>,
}

impl AcceptLayer {
    /// Create a new accept-negotiation layer advertising no extensions or
    /// profiles (plain `application/vnd.api+json`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advertise the extension URIs this server supports.
    #[must_use]
    pub fn ext(mut self, uris: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.ext.extend(uris.into_iter().map(Into::into));
        self
    }

    /// Advertise the profile URIs this server supports.
    #[must_use]
    pub fn profile(mut self, uris: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.profile.extend(uris.into_iter().map(Into::into));
        self
    }
}

impl<S> Layer<S> for AcceptLayer {
    type Service = GuardService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GuardService {
            inner,
            guard: Guard {
                check_content_type: false,
                accept: Some(AcceptConfig {
                    ext: self.ext.clone(),
                    profile: self.profile.clone(),
                }),
            },
        }
    }
}

/// Combined guard applying both [`ContentTypeLayer`] and [`AcceptLayer`] in one
/// pass: content type is checked first, then `Accept` is negotiated.
#[derive(Clone, Debug, Default)]
pub struct JsonApiLayer {
    ext: Vec<String>,
    profile: Vec<String>,
}

impl JsonApiLayer {
    /// Create a new combined guard advertising no extensions or profiles.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advertise the extension URIs this server supports.
    #[must_use]
    pub fn ext(mut self, uris: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.ext.extend(uris.into_iter().map(Into::into));
        self
    }

    /// Advertise the profile URIs this server supports.
    #[must_use]
    pub fn profile(mut self, uris: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.profile.extend(uris.into_iter().map(Into::into));
        self
    }
}

impl<S> Layer<S> for JsonApiLayer {
    type Service = GuardService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GuardService {
            inner,
            guard: Guard {
                check_content_type: true,
                accept: Some(AcceptConfig {
                    ext: self.ext.clone(),
                    profile: self.profile.clone(),
                }),
            },
        }
    }
}

// --- The shared guard service -------------------------------------------

#[derive(Clone, Debug)]
struct AcceptConfig {
    ext: Vec<String>,
    profile: Vec<String>,
}

#[derive(Clone, Debug)]
struct Guard {
    check_content_type: bool,
    accept: Option<AcceptConfig>,
}

/// The [`Service`] backing all three JSON:API guard layers. Constructed by the
/// layers' [`Layer::layer`] impls; not created directly.
#[derive(Clone, Debug)]
pub struct GuardService<S> {
    inner: S,
    guard: Guard,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for GuardService<S>
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
        let guard = self.guard.clone();

        Box::pin(async move {
            // Content-Type describes a request body, so only enforce it for
            // body-bearing methods; a GET/DELETE without a body must not 415.
            if guard.check_content_type
                && request_carries_body(req.method())
                && let Err(err) = check_content_type(req.headers())
            {
                return Ok(reject(&err));
            }

            if let Some(accept) = guard.accept {
                let ext: Vec<&str> = accept.ext.iter().map(String::as_str).collect();
                let profile: Vec<&str> = accept.profile.iter().map(String::as_str).collect();
                match negotiate(req.headers(), &ext, &profile) {
                    Ok(media) => {
                        req.extensions_mut().insert(media);
                    }
                    Err(err) => return Ok(reject(&err)),
                }
            }

            let response = inner.call(req).await?;
            Ok(response.map(box_inner))
        })
    }
}

// --- Body helpers --------------------------------------------------------

/// Whether a request method typically carries a body whose `Content-Type`
/// should be validated.
fn request_carries_body(method: &http::Method) -> bool {
    matches!(
        *method,
        http::Method::POST | http::Method::PUT | http::Method::PATCH
    )
}

/// Build a rejection response, boxing its `Bytes` body into [`ResponseBody`].
fn reject(err: &Error) -> Response<ResponseBody> {
    error_response_for(err).map(box_bytes)
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
    use crate::JSON_API_MEDIA_TYPE;
    use http::{StatusCode, header};
    use jsonapi_core::JsonApiMediaType;
    use std::convert::Infallible;
    use tower::ServiceExt;

    /// A `200 OK` inner service. A macro (not a function returning `impl Trait`)
    /// so the concrete closure type — and thus its `Send`/`'static`/`Future:
    /// Send` auto-traits — is visible at each call site, which
    /// [`GuardService`]'s `Service` impl requires.
    macro_rules! ok_service {
        () => {
            tower::service_fn(|_req: Request<Full<Bytes>>| async {
                Ok::<_, Infallible>(Response::new(Full::new(Bytes::from_static(b"ok"))))
            })
        };
    }

    fn request_with(
        method: &str,
        headers: &[(header::HeaderName, &str)],
    ) -> Request<Full<Bytes>> {
        let mut builder = Request::builder().method(method).uri("/articles");
        for (name, value) in headers {
            builder = builder.header(name.clone(), *value);
        }
        builder.body(Full::new(Bytes::new())).unwrap()
    }

    async fn body_string(response: Response<ResponseBody>) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[test]
    fn content_type_layer_passes_conforming_post() {
        pollster::block_on(async {
            let svc = ContentTypeLayer::new().layer(ok_service!());
            let req = request_with("POST", &[(header::CONTENT_TYPE, JSON_API_MEDIA_TYPE)]);
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(body_string(res).await, "ok");
        });
    }

    #[test]
    fn content_type_layer_rejects_wrong_type_post_with_415() {
        pollster::block_on(async {
            let svc = ContentTypeLayer::new().layer(ok_service!());
            let req = request_with("POST", &[(header::CONTENT_TYPE, "application/json")]);
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
            let body = body_string(res).await;
            assert!(body.contains("\"errors\""), "body: {body}");
        });
    }

    #[test]
    fn content_type_layer_skips_bodyless_get() {
        pollster::block_on(async {
            // A GET with no Content-Type must reach the handler, not 415.
            let svc = ContentTypeLayer::new().layer(ok_service!());
            let req = request_with("GET", &[]);
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
        });
    }

    #[test]
    fn accept_layer_rejects_unacceptable_with_406() {
        pollster::block_on(async {
            let svc = AcceptLayer::new().layer(ok_service!());
            let req = request_with("GET", &[(header::ACCEPT, "text/html")]);
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::NOT_ACCEPTABLE);
        });
    }

    #[test]
    fn accept_layer_inserts_negotiated_media_type_into_extensions() {
        pollster::block_on(async {
            // Inner service reflects whether the layer inserted the media type.
            let inner = tower::service_fn(|req: Request<Full<Bytes>>| async move {
                let body = if req.extensions().get::<JsonApiMediaType>().is_some() {
                    "present"
                } else {
                    "absent"
                };
                Ok::<_, Infallible>(Response::new(Full::new(Bytes::from_static(body.as_bytes()))))
            });
            let svc = AcceptLayer::new().layer(inner);
            let req = request_with("GET", &[(header::ACCEPT, JSON_API_MEDIA_TYPE)]);
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(body_string(res).await, "present");
        });
    }

    #[test]
    fn accept_layer_negotiates_advertised_ext_into_extensions() {
        pollster::block_on(async {
            const EXT: &str = "https://jsonapi.org/ext/atomic";
            // Inner service reflects the *negotiated ext* the layer stored, not
            // merely whether some media type is present.
            let inner = tower::service_fn(|req: Request<Full<Bytes>>| async move {
                let ext = req
                    .extensions()
                    .get::<JsonApiMediaType>()
                    .map(|m| m.ext.join(","))
                    .unwrap_or_default();
                Ok::<_, Infallible>(Response::new(Full::new(Bytes::from(ext))))
            });
            let svc = AcceptLayer::new().ext([EXT]).layer(inner);
            let req = request_with(
                "GET",
                &[(header::ACCEPT, &format!("{JSON_API_MEDIA_TYPE}; ext=\"{EXT}\""))],
            );
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(body_string(res).await, EXT);
        });
    }

    #[test]
    fn accept_layer_drops_unadvertised_ext() {
        pollster::block_on(async {
            // Server advertises nothing; a requested ext is dropped, yielding a
            // plain (empty-ext) negotiated media type rather than a 406.
            let inner = tower::service_fn(|req: Request<Full<Bytes>>| async move {
                let ext = req
                    .extensions()
                    .get::<JsonApiMediaType>()
                    .map(|m| m.ext.join(","))
                    .unwrap_or_else(|| "MISSING".to_string());
                Ok::<_, Infallible>(Response::new(Full::new(Bytes::from(ext))))
            });
            let svc = AcceptLayer::new().layer(inner);
            let req = request_with(
                "GET",
                &[(header::ACCEPT, &format!("{JSON_API_MEDIA_TYPE}; ext=\"https://unadvertised\""))],
            );
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(body_string(res).await, "", "requested ext must be dropped");
        });
    }

    #[test]
    fn content_type_layer_enforces_on_put_and_patch() {
        pollster::block_on(async {
            // request_carries_body covers POST/PUT/PATCH; guard each here.
            for method in ["PUT", "PATCH"] {
                let svc = ContentTypeLayer::new().layer(ok_service!());
                let req = request_with(method, &[(header::CONTENT_TYPE, "application/json")]);
                let res = svc.oneshot(req).await.unwrap();
                assert_eq!(
                    res.status(),
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "{method} with wrong content-type must 415"
                );
            }
        });
    }

    #[test]
    fn accept_layer_passes_json_api() {
        pollster::block_on(async {
            let svc = AcceptLayer::new().layer(ok_service!());
            let req = request_with("GET", &[(header::ACCEPT, JSON_API_MEDIA_TYPE)]);
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
        });
    }

    #[test]
    fn json_api_layer_checks_content_type_before_accept() {
        pollster::block_on(async {
            // Bad content type + bad accept on a POST: content-type is checked first, so 415.
            let svc = JsonApiLayer::new().layer(ok_service!());
            let req = request_with(
                "POST",
                &[
                    (header::CONTENT_TYPE, "application/json"),
                    (header::ACCEPT, "text/html"),
                ],
            );
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        });
    }

    #[test]
    fn json_api_layer_passes_fully_conforming_post() {
        pollster::block_on(async {
            let svc = JsonApiLayer::new().layer(ok_service!());
            let req = request_with(
                "POST",
                &[
                    (header::CONTENT_TYPE, JSON_API_MEDIA_TYPE),
                    (header::ACCEPT, JSON_API_MEDIA_TYPE),
                ],
            );
            let res = svc.oneshot(req).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(body_string(res).await, "ok");
        });
    }
}
