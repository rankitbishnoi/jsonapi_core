//! Relationship-endpoint payloads (G6).
//!
//! Endpoints like `/articles/1/relationships/author` operate on **resource
//! identifier objects** (`{type, id}`), not full resources: the request body is
//! `{ "data": <linkage> }` where linkage is `null`, a single identifier, or an
//! array of identifiers. This module provides:
//!
//! - [`JsonApiToOne`] — extracts a to-one linkage (`Option<ResourceIdentifier>`,
//!   `null` clearing it), rejecting an array payload.
//! - [`JsonApiToMany`] — extracts a to-many linkage (`Vec<ResourceIdentifier>`),
//!   rejecting `null`/object payloads.
//! - [`RelationshipResponse`] — a responder for a relationship document
//!   (linkage plus optional `links`/`meta`).
//!
//! These handle the **payload** only. The write *semantics* are the handler's to
//! apply: on a to-many relationship, `POST` **appends** to the set, `PATCH`
//! **replaces** the whole set, and `DELETE` **removes** the given members
//! (per JSON:API). This module does not infer or enforce that — it just gives the
//! handler the typed linkage to act on.

use axum::body::Body;
use axum::extract::{FromRequest, Request};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use http::{StatusCode, header};
use serde::Serialize;

use jsonapi_core::{
    ApiError, ErrorSource, JsonApiMediaType, Links, Meta, RelationshipData, ResourceIdentifier,
};
use jsonapi_http::{check_content_type, content_type_value};

use crate::error::JsonApiError;

/// Build a `400 Bad Request` JSON:API error for a malformed relationship body,
/// with `source.pointer` `/data`.
fn bad_linkage(detail: impl Into<String>) -> JsonApiError {
    JsonApiError::from_api_error(ApiError {
        status: Some("400".to_string()),
        title: Some("Bad Request".to_string()),
        detail: Some(detail.into()),
        source: Some(ErrorSource {
            pointer: Some("/data".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    })
}

/// Validate the content type, buffer the body, and parse its `data` member into
/// a [`RelationshipData`].
async fn linkage_from_request<S>(req: Request, state: &S) -> Result<RelationshipData, JsonApiError>
where
    S: Send + Sync,
{
    check_content_type(req.headers()).map_err(|err| JsonApiError::from_core(&err))?;

    let bytes = Bytes::from_request(req, state).await.map_err(|rejection| {
        JsonApiError::from_status(rejection.status(), Some(rejection.body_text()))
    })?;

    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|err| bad_linkage(err.to_string()))?;
    let data = value
        .get("data")
        .ok_or_else(|| bad_linkage("relationship document must have a `data` member"))?;

    serde_json::from_value::<RelationshipData>(data.clone())
        .map_err(|err| bad_linkage(format!("malformed relationship linkage: {err}")))
}

/// Extractor for a **to-one** relationship endpoint payload: `null` (clear) or a
/// single [`ResourceIdentifier`]. An array payload is rejected with `400`.
#[derive(Debug, Clone)]
pub struct JsonApiToOne(pub Option<ResourceIdentifier>);

impl<S> FromRequest<S> for JsonApiToOne
where
    S: Send + Sync,
{
    type Rejection = JsonApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match linkage_from_request(req, state).await? {
            RelationshipData::ToOne(identifier) => Ok(JsonApiToOne(identifier)),
            RelationshipData::ToMany(_) => Err(bad_linkage(
                "expected a to-one relationship linkage (an object or null), got an array",
            )),
            // `RelationshipData` is #[non_exhaustive]; any future shape is not a
            // valid to-one linkage.
            _ => Err(bad_linkage("unsupported relationship linkage for a to-one endpoint")),
        }
    }
}

/// Extractor for a **to-many** relationship endpoint payload: an array of
/// [`ResourceIdentifier`]s (possibly empty). A `null`/object payload is rejected
/// with `400`.
#[derive(Debug, Clone)]
pub struct JsonApiToMany(pub Vec<ResourceIdentifier>);

impl<S> FromRequest<S> for JsonApiToMany
where
    S: Send + Sync,
{
    type Rejection = JsonApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match linkage_from_request(req, state).await? {
            RelationshipData::ToMany(identifiers) => Ok(JsonApiToMany(identifiers)),
            RelationshipData::ToOne(_) => Err(bad_linkage(
                "expected a to-many relationship linkage (an array), got an object or null",
            )),
            _ => Err(bad_linkage("unsupported relationship linkage for a to-many endpoint")),
        }
    }
}

/// Borrowing serialization form for a relationship document.
#[derive(Serialize)]
struct RelationshipDocRepr<'a> {
    data: &'a RelationshipData,
    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<&'a Links>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<&'a Meta>,
}

/// A responder for a relationship document: top-level linkage plus optional
/// `links` (`self`/`related`) and `meta`, e.g. the response to a
/// `GET`/`PATCH`/`POST`/`DELETE` on `/articles/1/relationships/author`.
#[derive(Debug, Clone)]
pub struct RelationshipResponse {
    data: RelationshipData,
    links: Option<Links>,
    meta: Option<Meta>,
    status: StatusCode,
    media_type: JsonApiMediaType,
}

impl RelationshipResponse {
    /// Wrap relationship linkage as a `200 OK` relationship document.
    #[must_use]
    pub fn new(data: RelationshipData) -> Self {
        Self {
            data,
            links: None,
            meta: None,
            status: StatusCode::OK,
            media_type: JsonApiMediaType::plain(),
        }
    }

    /// Attach relationship `links` (typically
    /// [`links::relationship_links`](jsonapi_core::links::relationship_links)).
    #[must_use]
    pub fn links(mut self, links: Links) -> Self {
        self.links = Some(links);
        self
    }

    /// Attach relationship `meta`.
    #[must_use]
    pub fn meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Override the HTTP status (e.g. `204 No Content` semantics are usually a
    /// bare status instead, but `200` with the linkage is common).
    #[must_use]
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Set the response media type (to carry negotiated `ext`/`profile`).
    #[must_use]
    pub fn media_type(mut self, media_type: JsonApiMediaType) -> Self {
        self.media_type = media_type;
        self
    }
}

impl IntoResponse for RelationshipResponse {
    fn into_response(self) -> Response {
        let repr = RelationshipDocRepr {
            data: &self.data,
            links: self.links.as_ref(),
            meta: self.meta.as_ref(),
        };
        let body = serde_json::to_vec(&repr)
            .expect("serializing a relationship document cannot fail");
        Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, content_type_value(&self.media_type))
            .body(Body::from(body))
            .expect("status and content-type are valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::post;
    use http::Request as HttpRequest;
    use jsonapi_core::Identity;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    const JSON_API: &str = "application/vnd.api+json";

    async fn status_and_json(response: Response) -> (StatusCode, Value) {
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

    fn post_body(uri: &str, body: Value) -> Request {
        HttpRequest::builder()
            .method("POST")
            .uri(uri)
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[test]
    fn to_one_extracts_single_identifier() {
        pollster::block_on(async {
            async fn handler(JsonApiToOne(id): JsonApiToOne) -> String {
                match id {
                    Some(rid) => format!("{}:{}", rid.r#type, rid.identity.as_id().unwrap_or("?")),
                    None => "null".to_string(),
                }
            }
            let app = Router::new().route("/r", post(handler));
            let body = json!({"data": {"type": "people", "id": "9"}});
            let response = app.oneshot(post_body("/r", body)).await.unwrap();
            let (status, _) = status_and_json(response).await;
            assert_eq!(status, StatusCode::OK);
        });
    }

    #[test]
    fn to_one_accepts_null_to_clear() {
        pollster::block_on(async {
            async fn handler(JsonApiToOne(id): JsonApiToOne) -> String {
                if id.is_none() { "cleared" } else { "set" }.to_string()
            }
            let app = Router::new().route("/r", post(handler));
            let response = app
                .oneshot(post_body("/r", json!({ "data": null })))
                .await
                .unwrap();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(&bytes[..], b"cleared");
        });
    }

    #[test]
    fn to_one_rejects_array_with_400() {
        pollster::block_on(async {
            async fn handler(JsonApiToOne(_): JsonApiToOne) -> StatusCode {
                StatusCode::OK
            }
            let app = Router::new().route("/r", post(handler));
            let body = json!({"data": [{"type": "people", "id": "9"}]});
            let response = app.oneshot(post_body("/r", body)).await.unwrap();
            let (status, json) = status_and_json(response).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(json["errors"][0]["source"]["pointer"], "/data");
        });
    }

    #[test]
    fn to_many_extracts_array() {
        pollster::block_on(async {
            async fn handler(JsonApiToMany(ids): JsonApiToMany) -> String {
                ids.len().to_string()
            }
            let app = Router::new().route("/r", post(handler));
            let body = json!({"data": [
                {"type": "tags", "id": "1"},
                {"type": "tags", "id": "2"}
            ]});
            let response = app.oneshot(post_body("/r", body)).await.unwrap();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(&bytes[..], b"2");
        });
    }

    #[test]
    fn to_many_rejects_object_with_400() {
        pollster::block_on(async {
            async fn handler(JsonApiToMany(_): JsonApiToMany) -> StatusCode {
                StatusCode::OK
            }
            let app = Router::new().route("/r", post(handler));
            let body = json!({"data": {"type": "tags", "id": "1"}});
            let response = app.oneshot(post_body("/r", body)).await.unwrap();
            let (status, _) = status_and_json(response).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        });
    }

    #[test]
    fn malformed_identifier_is_400() {
        pollster::block_on(async {
            async fn handler(JsonApiToOne(_): JsonApiToOne) -> StatusCode {
                StatusCode::OK
            }
            let app = Router::new().route("/r", post(handler));
            // identifier missing `id`/`lid`.
            let body = json!({"data": {"type": "people"}});
            let response = app.oneshot(post_body("/r", body)).await.unwrap();
            let (status, _) = status_and_json(response).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        });
    }

    #[test]
    fn missing_data_member_is_400() {
        pollster::block_on(async {
            async fn handler(JsonApiToOne(_): JsonApiToOne) -> StatusCode {
                StatusCode::OK
            }
            let app = Router::new().route("/r", post(handler));
            let response = app
                .oneshot(post_body("/r", json!({"meta": {}})))
                .await
                .unwrap();
            let (status, json) = status_and_json(response).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(
                json["errors"][0]["detail"]
                    .as_str()
                    .unwrap()
                    .contains("`data`")
            );
        });
    }

    #[test]
    fn relationship_response_document_shape() {
        pollster::block_on(async {
            let data = RelationshipData::ToOne(Some(ResourceIdentifier {
                r#type: "people".into(),
                identity: Identity::Id("9".into()),
                meta: None,
            }));
            let links = jsonapi_core::links::relationship_links(
                "https://api.test",
                "articles",
                "1",
                "author",
            );
            let response = RelationshipResponse::new(data).links(links).into_response();

            assert_eq!(
                response
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok()),
                Some(JSON_API)
            );
            let (status, json) = status_and_json(response).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(json["data"]["type"], "people");
            assert_eq!(json["data"]["id"], "9");
            assert_eq!(
                json["links"]["self"],
                "https://api.test/articles/1/relationships/author"
            );
            assert_eq!(
                json["links"]["related"],
                "https://api.test/articles/1/author"
            );
        });
    }

    #[test]
    fn relationship_response_serializes_empty_to_many_as_array() {
        pollster::block_on(async {
            let response = RelationshipResponse::new(RelationshipData::ToMany(vec![])).into_response();
            let (_status, json) = status_and_json(response).await;
            assert!(json["data"].is_array());
            assert_eq!(json["data"].as_array().unwrap().len(), 0);
        });
    }
}
