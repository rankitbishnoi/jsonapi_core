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

    /// Unified slice view of every identifier inside the relationship,
    /// regardless of cardinality.
    ///
    /// - `ToOne(None)` → empty slice.
    /// - `ToOne(Some(rid))` → one-element slice.
    /// - `ToMany(vec)` → the full vec as a slice.
    #[must_use]
    pub fn identifiers(&self) -> &[ResourceIdentifier] {
        match &self.data {
            RelationshipData::ToOne(None) => &[],
            RelationshipData::ToOne(Some(rid)) => std::slice::from_ref(rid),
            RelationshipData::ToMany(rids) => rids.as_slice(),
        }
    }

    /// Iterator over server-assigned IDs, regardless of cardinality.
    /// Skips `Lid` identifiers and null to-one relationships.
    pub fn ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.identifiers().iter().filter_map(|rid| rid.identity.as_id())
    }

    /// The first server-assigned ID in the relationship, or `None` if the
    /// relationship is null-to-one, empty-to-many, or contains only local
    /// identifiers.
    #[must_use]
    pub fn first_id(&self) -> Option<&str> {
        self.ids().next()
    }

    /// The first identifier — server-assigned `id` or client-local `lid` —
    /// as a `&str`. Returns `None` for null-to-one or empty-to-many.
    /// Useful when you need *some* identifier without caring about kind
    /// (e.g. when assembling an atomic-operation ref).
    #[must_use]
    pub fn first_id_or_lid(&self) -> Option<&str> {
        self.identifiers()
            .first()
            .and_then(|rid| rid.identity.as_id().or_else(|| rid.identity.as_lid()))
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

    // ----- Relationship helpers (improvement #3) -----

    fn rid(type_: &str, id: &str) -> ResourceIdentifier {
        ResourceIdentifier {
            type_: type_.into(),
            identity: Identity::Id(id.into()),
            meta: None,
        }
    }

    fn lid_rid(type_: &str, lid: &str) -> ResourceIdentifier {
        ResourceIdentifier {
            type_: type_.into(),
            identity: Identity::Lid(lid.into()),
            meta: None,
        }
    }

    // Phantom target; `Relationship::<T>` only uses T for type-safe registry
    // lookups at the call site, so this is a fine stand-in for unit tests.
    struct Target;

    #[test]
    fn relationship_ids_skips_null_to_one() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToOne(None));
        let collected: Vec<&str> = rel.ids().collect();
        assert!(collected.is_empty());
    }

    #[test]
    fn relationship_ids_returns_single_id_for_to_one() {
        let rel: Relationship<Target> =
            Relationship::new(RelationshipData::ToOne(Some(rid("people", "9"))));
        let collected: Vec<&str> = rel.ids().collect();
        assert_eq!(collected, vec!["9"]);
    }

    #[test]
    fn relationship_ids_returns_all_ids_for_to_many() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToMany(vec![
            rid("tags", "1"),
            rid("tags", "2"),
            rid("tags", "3"),
        ]));
        let collected: Vec<&str> = rel.ids().collect();
        assert_eq!(collected, vec!["1", "2", "3"]);
    }

    #[test]
    fn relationship_ids_skips_lid_entries() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToMany(vec![
            rid("tags", "1"),
            lid_rid("tags", "local-a"),
            rid("tags", "3"),
        ]));
        let collected: Vec<&str> = rel.ids().collect();
        assert_eq!(collected, vec!["1", "3"]);
    }

    #[test]
    fn relationship_first_id_for_to_one() {
        let rel: Relationship<Target> =
            Relationship::new(RelationshipData::ToOne(Some(rid("people", "9"))));
        assert_eq!(rel.first_id(), Some("9"));
    }

    #[test]
    fn relationship_first_id_for_null_to_one_is_none() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToOne(None));
        assert_eq!(rel.first_id(), None);
    }

    #[test]
    fn relationship_first_id_for_to_many_returns_first() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToMany(vec![
            rid("tags", "first"),
            rid("tags", "second"),
        ]));
        assert_eq!(rel.first_id(), Some("first"));
    }

    #[test]
    fn relationship_first_id_for_empty_to_many_is_none() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToMany(vec![]));
        assert_eq!(rel.first_id(), None);
    }

    #[test]
    fn relationship_first_id_skips_lid_only_to_one() {
        let rel: Relationship<Target> =
            Relationship::new(RelationshipData::ToOne(Some(lid_rid("tags", "local"))));
        assert_eq!(rel.first_id(), None);
    }

    #[test]
    fn relationship_identifiers_for_null_to_one_is_empty() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToOne(None));
        assert!(rel.identifiers().is_empty());
    }

    #[test]
    fn relationship_identifiers_for_to_one_has_one_element() {
        let rel: Relationship<Target> =
            Relationship::new(RelationshipData::ToOne(Some(rid("people", "9"))));
        let slice = rel.identifiers();
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].type_, "people");
    }

    #[test]
    fn relationship_identifiers_for_to_many_returns_all() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToMany(vec![
            rid("tags", "1"),
            lid_rid("tags", "local"),
        ]));
        assert_eq!(rel.identifiers().len(), 2);
    }

    #[test]
    fn relationship_first_id_or_lid_returns_id() {
        let rel: Relationship<Target> =
            Relationship::new(RelationshipData::ToOne(Some(rid("tags", "42"))));
        assert_eq!(rel.first_id_or_lid(), Some("42"));
    }

    #[test]
    fn relationship_first_id_or_lid_returns_lid_when_that_is_all_there_is() {
        let rel: Relationship<Target> =
            Relationship::new(RelationshipData::ToOne(Some(lid_rid("tags", "local-a"))));
        assert_eq!(rel.first_id_or_lid(), Some("local-a"));
    }

    #[test]
    fn relationship_first_id_or_lid_returns_first_of_to_many() {
        let rel: Relationship<Target> = Relationship::new(RelationshipData::ToMany(vec![
            lid_rid("tags", "local"),
            rid("tags", "server-id"),
        ]));
        assert_eq!(rel.first_id_or_lid(), Some("local"));
    }
}
