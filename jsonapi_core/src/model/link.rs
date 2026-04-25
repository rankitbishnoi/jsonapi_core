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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Links(pub BTreeMap<String, Option<Link>>);

impl Links {
    /// Construct an empty link map.
    #[must_use]
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// `true` if the map contains the given link name, regardless of whether
    /// the link itself is `null`. Use [`Links::get`] when you need a non-null
    /// link.
    #[must_use]
    pub fn contains(&self, rel: &str) -> bool {
        self.0.contains_key(rel)
    }

    /// Borrow the link for `rel` if it is present *and* non-null.
    ///
    /// Returns `None` for both "key absent" and "key present, value `null`".
    /// Use `links.0.get(rel)` directly if you need to distinguish the two.
    #[must_use]
    pub fn get(&self, rel: &str) -> Option<&Link> {
        self.0.get(rel).and_then(Option::as_ref)
    }

    /// Iterate `(name, &Link)` pairs, skipping entries whose value is `null`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Link)> + '_ {
        self.0
            .iter()
            .filter_map(|(k, v)| v.as_ref().map(|link| (k.as_str(), link)))
    }

    /// Iterate the link names in lexicographic order.
    pub fn keys(&self) -> impl Iterator<Item = &str> + '_ {
        self.0.keys().map(String::as_str)
    }

    /// Total number of entries (counts `null`-valued entries too).
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` if there are no entries at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

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

    // ----- Links inherent helpers (improvement #4) -----

    fn sample_links() -> Links {
        let json = r#"{
            "self": "http://example.com/articles/1",
            "related": null,
            "next": {"href": "http://example.com/articles?page=2", "title": "Next page"}
        }"#;
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn links_new_is_empty() {
        let links = Links::new();
        assert!(links.is_empty());
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn links_default_matches_new() {
        assert_eq!(Links::default(), Links::new());
    }

    #[test]
    fn links_contains_returns_true_for_null_entry() {
        let links = sample_links();
        assert!(links.contains("self"));
        assert!(links.contains("related"));
        assert!(links.contains("next"));
        assert!(!links.contains("missing"));
    }

    #[test]
    fn links_get_returns_link_for_present_non_null() {
        let links = sample_links();
        match links.get("self") {
            Some(Link::String(s)) => assert_eq!(s, "http://example.com/articles/1"),
            other => panic!("expected Link::String, got {other:?}"),
        }
        assert!(matches!(links.get("next"), Some(Link::Object(_))));
    }

    #[test]
    fn links_get_returns_none_for_null_entry() {
        let links = sample_links();
        assert!(links.get("related").is_none());
    }

    #[test]
    fn links_get_returns_none_for_missing_key() {
        let links = sample_links();
        assert!(links.get("missing").is_none());
    }

    #[test]
    fn links_iter_skips_null_entries() {
        let links = sample_links();
        let names: Vec<&str> = links.iter().map(|(k, _)| k).collect();
        // BTreeMap iterates in lexicographic order; "related" is null and skipped.
        assert_eq!(names, vec!["next", "self"]);
    }

    #[test]
    fn links_keys_includes_null_entries() {
        let links = sample_links();
        let names: Vec<&str> = links.keys().collect();
        assert_eq!(names, vec!["next", "related", "self"]);
    }

    #[test]
    fn links_len_counts_all_entries() {
        let links = sample_links();
        assert_eq!(links.len(), 3);
        assert!(!links.is_empty());
    }
}
