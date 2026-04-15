use std::marker::PhantomData;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Links, Meta, ResourceIdentifier};

/// Resource linkage inside a relationship.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum RelationshipData {
    /// To-one: `null` (empty) or a single resource identifier.
    ToOne(Option<ResourceIdentifier>),
    /// To-many: an array of resource identifiers (may be empty).
    ToMany(Vec<ResourceIdentifier>),
}

impl Serialize for RelationshipData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            RelationshipData::ToOne(None) => serializer.serialize_none(),
            RelationshipData::ToOne(Some(rid)) => rid.serialize(serializer),
            RelationshipData::ToMany(rids) => rids.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for RelationshipData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(RelationshipData::ToOne(None)),
            serde_json::Value::Array(arr) => {
                let rids: Vec<ResourceIdentifier> = arr
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<_, _>>()
                    .map_err(de::Error::custom)?;
                Ok(RelationshipData::ToMany(rids))
            }
            serde_json::Value::Object(_) => {
                let rid: ResourceIdentifier =
                    serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(RelationshipData::ToOne(Some(rid)))
            }
            _ => Err(de::Error::custom(
                "relationship data must be null, object, or array",
            )),
        }
    }
}

/// Typed relationship reference. Carries the target type as a phantom
/// for type-safe registry lookups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship<T> {
    /// The relationship linkage data.
    pub data: RelationshipData,
    /// Relationship-level links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Links>,
    /// Relationship-level meta information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(skip)]
    _phantom: PhantomData<T>,
}

impl<T> Relationship<T> {
    /// Create a new relationship with the given linkage data.
    pub fn new(data: RelationshipData) -> Self {
        Self {
            data,
            links: None,
            meta: None,
            _phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Identity;

    #[test]
    fn test_relationship_data_to_one() {
        let json = r#"{"type":"people","id":"9"}"#;
        let data: RelationshipData = serde_json::from_str(json).unwrap();
        match &data {
            RelationshipData::ToOne(Some(rid)) => {
                assert_eq!(rid.type_, "people");
                assert_eq!(rid.identity, Identity::Id("9".into()));
            }
            _ => panic!("expected ToOne(Some(...))"),
        }
        assert_eq!(serde_json::to_string(&data).unwrap(), json);
    }

    #[test]
    fn test_relationship_data_to_one_null() {
        let json = "null";
        let data: RelationshipData = serde_json::from_str(json).unwrap();
        assert!(matches!(data, RelationshipData::ToOne(None)));
        assert_eq!(serde_json::to_string(&data).unwrap(), json);
    }

    #[test]
    fn test_relationship_data_to_many() {
        let json = r#"[{"type":"tags","id":"1"},{"type":"tags","id":"2"}]"#;
        let data: RelationshipData = serde_json::from_str(json).unwrap();
        match &data {
            RelationshipData::ToMany(rids) => assert_eq!(rids.len(), 2),
            _ => panic!("expected ToMany"),
        }
        assert_eq!(serde_json::to_string(&data).unwrap(), json);
    }

    #[test]
    fn test_relationship_data_to_many_empty() {
        let json = "[]";
        let data: RelationshipData = serde_json::from_str(json).unwrap();
        assert!(matches!(data, RelationshipData::ToMany(ref v) if v.is_empty()));
    }
}
