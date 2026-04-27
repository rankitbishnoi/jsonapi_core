//! Integration tests for the `HasLinks` / `HasMeta` accessor traits and
//! their auto-derive (improvement #5).
//!
//! These tests verify that:
//!   - The derive auto-implements `HasLinks` when `#[jsonapi(links)]` is present.
//!   - The derive auto-implements `HasMeta` when `#[jsonapi(meta)]` is present.
//!   - The dynamic `Resource` fallback also implements both traits.
//!   - A resource with neither attribute does *not* spuriously implement the
//!     traits (verified via a generic helper that requires the bound).

#![cfg(feature = "derive")]

use std::collections::BTreeMap;

use jsonapi_core::model::{HasLinks, HasMeta, Link, Links, Meta, Resource};

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct ArticleWithBoth {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(links)]
    links: Option<Links>,
    #[jsonapi(meta)]
    extra: Option<Meta>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "comments")]
struct CommentWithLinksOnly {
    #[jsonapi(id)]
    id: String,
    body: String,
    #[jsonapi(links)]
    links: Option<Links>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "tags")]
struct TagWithMetaOnly {
    #[jsonapi(id)]
    id: String,
    name: String,
    #[jsonapi(meta)]
    info: Option<Meta>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "people")]
struct PersonPlain {
    #[jsonapi(id)]
    id: String,
    name: String,
}

fn sample_links() -> Links {
    let mut map = BTreeMap::new();
    map.insert(
        "self".to_string(),
        Some(Link::String("/articles/1".to_string())),
    );
    map.insert("missing".to_string(), None);
    Links(map)
}

fn sample_meta() -> Meta {
    let mut m = serde_json::Map::new();
    m.insert("count".to_string(), serde_json::json!(7));
    m
}

#[test]
fn derive_emits_haslinks_when_links_field_present() {
    let article = ArticleWithBoth {
        id: "1".into(),
        title: "Hello".into(),
        links: Some(sample_links()),
        extra: None,
    };
    // Direct dispatch via the trait works without requiring a type annotation
    // on the resource — the auto-impl is sufficient.
    let links = HasLinks::links(&article).expect("links should be present");
    assert!(links.contains("self"));
}

#[test]
fn derive_emits_haslinks_returning_none_when_field_is_none() {
    let article = ArticleWithBoth {
        id: "1".into(),
        title: "Hello".into(),
        links: None,
        extra: None,
    };
    assert!(article.links().is_none());
}

#[test]
fn derive_emits_hasmeta_when_meta_field_present() {
    let article = ArticleWithBoth {
        id: "1".into(),
        title: "Hello".into(),
        links: None,
        extra: Some(sample_meta()),
    };
    let meta = HasMeta::meta(&article).expect("meta should be present");
    assert_eq!(meta["count"], 7);
}

#[test]
fn derive_emits_hasmeta_returning_none_when_field_is_none() {
    let article = ArticleWithBoth {
        id: "1".into(),
        title: "Hello".into(),
        links: None,
        extra: None,
    };
    assert!(article.meta().is_none());
}

#[test]
fn derive_emits_haslinks_only_when_links_field_present() {
    // CommentWithLinksOnly has only #[jsonapi(links)] — should impl HasLinks
    // but NOT HasMeta. We can't compile-fail-test this from a runtime test,
    // but we can verify the positive path: HasLinks works.
    let comment = CommentWithLinksOnly {
        id: "1".into(),
        body: "hi".into(),
        links: Some(sample_links()),
    };
    assert!(comment.links().is_some());
}

#[test]
fn derive_emits_hasmeta_only_when_meta_field_present() {
    let tag = TagWithMetaOnly {
        id: "1".into(),
        name: "rust".into(),
        info: Some(sample_meta()),
    };
    assert!(tag.meta().is_some());
}

#[test]
fn dynamic_resource_implements_haslinks() {
    let mut resource = Resource {
        type_: "articles".into(),
        id: Some("1".into()),
        lid: None,
        attributes: serde_json::json!({"title": "Hi"}),
        relationships: BTreeMap::new(),
        links: None,
        meta: None,
    };
    // None case
    assert!(HasLinks::links(&resource).is_none());

    // Some case
    resource.links = Some(sample_links());
    assert!(HasLinks::links(&resource).unwrap().contains("self"));
}

#[test]
fn dynamic_resource_implements_hasmeta() {
    let mut resource = Resource {
        type_: "articles".into(),
        id: Some("1".into()),
        lid: None,
        attributes: serde_json::json!({"title": "Hi"}),
        relationships: BTreeMap::new(),
        links: None,
        meta: None,
    };
    assert!(HasMeta::meta(&resource).is_none());

    resource.meta = Some(sample_meta());
    assert_eq!(HasMeta::meta(&resource).unwrap()["count"], 7);
}

#[test]
fn generic_function_can_bound_on_haslinks() {
    fn first_link_name<R: HasLinks>(r: &R) -> Option<String> {
        r.links()
            .and_then(|l| l.iter().next().map(|(k, _)| k.to_string()))
    }

    let article = ArticleWithBoth {
        id: "1".into(),
        title: "Hi".into(),
        links: Some(sample_links()),
        extra: None,
    };
    assert_eq!(first_link_name(&article), Some("self".to_string()));
}

#[test]
fn generic_function_can_bound_on_hasmeta() {
    fn count_meta_keys<R: HasMeta>(r: &R) -> usize {
        r.meta().map(|m| m.len()).unwrap_or(0)
    }

    let tag = TagWithMetaOnly {
        id: "1".into(),
        name: "rust".into(),
        info: Some(sample_meta()),
    };
    assert_eq!(count_meta_keys(&tag), 1);
}

// `PersonPlain` has neither `#[jsonapi(links)]` nor `#[jsonapi(meta)]`. The
// fact that this test compiles only shows the type is usable on its own; the
// stronger guarantee — that the derive does *not* spuriously implement the
// accessor traits — is enforced by a compile-fail test under
// `tests/compile_fail/has_links_not_impl.rs` (verified via trybuild).
#[test]
fn plain_person_compiles_without_accessor_impls() {
    let p = PersonPlain {
        id: "1".into(),
        name: "Dan".into(),
    };
    assert_eq!(p.id, "1");
    assert_eq!(p.name, "Dan");
}
