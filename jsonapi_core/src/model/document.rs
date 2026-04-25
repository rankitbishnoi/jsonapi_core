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

impl<P, I> Document<P, I> {
    /// Consume the document and return the single primary resource.
    ///
    /// Returns [`Error::UnexpectedDocumentShape`](crate::Error::UnexpectedDocumentShape)
    /// if the document is a collection, null, errors, or meta-only.
    ///
    /// This is the typed-primary fast path: write
    /// `let article = doc.into_single()?;` instead of pattern-matching on
    /// [`Document`] and [`PrimaryData`].
    pub fn into_single(self) -> crate::Result<P> {
        match self {
            Document::Data {
                data: PrimaryData::Single(boxed),
                ..
            } => Ok(*boxed),
            Document::Data {
                data: PrimaryData::Many(_),
                ..
            } => Err(unexpected_shape("single resource", "resource collection")),
            Document::Data {
                data: PrimaryData::Null,
                ..
            } => Err(unexpected_shape("single resource", "null primary data")),
            Document::Errors { .. } => {
                Err(unexpected_shape("single resource", "errors document"))
            }
            Document::Meta { .. } => {
                Err(unexpected_shape("single resource", "meta-only document"))
            }
        }
    }

    /// Consume the document and return the primary resources as a [`Vec`].
    ///
    /// Returns [`Error::UnexpectedDocumentShape`](crate::Error::UnexpectedDocumentShape)
    /// if the document is a single resource, null, errors, or meta-only.
    pub fn into_many(self) -> crate::Result<Vec<P>> {
        match self {
            Document::Data {
                data: PrimaryData::Many(items),
                ..
            } => Ok(items),
            Document::Data {
                data: PrimaryData::Single(_),
                ..
            } => Err(unexpected_shape("resource collection", "single resource")),
            Document::Data {
                data: PrimaryData::Null,
                ..
            } => Err(unexpected_shape("resource collection", "null primary data")),
            Document::Errors { .. } => {
                Err(unexpected_shape("resource collection", "errors document"))
            }
            Document::Meta { .. } => {
                Err(unexpected_shape("resource collection", "meta-only document"))
            }
        }
    }

    /// Consume the document and return its meta block.
    ///
    /// Succeeds for [`Document::Meta`] (the meta-only variant) and for
    /// [`Document::Data`] / [`Document::Errors`] when the top-level `meta`
    /// member is present. Returns
    /// [`Error::UnexpectedDocumentShape`](crate::Error::UnexpectedDocumentShape)
    /// when no meta is available.
    pub fn into_meta(self) -> crate::Result<Meta> {
        match self {
            Document::Meta { meta, .. } => Ok(meta),
            Document::Data {
                meta: Some(meta), ..
            } => Ok(meta),
            Document::Errors {
                meta: Some(meta), ..
            } => Ok(meta),
            Document::Data { .. } => Err(unexpected_shape(
                "meta-only or data-with-meta",
                "data document without meta",
            )),
            Document::Errors { .. } => Err(unexpected_shape(
                "meta-only or errors-with-meta",
                "errors document without meta",
            )),
        }
    }

    /// Borrow the single primary resource without consuming the document.
    ///
    /// See [`into_single`](Self::into_single) for the consuming version.
    pub fn as_single(&self) -> crate::Result<&P> {
        match self {
            Document::Data {
                data: PrimaryData::Single(boxed),
                ..
            } => Ok(boxed.as_ref()),
            Document::Data {
                data: PrimaryData::Many(_),
                ..
            } => Err(unexpected_shape("single resource", "resource collection")),
            Document::Data {
                data: PrimaryData::Null,
                ..
            } => Err(unexpected_shape("single resource", "null primary data")),
            Document::Errors { .. } => {
                Err(unexpected_shape("single resource", "errors document"))
            }
            Document::Meta { .. } => {
                Err(unexpected_shape("single resource", "meta-only document"))
            }
        }
    }

    /// Borrow the primary resource collection as a slice without consuming the document.
    ///
    /// See [`into_many`](Self::into_many) for the consuming version.
    pub fn as_many(&self) -> crate::Result<&[P]> {
        match self {
            Document::Data {
                data: PrimaryData::Many(items),
                ..
            } => Ok(items.as_slice()),
            Document::Data {
                data: PrimaryData::Single(_),
                ..
            } => Err(unexpected_shape("resource collection", "single resource")),
            Document::Data {
                data: PrimaryData::Null,
                ..
            } => Err(unexpected_shape("resource collection", "null primary data")),
            Document::Errors { .. } => {
                Err(unexpected_shape("resource collection", "errors document"))
            }
            Document::Meta { .. } => {
                Err(unexpected_shape("resource collection", "meta-only document"))
            }
        }
    }

    /// Borrow the primary data envelope without distinguishing single vs. many.
    ///
    /// Returns [`Error::UnexpectedDocumentShape`](crate::Error::UnexpectedDocumentShape)
    /// for [`Document::Errors`] and [`Document::Meta`].
    pub fn primary(&self) -> crate::Result<&PrimaryData<P>> {
        match self {
            Document::Data { data, .. } => Ok(data),
            Document::Errors { .. } => Err(unexpected_shape("data document", "errors document")),
            Document::Meta { .. } => {
                Err(unexpected_shape("data document", "meta-only document"))
            }
        }
    }

    /// Borrow the document's `included` slice. Returns an empty slice for
    /// [`Document::Errors`] and [`Document::Meta`] — cardinality is always
    /// "zero or more", never an error.
    #[must_use]
    pub fn included(&self) -> &[I] {
        match self {
            Document::Data { included, .. } => included.as_slice(),
            _ => &[],
        }
    }
}

#[inline]
fn unexpected_shape(expected: &'static str, found: &'static str) -> crate::Error {
    crate::Error::UnexpectedDocumentShape { expected, found }
}

impl<P, I> Document<P, I>
where
    P: ResourceObject + DeserializeOwned,
    I: DeserializeOwned,
{
    /// Parse a JSON:API document from a string with structural pre-validation.
    ///
    /// Identical in success cases to `serde_json::from_str::<Document<P, I>>(s)`,
    /// but surfaces structurally-detectable errors as typed
    /// [`Error`](crate::Error) variants ([`Error::TypeMismatch`](crate::Error::TypeMismatch),
    /// [`Error::MalformedRelationship`](crate::Error::MalformedRelationship))
    /// rather than opaque [`serde_json::Error`] strings. Consumers can map
    /// each typed error to the right HTTP status (e.g. 502 for type
    /// mismatch — upstream sent the wrong shape — vs. 422 for client
    /// validation problems).
    ///
    /// The pre-validation runs only when the primary type `P` carries a
    /// non-empty `type_name` (i.e. it was derived with
    /// `#[jsonapi(type = "...")]`); for the dynamic
    /// [`Resource`](crate::Resource) fallback the pre-pass is a no-op.
    pub fn from_str(s: &str) -> crate::Result<Self> {
        let value: serde_json::Value = serde_json::from_str(s)?;
        Self::from_value(value)
    }

    /// Parse a JSON:API document from a byte slice. See [`Document::from_str`]
    /// for semantics.
    pub fn from_slice(bytes: &[u8]) -> crate::Result<Self> {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        Self::from_value(value)
    }

    /// Parse a JSON:API document from a `serde_json::Value` with structural
    /// pre-validation. See [`Document::from_str`] for semantics.
    pub fn from_value(value: serde_json::Value) -> crate::Result<Self> {
        prevalidate::<P>(&value)?;
        serde_json::from_value(value).map_err(crate::Error::Json)
    }
}

/// Structural pre-pass: catches type mismatches and malformed relationships
/// before the generated `Deserialize` runs and converts them to typed errors.
fn prevalidate<P: ResourceObject>(value: &serde_json::Value) -> crate::Result<()> {
    let info = P::type_info();
    let expected = info.type_name;

    let obj = match value.as_object() {
        Some(obj) => obj,
        // Non-object payloads are caught by the regular Deserialize impl.
        None => return Ok(()),
    };

    let Some(data) = obj.get("data") else {
        // Errors / meta documents do not carry primary data; nothing to validate.
        return Ok(());
    };

    match data {
        serde_json::Value::Object(_) => check_resource(data, "data", expected)?,
        serde_json::Value::Array(arr) => {
            for (idx, item) in arr.iter().enumerate() {
                let location = format!("data[{idx}]");
                check_resource(item, &location, expected)?;
            }
        }
        // Null primary data is always valid structurally.
        _ => {}
    }
    Ok(())
}

fn check_resource(
    item: &serde_json::Value,
    location: &str,
    expected_type: &'static str,
) -> crate::Result<()> {
    let obj = match item.as_object() {
        Some(o) => o,
        None => return Ok(()),
    };

    // Only run the type check when P carries a known wire type — dynamic
    // resources (Resource) have an empty type_name, in which case any
    // wire-side `type` is acceptable.
    if !expected_type.is_empty()
        && let Some(serde_json::Value::String(got)) = obj.get("type")
        && got != expected_type
    {
        return Err(crate::Error::TypeMismatch {
            expected: expected_type,
            got: got.clone(),
            location: location.to_string(),
        });
    }

    if let Some(rels) = obj.get("relationships").and_then(|v| v.as_object()) {
        for (name, rel_value) in rels {
            check_relationship(name, rel_value, location)?;
        }
    }

    Ok(())
}

fn check_relationship(
    name: &str,
    rel_value: &serde_json::Value,
    location: &str,
) -> crate::Result<()> {
    let rel_obj = rel_value.as_object().ok_or_else(|| crate::Error::MalformedRelationship {
        name: name.to_string(),
        location: location.to_string(),
        reason: "relationship value must be an object".into(),
    })?;

    // A relationship may omit `data` (links/meta only), but if `data` is
    // present it must be null, an object, or an array.
    if let Some(data) = rel_obj.get("data") {
        match data {
            serde_json::Value::Null
            | serde_json::Value::Object(_)
            | serde_json::Value::Array(_) => {}
            other => {
                let kind = match other {
                    serde_json::Value::Bool(_) => "boolean",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::String(_) => "string",
                    _ => "unknown",
                };
                return Err(crate::Error::MalformedRelationship {
                    name: name.to_string(),
                    location: location.to_string(),
                    reason: format!(
                        "`data` must be null, an object, or an array; got {kind}"
                    ),
                });
            }
        }
    }

    Ok(())
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

    // ----- Document accessors (improvement #1) -----

    fn single_doc_json() -> &'static str {
        r#"{"data":{"type":"articles","id":"1","attributes":{"title":"Hello"}}}"#
    }

    fn many_doc_json() -> &'static str {
        r#"{"data":[{"type":"articles","id":"1","attributes":{}},{"type":"articles","id":"2","attributes":{}}]}"#
    }

    fn null_doc_json() -> &'static str {
        r#"{"data":null}"#
    }

    fn errors_doc_json() -> &'static str {
        r#"{"errors":[{"status":"404","title":"Not Found"}]}"#
    }

    fn meta_doc_json() -> &'static str {
        r#"{"meta":{"total":42}}"#
    }

    #[test]
    fn into_single_returns_resource() {
        let doc: Document<Resource> = serde_json::from_str(single_doc_json()).unwrap();
        let resource = doc.into_single().unwrap();
        assert_eq!(resource.resource_type(), "articles");
        assert_eq!(resource.resource_id(), Some("1"));
    }

    #[test]
    fn into_single_rejects_collection() {
        let doc: Document<Resource> = serde_json::from_str(many_doc_json()).unwrap();
        let err = doc.into_single().unwrap_err();
        match err {
            crate::Error::UnexpectedDocumentShape { expected, found } => {
                assert_eq!(expected, "single resource");
                assert_eq!(found, "resource collection");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn into_single_rejects_null() {
        let doc: Document<Resource> = serde_json::from_str(null_doc_json()).unwrap();
        let err = doc.into_single().unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnexpectedDocumentShape {
                found: "null primary data",
                ..
            }
        ));
    }

    #[test]
    fn into_single_rejects_errors() {
        let doc: Document<Resource> = serde_json::from_str(errors_doc_json()).unwrap();
        let err = doc.into_single().unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnexpectedDocumentShape {
                found: "errors document",
                ..
            }
        ));
    }

    #[test]
    fn into_single_rejects_meta() {
        let doc: Document<Resource> = serde_json::from_str(meta_doc_json()).unwrap();
        let err = doc.into_single().unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnexpectedDocumentShape {
                found: "meta-only document",
                ..
            }
        ));
    }

    #[test]
    fn into_many_returns_collection() {
        let doc: Document<Resource> = serde_json::from_str(many_doc_json()).unwrap();
        let resources = doc.into_many().unwrap();
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].resource_id(), Some("1"));
        assert_eq!(resources[1].resource_id(), Some("2"));
    }

    #[test]
    fn into_many_rejects_single() {
        let doc: Document<Resource> = serde_json::from_str(single_doc_json()).unwrap();
        let err = doc.into_many().unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnexpectedDocumentShape {
                expected: "resource collection",
                found: "single resource",
            }
        ));
    }

    #[test]
    fn into_meta_returns_meta() {
        let doc: Document<Resource> = serde_json::from_str(meta_doc_json()).unwrap();
        let meta = doc.into_meta().unwrap();
        assert_eq!(meta["total"], 42);
    }

    #[test]
    fn into_meta_returns_meta_from_data_doc() {
        let json =
            r#"{"data":{"type":"articles","id":"1","attributes":{}},"meta":{"page":7}}"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        let meta = doc.into_meta().unwrap();
        assert_eq!(meta["page"], 7);
    }

    #[test]
    fn into_meta_rejects_data_without_meta() {
        let doc: Document<Resource> = serde_json::from_str(single_doc_json()).unwrap();
        let err = doc.into_meta().unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnexpectedDocumentShape {
                found: "data document without meta",
                ..
            }
        ));
    }

    #[test]
    fn as_single_borrows_without_consuming() {
        let doc: Document<Resource> = serde_json::from_str(single_doc_json()).unwrap();
        let r1 = doc.as_single().unwrap();
        assert_eq!(r1.resource_id(), Some("1"));
        // Doc is still usable.
        let r2 = doc.as_single().unwrap();
        assert_eq!(r2.resource_id(), Some("1"));
    }

    #[test]
    fn as_many_borrows_slice() {
        let doc: Document<Resource> = serde_json::from_str(many_doc_json()).unwrap();
        let slice = doc.as_many().unwrap();
        assert_eq!(slice.len(), 2);
        // Doc is still usable for further borrows.
        let _ = doc.as_many().unwrap();
    }

    #[test]
    fn primary_returns_envelope() {
        let doc: Document<Resource> = serde_json::from_str(many_doc_json()).unwrap();
        match doc.primary().unwrap() {
            PrimaryData::Many(v) => assert_eq!(v.len(), 2),
            _ => panic!("expected Many"),
        }
    }

    #[test]
    fn primary_rejects_errors() {
        let doc: Document<Resource> = serde_json::from_str(errors_doc_json()).unwrap();
        let err = doc.primary().unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnexpectedDocumentShape {
                expected: "data document",
                found: "errors document",
            }
        ));
    }

    #[test]
    fn included_returns_slice_for_data() {
        let json = r#"{
            "data": {"type":"articles","id":"1","attributes":{"title":"Hello"}},
            "included": [{"type":"people","id":"9","attributes":{"name":"Dan"}}]
        }"#;
        let doc: Document<Resource> = serde_json::from_str(json).unwrap();
        assert_eq!(doc.included().len(), 1);
        assert_eq!(doc.included()[0].resource_type(), "people");
    }

    #[test]
    fn included_returns_empty_for_errors_and_meta() {
        let errors: Document<Resource> = serde_json::from_str(errors_doc_json()).unwrap();
        assert!(errors.included().is_empty());

        let meta: Document<Resource> = serde_json::from_str(meta_doc_json()).unwrap();
        assert!(meta.included().is_empty());
    }
}
