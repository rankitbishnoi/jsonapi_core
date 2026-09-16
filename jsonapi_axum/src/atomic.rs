//! Atomic Operations extension responder (feature `atomic-ops`).
//!
//! [`AtomicJsonApiResponse`] wraps a [`jsonapi_core::AtomicResponse`] and renders
//! it as a `200 OK` body with the atomic `Content-Type`
//! (`application/vnd.api+json; ext="https://jsonapi.org/ext/atomic"`) — the
//! counterpart to [`JsonApiResponse`](crate::JsonApiResponse) for the normal case.

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use http::{HeaderValue, StatusCode, header};

use jsonapi_core::{ATOMIC_EXT_URI, AtomicResponse, JsonApiMediaType};

use crate::error::JsonApiError;

/// An [`IntoResponse`] wrapper serializing an [`AtomicResponse`] with the atomic
/// extension `Content-Type`.
///
/// Defaults to `200 OK`. On the (rare) serialization failure it degrades to a
/// JSON:API `500` rather than panicking at the responder boundary, mirroring
/// [`JsonApiResponse`](crate::JsonApiResponse).
#[derive(Debug, Clone)]
pub struct AtomicJsonApiResponse(pub AtomicResponse);

impl From<AtomicResponse> for AtomicJsonApiResponse {
    fn from(response: AtomicResponse) -> Self {
        Self(response)
    }
}

impl IntoResponse for AtomicJsonApiResponse {
    fn into_response(self) -> Response {
        let body = match serde_json::to_vec(&self.0) {
            Ok(bytes) => bytes,
            Err(err) => return JsonApiError::internal(err.to_string()).into_response(),
        };

        let content_type = JsonApiMediaType::with_ext([ATOMIC_EXT_URI]).to_header_value();
        let mut response = Response::new(Body::from(body));
        *response.status_mut() = StatusCode::OK;
        // The header value is assembled from a static ext URI, so it is always a
        // valid ASCII header value; on the impossible parse failure we omit it
        // rather than panic in a responder.
        if let Ok(value) = HeaderValue::from_str(&content_type) {
            response.headers_mut().insert(header::CONTENT_TYPE, value);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonapi_core::AtomicResult;

    fn read(response: Response) -> (StatusCode, String, serde_json::Value) {
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let bytes =
            pollster::block_on(axum::body::to_bytes(response.into_body(), usize::MAX)).unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, content_type, json)
    }

    #[test]
    fn serializes_with_atomic_content_type() {
        let atomic = AtomicResponse {
            results: vec![AtomicResult::default()],
            ..Default::default()
        };
        let (status, content_type, json) = read(AtomicJsonApiResponse(atomic).into_response());

        assert_eq!(status, StatusCode::OK);
        assert!(
            content_type.starts_with("application/vnd.api+json")
                && content_type.contains("ext=\"https://jsonapi.org/ext/atomic\""),
            "content-type was {content_type:?}"
        );
        assert!(json.get("atomic:results").is_some(), "body: {json}");
    }
}
