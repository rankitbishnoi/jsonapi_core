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
use jsonapi_http::{
    ApiErrorExt, ApiErrors, error_response, error_response_for, error_response_for_status,
    id_conflict, with_status,
};

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

    /// Build a **404 Not Found** JSON:API error with a human-readable `detail`.
    #[must_use]
    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::from_api_error(with_status(StatusCode::NOT_FOUND).detail(detail))
    }

    /// Build a **403 Forbidden** JSON:API error with a human-readable `detail`.
    #[must_use]
    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::from_api_error(with_status(StatusCode::FORBIDDEN).detail(detail))
    }

    /// Build a **500 Internal Server Error** JSON:API error.
    ///
    /// The `detail` you pass is treated as internal, potentially sensitive text:
    /// it is **not** placed in the response body unless the `debug-errors`
    /// feature is enabled, so raw error strings never leak to clients by
    /// default. Log the raw message in your application if you need it. The
    /// response always carries the generic "Internal Server Error" title and no
    /// `detail`.
    #[must_use]
    pub fn internal(detail: impl Into<String>) -> Self {
        let _detail = detail.into();
        let error = with_status(StatusCode::INTERNAL_SERVER_ERROR);
        #[cfg(feature = "debug-errors")]
        let error = error.detail(_detail);
        Self::from_api_error(error)
    }
}

/// Convert a domain error into a [`JsonApiError`].
///
/// Implement this for your own error types to map them to a JSON:API response,
/// then use [`ResultExt::or_json_api`] for a clean `?` in handlers.
///
/// The idiomatic impl builds an [`ApiError`] with [`with_status`] (adding
/// `detail`, `source`, etc. via [`ApiErrorExt`](crate::ApiErrorExt)), then wraps
/// it with [`JsonApiError::from_api_error`]:
///
/// ```
/// use jsonapi_axum::{ApiError, ApiErrorExt, IntoJsonApiError, JsonApiError, with_status};
/// use http::StatusCode;
///
/// enum AppError {
///     NotFound(String),
///     Invalid { field: String },
/// }
///
/// impl IntoJsonApiError for AppError {
///     fn into_json_api_error(self) -> JsonApiError {
///         let error: ApiError = match self {
///             AppError::NotFound(what) => {
///                 with_status(StatusCode::NOT_FOUND).detail(format!("no such {what}"))
///             }
///             AppError::Invalid { field } => with_status(StatusCode::UNPROCESSABLE_ENTITY)
///                 .detail("invalid field")
///                 .pointer(format!("/data/attributes/{field}")),
///         };
///         JsonApiError::from_api_error(error)
///     }
/// }
///
/// // In a handler: `repo.load(id).or_json_api()?` yields a `JsonApiError` on the error arm.
/// ```
///
/// # Why not a blanket `From` impl?
///
/// A blanket `impl<E: IntoJsonApiError> From<E> for JsonApiError` would overlap
/// with the existing concrete [`From<jsonapi_core::Error>`](JsonApiError) and
/// `From<Box<ApiError>>` impls — nothing stops a downstream crate from also
/// implementing `IntoJsonApiError` for those types, so the coherence checker
/// rejects the blanket impl. Keep the explicit `.into_json_api_error()` /
/// `.or_json_api()` path instead; do not "simplify" it into a blanket `From`.
pub trait IntoJsonApiError {
    /// Map `self` into a [`JsonApiError`] response.
    fn into_json_api_error(self) -> JsonApiError;
}

/// Extension over [`Result`] to convert a domain error into a [`JsonApiError`]
/// at a `?`, for any `E: IntoJsonApiError`.
pub trait ResultExt<T> {
    /// Map the error arm through [`IntoJsonApiError::into_json_api_error`].
    ///
    /// # Errors
    /// Propagates the original error, converted to a [`JsonApiError`].
    // by-value rejection so it flows into a handler's `Result<_, JsonApiError>`
    // via `?`; boxing would break that path (see `JsonApi::require_id`).
    #[allow(clippy::result_large_err)]
    fn or_json_api(self) -> Result<T, JsonApiError>;
}

impl<T, E: IntoJsonApiError> ResultExt<T> for Result<T, E> {
    #[allow(clippy::result_large_err)]
    fn or_json_api(self) -> Result<T, JsonApiError> {
        self.map_err(IntoJsonApiError::into_json_api_error)
    }
}

#[cfg(feature = "anyhow")]
#[cfg_attr(docsrs, doc(cfg(feature = "anyhow")))]
impl From<anyhow::Error> for JsonApiError {
    /// Any `anyhow::Error` maps to a **500**; the raw message is scrubbed from
    /// the body unless `debug-errors` is enabled (see [`JsonApiError::internal`]).
    fn from(error: anyhow::Error) -> Self {
        Self::internal(error.to_string())
    }
}

#[cfg(feature = "sqlx")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx")))]
impl From<sqlx::Error> for JsonApiError {
    /// `RowNotFound` maps to **404**; every other variant to **500** (message
    /// scrubbed unless `debug-errors` is enabled).
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::RowNotFound => Self::not_found("resource not found"),
            other => Self::internal(other.to_string()),
        }
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

impl From<ApiErrors> for JsonApiError {
    fn from(errors: ApiErrors) -> Self {
        Self::from_api_errors(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use serde_json::Value;

    /// Read the status and JSON body from a response.
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
        let (status, json) = read(JsonApiError::from_api_errors(errors).into_response());
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

    #[cfg(not(feature = "debug-errors"))]
    #[test]
    fn internal_does_not_leak_raw_message() {
        let (status, json) =
            read(JsonApiError::internal("db url: postgres://user:hunter2@host/db").into_response());
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["errors"][0]["status"], "500");
        // Default build (no `debug-errors`): the raw message must be absent.
        let body = json.to_string();
        assert!(
            !body.contains("hunter2"),
            "raw internal message leaked: {body}"
        );
        assert!(json["errors"][0]["detail"].is_null());
    }

    #[cfg(feature = "debug-errors")]
    #[test]
    fn internal_includes_raw_message_when_debug_errors_enabled() {
        let (status, json) =
            read(JsonApiError::internal("db url: postgres://user:hunter2@host/db").into_response());
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            json["errors"][0]["detail"],
            "db url: postgres://user:hunter2@host/db"
        );
    }

    #[test]
    fn or_json_api_maps_domain_error_to_chosen_status() {
        struct Forbidden;
        impl IntoJsonApiError for Forbidden {
            fn into_json_api_error(self) -> JsonApiError {
                JsonApiError::forbidden("not your resource")
            }
        }

        #[allow(clippy::result_large_err)]
        fn handler() -> Result<(), JsonApiError> {
            Err(Forbidden).or_json_api()?;
            Ok(())
        }

        let (status, json) = read(handler().unwrap_err().into_response());
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["errors"][0]["status"], "403");
        assert_eq!(json["errors"][0]["detail"], "not your resource");
    }

    #[test]
    fn not_found_builds_404_with_detail() {
        let (status, json) = read(JsonApiError::not_found("article 99 missing").into_response());
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["errors"][0]["detail"], "article 99 missing");
    }

    #[cfg(feature = "anyhow")]
    #[test]
    fn anyhow_error_maps_to_500() {
        let err: JsonApiError = anyhow::anyhow!("boom").into();
        let (status, json) = read(err.into_response());
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["errors"][0]["status"], "500");
    }

    #[cfg(feature = "sqlx")]
    #[test]
    fn sqlx_row_not_found_maps_to_404_others_500() {
        let (status, _) = read(JsonApiError::from(sqlx::Error::RowNotFound).into_response());
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) =
            read(JsonApiError::from(sqlx::Error::Protocol("bad packet".into())).into_response());
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }
}
