#![cfg(feature = "derive")]

use jsonapi_core::model::{Identity, RelationshipData, ResourceIdentifier, ResourceObject};
use jsonapi_core::{FieldsetConfig, Relationship, SparseSerializer, TypeRegistry, sparse_filter};

// ── Test structs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct TestArticle {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
    #[jsonapi(relationship)]
    author: Relationship<TestPerson>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "people")]
struct TestPerson {
    #[jsonapi(id)]
    id: String,
    name: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
    #[jsonapi(relationship, type = "comments")]
    comments: Vec<Relationship<Comment>>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "comments")]
struct Comment {
    #[jsonapi(id)]
    id: String,
    body: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
}

fn test_article() -> TestArticle {
    TestArticle {
        id: "1".into(),
        title: "Hello".into(),
        body: "World".into(),
        author: Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            type_: "people".into(),
            identity: Identity::Id("9".into()),
            meta: None,
        }))),
    }
}

// ── Task 6: SparseSerializer tests ───────────────────────────────────────────

#[test]
fn test_sparse_serializer_filters_attributes() {
    let article = test_article();
    let config = FieldsetConfig::new().fields("articles", &["title"]);
    let json = serde_json::to_value(SparseSerializer::new(&article, &config)).unwrap();
    assert_eq!(json["type"], "articles");
    assert_eq!(json["id"], "1");
    assert_eq!(json["attributes"]["title"], "Hello");
    assert!(json["attributes"].get("body").is_none());
    assert!(json.get("relationships").is_none());
}

#[test]
fn test_sparse_serializer_no_fieldset_passes_all() {
    let article = test_article();
    let config = FieldsetConfig::new();
    let json = serde_json::to_value(SparseSerializer::new(&article, &config)).unwrap();
    assert_eq!(json["attributes"]["title"], "Hello");
    assert_eq!(json["attributes"]["body"], "World");
    assert!(json.get("relationships").is_some());
}

#[test]
fn test_sparse_serializer_id_type_always_present() {
    let article = test_article();
    let config = FieldsetConfig::new().fields("articles", &["title"]);
    let json = serde_json::to_value(SparseSerializer::new(&article, &config)).unwrap();
    assert_eq!(json["type"], "articles");
    assert_eq!(json["id"], "1");
}

#[test]
fn test_sparse_serializer_filters_relationships() {
    let article = test_article();
    let config = FieldsetConfig::new().fields("articles", &["author"]);
    let json = serde_json::to_value(SparseSerializer::new(&article, &config)).unwrap();
    assert!(json.get("attributes").is_none());
    assert!(json["relationships"].get("author").is_some());
}

#[test]
fn test_sparse_serializer_empty_fieldset_strips_all() {
    let article = test_article();
    let config = FieldsetConfig::new().fields("articles", &[]);
    let json = serde_json::to_value(SparseSerializer::new(&article, &config)).unwrap();
    assert_eq!(json["type"], "articles");
    assert_eq!(json["id"], "1");
    assert!(json.get("attributes").is_none());
    assert!(json.get("relationships").is_none());
}

// ── Task 10: Integration tests ────────────────────────────────────────────────

#[test]
fn test_type_info_from_derive() {
    let info = Article::type_info();
    assert_eq!(info.type_name, "articles");
    assert_eq!(info.field_names, &["title", "body", "author", "comments"]);
    assert_eq!(
        info.relationships,
        &[("author", "people"), ("comments", "comments")]
    );
}

#[test]
fn test_type_info_no_relationships() {
    let info = Person::type_info();
    assert_eq!(info.type_name, "people");
    assert_eq!(info.field_names, &["name"]);
    assert_eq!(info.relationships, &[] as &[(&str, &str)]);
}

#[test]
fn test_type_info_relationship_without_target_type() {
    #[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
    #[jsonapi(type = "posts")]
    struct Post {
        #[jsonapi(id)]
        id: String,
        title: String,
        #[jsonapi(relationship)]
        author: Relationship<Person>,
    }

    let info = Post::type_info();
    assert_eq!(info.field_names, &["title", "author"]);
    assert_eq!(info.relationships, &[] as &[(&str, &str)]);
}

#[test]
fn test_type_registry_with_derive() {
    let mut registry = TypeRegistry::new();
    registry
        .register::<Article>()
        .register::<Person>()
        .register::<Comment>();

    let info = registry.get("articles").unwrap();
    assert_eq!(
        info.relationships,
        &[("author", "people"), ("comments", "comments")]
    );

    let info = registry.get("comments").unwrap();
    assert_eq!(info.relationships, &[("author", "people")]);
}

#[test]
fn test_validate_paths_with_derive() {
    let mut registry = TypeRegistry::new();
    registry
        .register::<Article>()
        .register::<Person>()
        .register::<Comment>();

    assert!(
        registry
            .validate_include_paths("articles", &["author"])
            .is_ok()
    );
    assert!(
        registry
            .validate_include_paths("articles", &["comments"])
            .is_ok()
    );
    assert!(
        registry
            .validate_include_paths("articles", &["comments.author"])
            .is_ok()
    );
    assert!(
        registry
            .validate_include_paths("articles", &["author", "comments.author"])
            .is_ok()
    );
}

#[test]
fn test_validate_invalid_path_with_derive() {
    let mut registry = TypeRegistry::new();
    registry.register::<Article>();

    let err = registry
        .validate_include_paths("articles", &["nonexistent"])
        .unwrap_err();
    assert!(err.to_string().contains("nonexistent"));
    assert!(err.to_string().contains("articles"));
}

#[test]
fn test_sparse_serializer_end_to_end() {
    let article = Article {
        id: "1".into(),
        title: "Hello".into(),
        body: "World".into(),
        author: Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            type_: "people".into(),
            identity: Identity::Id("9".into()),
            meta: None,
        }))),
        comments: vec![],
    };

    let config = FieldsetConfig::new().fields("articles", &["title", "author"]);
    let json = serde_json::to_value(SparseSerializer::new(&article, &config)).unwrap();

    assert_eq!(json["type"], "articles");
    assert_eq!(json["id"], "1");
    assert_eq!(json["attributes"]["title"], "Hello");
    assert!(json["attributes"].get("body").is_none());
    assert!(json["relationships"].get("author").is_some());
}

#[test]
fn test_sparse_filter_document_end_to_end() {
    let doc = serde_json::json!({
        "data": {
            "type": "articles", "id": "1",
            "attributes": {"title": "Hello", "body": "World"},
            "relationships": {
                "author": {"data": {"type": "people", "id": "9"}},
                "comments": {"data": []}
            }
        },
        "included": [
            {
                "type": "people", "id": "9",
                "attributes": {"name": "Dan"}
            }
        ]
    });

    let config = FieldsetConfig::new()
        .fields("articles", &["title", "author"])
        .fields("people", &["name"]);
    let filtered = sparse_filter(&doc, &config);

    assert_eq!(filtered["data"]["attributes"]["title"], "Hello");
    assert!(filtered["data"]["attributes"].get("body").is_none());
    assert!(filtered["data"]["relationships"].get("author").is_some());
    assert!(filtered["data"]["relationships"].get("comments").is_none());
    assert_eq!(filtered["included"][0]["attributes"]["name"], "Dan");
}

#[test]
fn test_full_pipeline() {
    let mut type_registry = TypeRegistry::new();
    type_registry
        .register::<Article>()
        .register::<Person>()
        .register::<Comment>();

    assert!(
        type_registry
            .validate_include_paths("articles", &["author", "comments.author"])
            .is_ok()
    );

    let fieldset = FieldsetConfig::new()
        .fields("articles", &["title", "author"])
        .fields("people", &["name"]);

    let article = Article {
        id: "1".into(),
        title: "Hello".into(),
        body: "World".into(),
        author: Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            type_: "people".into(),
            identity: Identity::Id("9".into()),
            meta: None,
        }))),
        comments: vec![],
    };
    let json = serde_json::to_value(SparseSerializer::new(&article, &fieldset)).unwrap();
    assert_eq!(json["attributes"]["title"], "Hello");
    assert!(json["attributes"].get("body").is_none());
}
