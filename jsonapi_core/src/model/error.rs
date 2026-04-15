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
    pub type_: Option<Link>,
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
        assert!(err.links.as_ref().unwrap().type_.is_some());
    }
}
