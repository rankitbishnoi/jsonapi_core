//! axum extractors for JSON:API requests.
//!
//! Each extractor is a thin binding: it reads the relevant part of the request
//! and delegates to [`jsonapi_http`], turning failures into a [`JsonApiError`]
//! rejection.

use std::convert::Infallible;
use std::marker::PhantomData;
use std::sync::Arc;

use axum::extract::{FromRef, FromRequest, FromRequestParts, Request};
use bytes::Bytes;
use http::request::Parts;
use serde::de::DeserializeOwned;

use jsonapi_core::{Document, JsonApiMediaType, PrimaryData, Query, ResourceObject, TypeRegistry};
use jsonapi_http::{
    ClientIdPolicy, check_client_id, check_content_type, check_id_matches, deserialize_body,
    parse_query,
};

use crate::error::JsonApiError;

/// The response media type negotiated by [`AcceptLayer`](crate::AcceptLayer),
/// read back out of the request extensions where the layer stored it.
///
/// A responder's `IntoResponse` cannot see the request, so a handler that wants
/// the response `Content-Type` to reflect the negotiated `ext`/`profile`
/// parameters extracts this and passes it to
/// [`JsonApiResponse::media_type`](crate::JsonApiResponse::media_type):
///
/// ```no_run
/// use jsonapi_axum::{NegotiatedMediaType, JsonApiResponse};
/// # use jsonapi_axum::{Document, Resource};
/// async fn handler(NegotiatedMediaType(media): NegotiatedMediaType) -> JsonApiResponse<Resource> {
///     # let document: Document<Resource> = todo!();
///     JsonApiResponse::new(document).media_type(media)
/// }
/// ```
///
/// When no [`AcceptLayer`](crate::AcceptLayer) ran (nothing stored an extension),
/// it falls back to [`JsonApiMediaType::plain`], so it never fails.
#[derive(Debug, Clone)]
pub struct NegotiatedMediaType(pub JsonApiMediaType);

impl<S> FromRequestParts<S> for NegotiatedMediaType
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let media = parts
            .extensions
            .get::<JsonApiMediaType>()
            .cloned()
            .unwrap_or_else(JsonApiMediaType::plain);
        Ok(NegotiatedMediaType(media))
    }
}

/// Typed JSON:API request-body extractor. Validates the `Content-Type`, buffers
/// the body, and deserializes a [`Document<T>`], rejecting with a JSON:API error
/// document (415 / 400 / 409 / 422) on failure.
///
/// # PATCH
///
/// This is also the PATCH extractor: define a companion resource whose patchable
/// members are [`Field<T>`](jsonapi_core::Field) and use `JsonApi<ArticlePatch>`.
/// Absent members deserialize to `Field::Absent` (leave unchanged), `null` to
/// `Field::Null` (clear), and values to `Field::Set` — and such members are never
/// required, so a partial body does not 422. See `examples/crud_server.rs`.
#[derive(Debug, Clone)]
pub struct JsonApi<T>(pub Document<T>);

impl<T, S> FromRequest<S> for JsonApi<T>
where
    T: ResourceObject + DeserializeOwned + 'static,
    S: Send + Sync,
{
    type Rejection = JsonApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        check_content_type(req.headers()).map_err(|err| JsonApiError::from_core(&err))?;

        let bytes = Bytes::from_request(req, state).await.map_err(|rejection| {
            // Mirror axum's own status for the rejection rather than hardcoding
            // 400: a `DefaultBodyLimit` length-limit rejection reports 413
            // (Payload Too Large), while other read failures stay 400. Reading
            // `.status()` keeps future rejection variants correctly mapped.
            JsonApiError::from_status(rejection.status(), Some(rejection.body_text()))
        })?;

        let document = deserialize_body::<T>(&bytes).map_err(JsonApiError::from)?;
        Ok(JsonApi(document))
    }
}

impl<T: ResourceObject> JsonApi<T> {
    /// The primary resource's `id` from a single-resource body, if any.
    fn body_id(&self) -> Option<&str> {
        match &self.0 {
            Document::Data {
                data: PrimaryData::Single(primary),
                ..
            } => primary.resource_id(),
            _ => None,
        }
    }

    /// Assert the body's `data.id` matches `path_id` (the JSON:API `PATCH` rule),
    /// rejecting with a **409 Conflict** JSON:API error (`source.pointer`
    /// `/data/id`) on mismatch. An absent body id is accepted (identity comes
    /// from the URL). Delegates to [`jsonapi_http::check_id_matches`].
    ///
    /// ```no_run
    /// # use jsonapi_axum::{JsonApi, JsonApiError};
    /// # async fn h(id: String, doc: JsonApi<jsonapi_core::Resource>) -> Result<(), JsonApiError> {
    /// doc.require_id(&id)?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// A 409 [`JsonApiError`] when a present body id differs from `path_id`.
    // The `Err` is the crate's standard by-value rejection so handlers can `?`
    // it straight into their own `Result<_, JsonApiError>`; boxing it would break
    // that ergonomic, so the large-err lint is intentionally allowed here.
    #[allow(clippy::result_large_err)]
    pub fn require_id(&self, path_id: &str) -> Result<(), JsonApiError> {
        check_id_matches(self.body_id(), path_id).map_err(JsonApiError::from)
    }

    /// Apply a [`ClientIdPolicy`] to a create body's client-supplied `id`.
    /// Under [`ClientIdPolicy::Forbid`], a present id is rejected with a
    /// **403 Forbidden** JSON:API error. Delegates to
    /// [`jsonapi_http::check_client_id`].
    ///
    /// # Errors
    /// A 403 [`JsonApiError`] when `policy` is `Forbid` and the body carries an id.
    #[allow(clippy::result_large_err)] // by-value rejection for `?`; see `require_id`
    pub fn check_client_id(&self, policy: ClientIdPolicy) -> Result<(), JsonApiError> {
        check_client_id(policy, self.body_id()).map_err(JsonApiError::from)
    }
}

/// The application's base URL for building `self`/`related` links and the
/// `Location` header, provided from application state via
/// [`FromRef`].
///
/// The base URL is an explicit,
/// app-configured value rather than one derived from the request `Host` /
/// `X-Forwarded-*` headers, which are fragile behind proxies. Store it in your
/// state and implement [`FromRef`] (or make it the state itself); a handler then
/// extracts it directly:
///
/// ```no_run
/// use axum::extract::FromRef;
/// use jsonapi_axum::BaseUrl;
///
/// #[derive(Clone)]
/// struct AppState { base_url: BaseUrl }
/// impl FromRef<AppState> for BaseUrl {
///     fn from_ref(state: &AppState) -> BaseUrl { state.base_url.clone() }
/// }
///
/// async fn handler(BaseUrl(base): BaseUrl) -> String { base }
/// ```
#[derive(Debug, Clone)]
pub struct BaseUrl(pub String);

impl<S> FromRequestParts<S> for BaseUrl
where
    S: Send + Sync,
    Self: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::from_ref(state))
    }
}

/// JSON:API query-parameter extractor (`sort`/`page`/`filter`/`fields`/
/// `include`). Does **not** validate include paths — use
/// [`JsonApiQueryValidated`] for that. Requires no application state.
#[derive(Debug, Clone)]
pub struct JsonApiQuery(pub Query);

impl<S> FromRequestParts<S> for JsonApiQuery
where
    S: Send + Sync,
{
    type Rejection = JsonApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parse_query(&parts.uri).map_err(|err| JsonApiError::from_core(&err))?;
        Ok(JsonApiQuery(query))
    }
}

/// JSON:API query extractor that additionally validates `include` paths against
/// an [`Arc<TypeRegistry>`] pulled from application state, rooted at `T`'s
/// resource type. Opt-in: apps without a registry use [`JsonApiQuery`] instead.
#[derive(Debug, Clone)]
pub struct JsonApiQueryValidated<T> {
    /// The parsed and include-validated query.
    pub query: Query,
    _marker: PhantomData<T>,
}

impl<T, S> FromRequestParts<S> for JsonApiQueryValidated<T>
where
    T: ResourceObject + 'static,
    S: Send + Sync,
    Arc<TypeRegistry>: FromRef<S>,
{
    type Rejection = JsonApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let query = parse_query(&parts.uri).map_err(|err| JsonApiError::from_core(&err))?;

        let registry = Arc::<TypeRegistry>::from_ref(state);
        let root = T::type_info().type_name;
        let includes: Vec<&str> = query.include.iter().map(String::as_str).collect();
        registry
            .validate_include_paths(root, &includes)
            .map_err(|err| JsonApiError::from_core(&err))?;

        Ok(JsonApiQueryValidated {
            query,
            _marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::routing::{get, post};
    use http::{Request, StatusCode, header};
    use jsonapi_core::Relationship;
    use serde_json::Value;
    use tower::ServiceExt;

    #[derive(Debug, Clone, jsonapi_core::JsonApi)]
    #[jsonapi(type = "people")]
    struct Person {
        #[jsonapi(id)]
        id: String,
        #[allow(dead_code)]
        name: String,
    }

    #[derive(Debug, Clone, jsonapi_core::JsonApi)]
    #[jsonapi(type = "articles")]
    struct Article {
        #[jsonapi(id)]
        id: String,
        #[allow(dead_code)]
        title: String,
        #[jsonapi(relationship, type = "people")]
        #[allow(dead_code)]
        author: Relationship<Person>,
    }

    const VALID_ARTICLE: &str = r#"{"data":{"type":"articles","id":"1",
        "attributes":{"title":"Hi"},
        "relationships":{"author":{"data":{"type":"people","id":"9"}}}}}"#;

    fn registry() -> Arc<TypeRegistry> {
        let mut registry = TypeRegistry::new();
        registry.register::<Article>().register::<Person>();
        Arc::new(registry)
    }

    async fn status_of(response: axum::response::Response) -> (StatusCode, Value) {
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, value)
    }

    // --- JsonApi<T> body extractor ---

    fn body_router() -> Router {
        async fn create(JsonApi(_doc): JsonApi<Article>) -> StatusCode {
            StatusCode::CREATED
        }
        Router::new().route("/articles", post(create))
    }

    fn post_request(content_type: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/articles")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body.to_owned()))
            .unwrap()
    }

    #[test]
    fn body_extractor_accepts_valid_document() {
        pollster::block_on(async {
            let response = body_router()
                .oneshot(post_request("application/vnd.api+json", VALID_ARTICLE))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        });
    }

    #[test]
    fn body_extractor_rejects_wrong_content_type_with_415() {
        pollster::block_on(async {
            let response = body_router()
                .oneshot(post_request("application/json", VALID_ARTICLE))
                .await
                .unwrap();
            let (status, _) = status_of(response).await;
            assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        });
    }

    #[test]
    fn body_extractor_maps_body_limit_to_413() {
        pollster::block_on(async {
            // A tiny DefaultBodyLimit layer makes axum's Bytes extractor reject an
            // oversized body with a length-limit rejection; that must surface as a
            // JSON:API 413, not a generic 400.
            use axum::extract::DefaultBodyLimit;
            let router = body_router().layer(DefaultBodyLimit::max(8));
            let big_body = format!(
                r#"{{"data":{{"type":"articles","id":"1","attributes":{{"title":"{}"}}}}}}"#,
                "x".repeat(256)
            );
            let response = router
                .oneshot(post_request("application/vnd.api+json", &big_body))
                .await
                .unwrap();
            let (status, json) = status_of(response).await;
            assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
            assert_eq!(json["errors"][0]["status"], "413");
        });
    }

    #[test]
    fn body_extractor_maps_missing_attribute_to_422_with_pointer() {
        pollster::block_on(async {
            let body = r#"{"data":{"type":"articles","id":"1","attributes":{},
                "relationships":{"author":{"data":{"type":"people","id":"9"}}}}}"#;
            let response = body_router()
                .oneshot(post_request("application/vnd.api+json", body))
                .await
                .unwrap();
            let (status, json) = status_of(response).await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(
                json["errors"][0]["source"]["pointer"],
                "/data/attributes/title"
            );
        });
    }

    #[test]
    fn body_extractor_maps_type_mismatch_to_409_with_pointer() {
        pollster::block_on(async {
            // Wire `type` is "people" but the route deserializes into `Article`.
            let body = r#"{"data":{"type":"people","id":"1","attributes":{"title":"x"},
                "relationships":{"author":{"data":{"type":"people","id":"9"}}}}}"#;
            let response = body_router()
                .oneshot(post_request("application/vnd.api+json", body))
                .await
                .unwrap();
            let (status, json) = status_of(response).await;
            assert_eq!(status, StatusCode::CONFLICT);
            assert_eq!(json["errors"][0]["source"]["pointer"], "/data/type");
        });
    }

    // --- JsonApiQuery (no state) ---

    #[test]
    fn query_extractor_parses_values_through_to_the_handler() {
        pollster::block_on(async {
            // Reflect the parsed query into the body so we assert the values
            // actually reached the handler, not merely that extraction returned OK.
            async fn list(JsonApiQuery(q): JsonApiQuery) -> String {
                let field = &q.sort[0];
                let page_size = q.page.get("size").map(String::as_str).unwrap_or("none");
                format!(
                    "sort={}:{} page_size={page_size}",
                    field.field, field.descending
                )
            }
            let router = Router::new().route("/articles", get(list));
            let request = Request::builder()
                .uri("/articles?sort=-created&page[size]=2")
                .body(Body::empty())
                .unwrap();
            let (status, body) = {
                let response = router.oneshot(request).await.unwrap();
                let status = response.status();
                let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap();
                (status, String::from_utf8(bytes.to_vec()).unwrap())
            };
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body, "sort=created:true page_size=2");
        });
    }

    #[test]
    fn query_extractor_rejects_malformed_param_with_400() {
        pollster::block_on(async {
            async fn list(JsonApiQuery(_q): JsonApiQuery) -> StatusCode {
                StatusCode::OK
            }
            let router = Router::new().route("/articles", get(list));
            // `fields[` has no closing bracket -> QueryParse error.
            let request = Request::builder()
                .uri("/articles?fields[=title")
                .body(Body::empty())
                .unwrap();
            let response = router.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        });
    }

    // --- JsonApiQueryValidated<T> (registry from state) ---

    fn validated_router() -> Router {
        async fn list(_q: JsonApiQueryValidated<Article>) -> StatusCode {
            StatusCode::OK
        }
        Router::new()
            .route("/articles", get(list))
            .with_state(registry())
    }

    #[test]
    fn validated_query_accepts_known_include() {
        pollster::block_on(async {
            let request = Request::builder()
                .uri("/articles?include=author")
                .body(Body::empty())
                .unwrap();
            let response = validated_router().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        });
    }

    #[test]
    fn validated_query_rejects_unknown_include_with_400() {
        pollster::block_on(async {
            let request = Request::builder()
                .uri("/articles?include=bogus")
                .body(Body::empty())
                .unwrap();
            let response = validated_router().oneshot(request).await.unwrap();
            let (status, json) = status_of(response).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(json["errors"][0]["source"]["parameter"], Value::Null);
            assert!(
                json["errors"][0]["detail"]
                    .as_str()
                    .unwrap()
                    .contains("bogus")
            );
        });
    }

    // --- NegotiatedMediaType (G10) ---

    const TEST_PROFILE: &str = "https://example.com/p";

    async fn body_text(response: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[test]
    fn negotiated_media_type_reflects_accept_layer_profile() {
        pollster::block_on(async {
            // Reflect the negotiated profile into the body so we prove the layer's
            // stored media type reached the extractor.
            async fn handler(NegotiatedMediaType(media): NegotiatedMediaType) -> String {
                media.profile.join(",")
            }
            let router = Router::new()
                .route("/x", get(handler))
                .layer(crate::AcceptLayer::new().profile([TEST_PROFILE]));
            let request = Request::builder()
                .uri("/x")
                .header(
                    header::ACCEPT,
                    format!("application/vnd.api+json; profile=\"{TEST_PROFILE}\""),
                )
                .body(Body::empty())
                .unwrap();
            let response = router.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(body_text(response).await, TEST_PROFILE);
        });
    }

    #[test]
    fn negotiated_media_type_without_layer_falls_back_to_plain() {
        pollster::block_on(async {
            async fn handler(NegotiatedMediaType(media): NegotiatedMediaType) -> String {
                (media == JsonApiMediaType::plain()).to_string()
            }
            // No AcceptLayer: nothing stored in extensions.
            let router = Router::new().route("/x", get(handler));
            let request = Request::builder().uri("/x").body(Body::empty()).unwrap();
            let response = router.oneshot(request).await.unwrap();
            assert_eq!(body_text(response).await, "true");
        });
    }

    #[test]
    fn negotiated_media_type_drives_response_content_type_end_to_end() {
        pollster::block_on(async {
            use crate::JsonApiResponse;
            async fn handler(
                NegotiatedMediaType(media): NegotiatedMediaType,
            ) -> JsonApiResponse<jsonapi_core::Resource> {
                let document: Document<jsonapi_core::Resource> =
                    serde_json::from_str(r#"{"data":{"type":"articles","id":"1"}}"#).unwrap();
                JsonApiResponse::new(document).media_type(media)
            }
            let router = Router::new()
                .route("/x", get(handler))
                .layer(crate::AcceptLayer::new().profile([TEST_PROFILE]));
            let request = Request::builder()
                .uri("/x")
                .header(
                    header::ACCEPT,
                    format!("application/vnd.api+json; profile=\"{TEST_PROFILE}\""),
                )
                .body(Body::empty())
                .unwrap();
            let response = router.oneshot(request).await.unwrap();
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap()
                .to_string();
            assert_eq!(
                content_type,
                format!("application/vnd.api+json; profile=\"{TEST_PROFILE}\"")
            );
        });
    }
}
