use std::collections::BTreeMap;

use serde::de;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Links, Meta, RelationshipData};

/// Unifying trait for typed resources and the dynamic `Resource` fallback.
pub trait ResourceObject: Serialize + for<'de> Deserialize<'de> {
    /// The JSON:API type string (e.g. "articles").
    fn resource_type(&self) -> &str;

    /// The server-assigned identifier.
    fn resource_id(&self) -> Option<&str>;

    /// The local identifier (1.1 feature).
    fn resource_lid(&self) -> Option<&str> {
        None
    }

    /// Field names for sparse fieldset support.
    fn field_names() -> &'static [&'static str];

    /// Static type metadata for registry and fieldset support.
    ///
    /// The default implementation panics. Override this method (or use
    /// `#[derive(JsonApi)]`) to enable [`TypeRegistry`](crate::TypeRegistry) support.
    fn type_info() -> crate::type_registry::TypeInfo
    where
        Self: Sized,
    {
        unimplemented!("override type_info() for TypeRegistry support")
    }
}

/// Dynamic fallback for resources whose type is not known at compile time.
#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    /// The JSON:API type string (e.g. "articles").
    pub type_: String,
    /// Server-assigned identifier. None for create payloads.
    pub id: Option<String>,
    /// Client-generated local identifier (JSON:API 1.1).
    pub lid: Option<String>,
    /// Resource attributes as a raw JSON value.
    pub attributes: serde_json::Value,
    /// Relationship linkage data, keyed by relationship name.
    pub relationships: BTreeMap<String, RelationshipData>,
    /// Resource-level links.
    pub links: Option<Links>,
    /// Resource-level meta information.
    pub meta: Option<Meta>,
}

impl ResourceObject for Resource {
    fn resource_type(&self) -> &str {
        &self.type_
    }

    fn resource_id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    fn resource_lid(&self) -> Option<&str> {
        self.lid.as_deref()
    }

    fn field_names() -> &'static [&'static str] {
        &[] // Dynamic — fields not known at compile time
    }

    fn type_info() -> crate::type_registry::TypeInfo {
        crate::type_registry::TypeInfo {
            type_name: "",
            field_names: &[],
            relationships: &[],
        }
    }
}

impl Serialize for Resource {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;

        map.serialize_entry("type", &self.type_)?;
        if let Some(ref id) = self.id {
            map.serialize_entry("id", id)?;
        }
        if let Some(ref lid) = self.lid {
            map.serialize_entry("lid", lid)?;
        }
        if !self.attributes.is_null() {
            map.serialize_entry("attributes", &self.attributes)?;
        }
        if !self.relationships.is_empty() {
            let mut rels = serde_json::Map::new();
            for (name, data) in &self.relationships {
                let mut rel_obj = serde_json::Map::new();
                rel_obj.insert(
                    "data".to_string(),
                    serde_json::to_value(data).map_err(serde::ser::Error::custom)?,
                );
                rels.insert(name.clone(), serde_json::Value::Object(rel_obj));
            }
            map.serialize_entry("relationships", &rels)?;
        }
        if let Some(ref links) = self.links {
            map.serialize_entry("links", links)?;
        }
        if let Some(ref meta) = self.meta {
            map.serialize_entry("meta", meta)?;
        }

        map.end()
    }
}

impl<'de> Deserialize<'de> for Resource {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let obj = value
            .as_object()
            .ok_or_else(|| de::Error::custom("resource must be a JSON object"))?;

        let type_ = obj
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| de::Error::custom("resource must have a `type` string"))?
            .to_string();

        let id = obj.get("id").and_then(|v| v.as_str()).map(String::from);
        let lid = obj.get("lid").and_then(|v| v.as_str()).map(String::from);

        let attributes = obj
            .get("attributes")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let relationships = if let Some(rels_value) = obj.get("relationships") {
            let rels_obj = rels_value
                .as_object()
                .ok_or_else(|| de::Error::custom("`relationships` must be an object"))?;
            let mut map = BTreeMap::new();
            for (name, rel_value) in rels_obj {
                let rel_obj = rel_value
                    .as_object()
                    .ok_or_else(|| de::Error::custom("each relationship must be an object"))?;
                if let Some(data_value) = rel_obj.get("data") {
                    let data: RelationshipData =
                        serde_json::from_value(data_value.clone()).map_err(de::Error::custom)?;
                    map.insert(name.clone(), data);
                }
            }
            map
        } else {
            BTreeMap::new()
        };

        let links = obj
            .get("links")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(de::Error::custom)?;

        let meta = obj
            .get("meta")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(de::Error::custom)?;

        Ok(Resource {
            type_,
            id,
            lid,
            attributes,
            relationships,
            links,
            meta,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Identity, RelationshipData};

    #[test]
    fn test_resource_deserialize_simple() {
        let json = r#"{
            "type": "articles",
            "id": "1",
            "attributes": {
                "title": "Rails is Omakase"
            }
        }"#;
        let resource: Resource = serde_json::from_str(json).unwrap();
        assert_eq!(resource.type_, "articles");
        assert_eq!(resource.id.as_deref(), Some("1"));
        assert_eq!(resource.attributes["title"], "Rails is Omakase");
    }

    #[test]
    fn test_resource_serialize_simple() {
        let resource = Resource {
            type_: "articles".into(),
            id: Some("1".into()),
            lid: None,
            attributes: serde_json::json!({"title": "Hello"}),
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        };
        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["type"], "articles");
        assert_eq!(json["id"], "1");
        assert_eq!(json["attributes"]["title"], "Hello");
        assert!(json.get("relationships").is_none());
    }

    #[test]
    fn test_resource_with_relationships() {
        let json = r#"{
            "type": "articles",
            "id": "1",
            "attributes": {"title": "Hello"},
            "relationships": {
                "author": {
                    "data": {"type": "people", "id": "9"}
                }
            }
        }"#;
        let resource: Resource = serde_json::from_str(json).unwrap();
        assert!(resource.relationships.contains_key("author"));
        match &resource.relationships["author"] {
            RelationshipData::ToOne(Some(rid)) => {
                assert_eq!(rid.type_, "people");
                assert_eq!(rid.identity, Identity::Id("9".into()));
            }
            _ => panic!("expected to-one relationship"),
        }
    }

    #[test]
    fn test_resource_round_trip() {
        let resource = Resource {
            type_: "articles".into(),
            id: Some("1".into()),
            lid: None,
            attributes: serde_json::json!({"title": "Hello"}),
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        };
        let json = serde_json::to_string(&resource).unwrap();
        let deserialized: Resource = serde_json::from_str(&json).unwrap();
        assert_eq!(resource.type_, deserialized.type_);
        assert_eq!(resource.id, deserialized.id);
        assert_eq!(resource.attributes, deserialized.attributes);
    }

    #[test]
    fn test_resource_with_lid() {
        let json = r#"{"type":"articles","lid":"temp-1","attributes":{}}"#;
        let resource: Resource = serde_json::from_str(json).unwrap();
        assert_eq!(resource.lid.as_deref(), Some("temp-1"));
        assert!(resource.id.is_none());
    }

    #[test]
    fn test_resource_object_trait() {
        let resource = Resource {
            type_: "articles".into(),
            id: Some("1".into()),
            lid: None,
            attributes: serde_json::json!({}),
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        };
        assert_eq!(resource.resource_type(), "articles");
        assert_eq!(resource.resource_id(), Some("1"));
    }
}
