//! [`JsonApiResponse`] — an [`IntoResponse`] wrapper that serializes a
//! [`Document`] as a JSON:API response via [`jsonapi_http`].

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Serialize;

use jsonapi_core::{Document, FieldsetConfig, JsonApiMediaType, Resource, ResourceObject};
use jsonapi_http::{content_type_value, json_api_response, json_api_response_filtered};

/// A JSON:API success response wrapping a [`Document`].
///
/// Defaults to `200 OK` with a plain `application/vnd.api+json` `Content-Type`.
/// Use [`status`](Self::status) for `201`/`204`/etc. and
/// [`media_type`](Self::media_type) to attach negotiated `ext`/`profile`
/// parameters (typically the [`JsonApiMediaType`] a layer stored in the request
/// extensions).
#[derive(Debug, Clone)]
pub struct JsonApiResponse<P, I = Resource> {
    document: Document<P, I>,
    status: StatusCode,
    media_type: JsonApiMediaType,
    fields: Option<FieldsetConfig>,
}

impl<P, I> JsonApiResponse<P, I> {
    /// Wrap a document as a `200 OK` JSON:API response.
    #[must_use]
    pub fn new(document: Document<P, I>) -> Self {
        Self {
            document,
            status: StatusCode::OK,
            media_type: JsonApiMediaType::plain(),
            fields: None,
        }
    }

    /// Override the HTTP status.
    #[must_use]
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Set the response media type (to carry `ext` / `profile` parameters).
    #[must_use]
    pub fn media_type(mut self, media_type: JsonApiMediaType) -> Self {
        self.media_type = media_type;
        self
    }

    /// Apply a sparse-fieldset [`FieldsetConfig`] (typically `query.fields` from a
    /// [`JsonApiQuery`](crate::JsonApiQuery)) to the outgoing document.
    ///
    /// Filtering trims attributes/relationships on both primary and included
    /// resources per type, always retaining `type`/`id`. An empty config is a
    /// no-op: the document is serialized directly, unchanged, so passing
    /// `query.fields` when the client sent no `fields[...]` parameter is safe.
    #[must_use]
    pub fn fields(mut self, fields: FieldsetConfig) -> Self {
        self.fields = Some(fields);
        self
    }
}

impl<P, I> IntoResponse for JsonApiResponse<P, I>
where
    P: ResourceObject,
    I: Serialize,
{
    fn into_response(self) -> Response {
        let content_type = content_type_value(&self.media_type);
        // Only take the filtering path for a non-empty config: an empty config is
        // a semantic no-op, and skipping it avoids a serialize→Value→serialize
        // round trip (which would also reorder JSON keys).
        let response = match &self.fields {
            Some(fields) if !fields.is_empty() => {
                json_api_response_filtered(self.status, content_type, &self.document, fields)
            }
            _ => json_api_response(self.status, content_type, &self.document),
        };
        response.map(Body::from)
    }
}

impl<P, I> From<Document<P, I>> for JsonApiResponse<P, I> {
    fn from(document: Document<P, I>) -> Self {
        Self::new(document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn sample_document() -> Document<Resource> {
        serde_json::from_str(r#"{"data":{"type":"articles","id":"1"}}"#).unwrap()
    }

    /// Read status + JSON body, proving the `map(Body::from)` hop preserves bytes.
    fn read(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let bytes =
            pollster::block_on(axum::body::to_bytes(response.into_body(), usize::MAX)).unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[test]
    fn into_response_defaults_to_ok_json_api_with_body() {
        let response = JsonApiResponse::new(sample_document()).into_response();
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/vnd.api+json")
        );

        let (status, json) = read(response);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["type"], "articles");
        assert_eq!(json["data"]["id"], "1");
    }

    #[test]
    fn status_override_is_applied_with_intact_body() {
        let response = JsonApiResponse::new(sample_document())
            .status(StatusCode::CREATED)
            .into_response();
        let (status, json) = read(response);
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(json["data"]["id"], "1");
    }

    #[test]
    fn from_document_defaults_to_ok_response() {
        // The `From<Document>` impl (used via `.into()`) yields a 200 response
        // with the document intact.
        let response: JsonApiResponse<Resource> = sample_document().into();
        let (status, json) = read(response.into_response());
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["id"], "1");
    }

    #[test]
    fn collection_document_serializes_as_array() {
        let document: Document<Resource> = serde_json::from_str(
            r#"{"data":[{"type":"articles","id":"1"},{"type":"articles","id":"2"}]}"#,
        )
        .unwrap();
        let (status, json) = read(JsonApiResponse::new(document).into_response());
        assert_eq!(status, StatusCode::OK);
        let data = json["data"].as_array().expect("collection is an array");
        assert_eq!(data.len(), 2);
        assert_eq!(data[1]["id"], "2");
    }

    fn compound_document() -> Document<Resource> {
        serde_json::from_str(
            r#"{"data":{"type":"articles","id":"1",
                "attributes":{"title":"Hi","body":"World"},
                "relationships":{"author":{"data":{"type":"people","id":"9"}}}},
               "included":[{"type":"people","id":"9",
                "attributes":{"name":"Dan","email":"dan@example.com"}}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn fields_trims_primary_attributes_keeping_type_and_id() {
        let config = FieldsetConfig::new().fields("articles", &["title"]);
        let (status, json) =
            read(JsonApiResponse::new(compound_document()).fields(config).into_response());

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["type"], "articles");
        assert_eq!(json["data"]["id"], "1");
        assert_eq!(json["data"]["attributes"]["title"], "Hi");
        assert!(json["data"]["attributes"].get("body").is_none());
        // The unrequested `author` relationship is trimmed too.
        assert!(json["data"].get("relationships").is_none());
    }

    #[test]
    fn fields_trims_included_resource_by_its_own_type_entry() {
        // Primary (articles) and included (people) are each trimmed per their own
        // `fields[type]` entry; `type`/`id` are retained on both.
        let config = FieldsetConfig::new()
            .fields("articles", &["title"])
            .fields("people", &["name"]);
        let (_status, json) =
            read(JsonApiResponse::new(compound_document()).fields(config).into_response());

        assert_eq!(json["data"]["attributes"]["title"], "Hi");
        assert!(json["data"]["attributes"].get("body").is_none());

        assert_eq!(json["included"][0]["type"], "people");
        assert_eq!(json["included"][0]["id"], "9");
        assert_eq!(json["included"][0]["attributes"]["name"], "Dan");
        assert!(json["included"][0]["attributes"].get("email").is_none());
    }

    #[test]
    fn empty_and_absent_fields_produce_the_same_output_as_no_filter() {
        // Baseline: no `.fields(...)` at all.
        let (_s, baseline) = read(JsonApiResponse::new(compound_document()).into_response());

        // An explicitly empty config must be a no-op.
        let (_s, empty) = read(
            JsonApiResponse::new(compound_document())
                .fields(FieldsetConfig::new())
                .into_response(),
        );

        assert_eq!(baseline, empty);
        // And it must still carry every attribute (nothing trimmed).
        assert_eq!(empty["data"]["attributes"]["body"], "World");
        assert_eq!(empty["included"][0]["attributes"]["email"], "dan@example.com");
    }

    #[test]
    fn media_type_adds_profile_parameter() {
        let media = JsonApiMediaType::parse(
            "application/vnd.api+json; profile=\"https://example.com/p\"",
        )
        .unwrap();
        let response = JsonApiResponse::new(sample_document())
            .media_type(media)
            .into_response();
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/vnd.api+json; profile=\"https://example.com/p\"")
        );
    }
}
