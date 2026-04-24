//! End-to-end acceptance tests against a realistic Drupal-shaped
//! JSON:API payload. See `docs/superpowers/specs/2026-04-15-acceptance-rich-article-design.md`.

// Drupal member names (e.g. `drupal_internal__nid`) use a double underscore
// by convention; that violates the default `non_snake_case` lint. The names
// are intentional — they round-trip the wire shape — so allow them
// file-wide.
#![allow(non_snake_case)]

const RICH_ARTICLE_JSON: &str = include_str!("fixtures/rich_article.json");

#[test]
fn fixture_is_valid_json() {
    let v: serde_json::Value =
        serde_json::from_str(RICH_ARTICLE_JSON).expect("fixture must parse as JSON");
    assert!(v.is_object());
    assert_eq!(v["data"]["type"], "node--article");
    assert_eq!(
        v["included"]
            .as_array()
            .expect("included is an array")
            .len(),
        22
    );
}

// ---------- Test 1: shallow typed ----------

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
struct Path {
    alias: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
struct FieldComments {
    status: i64,
    last_comment_name: Option<String>,
    last_comment_timestamp: Option<String>,
    comment_count: i64,
}

// `Option<String>` fields whose wire value is literally `null` are handled
// by the derive — see IMPROVEMENTS.md #1. The Drupal fixture has such
// fields (`field_canonical_url`, `field_video_orientation`, `publish_on`),
// which we can now type as `Option<String>` without preprocessing.
#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "node--article")]
#[allow(dead_code)]
struct ArticleShallow {
    #[jsonapi(id)]
    id: String,
    title: String,
    field_headline_primary: bool,
    field_word_count: i64,
    field_type_of_work: Vec<String>,
    path: Path,
    field_comments: FieldComments,
    field_published_date: String,
    moderation_state: String,
    drupal_internal__nid: i64,
    field_canonical_url: Option<String>,
    field_video_orientation: Option<String>,
    publish_on: Option<String>,
}

#[test]
fn test_shallow_typed_article() {
    let v: serde_json::Value = serde_json::from_str(RICH_ARTICLE_JSON).unwrap();
    let article: ArticleShallow = serde_json::from_value(v["data"].clone()).unwrap();

    assert_eq!(article.id, "f2f2f2f2-f2f2-4f2f-8f2f-f2f2f2f2f2f2");
    assert_eq!(article.title, "Rich Acceptance Test Article");
    assert!(article.field_headline_primary);
    assert_eq!(article.field_word_count, 250);
    assert_eq!(article.field_type_of_work, vec!["news", "opinion"]);
    assert_eq!(article.path.alias, "/news/rich-acceptance-test");
    assert_eq!(article.field_comments.status, 1);
    assert_eq!(article.field_comments.comment_count, 0);
    assert_eq!(article.field_comments.last_comment_name, None);
    assert_eq!(article.field_comments.last_comment_timestamp, None);
    assert_eq!(article.field_published_date, "2024-03-01T09:00:00+13:00");
    assert_eq!(article.moderation_state, "published");
    assert_eq!(article.drupal_internal__nid, 60001);
    // Wire `null` Option<String> fields collapse to None (IMPROVEMENTS.md #1).
    assert_eq!(article.field_canonical_url, None);
    assert_eq!(article.field_video_orientation, None);
    assert_eq!(article.publish_on, None);
}

// ---------- Test 2: deep typed + registry ----------

use jsonapi_core::{Identity, Relationship, RelationshipData, Resource, ResourceIdentifier};

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "taxonomy_term--section")]
#[allow(dead_code)]
struct Section {
    #[jsonapi(id)]
    id: String,
    name: String,
    field_unique_key: String,
    drupal_internal__tid: i64,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "taxonomy_term--publication_channel")]
#[allow(dead_code)]
struct PublicationChannel {
    #[jsonapi(id)]
    id: String,
    name: String,
    field_unique_key: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "taxonomy_term--topics")]
#[allow(dead_code)]
struct Topic {
    #[jsonapi(id)]
    id: String,
    name: String,
    field_unique_key: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "node--author")]
#[allow(dead_code)]
struct Author {
    #[jsonapi(id)]
    id: String,
    title: String,
    stuff_asset_id: String,
    drupal_internal__nid: i64,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "paragraph--teaser")]
#[allow(dead_code)]
struct Teaser {
    #[jsonapi(id)]
    id: String,
    field_teaser_headline: String,
    field_intro: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
struct FieldBody {
    value: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "paragraph--text")]
#[allow(dead_code)]
struct TextPara {
    #[jsonapi(id)]
    id: String,
    field_body: FieldBody,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "paragraph--quote")]
#[allow(dead_code)]
struct QuotePara {
    #[jsonapi(id)]
    id: String,
    field_quote: String,
    field_source: String,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "node--article")]
#[allow(dead_code)]
struct ArticleFull {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship)]
    field_section: Relationship<Section>,
    #[jsonapi(relationship)]
    field_main_publication_channel: Relationship<PublicationChannel>,
    #[jsonapi(relationship)]
    field_topics: Relationship<Topic>,
    #[jsonapi(relationship)]
    field_author: Relationship<Author>,
    #[jsonapi(relationship)]
    field_teaser: Relationship<Teaser>,
    // Heterogeneous to-many: `Resource` is a phantom; dispatch happens at
    // assertion time via `RelationshipData::ToMany(rids)` and per-rid type_.
    #[jsonapi(relationship)]
    field_content: Relationship<Resource>,
}

fn id_of(identity: &Identity) -> &str {
    identity
        .as_id()
        .expect("expected server id, got lid or future variant")
}

#[test]
fn test_deep_typed_article_with_registry() {
    // One-shot parse: `Document<ArticleFull>` uses the default `I = Resource`,
    // which handles the heterogeneous `included` without a two-stage
    // deserialize.
    let doc: jsonapi_core::Document<ArticleFull> = serde_json::from_str(RICH_ARTICLE_JSON).unwrap();
    let article = match &doc {
        jsonapi_core::Document::Data {
            data: jsonapi_core::PrimaryData::Single(a),
            ..
        } => a.as_ref(),
        _ => panic!("expected Document::Data with a single primary resource"),
    };
    let registry = doc.registry().unwrap();

    assert_eq!(article.title, "Rich Acceptance Test Article");

    // Typed to-one — registry.get uses the relationship's phantom type.
    let section: Section = registry.get(&article.field_section).unwrap();
    assert_eq!(section.name, "News");
    assert_eq!(section.field_unique_key, "news");
    assert_eq!(section.drupal_internal__tid, 200);

    let channel: PublicationChannel = registry
        .get(&article.field_main_publication_channel)
        .unwrap();
    assert_eq!(channel.name, "Stuff");

    let teaser: Teaser = registry.get(&article.field_teaser).unwrap();
    assert_eq!(teaser.field_teaser_headline, "Rich Test Teaser");

    // Typed to-many via registry.get_many (reads RelationshipData::ToMany)
    let topics: Vec<Topic> = registry.get_many(&article.field_topics).unwrap();
    assert_eq!(topics.len(), 2);
    let topic_names: Vec<&str> = topics.iter().map(|t| t.name.as_str()).collect();
    assert!(topic_names.contains(&"Politics"));
    assert!(topic_names.contains(&"Environment"));

    let authors: Vec<Author> = registry.get_many(&article.field_author).unwrap();
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].title, "Rich Reporter");
    assert_eq!(authors[0].stuff_asset_id, "author-001");

    // Heterogeneous dispatch on field_content (11 mixed paragraph variants).
    let rids = match &article.field_content.data {
        RelationshipData::ToMany(rids) => rids,
        _ => panic!("expected to-many"),
    };
    assert_eq!(rids.len(), 11);

    let text_rid: &ResourceIdentifier = rids
        .iter()
        .find(|rid| rid.type_ == "paragraph--text")
        .expect("text paragraph present");
    let text: TextPara = registry
        .get_by_id(&text_rid.type_, id_of(&text_rid.identity))
        .unwrap();
    assert!(text.field_body.value.contains("Opening paragraph"));

    let quote_rid: &ResourceIdentifier = rids
        .iter()
        .find(|rid| rid.type_ == "paragraph--quote")
        .expect("quote paragraph present");
    let quote: QuotePara = registry
        .get_by_id(&quote_rid.type_, id_of(&quote_rid.identity))
        .unwrap();
    assert_eq!(quote.field_quote, "This is a pullquote for testing");
    assert_eq!(quote.field_source, "Quote Author");
}

// ---------- Test 2b: typed primary with default dynamic included ----------

use jsonapi_core::{Document, PrimaryData, ResourceObject};

#[test]
fn test_typed_primary_with_default_dynamic_included() {
    // With `Document<ArticleFull>` (default `I = Resource`), the envelope's
    // heterogeneous `included` deserialises cleanly — no two-stage parse.
    let doc: Document<ArticleFull> = serde_json::from_str(RICH_ARTICLE_JSON).unwrap();

    let article = match &doc {
        Document::Data {
            data: PrimaryData::Single(article),
            ..
        } => article,
        _ => panic!("expected Document::Data with a single primary resource"),
    };
    assert_eq!(article.title, "Rich Acceptance Test Article");

    let registry = doc.registry().unwrap();
    let section: Section = registry.get(&article.field_section).unwrap();
    assert_eq!(section.name, "News");
}

// ---------- Test 3: dynamic + round-trip ----------

#[test]
fn test_dynamic_lossless_round_trip() {
    let original: serde_json::Value = serde_json::from_str(RICH_ARTICLE_JSON).unwrap();
    let doc: Document<Resource> = serde_json::from_str(RICH_ARTICLE_JSON).unwrap();

    let (article, included) = match &doc {
        Document::Data {
            data: PrimaryData::Single(article),
            included,
            ..
        } => (article, included),
        _ => panic!("expected Document::Data with a single primary resource"),
    };
    assert_eq!(article.resource_type(), "node--article");
    assert_eq!(included.len(), 22);

    // Relationship-level meta preserved on the teaser's field_media_override.
    let teaser = included
        .iter()
        .find(|r| r.resource_type() == "paragraph--teaser")
        .expect("teaser present");
    let media_data = teaser
        .relationships
        .get("field_media_override")
        .expect("field_media_override present");
    let media_rid = match media_data {
        RelationshipData::ToOne(Some(rid)) => rid,
        _ => panic!("expected to-one present"),
    };
    let media_meta = media_rid
        .meta
        .as_ref()
        .expect("identifier-level meta present");
    assert_eq!(
        media_meta["caption"],
        serde_json::json!("Teaser image caption")
    );
    assert_eq!(media_meta["focal_point"], serde_json::json!("50,50"));

    // Relationship-level `null` preserved on taxonomy_term--source's field_logo.
    let source = included
        .iter()
        .find(|r| r.resource_type() == "taxonomy_term--source")
        .expect("source present");
    let logo_data = source
        .relationships
        .get("field_logo")
        .expect("field_logo present");
    assert!(
        matches!(logo_data, RelationshipData::ToOne(None)),
        "expected null to-one, got {logo_data:?}"
    );

    // Registry lookup works for Drupal-shaped types.
    let registry = doc.registry().unwrap();
    let fetched: Resource = registry
        .get_by_id(
            "taxonomy_term--section",
            "22222222-2222-4222-8222-222222222223",
        )
        .unwrap();
    assert_eq!(fetched.resource_type(), "taxonomy_term--section");

    // Lossless round-trip: the dynamic path preserves everything the
    // fixture sends. Value equality ignores object key order, so we don't
    // care if serialization reorders within an object.
    let reserialized: serde_json::Value = serde_json::to_value(&doc).unwrap();
    assert_eq!(reserialized, original);
}
