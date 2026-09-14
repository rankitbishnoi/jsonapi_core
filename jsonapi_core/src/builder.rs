//! Fluent builder for JSON:API response documents.

use std::collections::HashSet;

use crate::model::{
    Document, JsonApiObject, Link, Links, Meta, PrimaryData, Resource, ResourceObject,
};

/// Fluent builder for a JSON:API data document (`Document::Data`).
///
/// ```
/// use jsonapi_core::{DocumentBuilder, Resource};
/// # fn r(t: &str, id: &str) -> Resource {
/// #     Resource { r#type: t.into(), id: Some(id.into()), lid: None,
/// #         attributes: serde_json::json!({}), relationships: Default::default(),
/// #         links: None, meta: None }
/// # }
/// let doc = DocumentBuilder::single(r("articles", "1"))
///     .include(r("people", "9"))
///     .build();
/// ```
///
/// Every constructor and mutator returns `Self` and is `#[must_use]`, so a
/// builder can never be silently dropped without calling [`build`](Self::build).
pub struct DocumentBuilder<P, I = Resource> {
    data: PrimaryData<P>,
    included: Vec<I>,
    meta: Option<Meta>,
    jsonapi: Option<JsonApiObject>,
    links: Option<Links>,
}

impl<P: ResourceObject, I: ResourceObject> DocumentBuilder<P, I> {
    /// Start an empty builder (null primary data until `.data`/`.many`/`.no_data`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: PrimaryData::Null,
            included: Vec::new(),
            meta: None,
            jsonapi: None,
            links: None,
        }
    }

    /// Set the primary data to a single resource.
    #[must_use]
    pub fn data(mut self, primary: P) -> Self {
        self.data = PrimaryData::Single(Box::new(primary));
        self
    }

    /// Set the primary data to a collection.
    #[must_use]
    pub fn many(mut self, primary: Vec<P>) -> Self {
        self.data = PrimaryData::Many(primary);
        self
    }

    /// Set the primary data to `null`.
    #[must_use]
    pub fn no_data(mut self) -> Self {
        self.data = PrimaryData::Null;
        self
    }

    /// Add one included resource (deduplicated by `(type, id)` at build time).
    #[must_use]
    pub fn include(mut self, resource: I) -> Self {
        self.included.push(resource);
        self
    }

    /// Add many included resources.
    #[must_use]
    pub fn include_many(mut self, resources: impl IntoIterator<Item = I>) -> Self {
        self.included.extend(resources);
        self
    }

    /// Add one top-level link.
    #[must_use]
    pub fn link(mut self, rel: &str, link: Link) -> Self {
        self.links
            .get_or_insert_with(Links::new)
            .insert(rel, Some(link));
        self
    }

    /// Replace all top-level links. Discards any links previously added via [`link`](Self::link).
    #[must_use]
    pub fn links(mut self, links: Links) -> Self {
        self.links = Some(links);
        self
    }

    /// Set the top-level `meta`.
    #[must_use]
    pub fn meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Set the `jsonapi` member.
    #[must_use]
    pub fn jsonapi(mut self, obj: JsonApiObject) -> Self {
        self.jsonapi = Some(obj);
        self
    }

    /// Append a profile URI to the `jsonapi.profile` array.
    #[must_use]
    pub fn profile(mut self, uri: &str) -> Self {
        self.jsonapi
            .get_or_insert_with(JsonApiObject::default)
            .profile
            .get_or_insert_with(Vec::new)
            .push(uri.to_string());
        self
    }

    /// Append an extension URI to the `jsonapi.ext` array.
    #[must_use]
    pub fn ext(mut self, uri: &str) -> Self {
        self.jsonapi
            .get_or_insert_with(JsonApiObject::default)
            .ext
            .get_or_insert_with(Vec::new)
            .push(uri.to_string());
        self
    }

    /// Finish building. Deduplicates `included` by `(type, id)` and drops any
    /// included resource whose `(type, id)` matches a primary resource.
    #[must_use]
    pub fn build(self) -> Document<P, I> {
        let mut primary_keys: HashSet<(String, String)> = HashSet::new();

        match &self.data {
            PrimaryData::Single(p) => {
                if let Some(id) = p.resource_id() {
                    primary_keys.insert((p.resource_type().to_string(), id.to_string()));
                }
            }
            PrimaryData::Many(ps) => {
                for p in ps {
                    if let Some(id) = p.resource_id() {
                        primary_keys.insert((p.resource_type().to_string(), id.to_string()));
                    }
                }
            }
            PrimaryData::Null => {}
        }

        let mut seen: HashSet<(String, String)> = HashSet::new();
        let mut included = Vec::with_capacity(self.included.len());
        for r in self.included {
            match r.resource_id() {
                Some(id) => {
                    let key = (r.resource_type().to_string(), id.to_string());
                    if primary_keys.contains(&key) || !seen.insert(key) {
                        continue;
                    }
                    included.push(r);
                }
                None => included.push(r),
            }
        }

        Document::Data {
            data: self.data,
            included,
            meta: self.meta,
            jsonapi: self.jsonapi,
            links: self.links,
        }
    }
}

impl<P: ResourceObject, I: ResourceObject> Default for DocumentBuilder<P, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: ResourceObject> DocumentBuilder<P, Resource> {
    /// Convenience: start from a single primary resource (included type `Resource`).
    #[must_use]
    pub fn single(primary: P) -> Self {
        Self::new().data(primary)
    }

    /// Convenience: start from a collection (included type `Resource`).
    #[must_use]
    pub fn collection(primary: Vec<P>) -> Self {
        Self::new().many(primary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, Link, PrimaryData, Resource};

    fn res(type_: &str, id: &str) -> Resource {
        Resource {
            r#type: type_.into(),
            id: Some(id.into()),
            lid: None,
            attributes: serde_json::json!({}),
            relationships: std::collections::BTreeMap::new(),
            links: None,
            meta: None,
        }
    }

    #[test]
    fn builds_single_data_document() {
        let doc = DocumentBuilder::single(res("articles", "1")).build();
        match doc {
            Document::Data {
                data: PrimaryData::Single(_),
                included,
                ..
            } => {
                assert!(included.is_empty());
            }
            _ => panic!("expected single data document"),
        }
    }

    #[test]
    fn builds_collection_document() {
        let doc =
            DocumentBuilder::collection(vec![res("articles", "1"), res("articles", "2")]).build();
        assert!(
            matches!(doc, Document::Data { data: PrimaryData::Many(ref v), .. } if v.len() == 2)
        );
    }

    #[test]
    fn include_dedups_by_type_and_id() {
        let doc = DocumentBuilder::single(res("articles", "1"))
            .include(res("people", "9"))
            .include(res("people", "9"))
            .build();
        match doc {
            Document::Data { included, .. } => assert_eq!(included.len(), 1),
            _ => panic!(),
        }
    }

    #[test]
    fn include_skips_resource_equal_to_primary() {
        let doc = DocumentBuilder::single(res("articles", "1"))
            .include(res("articles", "1"))
            .include(res("people", "9"))
            .build();
        match doc {
            Document::Data { included, .. } => {
                assert_eq!(included.len(), 1);
                assert_eq!(included[0].r#type, "people");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn no_data_builds_null_primary() {
        let doc = DocumentBuilder::<Resource>::new().no_data().build();
        assert!(matches!(
            doc,
            Document::Data {
                data: PrimaryData::Null,
                ..
            }
        ));
    }

    #[test]
    fn include_skips_resource_equal_to_any_primary_in_many() {
        let doc = DocumentBuilder::collection(vec![res("articles", "1"), res("articles", "2")])
            .include(res("articles", "2")) // duplicates a primary in the Many set
            .include(res("people", "9"))
            .build();
        match doc {
            Document::Data { included, .. } => {
                assert_eq!(included.len(), 1);
                assert_eq!(included[0].r#type, "people");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn include_many_adds_all_resources() {
        let doc = DocumentBuilder::single(res("articles", "1"))
            .include_many(vec![res("people", "9"), res("tags", "1")])
            .build();
        match doc {
            Document::Data { included, .. } => assert_eq!(included.len(), 2),
            _ => panic!(),
        }
    }

    #[test]
    fn links_bulk_replaces_previously_added_links() {
        let replacement: crate::Links =
            serde_json::from_value(serde_json::json!({"next": "/articles?page=2"})).unwrap();
        let doc = DocumentBuilder::single(res("articles", "1"))
            .link("self", Link::String("/articles/1".into()))
            .links(replacement) // discards the "self" link added above
            .build();
        match doc {
            Document::Data { links, .. } => {
                let links = links.unwrap();
                assert!(links.contains("next"));
                assert!(!links.contains("self"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn accumulates_links_meta_jsonapi_profile_ext() {
        let doc = DocumentBuilder::single(res("articles", "1"))
            .link("self", Link::String("/articles/1".into()))
            .profile("https://example.com/p1")
            .ext("https://example.com/e1")
            .build();
        match doc {
            Document::Data { links, jsonapi, .. } => {
                assert!(links.unwrap().contains("self"));
                let j = jsonapi.unwrap();
                assert_eq!(
                    j.profile.unwrap(),
                    vec!["https://example.com/p1".to_string()]
                );
                assert_eq!(j.ext.unwrap(), vec!["https://example.com/e1".to_string()]);
            }
            _ => panic!(),
        }
    }
}
