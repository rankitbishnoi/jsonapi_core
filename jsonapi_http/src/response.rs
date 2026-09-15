//! Build JSON:API [`http::Response`]s from documents.
//!
//! The error-document path lives in [`crate::error`]; this module adds the
//! success path — serializing a [`Document`] and setting a `Content-Type` that
//! carries any negotiated `ext` / `profile` media-type parameters.
//!
//! [`json_api_response`] is the shared primitive both this module and
//! [`crate::error`] serialize through, so the wire encoding of every response —
//! success or error — is produced in exactly one place.

use bytes::Bytes;
use http::{HeaderValue, Response, StatusCode, header};
use serde::Serialize;

use jsonapi_core::{Document, FieldsetConfig, JsonApiMediaType, ResourceObject, sparse_filter};

/// Build the `Content-Type` header value for a (typically negotiated) JSON:API
/// media type, including any `ext` / `profile` parameters.
///
/// Reuses [`JsonApiMediaType::to_header_value`] for correct formatting and
/// quote-escaping. The media type's `ext`/`profile` URIs are server-controlled
/// (they come from negotiation against the server's advertised capabilities),
/// so an unrepresentable header value is a server misconfiguration and panics
/// rather than silently emitting a wrong media type.
///
/// ```
/// # use jsonapi_core::JsonApiMediaType;
/// # use jsonapi_http::response::content_type_value;
/// let ct = content_type_value(&JsonApiMediaType::plain());
/// assert_eq!(ct, "application/vnd.api+json");
/// ```
#[must_use]
pub fn content_type_value(media_type: &JsonApiMediaType) -> HeaderValue {
    HeaderValue::try_from(media_type.to_header_value())
        .expect("a negotiated JSON:API media type is always a valid header value")
}

/// Wrap an already-encoded body in a [`Response`]. The `status` is a
/// [`StatusCode`] and `content_type` an already-built [`HeaderValue`], so
/// building the response genuinely cannot fail.
fn response_from_body(
    status: StatusCode,
    content_type: HeaderValue,
    body: Vec<u8>,
) -> Response<Bytes> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(Bytes::from(body))
        .expect("status and content-type are valid")
}

/// The panic message shared by the infallible serialization wrappers. Each keeps
/// a `# Panics` section pointing at its `try_*` counterpart.
const SERIALIZE_PANIC: &str = "serializing a JSON:API document cannot fail for standard resources; use the `try_*` variant to handle a failing custom `Serialize`";

/// Serialize a [`Document`] into a JSON:API success [`Response`] with status
/// `200 OK` and a `Content-Type` reflecting `media_type`.
///
/// For any other status (`201 Created`, `204 No Content`, …) use
/// [`json_api_response`] directly.
///
/// # Panics
/// Panics if `document` cannot be serialized to JSON. This is impossible for
/// resources built from `#[derive(JsonApi)]` on standard field types, but is
/// reachable if an attribute type has a custom `Serialize` that errors or uses
/// non-string map keys. Use [`try_document_response`] to handle that case.
#[must_use]
pub fn document_response<P, I>(
    document: &Document<P, I>,
    media_type: &JsonApiMediaType,
) -> Response<Bytes>
where
    P: ResourceObject,
    I: Serialize,
{
    try_document_response(document, media_type).expect(SERIALIZE_PANIC)
}

/// Fallible counterpart to [`document_response`]: returns the serialization
/// error instead of panicking.
///
/// # Errors
/// Returns the [`serde_json::Error`] if `document` cannot be serialized to JSON
/// (e.g. an attribute with non-string map keys, or a custom `Serialize` that
/// fails).
pub fn try_document_response<P, I>(
    document: &Document<P, I>,
    media_type: &JsonApiMediaType,
) -> Result<Response<Bytes>, serde_json::Error>
where
    P: ResourceObject,
    I: Serialize,
{
    try_json_api_response(StatusCode::OK, content_type_value(media_type), document)
}

/// The shared serialization primitive: encode `document` as JSON and wrap it in
/// a [`Response`] with the given `status` and `Content-Type`.
///
/// # Panics
/// Panics if `document` cannot be serialized to JSON — impossible for standard
/// `#[derive(JsonApi)]` resources, but reachable for an attribute type with a
/// custom `Serialize` that errors or non-string map keys. Use
/// [`try_json_api_response`] to handle that case.
#[must_use]
pub fn json_api_response<P, I>(
    status: StatusCode,
    content_type: HeaderValue,
    document: &Document<P, I>,
) -> Response<Bytes>
where
    P: ResourceObject,
    I: Serialize,
{
    try_json_api_response(status, content_type, document).expect(SERIALIZE_PANIC)
}

/// Fallible counterpart to [`json_api_response`]: encode `document` as JSON and
/// wrap it in a [`Response`], returning the serialization error instead of
/// panicking.
///
/// # Errors
/// Returns the [`serde_json::Error`] if `document` cannot be serialized to JSON.
pub fn try_json_api_response<P, I>(
    status: StatusCode,
    content_type: HeaderValue,
    document: &Document<P, I>,
) -> Result<Response<Bytes>, serde_json::Error>
where
    P: ResourceObject,
    I: Serialize,
{
    let body = serde_json::to_vec(document)?;
    Ok(response_from_body(status, content_type, body))
}

/// Like [`json_api_response`], but applies a sparse-fieldset `fields` filter to
/// the serialized document before writing the body (reusing
/// [`jsonapi_core::sparse_filter`], which trims attributes/relationships on both
/// primary `data` and `included` resources per type while always retaining
/// `type`/`id`).
///
/// This serializes the document to an intermediate [`serde_json::Value`], filters
/// it, then re-serializes — so callers with an empty [`FieldsetConfig`] should
/// prefer [`json_api_response`] to skip the round trip (and preserve the
/// document's own key ordering).
///
/// # Panics
/// Panics if `document` cannot be serialized to JSON, for the same reason as
/// [`json_api_response`]. Use [`try_json_api_response_filtered`] to handle that
/// case.
#[must_use]
pub fn json_api_response_filtered<P, I>(
    status: StatusCode,
    content_type: HeaderValue,
    document: &Document<P, I>,
    fields: &FieldsetConfig,
) -> Response<Bytes>
where
    P: ResourceObject,
    I: Serialize,
{
    try_json_api_response_filtered(status, content_type, document, fields).expect(SERIALIZE_PANIC)
}

/// Fallible counterpart to [`json_api_response_filtered`]: returns the
/// serialization error instead of panicking.
///
/// # Errors
/// Returns the [`serde_json::Error`] if `document` cannot be serialized to JSON.
pub fn try_json_api_response_filtered<P, I>(
    status: StatusCode,
    content_type: HeaderValue,
    document: &Document<P, I>,
    fields: &FieldsetConfig,
) -> Result<Response<Bytes>, serde_json::Error>
where
    P: ResourceObject,
    I: Serialize,
{
    let value = serde_json::to_value(document)?;
    let filtered = sparse_filter(&value, fields);
    let body = serde_json::to_vec(&filtered)?;
    Ok(response_from_body(status, content_type, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonapi_core::Resource;
    use serde_json::Value;

    #[test]
    fn content_type_value_plain() {
        let ct = content_type_value(&JsonApiMediaType::plain());
        assert_eq!(ct, "application/vnd.api+json");
    }

    #[test]
    fn content_type_value_with_ext() {
        let mt = JsonApiMediaType::with_ext(["https://jsonapi.org/ext/atomic"]);
        let ct = content_type_value(&mt);
        assert_eq!(
            ct.to_str().unwrap(),
            "application/vnd.api+json; ext=\"https://jsonapi.org/ext/atomic\""
        );
    }

    #[test]
    fn content_type_value_with_profile() {
        let mt =
            JsonApiMediaType::parse("application/vnd.api+json; profile=\"https://example.com/p\"")
                .unwrap();
        let ct = content_type_value(&mt);
        assert_eq!(
            ct.to_str().unwrap(),
            "application/vnd.api+json; profile=\"https://example.com/p\""
        );
    }

    #[test]
    fn document_response_sets_status_content_type_and_body() {
        let document: Document<Resource> =
            serde_json::from_str(r#"{"data":{"type":"articles","id":"1"}}"#).unwrap();

        let response = document_response(&document, &JsonApiMediaType::plain());

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/vnd.api+json")
        );

        let body: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body["data"]["type"], "articles");
        assert_eq!(body["data"]["id"], "1");
    }

    #[test]
    fn json_api_response_filtered_trims_attributes_but_keeps_type_and_id() {
        let document: Document<Resource> = serde_json::from_str(
            r#"{"data":{"type":"articles","id":"1",
                "attributes":{"title":"Hi","body":"World"}}}"#,
        )
        .unwrap();
        let fields = FieldsetConfig::new().fields("articles", &["title"]);

        let response = json_api_response_filtered(
            StatusCode::OK,
            content_type_value(&JsonApiMediaType::plain()),
            &document,
            &fields,
        );

        let body: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body["data"]["type"], "articles");
        assert_eq!(body["data"]["id"], "1");
        assert_eq!(body["data"]["attributes"]["title"], "Hi");
        assert!(body["data"]["attributes"].get("body").is_none());
    }

    #[test]
    fn json_api_response_honors_custom_status() {
        let document: Document<Resource> =
            serde_json::from_str(r#"{"data":{"type":"articles","id":"1"}}"#).unwrap();

        let response = json_api_response(
            StatusCode::CREATED,
            content_type_value(&JsonApiMediaType::plain()),
            &document,
        );

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[test]
    fn try_json_api_response_returns_ok_with_serialized_body() {
        // The fallible primitive succeeds for a serializable document and wires
        // the status, content-type, and body identically to the infallible one.
        // (The Err path — a document that cannot be serialized — is proven
        // end-to-end at the axum `IntoResponse` boundary, where it degrades to a
        // JSON:API 500 instead of panicking.)
        let document: Document<Resource> =
            serde_json::from_str(r#"{"data":{"type":"articles","id":"1"}}"#).unwrap();

        let response = try_json_api_response(
            StatusCode::OK,
            content_type_value(&JsonApiMediaType::plain()),
            &document,
        )
        .expect("a serializable document must produce Ok");

        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body["data"]["id"], "1");
    }
}
