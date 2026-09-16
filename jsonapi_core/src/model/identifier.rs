use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Meta;

/// Server-assigned `id` vs client-local `lid` (JSON:API 1.1).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Identity {
    /// Server-assigned identifier.
    Id(String),
    /// Client-generated local identifier (JSON:API 1.1).
    Lid(String),
}

impl Identity {
    /// `Some(&str)` for a server-assigned `Id`, `None` for `Lid` or future
    /// `#[non_exhaustive]` variants.
    #[must_use]
    pub fn as_id(&self) -> Option<&str> {
        match self {
            Identity::Id(id) => Some(id),
            _ => None,
        }
    }

    /// `Some(&str)` for a client-local `Lid`, `None` for `Id` or future
    /// `#[non_exhaustive]` variants.
    #[must_use]
    pub fn as_lid(&self) -> Option<&str> {
        match self {
            Identity::Lid(lid) => Some(lid),
            _ => None,
        }
    }
}

impl Default for Identity {
    /// An empty server-assigned id — a scaffold for builder-style construction.
    fn default() -> Self {
        Identity::Id(String::new())
    }
}

/// JSON:API resource identifier object.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResourceIdentifier {
    /// The JSON:API type string.
    pub r#type: String,
    /// Server-assigned id or client-local lid.
    pub identity: Identity,
    /// Optional meta information.
    pub meta: Option<Meta>,
}

impl ResourceIdentifier {
    /// Build a resource identifier for a server-assigned `id`.
    ///
    /// ```
    /// # use jsonapi_core::{ResourceIdentifier, Identity};
    /// let rid = ResourceIdentifier::new("authors", "42");
    /// assert_eq!(rid.r#type, "authors");
    /// assert_eq!(rid.identity, Identity::Id("42".into()));
    /// ```
    #[must_use]
    pub fn new(r#type: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            r#type: r#type.into(),
            identity: Identity::Id(id.into()),
            meta: None,
        }
    }

    /// Build a resource identifier for a client-local `lid` (JSON:API 1.1).
    ///
    /// ```
    /// # use jsonapi_core::{ResourceIdentifier, Identity};
    /// let rid = ResourceIdentifier::with_lid("authors", "local-1");
    /// assert_eq!(rid.identity, Identity::Lid("local-1".into()));
    /// ```
    #[must_use]
    pub fn with_lid(r#type: impl Into<String>, lid: impl Into<String>) -> Self {
        Self {
            r#type: r#type.into(),
            identity: Identity::Lid(lid.into()),
            meta: None,
        }
    }

    /// Build a `Vec` of server-assigned identifiers sharing one `type` from an
    /// iterator of ids — the common shape for a to-many relationship's linkage.
    ///
    /// ```
    /// # use jsonapi_core::{Relationship, ResourceIdentifier};
    /// let rel = Relationship::<()>::to_many(ResourceIdentifier::many("tags", ["1", "2"]));
    /// assert_eq!(rel.ids().collect::<Vec<_>>(), ["1", "2"]);
    /// ```
    #[must_use]
    pub fn many(
        r#type: impl Into<String>,
        ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Vec<Self> {
        let r#type = r#type.into();
        ids.into_iter()
            .map(|id| Self {
                r#type: r#type.clone(),
                identity: Identity::Id(id.into()),
                meta: None,
            })
            .collect()
    }

    /// The server-assigned `id`, or an error when this identifier carries only a
    /// client-local `lid`. Use at a relationship endpoint that maps linkage to
    /// datastore ids and must reject `lid`-only members rather than skip them.
    ///
    /// # Errors
    /// [`Error::LidNotIndexed`](crate::Error::LidNotIndexed) when the identity is
    /// a `lid`.
    pub fn require_id(&self) -> crate::Result<&str> {
        self.identity.as_id().ok_or(crate::Error::LidNotIndexed)
    }
}

/// Borrowing representation used for serialization.
#[derive(Serialize)]
struct ResourceIdentifierSerRepr<'a> {
    #[serde(rename = "type")]
    type_: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lid: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<&'a Meta>,
}

/// Owned representation used for deserialization.
#[derive(Deserialize)]
struct ResourceIdentifierDeRepr {
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    lid: Option<String>,
    #[serde(default)]
    meta: Option<Meta>,
}

impl Serialize for ResourceIdentifier {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (id, lid) = match &self.identity {
            Identity::Id(id) => (Some(id.as_str()), None),
            Identity::Lid(lid) => (None, Some(lid.as_str())),
        };
        ResourceIdentifierSerRepr {
            type_: &self.r#type,
            id,
            lid,
            meta: self.meta.as_ref(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ResourceIdentifier {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let repr = ResourceIdentifierDeRepr::deserialize(deserializer)?;
        let identity = match (repr.id, repr.lid) {
            (Some(_), Some(_)) => {
                return Err(serde::de::Error::custom(
                    "resource identifier must not have both `id` and `lid`",
                ));
            }
            (Some(id), None) => Identity::Id(id),
            (None, Some(lid)) => Identity::Lid(lid),
            (None, None) => {
                return Err(serde::de::Error::custom(
                    "resource identifier must have `id` or `lid`",
                ));
            }
        };
        Ok(ResourceIdentifier {
            r#type: repr.type_,
            identity,
            meta: repr.meta,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_as_id_variant() {
        let id = Identity::Id("42".into());
        assert_eq!(id.as_id(), Some("42"));
        assert_eq!(id.as_lid(), None);
    }

    #[test]
    fn test_identity_as_lid_variant() {
        let lid = Identity::Lid("local-1".into());
        assert_eq!(lid.as_id(), None);
        assert_eq!(lid.as_lid(), Some("local-1"));
    }

    #[test]
    fn test_resource_identifier_new_builds_id() {
        let rid = ResourceIdentifier::new("people", "1");
        assert_eq!(rid.r#type, "people");
        assert_eq!(rid.identity, Identity::Id("1".into()));
        assert_eq!(rid.meta, None);
        assert_eq!(
            serde_json::to_string(&rid).unwrap(),
            r#"{"type":"people","id":"1"}"#
        );
    }

    #[test]
    fn test_resource_identifier_with_lid_builds_lid() {
        let rid = ResourceIdentifier::with_lid("people", "local-1");
        assert_eq!(rid.r#type, "people");
        assert_eq!(rid.identity, Identity::Lid("local-1".into()));
        assert_eq!(rid.meta, None);
        assert_eq!(
            serde_json::to_string(&rid).unwrap(),
            r#"{"type":"people","lid":"local-1"}"#
        );
    }

    #[test]
    fn test_resource_identifier_many_builds_shared_type_ids() {
        let rids = ResourceIdentifier::many("tags", ["1", "2", "3"]);
        assert_eq!(rids.len(), 3);
        assert!(rids.iter().all(|r| r.r#type == "tags"));
        assert_eq!(rids[2].identity, Identity::Id("3".into()));
    }

    #[test]
    fn test_resource_identifier_require_id_ok_for_id() {
        let rid = ResourceIdentifier::new("people", "9");
        assert_eq!(rid.require_id().unwrap(), "9");
    }

    #[test]
    fn test_resource_identifier_require_id_errors_for_lid() {
        let rid = ResourceIdentifier::with_lid("people", "local-1");
        assert!(matches!(rid.require_id(), Err(crate::Error::LidNotIndexed)));
    }

    #[test]
    fn test_resource_identifier_with_id() {
        let json = r#"{"type":"people","id":"1"}"#;
        let rid: ResourceIdentifier = serde_json::from_str(json).unwrap();
        assert_eq!(rid.r#type, "people");
        assert_eq!(rid.identity, Identity::Id("1".into()));
        assert_eq!(rid.meta, None);

        let serialized = serde_json::to_string(&rid).unwrap();
        assert_eq!(serialized, json);
    }

    #[test]
    fn test_resource_identifier_with_lid() {
        let json = r#"{"type":"people","lid":"local-1"}"#;
        let rid: ResourceIdentifier = serde_json::from_str(json).unwrap();
        assert_eq!(rid.identity, Identity::Lid("local-1".into()));

        let serialized = serde_json::to_string(&rid).unwrap();
        assert_eq!(serialized, json);
    }

    #[test]
    fn test_resource_identifier_with_meta() {
        let json = r#"{"type":"articles","id":"5","meta":{"created":true}}"#;
        let rid: ResourceIdentifier = serde_json::from_str(json).unwrap();
        assert_eq!(rid.r#type, "articles");
        assert!(rid.meta.is_some());
        assert_eq!(
            rid.meta.as_ref().unwrap()["created"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn test_resource_identifier_missing_identity() {
        let json = r#"{"type":"people"}"#;
        let result: std::result::Result<ResourceIdentifier, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_resource_identifier_rejects_both_id_and_lid() {
        let json = r#"{"type":"people","id":"1","lid":"local-1"}"#;
        let result: std::result::Result<ResourceIdentifier, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_resource_identifier_empty_id() {
        // JSON:API 1.1 does not forbid empty-string IDs at the wire level,
        // so deserialization should succeed. Validation is a separate concern.
        let json = r#"{"type":"people","id":""}"#;
        let rid: ResourceIdentifier = serde_json::from_str(json).unwrap();
        assert_eq!(rid.identity, Identity::Id("".into()));
    }
}
