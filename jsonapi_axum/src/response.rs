//! [`JsonApiResponse`] — an [`IntoResponse`] wrapper that serializes a
//! [`Document`] as a JSON:API response via [`jsonapi_http`].

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use http::{HeaderValue, StatusCode, header};
use serde::Serialize;

use jsonapi_core::{
    Document, DocumentBuilder, FieldsetConfig, JsonApiMediaType, Resource, ResourceObject,
};
use jsonapi_http::{content_type_value, try_json_api_response, try_json_api_response_filtered};

use crate::error::JsonApiError;

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
    location: Option<String>,
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
            location: None,
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

    /// Set the `Location` response header (e.g. a newly created resource's self
    /// link). Pair with [`status`](Self::status), or use [`created`](Self::created)
    /// to set both `201` and `Location` at once.
    #[must_use]
    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Shape a `201 Created` response: sets status `201` and the `Location`
    /// header to `self_link` (typically
    /// [`links::resource_self`](jsonapi_core::links::resource_self) built from a
    /// [`BaseUrl`](crate::BaseUrl)).
    #[must_use]
    pub fn created(self, self_link: impl Into<String>) -> Self {
        self.status(StatusCode::CREATED).location(self_link)
    }
}

impl<P: ResourceObject> JsonApiResponse<P, Resource> {
    /// Wrap a single primary resource as a `200 OK` data document — shorthand for
    /// `JsonApiResponse::new(DocumentBuilder::single(primary).build())` for the
    /// common no-includes handler.
    #[must_use]
    pub fn single(primary: P) -> Self {
        Self::new(DocumentBuilder::single(primary).build())
    }

    /// Wrap a collection of primary resources as a `200 OK` data document —
    /// shorthand for `JsonApiResponse::new(DocumentBuilder::collection(primary).build())`.
    #[must_use]
    pub fn collection(primary: Vec<P>) -> Self {
        Self::new(DocumentBuilder::collection(primary).build())
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
        let result = match &self.fields {
            Some(fields) if !fields.is_empty() => {
                try_json_api_response_filtered(self.status, content_type, &self.document, fields)
            }
            _ => try_json_api_response(self.status, content_type, &self.document),
        };
        // A payload that cannot be serialized to JSON — e.g. an attribute with
        // non-string map keys, or a failing custom `Serialize` — must not panic
        // at the `IntoResponse` boundary. Degrade to a JSON:API 500; the raw
        // error is scrubbed from the body unless `debug-errors` is enabled.
        let response = match result {
            Ok(response) => response,
            Err(err) => return JsonApiError::internal(err.to_string()).into_response(),
        };
        let mut response = response.map(Body::from);
        // A self link is server-controlled and path-percent-encoded, so it is a
        // valid ASCII header value; on the impossible parse failure we omit the
        // header rather than panic in a responder.
        if let Some(location) = self.location
            && let Ok(value) = HeaderValue::from_str(&location)
        {
            response.headers_mut().insert(header::LOCATION, value);
        }
        response
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

    /// Read the status and JSON body from a response.
    fn read(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let bytes =
            pollster::block_on(axum::body::to_bytes(response.into_body(), usize::MAX)).unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[test]
    fn created_sets_201_and_location_header_with_body_intact() {
        let response = JsonApiResponse::new(sample_document())
            .created("https://api.test/articles/1")
            .into_response();
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("https://api.test/articles/1")
        );
        let (status, json) = read(response);
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(json["data"]["id"], "1");
    }

    #[test]
    fn location_without_created_sets_header_but_keeps_status() {
        let response = JsonApiResponse::new(sample_document())
            .location("https://api.test/articles/1")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("https://api.test/articles/1")
        );
    }

    #[test]
    fn no_location_header_by_default() {
        let response = JsonApiResponse::new(sample_document()).into_response();
        assert!(response.headers().get(header::LOCATION).is_none());
    }

    fn sample_resource(id: &str) -> Resource {
        serde_json::from_str(&format!(r#"{{"type":"articles","id":"{id}"}}"#)).unwrap()
    }

    #[test]
    fn single_wraps_one_resource_as_ok_data_document() {
        let response = JsonApiResponse::single(sample_resource("1")).into_response();
        let (status, json) = read(response);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["type"], "articles");
        assert_eq!(json["data"]["id"], "1");
    }

    #[test]
    fn collection_wraps_many_resources_as_ok_data_document() {
        let response =
            JsonApiResponse::collection(vec![sample_resource("1"), sample_resource("2")])
                .into_response();
        let (status, json) = read(response);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"].as_array().unwrap().len(), 2);
        assert_eq!(json["data"][1]["id"], "2");
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
        let (status, json) = read(
            JsonApiResponse::new(compound_document())
                .fields(config)
                .into_response(),
        );

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
        let (_status, json) = read(
            JsonApiResponse::new(compound_document())
                .fields(config)
                .into_response(),
        );

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
        assert_eq!(
            empty["included"][0]["attributes"]["email"],
            "dan@example.com"
        );
    }

    #[test]
    fn media_type_adds_profile_parameter() {
        let media =
            JsonApiMediaType::parse("application/vnd.api+json; profile=\"https://example.com/p\"")
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
