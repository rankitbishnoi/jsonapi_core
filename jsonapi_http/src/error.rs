//! Map [`jsonapi_core::Error`] to HTTP status codes and JSON:API error
//! documents.
//!
//! This is the foundation every other module builds on: extractors and
//! responders all funnel their failures through [`error_response`] so the wire
//! format of an error is identical no matter where in the pipeline it was
//! raised.

use bytes::Bytes;
use http::{HeaderValue, Response, StatusCode};

use jsonapi_core::{ApiError, Document, Error, ErrorLinks, ErrorSource, Link, Meta, Resource};

use crate::JSON_API_MEDIA_TYPE;
use crate::response::json_api_response;

/// Map a [`jsonapi_core::Error`] to the HTTP status code it should produce.
///
/// The mapping follows JSON:API v1.1 semantics:
///
/// - **400 Bad Request** — malformed body/query/document structure.
/// - **406 Not Acceptable** — no acceptable media type in `Accept`.
/// - **409 Conflict** — resource `type` on the wire conflicts with the target
///   (JSON:API's prescribed status for a type mismatch on write).
/// - **415 Unsupported Media Type** — bad or unsupported `Content-Type`.
/// - **422 Unprocessable Entity** — semantic validation (missing required
///   attribute).
/// - **500 Internal Server Error** — server-side resolution faults, and any
///   future [`Error`] variant not yet mapped (the enum is `#[non_exhaustive]`,
///   so a new variant defaults to 500 rather than silently becoming a
///   misleading 4xx).
#[must_use]
pub fn status_for(err: &Error) -> StatusCode {
    match err {
        // --- Client errors: malformed request body / query / document ---
        Error::Json(_)
        | Error::QueryParse { .. }
        | Error::Structure(_)
        | Error::InvalidIncludePath { .. }
        | Error::InvalidMemberName { .. }
        | Error::InvalidAtomicOperation { .. }
        | Error::MalformedRelationship { .. }
        | Error::IncludedRefMissing { .. }
        | Error::UnexpectedDocumentShape { .. }
        | Error::MediaTypeParse(_) => StatusCode::BAD_REQUEST,

        // --- Semantic validation ---
        Error::MissingAttribute { .. } => StatusCode::UNPROCESSABLE_ENTITY,

        // --- Resource `type` conflict on write ---
        Error::TypeMismatch { .. } => StatusCode::CONFLICT,

        // --- Content negotiation ---
        Error::MediaTypeMismatch { .. } | Error::UnsupportedMediaTypeParam { .. } => {
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        }
        Error::NoAcceptableMediaType | Error::AllMediaTypesUnsupportedParams => {
            StatusCode::NOT_ACCEPTABLE
        }

        // --- Server-side resolution faults ---
        Error::RegistryLookup { .. }
        | Error::NullRelationship
        | Error::RelationshipCardinalityMismatch { .. }
        | Error::LidNotIndexed => StatusCode::INTERNAL_SERVER_ERROR,

        // `Error` is #[non_exhaustive]: a future variant defaults to 500.
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Convert a [`jsonapi_core::Error`] into a single JSON:API [`ApiError`].
///
/// Sets `status`, a human-readable `title` (the HTTP reason phrase), and
/// `detail` (the error's `Display`). Where the variant identifies the source of
/// the problem, `source` is populated:
///
/// - query-parameter errors set `source.parameter`;
/// - document-body errors that carry a `location` set `source.pointer` as an
///   RFC 6901 JSON pointer (e.g. `/data/attributes/title`, `/data/type`,
///   `/data/relationships/author`).
#[must_use]
pub fn to_api_error(err: &Error) -> ApiError {
    let status = status_for(err);

    ApiError {
        status: Some(status.as_u16().to_string()),
        title: status.canonical_reason().map(str::to_string),
        detail: Some(err.to_string()),
        source: source_for(err),
        ..Default::default()
    }
}

/// Derive the JSON:API `source` object for an error, if the variant identifies
/// where the problem is.
fn source_for(err: &Error) -> Option<ErrorSource> {
    let pointer = match err {
        Error::QueryParse { param, .. } => {
            return Some(ErrorSource {
                parameter: Some(param.clone()),
                ..Default::default()
            });
        }
        // `location` is the resource path; the offending value is its `type`.
        Error::TypeMismatch { location, .. } => format!("{}/type", location_to_pointer(location)),
        // `location` is the resource path; the offending value is the attribute.
        Error::MissingAttribute {
            attribute,
            location,
            ..
        } => format!("{}/attributes/{attribute}", location_to_pointer(location)),
        // `location` is the resource path; the offending value is the relationship.
        Error::MalformedRelationship { name, location, .. } => {
            format!("{}/relationships/{name}", location_to_pointer(location))
        }
        // `location` is already the full relationship path.
        Error::IncludedRefMissing { location, .. } => location_to_pointer(location),
        _ => return None,
    };

    Some(ErrorSource {
        pointer: Some(pointer),
        ..Default::default()
    })
}

/// Convert a `jsonapi_core` dotted/bracketed `location` (e.g.
/// `data[1].relationships.author`) into an RFC 6901 JSON pointer
/// (`/data/1/relationships/author`).
///
/// `.` and `[` become pointer separators, `]` is dropped, and the reference
/// tokens `~` / `/` are escaped as `~0` / `~1` per RFC 6901.
fn location_to_pointer(location: &str) -> String {
    let mut pointer = String::with_capacity(location.len() + 1);
    pointer.push('/');
    for ch in location.chars() {
        match ch {
            '.' | '[' => pointer.push('/'),
            ']' => {}
            '~' => pointer.push_str("~0"),
            '/' => pointer.push_str("~1"),
            other => pointer.push(other),
        }
    }
    pointer
}

/// Build a JSON:API error-document [`Response`] from one or more [`ApiError`]s.
///
/// The response body is a JSON:API `errors` document and the `Content-Type` is
/// `application/vnd.api+json`. The top-level HTTP status is chosen from the
/// member errors' `status` fields:
///
/// - all errors share one status → that status,
/// - otherwise, if any is a 5xx → `500`,
/// - otherwise → `400`.
///
/// (This mirrors JSON:API's guidance to use the most generally applicable code
/// when several problems are reported at once.)
#[must_use]
pub fn error_response<E>(errors: E) -> Response<Bytes>
where
    E: IntoIterator<Item = ApiError>,
{
    let errors: Vec<ApiError> = errors.into_iter().collect();
    let status = top_level_status(&errors);
    let document: Document<Resource> = Document::errors(errors);
    json_api_response(
        status,
        HeaderValue::from_static(JSON_API_MEDIA_TYPE),
        &document,
    )
}

/// Convenience: build an error [`Response`] directly from a
/// [`jsonapi_core::Error`].
#[must_use]
pub fn error_response_for(err: &Error) -> Response<Bytes> {
    error_response(std::iter::once(to_api_error(err)))
}

/// Build a single [`ApiError`] carrying a bare HTTP `status`, with `title` set to
/// the status's canonical reason phrase and an optional human-readable `detail`.
///
/// This is the synthesis primitive for errors that originate as a plain HTTP
/// status rather than a [`jsonapi_core::Error`] — e.g. a framework-generated
/// `404`/`405`/`413` that an adapter needs to re-shape as a JSON:API document.
#[must_use]
pub fn api_error_for_status(status: StatusCode, detail: Option<String>) -> ApiError {
    ApiError {
        status: Some(status.as_u16().to_string()),
        title: status.canonical_reason().map(str::to_string),
        detail,
        ..Default::default()
    }
}

/// Build a JSON:API error-document [`Response`] carrying `status` (and optional
/// `detail`). The top-level HTTP status matches `status` because the single
/// member error carries it (see [`error_response`]).
#[must_use]
pub fn error_response_for_status(status: StatusCode, detail: Option<String>) -> Response<Bytes> {
    error_response(std::iter::once(api_error_for_status(status, detail)))
}

/// Start building an [`ApiError`] for the given HTTP `status`.
///
/// Sets the `status` member to the numeric string (e.g. `"422"`) and `title` to
/// the status's canonical reason phrase (`"Unprocessable Entity"`) when `status`
/// is a recognised HTTP code. Chain the [`ApiErrorExt`] setters to fill in the
/// rest; a later [`ApiErrorExt::title`] overrides the default.
///
/// This is the fluent counterpart to [`api_error_for_status`]: prefer it when
/// hand-building an error in a handler.
///
/// Taking a typed [`StatusCode`] (rather than a bare `u16`) makes it impossible
/// to build an error for a nonsense code like `99` or `1000`.
///
/// ```
/// use jsonapi_http::{with_status, ApiErrorExt};
/// use http::StatusCode;
/// let err = with_status(StatusCode::UNPROCESSABLE_ENTITY)
///     .pointer("/data/attributes/title")
///     .detail("must not be empty");
/// assert_eq!(err.status.as_deref(), Some("422"));
/// assert_eq!(err.title.as_deref(), Some("Unprocessable Entity"));
/// assert_eq!(
///     err.source.unwrap().pointer.as_deref(),
///     Some("/data/attributes/title")
/// );
/// ```
#[must_use]
pub fn with_status(status: StatusCode) -> ApiError {
    ApiError {
        status: Some(status.as_u16().to_string()),
        title: status.canonical_reason().map(str::to_string),
        ..Default::default()
    }
}

/// Chainable, `#[must_use]` setters for [`ApiError`], letting an error be built
/// fluently from [`with_status`].
///
/// The setters live here — as an extension trait — rather than as inherent
/// methods on [`ApiError`] so that `jsonapi_core` stays a pure data model with
/// no HTTP dependency. Import the trait to bring the setters into scope.
pub trait ApiErrorExt: Sized {
    /// Set `source.pointer` — an RFC 6901 JSON pointer to the offending value
    /// (e.g. `/data/attributes/title`).
    #[must_use]
    fn pointer(self, pointer: impl Into<String>) -> Self;
    /// Set `detail` — a human-readable explanation of this occurrence.
    #[must_use]
    fn detail(self, detail: impl Into<String>) -> Self;
    /// Set `code` — an application-specific error code.
    #[must_use]
    fn code(self, code: impl Into<String>) -> Self;
    /// Set `title`, overriding the default canonical reason from [`with_status`].
    #[must_use]
    fn title(self, title: impl Into<String>) -> Self;
    /// Set `id` — a unique identifier for this particular occurrence.
    #[must_use]
    fn id(self, id: impl Into<String>) -> Self;
    /// Set error-level `meta`.
    #[must_use]
    fn meta(self, meta: Meta) -> Self;
    /// Set `links.about` — a bare-URL link to further details about the error.
    #[must_use]
    fn about_link(self, href: impl Into<String>) -> Self;
}

impl ApiErrorExt for ApiError {
    fn pointer(mut self, pointer: impl Into<String>) -> Self {
        self.source.get_or_insert_with(ErrorSource::default).pointer = Some(pointer.into());
        self
    }

    fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    fn code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    fn meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    fn about_link(mut self, href: impl Into<String>) -> Self {
        self.links.get_or_insert_with(ErrorLinks::default).about = Some(Link::String(href.into()));
        self
    }
}

/// A small accumulator for building a JSON:API `errors` list — collect one
/// [`ApiError`] per problem (e.g. per invalid attribute), then turn the batch
/// into a single errors document.
///
/// It is `IntoIterator`, so it flows straight into [`error_response`] or
/// `JsonApiError::from_api_errors`; `jsonapi_axum` also provides
/// `From<ApiErrors> for JsonApiError` for `.into()`.
#[derive(Debug, Clone, Default)]
pub struct ApiErrors(Vec<ApiError>);

impl ApiErrors {
    /// An empty accumulator.
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Append an error.
    pub fn push(&mut self, error: ApiError) {
        self.0.push(error);
    }

    /// `true` when no errors have been collected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The number of collected errors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl From<ApiErrors> for Vec<ApiError> {
    fn from(errors: ApiErrors) -> Self {
        errors.0
    }
}

impl IntoIterator for ApiErrors {
    type Item = ApiError;
    type IntoIter = std::vec::IntoIter<ApiError>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<ApiError> for ApiErrors {
    fn from_iter<I: IntoIterator<Item = ApiError>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Choose the top-level HTTP status for a set of error objects.
fn top_level_status(errors: &[ApiError]) -> StatusCode {
    let codes: Vec<StatusCode> = errors
        .iter()
        .map(|e| {
            e.status
                .as_deref()
                .and_then(|s| s.parse::<u16>().ok())
                .and_then(|n| StatusCode::from_u16(n).ok())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
        })
        .collect();

    match codes.split_first() {
        // An empty errors list is a caller bug; 500 is the safe signal.
        None => StatusCode::INTERNAL_SERVER_ERROR,
        Some((first, rest)) => {
            if rest.iter().all(|code| code == first) {
                *first
            } else if codes.iter().any(StatusCode::is_server_error) {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::BAD_REQUEST
            }
        }
    }
}

/// Stamp a correlation `id` onto every member of a JSON:API `errors` array that
/// does not already carry an `id`, in place.
///
/// This is the reusable half of request-id correlation: an adapter buffers
/// an error response body, runs this, and rebuilds it. It is deliberately
/// conservative:
///
/// - only documents with an `errors` array are touched — a `data`/`meta`
///   document is left exactly as it was;
/// - an error member that already has an `id` (e.g. one a handler set) is never
///   overwritten.
///
/// Returns `true` when at least one `id` was added (so a caller can skip
/// re-serializing an unchanged document).
pub fn stamp_error_ids(document: &mut serde_json::Value, id: &str) -> bool {
    let Some(errors) = document.get_mut("errors").and_then(|e| e.as_array_mut()) else {
        return false;
    };

    let mut changed = false;
    for error in errors {
        if let Some(object) = error.as_object_mut()
            && !object.contains_key("id")
        {
            object.insert("id".to_string(), serde_json::Value::String(id.to_string()));
            changed = true;
        }
    }
    changed
}

/// Byte-oriented [`stamp_error_ids`]: parse `body` as JSON, stamp missing
/// `errors[].id` with `id`, and re-serialize.
///
/// Returns the input **unchanged** (a borrowed [`Cow`](std::borrow::Cow)) when
/// the body is not JSON, is not an `errors` document, or already carries an `id`
/// on every member — so it is a safe no-op on non-error and non-JSON bodies.
#[must_use]
pub fn stamp_error_ids_in_bytes<'a>(body: &'a [u8], id: &str) -> std::borrow::Cow<'a, [u8]> {
    use std::borrow::Cow;

    let Ok(mut document) = serde_json::from_slice::<serde_json::Value>(body) else {
        return Cow::Borrowed(body);
    };
    if stamp_error_ids(&mut document, id) {
        match serde_json::to_vec(&document) {
            Ok(bytes) => Cow::Owned(bytes),
            // Re-serializing a value we just parsed should never fail; keep the
            // original body rather than dropping the response if it somehow does.
            Err(_) => Cow::Borrowed(body),
        }
    } else {
        Cow::Borrowed(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonapi_core::Cardinality;
    use serde_json::Value;

    /// Every current `Error` variant maps to the documented status. Guards
    /// against a new variant silently inheriting the 500 fallback unnoticed.
    #[test]
    fn status_mapping_is_exhaustive_for_known_variants() {
        let json_err = serde_json::from_str::<u8>("\"x\"").unwrap_err();

        let cases: Vec<(Error, StatusCode)> = vec![
            (Error::Json(json_err), StatusCode::BAD_REQUEST),
            (
                Error::InvalidMemberName {
                    name: "a b".into(),
                    reason: "space".into(),
                },
                StatusCode::BAD_REQUEST,
            ),
            (
                Error::RegistryLookup {
                    r#type: "people".into(),
                    id: "9".into(),
                },
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (Error::NullRelationship, StatusCode::INTERNAL_SERVER_ERROR),
            (
                Error::RelationshipCardinalityMismatch {
                    expected: Cardinality::ToOne,
                },
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (Error::LidNotIndexed, StatusCode::INTERNAL_SERVER_ERROR),
            (
                Error::MediaTypeMismatch {
                    expected: "application/vnd.api+json".into(),
                    got: "text/plain".into(),
                },
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (
                Error::UnsupportedMediaTypeParam {
                    param: "charset".into(),
                },
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (Error::MediaTypeParse("bad".into()), StatusCode::BAD_REQUEST),
            (Error::NoAcceptableMediaType, StatusCode::NOT_ACCEPTABLE),
            (
                Error::AllMediaTypesUnsupportedParams,
                StatusCode::NOT_ACCEPTABLE,
            ),
            (
                Error::QueryParse {
                    param: "page[size]".into(),
                    reason: "not an int".into(),
                },
                StatusCode::BAD_REQUEST,
            ),
            (Error::Structure("bad".into()), StatusCode::BAD_REQUEST),
            (
                Error::InvalidIncludePath {
                    path: "a.b".into(),
                    segment: "b".into(),
                    type_name: "a".into(),
                },
                StatusCode::BAD_REQUEST,
            ),
            (
                Error::InvalidAtomicOperation {
                    index: 0,
                    reason: "dangling lid".into(),
                },
                StatusCode::BAD_REQUEST,
            ),
            (
                Error::UnexpectedDocumentShape {
                    expected: "single resource",
                    found: "errors document",
                },
                StatusCode::BAD_REQUEST,
            ),
            (
                Error::TypeMismatch {
                    expected: "articles",
                    got: "people".into(),
                    location: "data".into(),
                },
                StatusCode::CONFLICT,
            ),
            (
                Error::MalformedRelationship {
                    name: "author".into(),
                    location: "data".into(),
                    reason: "missing data".into(),
                },
                StatusCode::BAD_REQUEST,
            ),
            (
                Error::MissingAttribute {
                    resource_type: "articles",
                    attribute: "title",
                    location: "data".into(),
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                Error::IncludedRefMissing {
                    name: "author".into(),
                    r#type: "people".into(),
                    id: "9".into(),
                    location: "data.relationships.author".into(),
                },
                StatusCode::BAD_REQUEST,
            ),
        ];

        for (err, expected) in cases {
            assert_eq!(status_for(&err), expected, "wrong status for {err:?}");
        }
    }

    #[test]
    fn to_api_error_sets_status_title_detail() {
        let err = Error::NoAcceptableMediaType;
        let api = to_api_error(&err);

        assert_eq!(api.status.as_deref(), Some("406"));
        assert_eq!(api.title.as_deref(), Some("Not Acceptable"));
        assert_eq!(api.detail.as_deref(), Some(err.to_string().as_str()));
        assert!(api.source.is_none());
    }

    #[test]
    fn to_api_error_populates_source_parameter_for_query_errors() {
        let err = Error::QueryParse {
            param: "sort".into(),
            reason: "unknown field".into(),
        };
        let api = to_api_error(&err);

        assert_eq!(api.status.as_deref(), Some("400"));
        assert_eq!(
            api.source.as_ref().and_then(|s| s.parameter.as_deref()),
            Some("sort")
        );
    }

    #[test]
    fn to_api_error_sets_pointer_for_missing_attribute() {
        let err = Error::MissingAttribute {
            resource_type: "articles",
            attribute: "title",
            location: "data".into(),
        };
        let api = to_api_error(&err);

        assert_eq!(api.status.as_deref(), Some("422"));
        assert_eq!(
            api.source.as_ref().and_then(|s| s.pointer.as_deref()),
            Some("/data/attributes/title")
        );
    }

    #[test]
    fn to_api_error_sets_pointer_for_type_mismatch() {
        let err = Error::TypeMismatch {
            expected: "articles",
            got: "people".into(),
            location: "data[3]".into(),
        };
        let api = to_api_error(&err);

        assert_eq!(api.status.as_deref(), Some("409"));
        assert_eq!(
            api.source.as_ref().and_then(|s| s.pointer.as_deref()),
            Some("/data/3/type")
        );
    }

    #[test]
    fn to_api_error_sets_pointer_for_malformed_relationship() {
        let err = Error::MalformedRelationship {
            name: "author".into(),
            location: "data".into(),
            reason: "relationship value must be an object".into(),
        };
        let api = to_api_error(&err);

        assert_eq!(
            api.source.as_ref().and_then(|s| s.pointer.as_deref()),
            Some("/data/relationships/author")
        );
    }

    #[test]
    fn to_api_error_uses_relationship_path_for_included_ref_missing() {
        let err = Error::IncludedRefMissing {
            name: "author".into(),
            r#type: "people".into(),
            id: "9".into(),
            location: "data.relationships.author".into(),
        };
        let api = to_api_error(&err);

        assert_eq!(
            api.source.as_ref().and_then(|s| s.pointer.as_deref()),
            Some("/data/relationships/author")
        );
    }

    #[test]
    fn location_to_pointer_handles_nested_paths() {
        assert_eq!(location_to_pointer("data"), "/data");
        assert_eq!(location_to_pointer("data[3]"), "/data/3");
        assert_eq!(location_to_pointer("included[2]"), "/included/2");
        assert_eq!(
            location_to_pointer("data[1].relationships.author"),
            "/data/1/relationships/author"
        );
    }

    #[test]
    fn location_to_pointer_escapes_rfc6901_reference_tokens() {
        assert_eq!(location_to_pointer("data.foo~bar"), "/data/foo~0bar");
        assert_eq!(location_to_pointer("data.foo/bar"), "/data/foo~1bar");
    }

    #[test]
    fn error_response_sets_status_content_type_and_body() {
        let response = error_response_for(&Error::NoAcceptableMediaType);

        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some(JSON_API_MEDIA_TYPE)
        );

        let body: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body["errors"][0]["status"], "406");
        assert!(body["data"].is_null());
    }

    #[test]
    fn error_response_for_status_carries_status_title_and_detail() {
        let response = error_response_for_status(
            StatusCode::PAYLOAD_TOO_LARGE,
            Some("body too large".to_string()),
        );

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body["errors"][0]["status"], "413");
        assert_eq!(body["errors"][0]["title"], "Payload Too Large");
        assert_eq!(body["errors"][0]["detail"], "body too large");
    }

    #[test]
    fn api_error_for_status_omits_detail_when_none() {
        let api = api_error_for_status(StatusCode::NOT_FOUND, None);
        assert_eq!(api.status.as_deref(), Some("404"));
        assert_eq!(api.title.as_deref(), Some("Not Found"));
        assert!(api.detail.is_none());
    }

    #[test]
    fn top_level_status_all_same() {
        let errors = vec![
            ApiError {
                status: Some("404".into()),
                ..Default::default()
            },
            ApiError {
                status: Some("404".into()),
                ..Default::default()
            },
        ];
        assert_eq!(top_level_status(&errors), StatusCode::NOT_FOUND);
    }

    #[test]
    fn top_level_status_mixed_4xx_collapses_to_400() {
        let errors = vec![
            ApiError {
                status: Some("404".into()),
                ..Default::default()
            },
            ApiError {
                status: Some("422".into()),
                ..Default::default()
            },
        ];
        assert_eq!(top_level_status(&errors), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn top_level_status_any_5xx_collapses_to_500() {
        let errors = vec![
            ApiError {
                status: Some("400".into()),
                ..Default::default()
            },
            ApiError {
                status: Some("503".into()),
                ..Default::default()
            },
        ];
        assert_eq!(top_level_status(&errors), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn with_status_sets_numeric_status_and_canonical_title() {
        let err = with_status(StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.status.as_deref(), Some("422"));
        assert_eq!(err.title.as_deref(), Some("Unprocessable Entity"));
        assert!(err.source.is_none());
    }

    #[test]
    fn with_status_leaves_title_absent_for_unknown_code() {
        let err = with_status(StatusCode::from_u16(799).unwrap());
        assert_eq!(err.status.as_deref(), Some("799"));
        assert!(err.title.is_none());
    }

    #[test]
    fn ext_setters_populate_expected_fields() {
        let mut meta = Meta::new();
        meta.insert("trace".into(), serde_json::json!("abc"));
        let err = with_status(StatusCode::UNPROCESSABLE_ENTITY)
            .pointer("/data/attributes/title")
            .detail("must not be empty")
            .code("blank")
            .title("Blank title")
            .id("err-1")
            .meta(meta)
            .about_link("https://example.com/errors/blank");

        assert_eq!(err.status.as_deref(), Some("422"));
        assert_eq!(err.title.as_deref(), Some("Blank title"));
        assert_eq!(err.detail.as_deref(), Some("must not be empty"));
        assert_eq!(err.code.as_deref(), Some("blank"));
        assert_eq!(err.id.as_deref(), Some("err-1"));
        assert_eq!(
            err.source.as_ref().unwrap().pointer.as_deref(),
            Some("/data/attributes/title")
        );
        assert_eq!(
            err.meta.as_ref().unwrap()["trace"],
            serde_json::json!("abc")
        );
        assert_eq!(
            err.links.unwrap().about,
            Some(Link::String("https://example.com/errors/blank".into()))
        );
    }

    #[test]
    fn api_errors_accumulates_and_aggregates_into_one_document() {
        let mut errors = ApiErrors::new();
        assert!(errors.is_empty());
        for field in ["title", "body", "author"] {
            errors.push(
                with_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .pointer(format!("/data/attributes/{field}"))
                    .detail(format!("{field} is required")),
            );
        }
        assert_eq!(errors.len(), 3);

        let response = error_response(errors);
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let json: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(json["errors"].as_array().unwrap().len(), 3);
        assert_eq!(
            json["errors"][2]["source"]["pointer"],
            "/data/attributes/author"
        );
    }

    #[test]
    fn stamp_error_ids_fills_missing_ids_and_preserves_existing() {
        let mut doc = serde_json::json!({
            "errors": [
                { "status": "422", "detail": "a" },
                { "id": "kept", "status": "422", "detail": "b" }
            ]
        });
        assert!(stamp_error_ids(&mut doc, "req-1"));
        assert_eq!(doc["errors"][0]["id"], "req-1");
        assert_eq!(doc["errors"][1]["id"], "kept");
    }

    #[test]
    fn stamp_error_ids_ignores_a_data_document() {
        let mut doc = serde_json::json!({ "data": { "type": "articles", "id": "1" } });
        assert!(!stamp_error_ids(&mut doc, "req-1"));
        assert!(doc.get("errors").is_none());
        assert_eq!(doc["data"]["id"], "1");
    }

    #[test]
    fn stamp_error_ids_in_bytes_stamps_error_documents() {
        let body = br#"{"errors":[{"status":"404"}]}"#;
        let out = stamp_error_ids_in_bytes(body, "req-9");
        let json: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["errors"][0]["id"], "req-9");
    }

    #[test]
    fn stamp_error_ids_in_bytes_is_a_noop_on_non_json_and_data_bodies() {
        // Non-JSON bytes are returned byte-for-byte, borrowed.
        let plain = b"not json at all";
        assert!(matches!(
            stamp_error_ids_in_bytes(plain, "req-1"),
            std::borrow::Cow::Borrowed(b) if b == plain
        ));

        // A data document parses but has no `errors` array → unchanged, borrowed.
        let data = br#"{"data":{"type":"articles","id":"1"}}"#;
        assert!(matches!(
            stamp_error_ids_in_bytes(data, "req-1"),
            std::borrow::Cow::Borrowed(_)
        ));
    }
}
