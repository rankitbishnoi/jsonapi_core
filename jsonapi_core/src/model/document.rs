use serde::de::{self, DeserializeOwned};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{ApiError, JsonApiObject, Links, Meta, Resource, ResourceObject};

/// Primary data: null (empty to-one), single resource, or collection.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum PrimaryData<T> {
    /// Empty to-one relationship (`null` in JSON).
    Null,
    /// A single resource object.
    Single(Box<T>),
    /// A collection of resource objects.
    Many(Vec<T>),
}

impl<T: Serialize> Serialize for PrimaryData<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            PrimaryData::Null => serializer.serialize_none(),
            PrimaryData::Single(item) => item.serialize(serializer),
            PrimaryData::Many(items) => items.serialize(serializer),
        }
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for PrimaryData<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(PrimaryData::Null),
            serde_json::Value::Array(arr) => {
                let items: Vec<T> = arr
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<_, _>>()
                    .map_err(de::Error::custom)?;
                Ok(PrimaryData::Many(items))
            }
            other => {
                let item: T = serde_json::from_value(other).map_err(de::Error::custom)?;
                Ok(PrimaryData::Single(Box::new(item)))
            }
        }
    }
}

/// Top-level JSON:API document.
/// `data` and `errors` are mutually exclusive per the spec.
///
/// Generic over the primary type `P` and the included type `I`. The default
/// `I = Resource` keeps the included array open-set, which is what most
/// real-world payloads need — `included` in a compound document is typically
/// heterogeneous (authors, comments, tags, taxonomies, …). If you want a
/// fully-typed homogeneous document, write `Document<T, T>` explicitly.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Document<P, I = Resource> {
    /// A successful response containing primary data.
    Data {
        /// The primary resource data.
        data: PrimaryData<P>,
        /// Resources related to the primary data (compound document).
        included: Vec<I>,
        /// Top-level meta information.
        meta: Option<Meta>,
        /// The `jsonapi` member describing server implementation.
        jsonapi: Option<JsonApiObject>,
        /// Top-level links.
        links: Option<Links>,
    },
    /// An error response.
    Errors {
        /// One or more error objects.
        errors: Vec<ApiError>,
        /// Top-level meta information.
        meta: Option<Meta>,
        /// The `jsonapi` member describing server implementation.
        jsonapi: Option<JsonApiObject>,
        /// Top-level links.
        links: Option<Links>,
    },
    /// A meta-only response (no `data` or `errors`).
    Meta {
        /// The required meta object.
        meta: Meta,
        /// The `jsonapi` member describing server implementation.
        jsonapi: Option<JsonApiObject>,
        /// Top-level links.
        links: Option<Links>,
    },
}

impl<P: ResourceObject, I: Serialize> Serialize for Document<P, I> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;

        match self {
            Document::Data {
                data,
                included,
                meta,
                jsonapi,
                links,
            } => {
                map.serialize_entry("data", data)?;
                if !included.is_empty() {
                    map.serialize_entry("included", included)?;
                }
                if let Some(m) = meta {
                    map.serialize_entry("meta", m)?;
                }
                if let Some(j) = jsonapi {
                    map.serialize_entry("jsonapi", j)?;
                }
                if let Some(l) = links {
                    map.serialize_entry("links", l)?;
                }
            }
            Document::Errors {
                errors,
                meta,
                jsonapi,
                links,
            } => {
                map.serialize_entry("errors", errors)?;
                if let Some(m) = meta {
                    map.serialize_entry("meta", m)?;
                }
                if let Some(j) = jsonapi {
                    map.serialize_entry("jsonapi", j)?;
                }
                if let Some(l) = links {
                    map.serialize_entry("links", l)?;
                }
            }
            Document::Meta {
                meta,
                jsonapi,
                links,
            } => {
                map.serialize_entry("meta", meta)?;
                if let Some(j) = jsonapi {
                    map.serialize_entry("jsonapi", j)?;
                }
                if let Some(l) = links {
                    map.serialize_entry("links", l)?;
                }
            }
        }

        map.end()
    }
}

impl<'de, P, I> Deserialize<'de> for Document<P, I>
where
    P: ResourceObject + DeserializeOwned,
    I: DeserializeOwned,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let obj = value
            .as_object()
            .ok_or_else(|| de::Error::custom("document must be a JSON object"))?;

        let has_data = obj.contains_key("data");
        let has_errors = obj.contains_key("errors");

        if has_data && has_errors {
            return Err(de::Error::custom(
                "document must not contain both `data` and `errors`",
            ));
        }

        let meta: Option<Meta> = obj
            .get("meta")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(de::Error::custom)?;
        let jsonapi: Option<JsonApiObject> = obj
            .get("jsonapi")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(de::Error::custom)?;
        let links: Option<Links> = obj
            .get("links")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(de::Error::custom)?;

        if has_data {
            let data: PrimaryData<P> = serde_json::from_value(obj["data"].clone())
                .map_err(|e| de::Error::custom(format!("in primary data: {e}")))?;
            // Deserialize each included entry individually so errors can name
            // the offending index.
            let included: Vec<I> = match obj.get("included") {
                Some(v) => {
                    let arr = v
                        .as_array()
                        .ok_or_else(|| de::Error::custom("`included` must be a JSON array"))?;
                    let mut out = Vec::with_capacity(arr.len());
                    for (idx, entry) in arr.iter().enumerate() {
                        let parsed: I = serde_json::from_value(entry.clone())
                            .map_err(|e| de::Error::custom(format!("in included[{idx}]: {e}")))?;
                        out.push(parsed);
                    }
                    out
                }
                None => Vec::new(),
            };
            Ok(Document::Data {
                data,
                included,
                meta,
                jsonapi,
                links,
            })
        } else if has_errors {
            let errors: Vec<ApiError> =
                serde_json::from_value(obj["errors"].clone()).map_err(de::Error::custom)?;
            Ok(Document::Errors {
                errors,
                meta,
                jsonapi,
                links,
            })
        } else if let Some(m) = meta {
            Ok(Document::Meta {
                meta: m,
                jsonapi,
                links,
            })
        } else {
            Err(de::Error::custom(
                "document must contain `data`, `errors`, or `meta`",
            ))
        }
    }
}

impl<P, I: ResourceObject> Document<P, I> {
    /// Build a [`Registry`](crate::registry::Registry) from this document's `included` resources.
    /// Returns an empty registry for `Errors` and `Meta` variants.
    ///
    /// The included type `I` must implement [`ResourceObject`]. The default
    /// `I = Resource` always satisfies that; if you override `I` with a custom
    /// type that doesn't implement `ResourceObject`, `.registry()` becomes
    /// unavailable and you'll need to build the registry manually.
    pub fn registry(&self) -> crate::Result<crate::registry::Registry> {
        match self {
            Document::Data { included, .. } => crate::registry::Registry::from_included(included),
            _ => Ok(crate::registry::Registry::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Resource;

    #[test]
    fn test_primary_data_null() {
        let json = "null";
        let data: PrimaryData<Resource> = serde_json::from_str(json).unwrap();
        assert!(matches!(data, PrimaryData::Null));
        assert_eq!(serde_json::to_string(&data).unwrap(), json);
    }

    #[test]
    fn test_primary_data_single() {
        let json = r#"{"type":"articles","id":"1","attributes":{"title":"Hello"}}"#;
        let data: PrimaryData<Resource> = serde_json::from_str(json).unwrap();
        match &data {
            PrimaryData::Single(r) => assert_eq!(r.resource_type(), "articles"),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn test_primary_data_many() {
        let json = r#"[{"type":"articles","id":"1","attributes":{}},{"type":"articles","id":"2","attributes":{}}]"#;
        let data: PrimaryData<Resource> = serde_json::from_str(json).unwrap();
        match &data {
            PrimaryData::Many(v) => assert_eq!(v.len(), 2),
            _ => panic!("expected Many"),
        }
    }

    #[test]
    fn test_document_data() {
        let json = r#"{
            "data": {"type":"articles","id":"1","attributes":{"title":"Hello"}},
            "jsonapi": {"version":"1.1"}
        }"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        match &doc {
            Document::Data { data, jsonapi, .. } => {
                assert!(matches!(data, PrimaryData::Single(_)));
                assert_eq!(jsonapi.as_ref().unwrap().version.as_deref(), Some("1.1"));
            }
            _ => panic!("expected Document::Data"),
        }
    }

    #[test]
    fn test_document_data_collection() {
        let json = r#"{"data":[{"type":"articles","id":"1","attributes":{}},{"type":"articles","id":"2","attributes":{}}]}"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        match &doc {
            Document::Data {
                data: PrimaryData::Many(v),
                ..
            } => assert_eq!(v.len(), 2),
            _ => panic!("expected Document::Data with Many"),
        }
    }

    #[test]
    fn test_document_data_null() {
        let json = r#"{"data":null}"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        match &doc {
            Document::Data {
                data: PrimaryData::Null,
                ..
            } => {}
            _ => panic!("expected Document::Data with Null"),
        }
    }

    #[test]
    fn test_document_errors() {
        let json = r#"{"errors":[{"status":"404","title":"Not Found"}]}"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        match &doc {
            Document::Errors { errors, .. } => {
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].status.as_deref(), Some("404"));
            }
            _ => panic!("expected Document::Errors"),
        }
    }

    #[test]
    fn test_document_meta_only() {
        let json = r#"{"meta":{"total":0}}"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        assert!(matches!(doc, Document::Meta { .. }));
    }

    #[test]
    fn test_document_rejects_data_and_errors() {
        let json = r#"{"data":null,"errors":[]}"#;
        let result: std::result::Result<Document<Resource>, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_document_with_included() {
        let json = r#"{
            "data": {"type":"articles","id":"1","attributes":{"title":"Hello"}},
            "included": [{"type":"people","id":"9","attributes":{"name":"Dan"}}]
        }"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        match &doc {
            Document::Data { included, .. } => {
                assert_eq!(included.len(), 1);
                assert_eq!(included[0].resource_type(), "people");
            }
            _ => panic!("expected Document::Data"),
        }
    }

    #[test]
    fn test_document_data_round_trip() {
        let doc: Document<Resource> = Document::Data {
            data: PrimaryData::Single(Box::new(Resource {
                type_: "articles".into(),
                id: Some("1".into()),
                lid: None,
                attributes: serde_json::json!({"title": "Hello"}),
                relationships: std::collections::BTreeMap::new(),
                links: None,
                meta: None,
            })),
            included: vec![],
            meta: None,
            jsonapi: None,
            links: None,
        };
        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: Document<Resource> = serde_json::from_str(&json).unwrap();
        match (&doc, &deserialized) {
            (Document::Data { data: d1, .. }, Document::Data { data: d2, .. }) => match (d1, d2) {
                (PrimaryData::Single(a), PrimaryData::Single(b)) => {
                    assert_eq!(a.type_, b.type_);
                    assert_eq!(a.id, b.id);
                }
                _ => panic!("mismatch"),
            },
            _ => panic!("mismatch"),
        }
    }

    #[test]
    fn test_document_errors_round_trip() {
        let doc: Document<Resource> = Document::Errors {
            errors: vec![ApiError {
                id: None,
                links: None,
                status: Some("500".into()),
                code: None,
                title: Some("Internal Server Error".into()),
                detail: None,
                source: None,
                meta: None,
            }],
            meta: None,
            jsonapi: None,
            links: None,
        };
        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: Document<Resource> = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, Document::Errors { .. }));
    }

    #[test]
    fn test_document_registry_with_included() {
        let json = r#"{
            "data": {"type":"articles","id":"1","attributes":{"title":"Hello"}},
            "included": [{"type":"people","id":"9","attributes":{"name":"Dan"}}]
        }"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        let registry = doc.registry().unwrap();
        let person: Resource = registry.get_by_id("people", "9").unwrap();
        assert_eq!(person.attributes["name"], "Dan");
    }

    #[test]
    fn test_document_registry_empty_included() {
        let json = r#"{"data":{"type":"articles","id":"1","attributes":{}}}"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        let registry = doc.registry().unwrap();
        let result: std::result::Result<Resource, _> = registry.get_by_id("people", "9");
        assert!(result.is_err());
    }

    #[test]
    fn test_document_registry_errors_variant() {
        let json = r#"{"errors":[{"status":"404","title":"Not Found"}]}"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        let registry = doc.registry().unwrap();
        let result: std::result::Result<Resource, _> = registry.get_by_id("people", "9");
        assert!(result.is_err());
    }
}
