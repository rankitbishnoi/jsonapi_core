//! [`JsonApiError`] — the single error type used both as an extractor
//! `Rejection` and as an error responder.
//!
//! It carries a pre-built JSON:API error [`Response`](http::Response) produced by
//! [`jsonapi_http`], so every error — whether raised by an extractor, a layer,
//! or a handler — renders identically.

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use http::StatusCode;

use jsonapi_core::{ApiError, Error};
use jsonapi_http::{error_response, error_response_for, error_response_for_status, id_conflict};

/// A JSON:API error response, usable both as an axum extractor `Rejection` and
/// as an [`IntoResponse`] error returned from a handler.
#[derive(Debug, Clone)]
pub struct JsonApiError {
    response: http::Response<Bytes>,
}

impl JsonApiError {
    /// Build from a [`jsonapi_core::Error`], mapping it to the correct status and
    /// a JSON:API error document.
    #[must_use]
    pub fn from_core(error: &Error) -> Self {
        Self {
            response: error_response_for(error),
        }
    }

    /// Build from one or more [`ApiError`]s.
    #[must_use]
    pub fn from_api_errors(errors: impl IntoIterator<Item = ApiError>) -> Self {
        Self {
            response: error_response(errors),
        }
    }

    /// Build from a single [`ApiError`].
    #[must_use]
    pub fn from_api_error(error: ApiError) -> Self {
        Self::from_api_errors(std::iter::once(error))
    }

    /// Build a JSON:API error response for a bare HTTP `status` (with optional
    /// human-readable `detail`).
    ///
    /// Used to re-shape errors that originate as a plain status rather than a
    /// [`jsonapi_core::Error`] — e.g. an axum body-size-limit rejection (`413`)
    /// or an unmatched-route fallback (`404`).
    #[must_use]
    pub fn from_status(status: StatusCode, detail: Option<String>) -> Self {
        Self {
            response: error_response_for_status(status, detail),
        }
    }

    /// Build a **409 Conflict** JSON:API error for an id collision (a chosen id
    /// that already exists), with `source.pointer` `/data/id`. Collision
    /// *detection* is the application's job; this produces the error to return.
    #[must_use]
    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::from_api_error(id_conflict(detail))
    }
}

impl IntoResponse for JsonApiError {
    fn into_response(self) -> Response {
        self.response.map(Body::from)
    }
}

impl From<Error> for JsonApiError {
    fn from(error: Error) -> Self {
        Self::from_core(&error)
    }
}

impl From<Box<ApiError>> for JsonApiError {
    fn from(error: Box<ApiError>) -> Self {
        Self::from_api_error(*error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use serde_json::Value;

    /// Read status + JSON body, proving the `map(Body::from)` hop preserves bytes.
    fn read(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let bytes =
            pollster::block_on(axum::body::to_bytes(response.into_body(), usize::MAX)).unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[test]
    fn into_response_uses_mapped_status_and_body() {
        let response = JsonApiError::from_core(&Error::NoAcceptableMediaType).into_response();
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/vnd.api+json")
        );

        let (status, json) = read(response);
        assert_eq!(status, StatusCode::NOT_ACCEPTABLE);
        assert_eq!(json["errors"][0]["status"], "406");
    }

    #[test]
    fn from_boxed_api_error_preserves_status_and_body() {
        let api = ApiError {
            status: Some("422".to_string()),
            ..Default::default()
        };
        let (status, json) = read(JsonApiError::from(Box::new(api)).into_response());
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["errors"][0]["status"], "422");
    }

    #[test]
    fn from_api_errors_aggregates_multiple_into_one_document() {
        // Two 422s aggregate into a single errors document; the shared status
        // becomes the top-level HTTP status.
        let errors = [
            ApiError {
                status: Some("422".to_string()),
                detail: Some("title is required".to_string()),
                ..Default::default()
            },
            ApiError {
                status: Some("422".to_string()),
                detail: Some("body is required".to_string()),
                ..Default::default()
            },
        ];
        let (status, json) =
            read(JsonApiError::from_api_errors(errors).into_response());
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["errors"].as_array().unwrap().len(), 2);
        assert_eq!(json["errors"][0]["detail"], "title is required");
        assert_eq!(json["errors"][1]["detail"], "body is required");
    }

    #[test]
    fn from_status_builds_error_document_with_status_and_detail() {
        let (status, json) = read(
            JsonApiError::from_status(StatusCode::PAYLOAD_TOO_LARGE, Some("too big".into()))
                .into_response(),
        );
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(json["errors"][0]["status"], "413");
        assert_eq!(json["errors"][0]["detail"], "too big");
    }

    #[test]
    fn from_core_conversion_builds_error_document() {
        let response: JsonApiError = Error::NoAcceptableMediaType.into();
        let (status, json) = read(response.into_response());
        assert_eq!(status, StatusCode::NOT_ACCEPTABLE);
        assert_eq!(json["errors"][0]["status"], "406");
    }
}
