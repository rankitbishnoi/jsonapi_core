use serde::{Deserialize, Serialize};

use super::Meta;

/// The `jsonapi` top-level member describing the server's implementation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct JsonApiObject {
    /// The JSON:API version (e.g. `"1.1"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Applied extension URIs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<Vec<String>>,
    /// Applied profile URIs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<Vec<String>>,
    /// Implementation-level meta information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonapi_object_version_only() {
        let json = r#"{"version":"1.1"}"#;
        let obj: JsonApiObject = serde_json::from_str(json).unwrap();
        assert_eq!(obj.version.as_deref(), Some("1.1"));
        assert_eq!(serde_json::to_string(&obj).unwrap(), json);
    }

    #[test]
    fn test_jsonapi_object_with_ext_and_profile() {
        let json = r#"{"version":"1.1","ext":["http://example.com/ext/1"],"profile":["http://example.com/profile/1"]}"#;
        let obj: JsonApiObject = serde_json::from_str(json).unwrap();
        assert_eq!(obj.ext.as_ref().unwrap().len(), 1);
        assert_eq!(obj.profile.as_ref().unwrap().len(), 1);
    }
}
