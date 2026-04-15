use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Meta;

/// Language tag(s) for a link — single string or array.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Hreflang {
    /// A single language tag (e.g. `"en"`).
    Single(String),
    /// Multiple language tags (e.g. `["en", "fr"]`).
    Multiple(Vec<String>),
}

/// JSON:API link object (as opposed to a bare URL string).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkObject {
    /// The link's URI.
    pub href: String,
    /// The link's relation type (RFC 8288).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel: Option<String>,
    /// A link that describes the target resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub describedby: Option<Box<Link>>,
    /// Human-readable label for the link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Media type hint for the target resource.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    /// Language tag(s) for the target resource (RFC 5646).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hreflang: Option<Hreflang>,
    /// Link-level meta information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// A link value: either a bare URL string or a link object.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Link {
    /// A bare URL string.
    String(String),
    /// A full link object with optional metadata.
    Object(LinkObject),
}

/// Map of link names to link values. Null links are represented as `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Links(pub BTreeMap<String, Option<Link>>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_string() {
        let json = r#""http://example.com/articles/1""#;
        let link: Link = serde_json::from_str(json).unwrap();
        assert!(matches!(link, Link::String(_)));
        assert_eq!(serde_json::to_string(&link).unwrap(), json);
    }

    #[test]
    fn test_link_object() {
        let json = r#"{"href":"http://example.com","title":"Example"}"#;
        let link: Link = serde_json::from_str(json).unwrap();
        match &link {
            Link::Object(obj) => {
                assert_eq!(obj.href, "http://example.com");
                assert_eq!(obj.title.as_deref(), Some("Example"));
            }
            _ => panic!("expected Link::Object"),
        }
    }

    #[test]
    fn test_links_with_null_value() {
        let json = r#"{"self":"http://example.com","related":null}"#;
        let links: Links = serde_json::from_str(json).unwrap();
        assert!(links.0["self"].is_some());
        assert!(links.0["related"].is_none());
    }

    #[test]
    fn test_hreflang_single() {
        let json = r#""en""#;
        let h: Hreflang = serde_json::from_str(json).unwrap();
        assert!(matches!(h, Hreflang::Single(ref s) if s == "en"));
        assert_eq!(serde_json::to_string(&h).unwrap(), json);
    }

    #[test]
    fn test_hreflang_multiple() {
        let json = r#"["en","fr"]"#;
        let h: Hreflang = serde_json::from_str(json).unwrap();
        assert!(matches!(h, Hreflang::Multiple(ref v) if v.len() == 2));
    }

    #[test]
    fn test_link_object_with_describedby() {
        let json = r#"{"href":"http://example.com","describedby":"http://schema.example.com"}"#;
        let link: Link = serde_json::from_str(json).unwrap();
        match link {
            Link::Object(obj) => {
                assert!(matches!(obj.describedby.as_deref(), Some(Link::String(_))));
            }
            _ => panic!("expected Link::Object"),
        }
    }
}
