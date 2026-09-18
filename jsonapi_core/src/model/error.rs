use serde::{Deserialize, Serialize};

use super::{Link, Meta};

/// Source of a JSON:API error.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ErrorSource {
    /// A JSON pointer (RFC 6901) to the value in the request document that caused the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    /// The name of the query parameter that caused the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    /// The name of the request header that caused the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
}

/// Links specific to an error object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ErrorLinks {
    /// A link that leads to further details about the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<Link>,
    /// A link that identifies the type of error (RFC 7807).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<Link>,
}

/// A JSON:API error object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ApiError {
    /// A unique identifier for this particular occurrence of the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Links related to the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ErrorLinks>,
    /// The HTTP status code applicable to the error, as a string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// An application-specific error code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// A short, human-readable summary of the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A human-readable explanation specific to this occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// An object indicating the source of the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ErrorSource>,
    /// Error-level meta information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

impl ErrorSource {
    /// Build a source identifying a request-document location by RFC 6901 JSON
    /// pointer (e.g. `/data/attributes/title`).
    #[must_use]
    pub fn pointer(pointer: impl Into<String>) -> Self {
        Self {
            pointer: Some(pointer.into()),
            ..Default::default()
        }
    }

    /// Build a source identifying the offending query parameter (e.g. `sort`).
    #[must_use]
    pub fn parameter(parameter: impl Into<String>) -> Self {
        Self {
            parameter: Some(parameter.into()),
            ..Default::default()
        }
    }

    /// Build a source identifying the offending request header.
    #[must_use]
    pub fn header(header: impl Into<String>) -> Self {
        Self {
            header: Some(header.into()),
            ..Default::default()
        }
    }
}

impl ErrorLinks {
    /// Start building an empty [`ErrorLinks`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `about` — a link to further details about this error.
    #[must_use]
    pub fn about(mut self, link: Link) -> Self {
        self.about = Some(link);
        self
    }

    /// Set `type` — a link identifying the type of error (RFC 7807).
    #[must_use]
    pub fn r#type(mut self, link: Link) -> Self {
        self.r#type = Some(link);
        self
    }
}

impl ApiError {
    /// Start building an empty [`ApiError`]. Chain the `with_*` setters to fill
    /// in members.
    ///
    /// ```
    /// # use jsonapi_core::{ApiError, ErrorSource};
    /// let err = ApiError::new()
    ///     .with_status("422")
    ///     .with_title("Unprocessable Entity")
    ///     .with_source(ErrorSource::pointer("/data/attributes/title"));
    /// assert_eq!(err.status.as_deref(), Some("422"));
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `id` — a unique identifier for this particular occurrence.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set `status` — the HTTP status code as a string (e.g. `"422"`).
    #[must_use]
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Set `code` — an application-specific error code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Set `title` — a short, human-readable summary.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set `detail` — a human-readable explanation specific to this occurrence.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set `source` — the location in the request that caused the error.
    #[must_use]
    pub fn with_source(mut self, source: ErrorSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Set `links` — links related to the error.
    #[must_use]
    pub fn with_links(mut self, links: ErrorLinks) -> Self {
        self.links = Some(links);
        self
    }

    /// Set `meta` — error-level meta information.
    #[must_use]
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_round_trip() {
        let json = r#"{"status":"404","title":"Not Found","detail":"Article 99 not found"}"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert_eq!(err.status.as_deref(), Some("404"));
        assert_eq!(err.title.as_deref(), Some("Not Found"));
        assert_eq!(serde_json::to_string(&err).unwrap(), json);
    }

    #[test]
    fn test_api_error_with_source_pointer() {
        let json = r#"{"status":"422","source":{"pointer":"/data/attributes/title"}}"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert_eq!(
            err.source.as_ref().unwrap().pointer.as_deref(),
            Some("/data/attributes/title")
        );
    }

    #[test]
    fn test_api_error_with_type_link() {
        let json = r#"{"status":"422","links":{"type":"http://example.com/errors/invalid"}}"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert!(err.links.as_ref().unwrap().r#type.is_some());
    }

    #[test]
    fn api_error_builder_sets_every_member() {
        let err = ApiError::new()
            .with_id("occ-1")
            .with_status("422")
            .with_code("invalid")
            .with_title("Unprocessable Entity")
            .with_detail("title must not be empty")
            .with_source(ErrorSource::pointer("/data/attributes/title"))
            .with_links(ErrorLinks::new().about(Link::String("https://errors/1".into())))
            .with_meta(Meta::new());
        assert_eq!(err.id.as_deref(), Some("occ-1"));
        assert_eq!(err.status.as_deref(), Some("422"));
        assert_eq!(err.code.as_deref(), Some("invalid"));
        assert_eq!(err.title.as_deref(), Some("Unprocessable Entity"));
        assert_eq!(err.detail.as_deref(), Some("title must not be empty"));
        assert_eq!(
            err.source.unwrap().pointer.as_deref(),
            Some("/data/attributes/title")
        );
        assert!(err.links.unwrap().about.is_some());
        assert!(err.meta.is_some());
    }

    #[test]
    fn error_source_constructors_set_one_member() {
        assert_eq!(ErrorSource::pointer("/data").pointer.as_deref(), Some("/data"));
        assert_eq!(ErrorSource::parameter("sort").parameter.as_deref(), Some("sort"));
        assert_eq!(ErrorSource::header("Accept").header.as_deref(), Some("Accept"));
    }
}
