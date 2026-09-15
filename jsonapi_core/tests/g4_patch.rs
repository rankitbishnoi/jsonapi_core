//! G4 — `Field<T>` tri-state PATCH members through `#[derive(JsonApi)]`.
//!
//! Proves the derive generates presence-aware (de)serialization: a wire key that
//! is absent → [`Field::Absent`], `null` → [`Field::Null`], a value →
//! [`Field::Set`]; serialization omits Absent, emits `null` for Null, and the
//! value for Set. Also proves a `Field<T>` attribute never triggers the
//! `MissingAttribute` (422) pre-pass even when it is absent.

#![cfg(feature = "derive")]

use jsonapi_core::{Document, Field, Relationship, RelationshipData, ResourceObject};
use serde_json::{Value, json};

/// A companion "patch" resource: every attribute is a tri-state `Field<T>`.
#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct ArticlePatch {
    #[jsonapi(id)]
    id: String,
    title: Field<String>,
    // A nullable-on-the-domain attribute: PATCH may set it, clear it (null), or
    // leave it (absent).
    summary: Field<String>,
}

fn parse(body: Value) -> ArticlePatch {
    Document::<ArticlePatch>::from_slice(body.to_string().as_bytes())
        .expect("valid patch document")
        .into_single()
        .expect("single resource")
}

#[test]
fn absent_key_deserializes_to_absent() {
    // Only `title` is present; `summary` is omitted entirely.
    let patch = parse(json!({
        "data": {"type": "articles", "id": "1", "attributes": {"title": "New"}}
    }));
    assert_eq!(patch.title, Field::Set("New".to_string()));
    assert_eq!(patch.summary, Field::Absent);
}

#[test]
fn explicit_null_deserializes_to_null() {
    let patch = parse(json!({
        "data": {"type": "articles", "id": "1", "attributes": {"summary": null}}
    }));
    assert_eq!(patch.summary, Field::Null);
    assert_eq!(patch.title, Field::Absent);
}

#[test]
fn present_value_deserializes_to_set() {
    let patch = parse(json!({
        "data": {"type": "articles", "id": "1",
                 "attributes": {"title": "T", "summary": "S"}}
    }));
    assert_eq!(patch.title, Field::Set("T".to_string()));
    assert_eq!(patch.summary, Field::Set("S".to_string()));
}

#[test]
fn patch_omitting_every_attribute_does_not_error() {
    // No attributes at all: a full-typed struct would 422 on required members,
    // but every field here is a tri-state, so all resolve to Absent.
    let patch = parse(json!({
        "data": {"type": "articles", "id": "1"}
    }));
    assert_eq!(patch.title, Field::Absent);
    assert_eq!(patch.summary, Field::Absent);
}

#[test]
fn field_attributes_are_not_required() {
    // The 422 pre-pass reads `required_attribute_names`; a Field<T> attribute
    // must never appear there.
    let required = ArticlePatch::type_info().required_attribute_names;
    assert!(
        required.is_empty(),
        "Field<T> attributes must not be required, got {required:?}"
    );
}

#[test]
fn serialize_omits_absent_emits_null_and_value() {
    let patch = ArticlePatch {
        id: "1".to_string(),
        title: Field::Set("Hello".to_string()),
        summary: Field::Null,
    };
    let value = serde_json::to_value(&patch).unwrap();

    let attrs = &value["attributes"];
    assert_eq!(attrs["title"], "Hello");
    // Null is emitted explicitly (clears the member on the server).
    assert!(attrs.get("summary").is_some());
    assert_eq!(attrs["summary"], Value::Null);

    // An all-absent patch omits `attributes` entirely (nothing to change).
    let empty = ArticlePatch {
        id: "1".to_string(),
        title: Field::Absent,
        summary: Field::Absent,
    };
    let empty_value = serde_json::to_value(&empty).unwrap();
    assert!(empty_value.get("attributes").is_none());
}

#[test]
fn round_trips_all_three_states() {
    for (attrs, expected_title, expected_summary) in [
        (json!({"title": "T"}), Field::Set("T".to_string()), Field::Absent),
        (json!({"summary": null}), Field::Absent, Field::Null),
        (
            json!({"title": "T", "summary": "S"}),
            Field::Set("T".to_string()),
            Field::Set("S".to_string()),
        ),
    ] {
        let patch = parse(json!({
            "data": {"type": "articles", "id": "1", "attributes": attrs}
        }));
        assert_eq!(patch.title, expected_title);
        assert_eq!(patch.summary, expected_summary);

        // Re-serialize and re-parse: the tri-state survives the round trip.
        let reserialized = serde_json::to_value(&patch).unwrap();
        let reparsed = parse(json!({ "data": reserialized }));
        assert_eq!(reparsed, patch);
    }
}

/// A `Field`-wrapped relationship: `Absent` leaves it, `Set` replaces it
/// (including clearing a to-one via null linkage).
#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct ArticleRelPatch {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(relationship, type = "people")]
    author: Field<Relationship<Person>>,
}

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    #[allow(dead_code)]
    name: String,
}

#[test]
fn field_relationship_absent_vs_replace() {
    // Absent relationship → left untouched.
    let doc = Document::<ArticleRelPatch>::from_slice(
        json!({"data": {"type": "articles", "id": "1"}})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(doc.into_single().unwrap().author, Field::Absent);

    // Present relationship → Set(replacement).
    let doc = Document::<ArticleRelPatch>::from_slice(
        json!({"data": {"type": "articles", "id": "1",
            "relationships": {"author": {"data": {"type": "people", "id": "9"}}}}})
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    let patch = doc.into_single().unwrap();
    match patch.author {
        Field::Set(rel) => assert_eq!(rel.first_id(), Some("9")),
        other => panic!("expected Set, got {other:?}"),
    }

    // Replace-with-null-linkage clears a to-one; serialization round-trips it.
    let clear = ArticleRelPatch {
        id: "1".to_string(),
        author: Field::Set(Relationship::new(RelationshipData::ToOne(None))),
    };
    let value = serde_json::to_value(&clear).unwrap();
    assert_eq!(value["relationships"]["author"]["data"], Value::Null);
}
