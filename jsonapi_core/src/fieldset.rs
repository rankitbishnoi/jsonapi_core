//! Sparse fieldset filtering.
//!
//! [`FieldsetConfig`] specifies which fields to include per resource type (matching
//! the `?fields[type]=field1,field2` query parameter). Two filtering paths are
//! available: [`SparseSerializer`] wraps a typed [`ResourceObject`]
//! and filters at serialization time, while [`sparse_filter()`] takes a raw
//! `serde_json::Value` document and returns a filtered clone.

use std::collections::{HashMap, HashSet};

use serde::ser::Serialize;

use crate::model::ResourceObject;

/// Configuration for sparse fieldset filtering.
#[must_use]
#[derive(Debug, Clone)]
pub struct FieldsetConfig {
    fields: HashMap<String, HashSet<String>>,
}

impl FieldsetConfig {
    /// Create an empty fieldset configuration. Resources with no entry pass through unfiltered.
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    /// Specify which fields to include for a resource type.
    pub fn fields(mut self, type_name: &str, fields: &[&str]) -> Self {
        self.fields.insert(
            type_name.to_string(),
            fields.iter().map(|f| f.to_string()).collect(),
        );
        self
    }

    /// Check whether this config has an entry for the given type.
    pub fn has_type(&self, type_name: &str) -> bool {
        self.fields.contains_key(type_name)
    }

    /// Returns true if: type has no fieldset entry, OR the field is in the type's fieldset list.
    pub fn is_included(&self, type_name: &str, field_name: &str) -> bool {
        match self.fields.get(type_name) {
            None => true,
            Some(allowed) => allowed.contains(field_name),
        }
    }
}

impl Default for FieldsetConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply fieldset filtering to a mutable JSON object's attributes and relationships.
fn filter_resource_fields(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    type_name: &str,
    config: &FieldsetConfig,
) {
    if let Some(attrs) = obj.get_mut("attributes").and_then(|v| v.as_object_mut()) {
        attrs.retain(|key, _| config.is_included(type_name, key));
    }
    if obj
        .get("attributes")
        .and_then(|v| v.as_object())
        .is_some_and(|m| m.is_empty())
    {
        obj.remove("attributes");
    }

    if let Some(rels) = obj.get_mut("relationships").and_then(|v| v.as_object_mut()) {
        rels.retain(|key, _| config.is_included(type_name, key));
    }
    if obj
        .get("relationships")
        .and_then(|v| v.as_object())
        .is_some_and(|m| m.is_empty())
    {
        obj.remove("relationships");
    }
}

/// Wrapper that serializes a ResourceObject with sparse fieldset filtering.
pub struct SparseSerializer<'a, T: ResourceObject> {
    resource: &'a T,
    config: &'a FieldsetConfig,
}

impl<T: ResourceObject + std::fmt::Debug> std::fmt::Debug for SparseSerializer<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SparseSerializer")
            .field("resource", &self.resource)
            .field("config", &self.config)
            .finish()
    }
}

impl<'a, T: ResourceObject> SparseSerializer<'a, T> {
    /// Create a sparse serializer that filters the resource's fields during serialization.
    ///
    /// If the config has no entry for the resource's type, serialization is unfiltered.
    #[must_use]
    pub fn new(resource: &'a T, config: &'a FieldsetConfig) -> Self {
        Self { resource, config }
    }
}

impl<T: ResourceObject> Serialize for SparseSerializer<'_, T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let type_name = self.resource.resource_type();

        if !self.config.has_type(type_name) {
            return self.resource.serialize(serializer);
        }

        let mut value = serde_json::to_value(self.resource).map_err(serde::ser::Error::custom)?;

        if let Some(obj) = value.as_object_mut() {
            filter_resource_fields(obj, type_name, self.config);
        }

        value.serialize(serializer)
    }
}

/// Filter a JSON:API document Value according to sparse fieldset config.
#[must_use]
pub fn sparse_filter(document: &serde_json::Value, config: &FieldsetConfig) -> serde_json::Value {
    let mut doc = document.clone();

    if let Some(obj) = doc.as_object_mut() {
        if let Some(data) = obj.get_mut("data") {
            filter_resource_or_array(data, config);
        }
        if let Some(included) = obj.get_mut("included").and_then(|v| v.as_array_mut()) {
            for resource in included.iter_mut() {
                filter_single_resource(resource, config);
            }
        }
    }

    doc
}

fn filter_resource_or_array(value: &mut serde_json::Value, config: &FieldsetConfig) {
    match value {
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                filter_single_resource(item, config);
            }
        }
        serde_json::Value::Object(_) => {
            filter_single_resource(value, config);
        }
        _ => {}
    }
}

fn filter_single_resource(value: &mut serde_json::Value, config: &FieldsetConfig) {
    let type_name = match value.get("type").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => return,
    };

    if !config.has_type(&type_name) {
        return;
    }

    if let Some(obj) = value.as_object_mut() {
        filter_resource_fields(obj, &type_name, config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fieldset_config_new() {
        let config = FieldsetConfig::new();
        assert!(!config.has_type("articles"));
    }

    #[test]
    fn test_fieldset_config_fields() {
        let config = FieldsetConfig::new()
            .fields("articles", &["title", "body"])
            .fields("people", &["name"]);
        assert!(config.has_type("articles"));
        assert!(config.has_type("people"));
        assert!(!config.has_type("tags"));
    }

    #[test]
    fn test_is_included_with_fieldset() {
        let config = FieldsetConfig::new().fields("articles", &["title"]);
        assert!(config.is_included("articles", "title"));
        assert!(!config.is_included("articles", "body"));
    }

    #[test]
    fn test_is_included_no_fieldset_passes_all() {
        let config = FieldsetConfig::new();
        assert!(config.is_included("articles", "title"));
        assert!(config.is_included("anything", "whatever"));
    }

    #[test]
    fn test_is_included_unknown_field_in_config() {
        let config = FieldsetConfig::new().fields("articles", &["title", "nonexistent"]);
        assert!(config.is_included("articles", "title"));
        assert!(!config.is_included("articles", "body"));
        assert!(config.is_included("articles", "nonexistent"));
    }

    #[test]
    fn test_sparse_filter_single_resource() {
        let doc = serde_json::json!({
            "data": {
                "type": "articles", "id": "1",
                "attributes": {"title": "Hello", "body": "World"},
                "relationships": {
                    "author": {"data": {"type": "people", "id": "9"}}
                }
            }
        });
        let config = FieldsetConfig::new().fields("articles", &["title"]);
        let filtered = sparse_filter(&doc, &config);
        assert_eq!(filtered["data"]["type"], "articles");
        assert_eq!(filtered["data"]["id"], "1");
        assert_eq!(filtered["data"]["attributes"]["title"], "Hello");
        assert!(filtered["data"]["attributes"].get("body").is_none());
        assert!(filtered["data"].get("relationships").is_none());
    }

    #[test]
    fn test_sparse_filter_array_data() {
        let doc = serde_json::json!({
            "data": [
                {"type": "articles", "id": "1", "attributes": {"title": "First", "body": "One"}},
                {"type": "articles", "id": "2", "attributes": {"title": "Second", "body": "Two"}}
            ]
        });
        let config = FieldsetConfig::new().fields("articles", &["title"]);
        let filtered = sparse_filter(&doc, &config);
        assert_eq!(filtered["data"][0]["attributes"]["title"], "First");
        assert!(filtered["data"][0]["attributes"].get("body").is_none());
        assert_eq!(filtered["data"][1]["attributes"]["title"], "Second");
        assert!(filtered["data"][1]["attributes"].get("body").is_none());
    }

    #[test]
    fn test_sparse_filter_included() {
        let doc = serde_json::json!({
            "data": {
                "type": "articles", "id": "1",
                "attributes": {"title": "Hello", "body": "World"}
            },
            "included": [{
                "type": "people", "id": "9",
                "attributes": {"name": "Dan", "email": "dan@example.com"}
            }]
        });
        let config = FieldsetConfig::new()
            .fields("articles", &["title"])
            .fields("people", &["name"]);
        let filtered = sparse_filter(&doc, &config);
        assert_eq!(filtered["data"]["attributes"]["title"], "Hello");
        assert!(filtered["data"]["attributes"].get("body").is_none());
        assert_eq!(filtered["included"][0]["attributes"]["name"], "Dan");
        assert!(filtered["included"][0]["attributes"].get("email").is_none());
    }

    #[test]
    fn test_sparse_filter_no_config_passes_through() {
        let doc = serde_json::json!({
            "data": {
                "type": "articles", "id": "1",
                "attributes": {"title": "Hello", "body": "World"}
            }
        });
        let config = FieldsetConfig::new();
        let filtered = sparse_filter(&doc, &config);
        assert_eq!(filtered["data"]["attributes"]["title"], "Hello");
        assert_eq!(filtered["data"]["attributes"]["body"], "World");
    }

    #[test]
    fn test_sparse_filter_null_data() {
        let doc = serde_json::json!({
            "data": null
        });
        let config = FieldsetConfig::new().fields("articles", &["title"]);
        let filtered = sparse_filter(&doc, &config);
        assert!(filtered["data"].is_null());
    }

    #[test]
    fn test_sparse_filter_preserves_non_data_fields() {
        let doc = serde_json::json!({
            "data": {
                "type": "articles", "id": "1",
                "attributes": {"title": "Hello"}
            },
            "meta": {"total": 1},
            "jsonapi": {"version": "1.1"}
        });
        let config = FieldsetConfig::new().fields("articles", &["title"]);
        let filtered = sparse_filter(&doc, &config);
        assert_eq!(filtered["meta"]["total"], 1);
        assert_eq!(filtered["jsonapi"]["version"], "1.1");
    }
}
