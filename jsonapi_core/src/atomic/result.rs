//! Atomic operations response types.

use serde::{Deserialize, Serialize};

use crate::{JsonApiObject, Links, Meta, PrimaryData, Resource};

/// Result of a single atomic operation.
///
/// For a successful `remove`, the wire form is often an empty object `{}`
/// (all fields omitted). For `add` and `update`, `data` carries the
/// resource(s) produced or modified.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AtomicResult {
    /// Resource(s) produced or modified by the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<PrimaryData<Resource>>,

    /// Optional meta object describing the operation result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,

    /// Optional links related to the operation result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<Links>,
}

/// Response envelope mirroring [`AtomicRequest`](crate::AtomicRequest).
///
/// Contains an ordered `results` array aligning 1:1 with the request's
/// `operations`. Top-level `jsonapi`, `meta`, and `links` members are
/// optional, matching normal JSON:API document semantics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AtomicResponse {
    /// One result per operation in the corresponding request, in order.
    #[serde(rename = "atomic:results")]
    pub results: Vec<AtomicResult>,

    /// Optional `jsonapi` member (server may declare version/ext).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jsonapi: Option<JsonApiObject>,

    /// Optional top-level meta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,

    /// Optional top-level links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<Links>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_result_serializes_as_empty_object() {
        let r = AtomicResult::default();
        assert_eq!(serde_json::to_string(&r).unwrap(), "{}");
    }

    #[test]
    fn empty_result_deserializes_from_empty_object() {
        let r: AtomicResult = serde_json::from_str("{}").unwrap();
        assert_eq!(r, AtomicResult::default());
    }

    #[test]
    fn result_with_single_data() {
        let r = AtomicResult {
            data: Some(PrimaryData::Single(Box::new(Resource {
                type_: "articles".into(),
                id: Some("1".into()),
                lid: None,
                attributes: serde_json::json!({"title": "Hello"}),
                relationships: std::collections::BTreeMap::new(),
                links: None,
                meta: None,
            }))),
            meta: None,
            links: None,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["data"]["type"], "articles");
        assert_eq!(json["data"]["attributes"]["title"], "Hello");
    }

    #[test]
    fn result_with_null_data_preserves_null() {
        let r = AtomicResult {
            data: Some(PrimaryData::Null),
            meta: None,
            links: None,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert!(json["data"].is_null());
    }

    #[test]
    fn empty_response_round_trips() {
        let resp = AtomicResponse::default();
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"atomic:results":[]}"#);
        let round: AtomicResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(round, resp);
    }

    #[test]
    fn response_with_results_and_meta() {
        let mut meta = serde_json::Map::new();
        meta.insert("processed".into(), serde_json::json!(2));
        let resp = AtomicResponse {
            results: vec![AtomicResult::default(), AtomicResult::default()],
            jsonapi: Some(JsonApiObject {
                version: Some("1.1".into()),
                ext: None,
                profile: None,
                meta: None,
            }),
            meta: Some(meta),
            links: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["atomic:results"].as_array().unwrap().len(), 2);
        assert_eq!(json["jsonapi"]["version"], "1.1");
    }

    #[test]
    fn response_missing_results_field_is_error() {
        let result: std::result::Result<AtomicResponse, _> = serde_json::from_str("{}");
        assert!(result.is_err());
    }
}
