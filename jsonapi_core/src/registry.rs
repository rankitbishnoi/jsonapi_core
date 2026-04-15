//! Included resource registry and recursive resolver.
//!
//! The [`Registry`] is a lookup table populated from a document's `included` array.
//! It provides typed lookups via [`get()`](Registry::get) and
//! [`get_many()`](Registry::get_many), and a dynamic
//! [`resolve()`](Registry::resolve) method that produces kitsu-core-style
//! flattened output with cycle detection.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

use serde::de::DeserializeOwned;

use crate::error::Error;
use crate::model::{Identity, Relationship, RelationshipData, ResourceObject};

/// Lookup table populated from the `included` array.
/// Keyed by type then id for O(1) lookups without per-call allocation.
#[derive(Debug)]
pub struct Registry {
    resources: HashMap<String, BTreeMap<String, serde_json::Value>>,
}

/// Configuration for recursive resolution.
#[derive(Debug, Clone)]
pub struct ResolveConfig {
    /// Maximum recursion depth. Default: 10.
    pub max_depth: usize,
}

impl Default for ResolveConfig {
    fn default() -> Self {
        Self { max_depth: 10 }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
        }
    }

    /// Internal zero-allocation lookup by type and id.
    fn lookup(&self, type_: &str, id: &str) -> Option<&serde_json::Value> {
        self.resources.get(type_).and_then(|by_id| by_id.get(id))
    }

    /// Build from the included array of a deserialized document.
    #[must_use = "registry should be stored for subsequent lookups"]
    pub fn from_included<T: ResourceObject>(included: &[T]) -> crate::Result<Self> {
        let mut resources: HashMap<String, BTreeMap<String, serde_json::Value>> = HashMap::new();
        for item in included {
            let type_ = item.resource_type().to_string();
            if let Some(id) = item.resource_id() {
                let value = serde_json::to_value(item)?;
                resources
                    .entry(type_)
                    .or_default()
                    .insert(id.to_string(), value);
            }
        }
        Ok(Self { resources })
    }

    /// Look up a related resource by its relationship reference.
    /// Only works for to-one relationships with a server-assigned id.
    #[must_use = "registry lookup result should be used"]
    pub fn get<T: DeserializeOwned>(&self, rel: &Relationship<T>) -> Result<T, Error> {
        match &rel.data {
            RelationshipData::ToOne(Some(rid)) => {
                let id = match &rid.identity {
                    Identity::Id(id) => id,
                    Identity::Lid(_) => return Err(Error::LidNotIndexed),
                };
                self.get_by_id(&rid.type_, id)
            }
            RelationshipData::ToOne(None) => Err(Error::NullRelationship),
            RelationshipData::ToMany(_) => {
                Err(Error::RelationshipCardinalityMismatch { expected: "to-one" })
            }
        }
    }

    /// Resolve a to-many relationship to a Vec of typed resources.
    /// Returns `Error::RelationshipCardinalityMismatch` if called on a to-one relationship.
    #[must_use = "registry lookup result should be used"]
    pub fn get_many<T: DeserializeOwned>(&self, rel: &Relationship<T>) -> Result<Vec<T>, Error> {
        match &rel.data {
            RelationshipData::ToMany(rids) => {
                let mut results = Vec::with_capacity(rids.len());
                for rid in rids {
                    let id = match &rid.identity {
                        Identity::Id(id) => id,
                        Identity::Lid(_) => return Err(Error::LidNotIndexed),
                    };
                    results.push(self.get_by_id(&rid.type_, id)?);
                }
                Ok(results)
            }
            RelationshipData::ToOne(_) => Err(Error::RelationshipCardinalityMismatch {
                expected: "to-many",
            }),
        }
    }

    /// Get all resources matching a type string, deserialized as T.
    /// Entries that fail to deserialize as T are silently omitted — this is
    /// intentional since the registry may contain different resource shapes.
    #[must_use]
    pub fn get_all<T: DeserializeOwned>(&self, type_: &str) -> Vec<T> {
        self.resources
            .get(type_)
            .map(|by_id| {
                by_id
                    .values()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Look up by explicit type and id.
    #[must_use = "registry lookup result should be used"]
    pub fn get_by_id<T: DeserializeOwned>(&self, type_: &str, id: &str) -> Result<T, Error> {
        match self.lookup(type_, id) {
            Some(value) => serde_json::from_value(value.clone()).map_err(Error::Json),
            None => Err(Error::RegistryLookup {
                type_: type_.to_string(),
                id: id.to_string(),
            }),
        }
    }

    /// Recursively resolve a JSON:API resource into kitsu-core-style flattened output.
    ///
    /// Attributes are hoisted onto the resource object. Relationships are replaced
    /// with full resolved resources from the registry. Relationship-level links/meta
    /// are dropped. Resource-level meta/links are preserved.
    ///
    /// Cycles are detected and broken (left as `{type, id}` identifiers).
    /// Missing resources are left as `{type, id}` identifiers.
    #[must_use]
    pub fn resolve(&self, value: &serde_json::Value, config: &ResolveConfig) -> serde_json::Value {
        let mut ancestors = HashSet::new();
        // Add root resource to ancestors so back-references to it are detected as cycles
        if let Some((type_, id)) = self.extract_type_id(value) {
            ancestors.insert((type_.to_string(), id.to_string()));
        }
        self.resolve_inner(value, &mut ancestors, 0, config)
    }

    fn resolve_inner(
        &self,
        value: &serde_json::Value,
        ancestors: &mut HashSet<(String, String)>,
        depth: usize,
        config: &ResolveConfig,
    ) -> serde_json::Value {
        let obj = match value.as_object() {
            Some(obj) => obj,
            None => return value.clone(),
        };

        // Must have "type" to be a JSON:API resource worth flattening
        if !obj.contains_key("type") {
            return value.clone();
        }

        let mut flat = serde_json::Map::new();

        // Hoist type, id, lid
        if let Some(v) = obj.get("type") {
            flat.insert("type".into(), v.clone());
        }
        if let Some(v) = obj.get("id") {
            flat.insert("id".into(), v.clone());
        }
        if let Some(v) = obj.get("lid") {
            flat.insert("lid".into(), v.clone());
        }

        // Hoist attributes
        if let Some(attrs) = obj.get("attributes").and_then(|v| v.as_object()) {
            for (k, v) in attrs {
                flat.insert(k.clone(), v.clone());
            }
        }

        // Process relationships
        if let Some(rels) = obj.get("relationships").and_then(|v| v.as_object()) {
            for (name, rel_obj) in rels {
                let data = rel_obj.as_object().and_then(|r| r.get("data"));
                let resolved = match data {
                    None => continue,
                    Some(serde_json::Value::Null) => serde_json::Value::Null,
                    Some(serde_json::Value::Array(arr)) => {
                        if depth < config.max_depth {
                            let items: Vec<serde_json::Value> = arr
                                .iter()
                                .map(|item| self.resolve_identifier(item, ancestors, depth, config))
                                .collect();
                            serde_json::Value::Array(items)
                        } else {
                            serde_json::Value::Array(arr.clone())
                        }
                    }
                    Some(identifier) => {
                        if depth < config.max_depth {
                            self.resolve_identifier(identifier, ancestors, depth, config)
                        } else {
                            identifier.clone()
                        }
                    }
                };
                flat.insert(name.clone(), resolved);
            }
        }

        // Hoist resource-level meta, links
        if let Some(v) = obj.get("meta") {
            flat.insert("meta".into(), v.clone());
        }
        if let Some(v) = obj.get("links") {
            flat.insert("links".into(), v.clone());
        }

        serde_json::Value::Object(flat)
    }

    fn resolve_identifier(
        &self,
        identifier: &serde_json::Value,
        ancestors: &mut HashSet<(String, String)>,
        depth: usize,
        config: &ResolveConfig,
    ) -> serde_json::Value {
        let (type_, id) = match self.extract_type_id(identifier) {
            Some(pair) => pair,
            None => return identifier.clone(),
        };

        let key = (type_.to_string(), id.to_string());

        // Cycle detection: check ancestor chain
        if ancestors.contains(&key) {
            return identifier.clone();
        }

        // Look up in registry
        let resource = match self.lookup(type_, id) {
            Some(v) => v,
            None => return identifier.clone(),
        };

        // Recurse with this resource as an ancestor
        ancestors.insert(key.clone());
        let resolved = self.resolve_inner(resource, ancestors, depth + 1, config);
        ancestors.remove(&key);

        resolved
    }

    fn extract_type_id<'a>(&self, value: &'a serde_json::Value) -> Option<(&'a str, &'a str)> {
        let obj = value.as_object()?;
        let type_ = obj.get("type")?.as_str()?;
        let id = obj.get("id")?.as_str()?;
        Some((type_, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Identity, Relationship, RelationshipData, Resource, ResourceIdentifier};
    use std::collections::BTreeMap;

    fn make_resource(type_: &str, id: &str, attrs: serde_json::Value) -> Resource {
        Resource {
            type_: type_.into(),
            id: Some(id.into()),
            lid: None,
            attributes: attrs,
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        }
    }

    fn make_resource_with_rels(
        type_: &str,
        id: &str,
        attrs: serde_json::Value,
        rels: BTreeMap<String, RelationshipData>,
    ) -> Resource {
        Resource {
            type_: type_.into(),
            id: Some(id.into()),
            lid: None,
            attributes: attrs,
            relationships: rels,
            links: None,
            meta: None,
        }
    }

    #[test]
    fn test_registry_get_by_id() {
        let included = vec![
            make_resource("people", "9", serde_json::json!({"name": "Dan"})),
            make_resource("comments", "5", serde_json::json!({"body": "Hi"})),
        ];
        let registry = Registry::from_included(&included).unwrap();

        let result: Resource = registry.get_by_id("people", "9").unwrap();
        assert_eq!(result.resource_type(), "people");
        assert_eq!(result.attributes["name"], "Dan");
    }

    #[test]
    fn test_registry_get_missing() {
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let result: std::result::Result<Resource, _> = registry.get_by_id("people", "99");
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::Error::RegistryLookup { type_, id } => {
                assert_eq!(type_, "people");
                assert_eq!(id, "99");
            }
            other => panic!("expected RegistryLookup, got: {other}"),
        }
    }

    #[test]
    fn test_registry_get_via_relationship() {
        let included = vec![make_resource(
            "people",
            "9",
            serde_json::json!({"name": "Dan"}),
        )];
        let registry = Registry::from_included(&included).unwrap();

        let rel: Relationship<Resource> =
            Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
                type_: "people".into(),
                identity: Identity::Id("9".into()),
                meta: None,
            })));

        let person: Resource = registry.get(&rel).unwrap();
        assert_eq!(person.attributes["name"], "Dan");
    }

    #[test]
    fn test_registry_get_null_relationship() {
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let rel: Relationship<Resource> = Relationship::new(RelationshipData::ToOne(None));
        let result: std::result::Result<Resource, _> = registry.get(&rel);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_many_resolves() {
        let included = vec![
            make_resource("tags", "1", serde_json::json!({"label": "rust"})),
            make_resource("tags", "2", serde_json::json!({"label": "serde"})),
        ];
        let registry = Registry::from_included(&included).unwrap();
        let rel: Relationship<Resource> = Relationship::new(RelationshipData::ToMany(vec![
            ResourceIdentifier {
                type_: "tags".into(),
                identity: Identity::Id("1".into()),
                meta: None,
            },
            ResourceIdentifier {
                type_: "tags".into(),
                identity: Identity::Id("2".into()),
                meta: None,
            },
        ]));
        let tags: Vec<Resource> = registry.get_many(&rel).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].attributes["label"], "rust");
        assert_eq!(tags[1].attributes["label"], "serde");
    }

    #[test]
    fn test_get_many_empty() {
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let rel: Relationship<Resource> = Relationship::new(RelationshipData::ToMany(vec![]));
        let tags: Vec<Resource> = registry.get_many(&rel).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn test_get_many_missing_resource() {
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let rel: Relationship<Resource> =
            Relationship::new(RelationshipData::ToMany(vec![ResourceIdentifier {
                type_: "tags".into(),
                identity: Identity::Id("99".into()),
                meta: None,
            }]));
        let result: crate::Result<Vec<Resource>> = registry.get_many(&rel);
        assert!(matches!(
            result.unwrap_err(),
            crate::Error::RegistryLookup { .. }
        ));
    }

    #[test]
    fn test_get_many_on_to_one_errors() {
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let rel: Relationship<Resource> = Relationship::new(RelationshipData::ToOne(None));
        let result: crate::Result<Vec<Resource>> = registry.get_many(&rel);
        assert!(matches!(
            result.unwrap_err(),
            crate::Error::RelationshipCardinalityMismatch { .. }
        ));
    }

    #[test]
    fn test_get_all() {
        let included = vec![
            make_resource("people", "1", serde_json::json!({"name": "Alice"})),
            make_resource("people", "2", serde_json::json!({"name": "Bob"})),
            make_resource("tags", "1", serde_json::json!({"label": "rust"})),
        ];
        let registry = Registry::from_included(&included).unwrap();
        let people: Vec<Resource> = registry.get_all("people");
        assert_eq!(people.len(), 2);
        let tags: Vec<Resource> = registry.get_all("tags");
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn test_get_all_missing_type() {
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let people: Vec<Resource> = registry.get_all("people");
        assert!(people.is_empty());
    }

    #[test]
    fn test_get_all_skips_deserialization_failures() {
        // Insert three JSON:API resource envelopes under "people", but one has
        // a missing "id" (required by Resource). Deserializing as Resource should
        // succeed for the two valid ones and silently skip the malformed one.
        let mut resources: HashMap<String, BTreeMap<String, serde_json::Value>> = HashMap::new();
        let by_id = resources.entry("people".into()).or_default();

        // Valid envelope
        by_id.insert(
            "1".into(),
            serde_json::json!({
                "type": "people", "id": "1",
                "attributes": {"name": "Alice"},
                "relationships": {}
            }),
        );
        // Malformed: missing "type" field entirely — will fail Resource deserialization
        by_id.insert(
            "2".into(),
            serde_json::json!({
                "id": "2",
                "attributes": {"name": "Bad"}
            }),
        );
        // Valid envelope
        by_id.insert(
            "3".into(),
            serde_json::json!({
                "type": "people", "id": "3",
                "attributes": {"name": "Bob"},
                "relationships": {}
            }),
        );

        let registry = Registry { resources };
        let people: Vec<Resource> = registry.get_all("people");
        // Only 2 of 3 should succeed; the malformed one is silently skipped
        assert_eq!(people.len(), 2);
    }

    #[test]
    fn test_registry_multiple_types() {
        let included = vec![
            make_resource("people", "1", serde_json::json!({"name": "Alice"})),
            make_resource("people", "2", serde_json::json!({"name": "Bob"})),
            make_resource("tags", "1", serde_json::json!({"label": "rust"})),
        ];
        let registry = Registry::from_included(&included).unwrap();

        let person: Resource = registry.get_by_id("people", "2").unwrap();
        assert_eq!(person.attributes["name"], "Bob");

        let tag: Resource = registry.get_by_id("tags", "1").unwrap();
        assert_eq!(tag.attributes["label"], "rust");

        // people/1 and tags/1 are distinct
        let p1: Resource = registry.get_by_id("people", "1").unwrap();
        assert_eq!(p1.attributes["name"], "Alice");
    }

    #[test]
    fn test_resolve_simple_flatten_and_resolve() {
        let author = make_resource("people", "9", serde_json::json!({"name": "Dan"}));
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([(
                "author".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "people".into(),
                    identity: Identity::Id("9".into()),
                    meta: None,
                })),
            )]),
        );
        let registry = Registry::from_included(&[author]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        assert_eq!(resolved["type"], "articles");
        assert_eq!(resolved["id"], "1");
        assert_eq!(resolved["title"], "Hello");
        assert_eq!(resolved["author"]["type"], "people");
        assert_eq!(resolved["author"]["id"], "9");
        assert_eq!(resolved["author"]["name"], "Dan");
        // Envelope stripped
        assert!(resolved.get("attributes").is_none());
        assert!(resolved.get("relationships").is_none());
    }

    #[test]
    fn test_resolve_to_many() {
        let tag1 = make_resource("tags", "1", serde_json::json!({"label": "rust"}));
        let tag2 = make_resource("tags", "2", serde_json::json!({"label": "serde"}));
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([(
                "tags".to_string(),
                RelationshipData::ToMany(vec![
                    ResourceIdentifier {
                        type_: "tags".into(),
                        identity: Identity::Id("1".into()),
                        meta: None,
                    },
                    ResourceIdentifier {
                        type_: "tags".into(),
                        identity: Identity::Id("2".into()),
                        meta: None,
                    },
                ]),
            )]),
        );
        let registry = Registry::from_included(&[tag1, tag2]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        assert_eq!(resolved["tags"][0]["label"], "rust");
        assert_eq!(resolved["tags"][1]["label"], "serde");
    }

    #[test]
    fn test_resolve_no_relationships() {
        let resource = make_resource("articles", "1", serde_json::json!({"title": "Hello"}));
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let input = serde_json::to_value(&resource).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        assert_eq!(resolved["type"], "articles");
        assert_eq!(resolved["title"], "Hello");
        assert!(resolved.get("attributes").is_none());
    }

    #[test]
    fn test_resolve_nested() {
        let org = make_resource("orgs", "5", serde_json::json!({"name": "Acme"}));
        let person = make_resource_with_rels(
            "people",
            "9",
            serde_json::json!({"name": "Dan"}),
            BTreeMap::from([(
                "org".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "orgs".into(),
                    identity: Identity::Id("5".into()),
                    meta: None,
                })),
            )]),
        );
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([(
                "author".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "people".into(),
                    identity: Identity::Id("9".into()),
                    meta: None,
                })),
            )]),
        );
        let registry = Registry::from_included(&[person, org]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        assert_eq!(resolved["author"]["name"], "Dan");
        assert_eq!(resolved["author"]["org"]["name"], "Acme");
    }

    #[test]
    fn test_resolve_cycle() {
        // Article 1 -> author Person 9 -> articles [Article 1] (cycle)
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([(
                "author".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "people".into(),
                    identity: Identity::Id("9".into()),
                    meta: None,
                })),
            )]),
        );
        let person = make_resource_with_rels(
            "people",
            "9",
            serde_json::json!({"name": "Dan"}),
            BTreeMap::from([(
                "articles".to_string(),
                RelationshipData::ToMany(vec![ResourceIdentifier {
                    type_: "articles".into(),
                    identity: Identity::Id("1".into()),
                    meta: None,
                }]),
            )]),
        );
        let registry = Registry::from_included(&[article.clone(), person]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        // Author resolved
        assert_eq!(resolved["author"]["name"], "Dan");
        // Back-reference to article is left as identifier (cycle broken)
        assert_eq!(resolved["author"]["articles"][0]["type"], "articles");
        assert_eq!(resolved["author"]["articles"][0]["id"], "1");
        assert!(resolved["author"]["articles"][0].get("title").is_none());
    }

    #[test]
    fn test_resolve_missing_resource() {
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([(
                "author".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "people".into(),
                    identity: Identity::Id("99".into()),
                    meta: None,
                })),
            )]),
        );
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        // Author left as identifier
        assert_eq!(resolved["author"]["type"], "people");
        assert_eq!(resolved["author"]["id"], "99");
        assert!(resolved["author"].get("name").is_none());
    }

    #[test]
    fn test_resolve_null_to_one() {
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([("author".to_string(), RelationshipData::ToOne(None))]),
        );
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        assert!(resolved["author"].is_null());
    }

    #[test]
    fn test_resolve_empty_to_many() {
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([("tags".to_string(), RelationshipData::ToMany(vec![]))]),
        );
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        assert_eq!(resolved["tags"], serde_json::json!([]));
    }

    #[test]
    fn test_resolve_depth_limit() {
        let org = make_resource("orgs", "5", serde_json::json!({"name": "Acme"}));
        let person = make_resource_with_rels(
            "people",
            "9",
            serde_json::json!({"name": "Dan"}),
            BTreeMap::from([(
                "org".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "orgs".into(),
                    identity: Identity::Id("5".into()),
                    meta: None,
                })),
            )]),
        );
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([(
                "author".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "people".into(),
                    identity: Identity::Id("9".into()),
                    meta: None,
                })),
            )]),
        );
        let registry = Registry::from_included(&[person, org]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig { max_depth: 1 });

        // Author resolved (depth 0 < max_depth 1)
        assert_eq!(resolved["author"]["name"], "Dan");
        // Org NOT resolved (depth 1 is not < max_depth 1)
        assert_eq!(resolved["author"]["org"]["type"], "orgs");
        assert_eq!(resolved["author"]["org"]["id"], "5");
        assert!(resolved["author"]["org"].get("name").is_none());
    }

    #[test]
    fn test_resolve_preserves_resource_meta_links() {
        let input = serde_json::json!({
            "type": "articles", "id": "1",
            "attributes": {"title": "Hello"},
            "meta": {"featured": true},
            "links": {"self": "/articles/1"}
        });
        let registry = Registry::from_included::<Resource>(&[]).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        assert_eq!(resolved["title"], "Hello");
        assert_eq!(resolved["meta"]["featured"], true);
        assert_eq!(resolved["links"]["self"], "/articles/1");
    }

    #[test]
    fn test_resolve_depth_zero() {
        let author = make_resource("people", "9", serde_json::json!({"name": "Dan"}));
        let article = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([(
                "author".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "people".into(),
                    identity: Identity::Id("9".into()),
                    meta: None,
                })),
            )]),
        );
        let registry = Registry::from_included(&[author]).unwrap();
        let input = serde_json::to_value(&article).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig { max_depth: 0 });

        // With max_depth: 0, no relationships should be resolved
        assert_eq!(resolved["title"], "Hello");
        assert_eq!(resolved["author"]["type"], "people");
        assert_eq!(resolved["author"]["id"], "9");
        assert!(resolved["author"].get("name").is_none());
    }

    #[test]
    fn test_from_included_skips_lid_only_resources() {
        let lid_only = Resource {
            type_: "drafts".into(),
            id: None,
            lid: Some("temp-1".into()),
            attributes: serde_json::json!({"title": "Draft"}),
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        };
        let with_id = make_resource("articles", "1", serde_json::json!({"title": "Hello"}));
        let registry = Registry::from_included(&[lid_only, with_id]).unwrap();

        // lid-only resource not findable
        let result: std::result::Result<Resource, _> = registry.get_by_id("drafts", "temp-1");
        assert!(result.is_err());

        // id-bearing resource is findable
        let article: Resource = registry.get_by_id("articles", "1").unwrap();
        assert_eq!(article.attributes["title"], "Hello");
    }

    #[test]
    fn test_resolve_two_hop_cycle() {
        // A -> B -> A (different types, two-hop cycle)
        let a = make_resource_with_rels(
            "articles",
            "1",
            serde_json::json!({"title": "Hello"}),
            BTreeMap::from([(
                "reviewer".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "people".into(),
                    identity: Identity::Id("9".into()),
                    meta: None,
                })),
            )]),
        );
        let b = make_resource_with_rels(
            "people",
            "9",
            serde_json::json!({"name": "Dan"}),
            BTreeMap::from([(
                "favorite".to_string(),
                RelationshipData::ToOne(Some(ResourceIdentifier {
                    type_: "articles".into(),
                    identity: Identity::Id("1".into()),
                    meta: None,
                })),
            )]),
        );
        let registry = Registry::from_included(&[a.clone(), b]).unwrap();
        let input = serde_json::to_value(&a).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        // Reviewer resolved
        assert_eq!(resolved["reviewer"]["name"], "Dan");
        // Back-reference to article is a cycle — left as identifier
        assert_eq!(resolved["reviewer"]["favorite"]["type"], "articles");
        assert_eq!(resolved["reviewer"]["favorite"]["id"], "1");
        assert!(resolved["reviewer"]["favorite"].get("title").is_none());
    }

    #[test]
    fn test_resolve_drops_relationship_links_meta() {
        // Construct raw JSON with relationship-level links/meta
        // (Resource struct doesn't store these, so we use json! directly)
        let input = serde_json::json!({
            "type": "articles", "id": "1",
            "attributes": {"title": "Hello"},
            "relationships": {
                "author": {
                    "data": {"type": "people", "id": "9"},
                    "links": {"self": "/articles/1/relationships/author"},
                    "meta": {"verified": true}
                }
            }
        });
        let author = make_resource("people", "9", serde_json::json!({"name": "Dan"}));
        let registry = Registry::from_included(&[author]).unwrap();
        let resolved = registry.resolve(&input, &ResolveConfig::default());

        // Author resolved from registry (which has no links/meta)
        assert_eq!(resolved["author"]["name"], "Dan");
        // Relationship-level links/meta did not leak into resolved output
        assert!(resolved["author"].get("links").is_none());
        assert!(resolved["author"].get("meta").is_none());
    }
}
