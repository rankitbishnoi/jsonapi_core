// Some fixtures intentionally use camelCase field names to exercise the
// `#[jsonapi(case = "camelCase")]` conversion path. Allow non-snake-case at
// the module level so that rustc's default lints don't flag them.
#![allow(non_snake_case)]

use jsonapi_core::Relationship;
use jsonapi_core::model::{
    Document, Identity, Links, Meta, PrimaryData, RelationshipData, ResourceIdentifier,
    ResourceObject,
};

// ============================================================
// Test structs
// ============================================================

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
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
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "posts")]
struct Post {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship)]
    author: Relationship<Person>,
    #[jsonapi(relationship)]
    comments: Vec<Relationship<Comment>>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "drafts")]
struct Draft {
    #[jsonapi(id)]
    id: Option<String>,
    #[jsonapi(lid)]
    local_id: Option<String>,
    title: String,
    subtitle: Option<String>,
    #[jsonapi(meta)]
    extra: Option<Meta>,
    #[jsonapi(links)]
    resource_links: Option<Links>,
    #[jsonapi(skip)]
    internal_cache: Option<String>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "events")]
struct Event {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(rename = "event-name")]
    name: String,
    #[jsonapi(rename = "start-date")]
    starts_at: String,
    location: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "profiles")]
struct Profile {
    #[jsonapi(id)]
    id: String,
    first_name: String,
    last_name: String,
    phone_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "users", case = "camelCase")]
struct CamelUser {
    #[jsonapi(id)]
    id: String,
    first_name: String,
    last_name: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "users", case = "kebab-case")]
struct KebabUser {
    #[jsonapi(id)]
    id: String,
    first_name: String,
    last_name: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "users", case = "snake_case")]
struct SnakeUser {
    #[jsonapi(id)]
    id: String,
    #[allow(non_snake_case)]
    firstName: String,
    #[allow(non_snake_case)]
    lastName: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "users", case = "PascalCase")]
struct PascalUser {
    #[jsonapi(id)]
    id: String,
    first_name: String,
    last_name: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "events", case = "camelCase")]
struct CamelEvent {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(rename = "event-name")]
    name: String,
    start_date: String,
}

// ============================================================
// Task 5: Basic struct round-trip
// ============================================================

#[test]
fn basic_resource_object_trait() {
    let article = Article {
        id: "1".into(),
        title: "Hello".into(),
        body: "World".into(),
    };
    assert_eq!(article.resource_type(), "articles");
    assert_eq!(article.resource_id(), Some("1"));
    assert_eq!(article.resource_lid(), None);
    assert_eq!(Article::field_names(), &["title", "body"]);
}

#[test]
fn basic_serialize() {
    let article = Article {
        id: "1".into(),
        title: "Hello".into(),
        body: "World".into(),
    };
    let json = serde_json::to_value(&article).unwrap();
    assert_eq!(json["type"], "articles");
    assert_eq!(json["id"], "1");
    assert_eq!(json["attributes"]["title"], "Hello");
    assert_eq!(json["attributes"]["body"], "World");
    assert!(json.get("relationships").is_none());
    assert!(json.get("links").is_none());
    assert!(json.get("meta").is_none());
}

#[test]
fn basic_deserialize() {
    let json = r#"{
        "type": "articles",
        "id": "1",
        "attributes": {
            "title": "Hello",
            "body": "World"
        }
    }"#;
    let article: Article = serde_json::from_str(json).unwrap();
    assert_eq!(article.id, "1");
    assert_eq!(article.title, "Hello");
    assert_eq!(article.body, "World");
}

#[test]
fn basic_round_trip() {
    let article = Article {
        id: "1".into(),
        title: "Hello".into(),
        body: "World".into(),
    };
    let json = serde_json::to_string(&article).unwrap();
    let deserialized: Article = serde_json::from_str(&json).unwrap();
    assert_eq!(article, deserialized);
}

#[test]
fn deserialize_wrong_type_errors() {
    let json = r#"{
        "type": "people",
        "id": "1",
        "attributes": {"title": "Hello", "body": "World"}
    }"#;
    let err = serde_json::from_str::<Article>(json).unwrap_err();
    assert!(err.to_string().contains("expected type \"articles\""));
}

#[test]
fn empty_attributes_struct() {
    #[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
    #[jsonapi(type = "tags")]
    struct Tag {
        #[jsonapi(id)]
        id: String,
    }

    let tag = Tag { id: "1".into() };
    let json = serde_json::to_value(&tag).unwrap();
    assert_eq!(json["type"], "tags");
    assert_eq!(json["id"], "1");
    assert!(json.get("attributes").is_none());

    let deserialized: Tag = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, tag);
}

// ============================================================
// Task 6: Relationships, optional id, lid, meta, links, skip
// ============================================================

#[test]
fn serialize_with_relationships() {
    let post = Post {
        id: "1".into(),
        title: "Hello".into(),
        author: Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            type_: "people".into(),
            identity: Identity::Id("9".into()),
            meta: None,
        }))),
        comments: vec![],
    };
    let json = serde_json::to_value(&post).unwrap();
    assert_eq!(json["type"], "posts");
    assert_eq!(json["attributes"]["title"], "Hello");
    assert_eq!(json["relationships"]["author"]["data"]["type"], "people");
    assert_eq!(json["relationships"]["author"]["data"]["id"], "9");
}

#[test]
fn deserialize_with_relationships() {
    // comments is Vec<Relationship<Comment>>; each element is a relationship wrapper object
    let json = r#"{
        "type": "posts",
        "id": "1",
        "attributes": {"title": "Hello"},
        "relationships": {
            "author": {
                "data": {"type": "people", "id": "9"}
            },
            "comments": [
                {"data": {"type": "comments", "id": "1"}},
                {"data": {"type": "comments", "id": "2"}}
            ]
        }
    }"#;
    let post: Post = serde_json::from_str(json).unwrap();
    assert_eq!(post.id, "1");
    assert_eq!(post.title, "Hello");
    assert_eq!(post.comments.len(), 2);
}

#[test]
fn missing_to_many_defaults_to_empty() {
    let json = r#"{
        "type": "posts",
        "id": "1",
        "attributes": {"title": "Hello"},
        "relationships": {
            "author": {"data": {"type": "people", "id": "9"}}
        }
    }"#;
    let post: Post = serde_json::from_str(json).unwrap();
    assert!(post.comments.is_empty());
}

#[test]
fn optional_id_present() {
    let draft = Draft {
        id: Some("1".into()),
        local_id: None,
        title: "Hello".into(),
        subtitle: None,
        extra: None,
        resource_links: None,
        internal_cache: None,
    };
    let json = serde_json::to_value(&draft).unwrap();
    assert_eq!(json["id"], "1");
    assert!(json.get("lid").is_none());
}

#[test]
fn optional_id_absent() {
    let draft = Draft {
        id: None,
        local_id: Some("temp-1".into()),
        title: "Hello".into(),
        subtitle: None,
        extra: None,
        resource_links: None,
        internal_cache: None,
    };
    let json = serde_json::to_value(&draft).unwrap();
    assert!(json.get("id").is_none());
    assert_eq!(json["lid"], "temp-1");
}

#[test]
fn optional_id_resource_object_trait() {
    let draft = Draft {
        id: Some("1".into()),
        local_id: Some("temp-1".into()),
        title: "Hello".into(),
        subtitle: None,
        extra: None,
        resource_links: None,
        internal_cache: None,
    };
    assert_eq!(draft.resource_id(), Some("1"));
    assert_eq!(draft.resource_lid(), Some("temp-1"));
}

#[test]
fn optional_attribute_omitted_when_none() {
    let draft = Draft {
        id: Some("1".into()),
        local_id: None,
        title: "Hello".into(),
        subtitle: None,
        extra: None,
        resource_links: None,
        internal_cache: None,
    };
    let json = serde_json::to_value(&draft).unwrap();
    let attrs = json["attributes"].as_object().unwrap();
    assert!(attrs.contains_key("title"));
    assert!(!attrs.contains_key("subtitle"));
}

#[test]
fn optional_attribute_present_when_some() {
    let draft = Draft {
        id: Some("1".into()),
        local_id: None,
        title: "Hello".into(),
        subtitle: Some("Sub".into()),
        extra: None,
        resource_links: None,
        internal_cache: None,
    };
    let json = serde_json::to_value(&draft).unwrap();
    assert_eq!(json["attributes"]["subtitle"], "Sub");
}

#[test]
fn skip_field_not_in_json() {
    let draft = Draft {
        id: Some("1".into()),
        local_id: None,
        title: "Hello".into(),
        subtitle: None,
        extra: None,
        resource_links: None,
        internal_cache: Some("cached".into()),
    };
    let json = serde_json::to_string(&draft).unwrap();
    assert!(!json.contains("internal_cache"));
    assert!(!json.contains("cached"));
}

#[test]
fn skip_field_defaults_on_deserialize() {
    let json = r#"{
        "type": "drafts",
        "id": "1",
        "attributes": {"title": "Hello"}
    }"#;
    let draft: Draft = serde_json::from_str(json).unwrap();
    assert_eq!(draft.internal_cache, None);
}

#[test]
fn deserialize_optional_attribute_missing() {
    let json = r#"{
        "type": "drafts",
        "id": "1",
        "attributes": {"title": "Hello"}
    }"#;
    let draft: Draft = serde_json::from_str(json).unwrap();
    assert_eq!(draft.subtitle, None);
}

#[test]
fn meta_and_links_serialize() {
    let mut meta = serde_json::Map::new();
    meta.insert("version".into(), serde_json::json!(2));

    let draft = Draft {
        id: Some("1".into()),
        local_id: None,
        title: "Hello".into(),
        subtitle: None,
        extra: Some(meta),
        resource_links: None,
        internal_cache: None,
    };
    let json = serde_json::to_value(&draft).unwrap();
    assert_eq!(json["meta"]["version"], 2);
    assert!(json.get("links").is_none());
}

#[test]
fn lid_round_trip() {
    let json = r#"{"type":"drafts","lid":"temp-1","attributes":{"title":"Hello"}}"#;
    let draft: Draft = serde_json::from_str(json).unwrap();
    assert_eq!(draft.local_id.as_deref(), Some("temp-1"));
    assert_eq!(draft.id, None);
}

// ============================================================
// Task 7: Rename
// ============================================================

#[test]
fn rename_serialize() {
    let event = Event {
        id: "1".into(),
        name: "Conf".into(),
        starts_at: "2026-04-14".into(),
        location: "NYC".into(),
    };
    let json = serde_json::to_value(&event).unwrap();
    let attrs = json["attributes"].as_object().unwrap();
    assert!(attrs.contains_key("event-name"));
    assert!(attrs.contains_key("start-date"));
    assert!(attrs.contains_key("location"));
    assert!(!attrs.contains_key("name"));
    assert!(!attrs.contains_key("starts_at"));
}

#[test]
fn rename_deserialize() {
    let json = r#"{
        "type": "events",
        "id": "1",
        "attributes": {
            "event-name": "Conf",
            "start-date": "2026-04-14",
            "location": "NYC"
        }
    }"#;
    let event: Event = serde_json::from_str(json).unwrap();
    assert_eq!(event.name, "Conf");
    assert_eq!(event.starts_at, "2026-04-14");
    assert_eq!(event.location, "NYC");
}

#[test]
fn rename_in_field_names() {
    assert!(Event::field_names().contains(&"event-name"));
    assert!(Event::field_names().contains(&"start-date"));
    assert!(Event::field_names().contains(&"location"));
    assert!(!Event::field_names().contains(&"name"));
}

// ============================================================
// Task 8: Fuzzy case matching
// ============================================================

#[test]
fn fuzzy_deser_camel_case_input() {
    let json = r#"{
        "type": "profiles",
        "id": "1",
        "attributes": {
            "firstName": "John",
            "lastName": "Doe"
        }
    }"#;
    let profile: Profile = serde_json::from_str(json).unwrap();
    assert_eq!(profile.first_name, "John");
    assert_eq!(profile.last_name, "Doe");
}

#[test]
fn fuzzy_deser_kebab_case_input() {
    let json = r#"{
        "type": "profiles",
        "id": "1",
        "attributes": {
            "first-name": "Jane",
            "last-name": "Doe"
        }
    }"#;
    let profile: Profile = serde_json::from_str(json).unwrap();
    assert_eq!(profile.first_name, "Jane");
    assert_eq!(profile.last_name, "Doe");
}

#[test]
fn fuzzy_deser_snake_case_input() {
    let json = r#"{
        "type": "profiles",
        "id": "1",
        "attributes": {
            "first_name": "Alice",
            "last_name": "Smith"
        }
    }"#;
    let profile: Profile = serde_json::from_str(json).unwrap();
    assert_eq!(profile.first_name, "Alice");
}

#[test]
fn fuzzy_deser_pascal_case_input() {
    let json = r#"{
        "type": "profiles",
        "id": "1",
        "attributes": {
            "FirstName": "Bob",
            "LastName": "Jones"
        }
    }"#;
    let profile: Profile = serde_json::from_str(json).unwrap();
    assert_eq!(profile.first_name, "Bob");
}

#[test]
fn fuzzy_deser_mixed_case_input() {
    let json = r#"{
        "type": "profiles",
        "id": "1",
        "attributes": {
            "firstName": "Mixed",
            "last-name": "Cases",
            "phone_number": "555-1234"
        }
    }"#;
    let profile: Profile = serde_json::from_str(json).unwrap();
    assert_eq!(profile.first_name, "Mixed");
    assert_eq!(profile.last_name, "Cases");
    assert_eq!(profile.phone_number, Some("555-1234".into()));
}

#[test]
fn fuzzy_deser_optional_field_missing() {
    let json = r#"{
        "type": "profiles",
        "id": "1",
        "attributes": {
            "first_name": "John",
            "last_name": "Doe"
        }
    }"#;
    let profile: Profile = serde_json::from_str(json).unwrap();
    assert_eq!(profile.phone_number, None);
}

// ============================================================
// Task 9: Case convention on serialization
// ============================================================

#[test]
fn case_camel_serialize() {
    let user = CamelUser {
        id: "1".into(),
        first_name: "John".into(),
        last_name: "Doe".into(),
    };
    let json = serde_json::to_value(&user).unwrap();
    let attrs = json["attributes"].as_object().unwrap();
    assert!(attrs.contains_key("firstName"));
    assert!(attrs.contains_key("lastName"));
    assert!(!attrs.contains_key("first_name"));
}

#[test]
fn case_kebab_serialize() {
    let user = KebabUser {
        id: "1".into(),
        first_name: "John".into(),
        last_name: "Doe".into(),
    };
    let json = serde_json::to_value(&user).unwrap();
    let attrs = json["attributes"].as_object().unwrap();
    assert!(attrs.contains_key("first-name"));
    assert!(attrs.contains_key("last-name"));
}

#[test]
fn case_snake_serialize() {
    let user = SnakeUser {
        id: "1".into(),
        firstName: "John".into(),
        lastName: "Doe".into(),
    };
    let json = serde_json::to_value(&user).unwrap();
    let attrs = json["attributes"].as_object().unwrap();
    assert!(attrs.contains_key("first_name"));
    assert!(attrs.contains_key("last_name"));
}

#[test]
fn case_pascal_serialize() {
    let user = PascalUser {
        id: "1".into(),
        first_name: "John".into(),
        last_name: "Doe".into(),
    };
    let json = serde_json::to_value(&user).unwrap();
    let attrs = json["attributes"].as_object().unwrap();
    assert!(attrs.contains_key("FirstName"));
    assert!(attrs.contains_key("LastName"));
}

#[test]
fn case_does_not_affect_type_string() {
    let user = CamelUser {
        id: "1".into(),
        first_name: "John".into(),
        last_name: "Doe".into(),
    };
    let json = serde_json::to_value(&user).unwrap();
    assert_eq!(json["type"], "users");
}

#[test]
fn rename_overrides_case() {
    let event = CamelEvent {
        id: "1".into(),
        name: "Conf".into(),
        start_date: "2026-04-14".into(),
    };
    let json = serde_json::to_value(&event).unwrap();
    let attrs = json["attributes"].as_object().unwrap();
    assert!(attrs.contains_key("event-name"));
    assert!(attrs.contains_key("startDate"));
}

#[test]
fn case_camel_round_trip() {
    let user = CamelUser {
        id: "1".into(),
        first_name: "John".into(),
        last_name: "Doe".into(),
    };
    let json = serde_json::to_string(&user).unwrap();
    let deserialized: CamelUser = serde_json::from_str(&json).unwrap();
    assert_eq!(user, deserialized);
}

#[test]
fn case_camel_field_names() {
    assert!(CamelUser::field_names().contains(&"firstName"));
    assert!(CamelUser::field_names().contains(&"lastName"));
}

// ============================================================
// Task 10: Document integration
// ============================================================

#[test]
fn document_with_derived_type() {
    let json = r#"{
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {
                "title": "Hello",
                "body": "World"
            }
        }
    }"#;
    let doc: Document<Article> = serde_json::from_str(json).unwrap();
    match &doc {
        Document::Data { data, .. } => match data {
            PrimaryData::Single(article) => {
                assert_eq!(article.id, "1");
                assert_eq!(article.title, "Hello");
            }
            _ => panic!("expected single resource"),
        },
        _ => panic!("expected data document"),
    }
}

#[test]
fn document_collection_with_derived_type() {
    let json = r#"{
        "data": [
            {
                "type": "articles",
                "id": "1",
                "attributes": {"title": "First", "body": "One"}
            },
            {
                "type": "articles",
                "id": "2",
                "attributes": {"title": "Second", "body": "Two"}
            }
        ]
    }"#;
    let doc: Document<Article> = serde_json::from_str(json).unwrap();
    match &doc {
        Document::Data { data, .. } => match data {
            PrimaryData::Many(articles) => {
                assert_eq!(articles.len(), 2);
                assert_eq!(articles[0].id, "1");
                assert_eq!(articles[1].id, "2");
            }
            _ => panic!("expected collection"),
        },
        _ => panic!("expected data document"),
    }
}

#[test]
fn document_serialize_derived_type() {
    let article = Article {
        id: "1".into(),
        title: "Hello".into(),
        body: "World".into(),
    };
    let doc = Document::<Article>::Data {
        data: PrimaryData::Single(Box::new(article)),
        included: vec![],
        meta: None,
        jsonapi: None,
        links: None,
    };
    let json = serde_json::to_value(&doc).unwrap();
    assert_eq!(json["data"]["type"], "articles");
    assert_eq!(json["data"]["attributes"]["title"], "Hello");
}
