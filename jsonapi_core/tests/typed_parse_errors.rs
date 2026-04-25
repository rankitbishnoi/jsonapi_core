//! Integration tests for typed parse errors (improvement #7).
//!
//! Verifies that `Document::from_str` / `from_slice` / `from_value`
//! surface `Error::TypeMismatch` and `Error::MalformedRelationship` as
//! structured variants instead of opaque `serde_json::Error` strings,
//! so consumers can map upstream-format failures to the right HTTP status.

#![cfg(feature = "derive")]

use jsonapi_core::model::{Document, Resource};
use jsonapi_core::{Error, JsonApi, Relationship, ResourceObject};

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
}

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
}

// ----- Happy path -----

#[test]
fn from_str_parses_matching_type() {
    let json = r#"{
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {"title": "Hello"},
            "relationships": {
                "author": {"data": {"type": "people", "id": "9"}}
            }
        }
    }"#;
    let doc: Document<Article> = Document::from_str(json).unwrap();
    let article = doc.into_single().unwrap();
    assert_eq!(article.id, "1");
    assert_eq!(article.title, "Hello");
}

#[test]
fn from_slice_parses_matching_type() {
    let bytes = br#"{"data":{"type":"articles","id":"1","attributes":{"title":"Hi"},"relationships":{"author":{"data":{"type":"people","id":"9"}}}}}"#;
    let doc: Document<Article> = Document::from_slice(bytes).unwrap();
    assert_eq!(doc.into_single().unwrap().id, "1");
}

#[test]
fn from_value_parses_matching_type() {
    let value = serde_json::json!({
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {"title": "Hi"},
            "relationships": {"author": {"data": {"type": "people", "id": "9"}}}
        }
    });
    let doc: Document<Article> = Document::from_value(value).unwrap();
    assert_eq!(doc.into_single().unwrap().id, "1");
}

// ----- TypeMismatch -----

#[test]
fn type_mismatch_at_data_root() {
    let json = r#"{
        "data": {
            "type": "stories",
            "id": "1",
            "attributes": {"title": "Hi"}
        }
    }"#;
    let err = Document::<Article>::from_str(json).unwrap_err();
    match err {
        Error::TypeMismatch {
            expected,
            got,
            location,
        } => {
            assert_eq!(expected, "articles");
            assert_eq!(got, "stories");
            assert_eq!(location, "data");
        }
        other => panic!("expected TypeMismatch, got {other:?}"),
    }
}

#[test]
fn type_mismatch_inside_collection() {
    let json = r#"{
        "data": [
            {"type": "articles", "id": "1", "attributes": {"title": "A"}, "relationships": {"author": {"data": {"type": "people", "id": "9"}}}},
            {"type": "stories",  "id": "2", "attributes": {"title": "B"}, "relationships": {"author": {"data": {"type": "people", "id": "9"}}}}
        ]
    }"#;
    let err = Document::<Article>::from_str(json).unwrap_err();
    match err {
        Error::TypeMismatch {
            expected,
            got,
            location,
        } => {
            assert_eq!(expected, "articles");
            assert_eq!(got, "stories");
            assert_eq!(location, "data[1]");
        }
        other => panic!("expected TypeMismatch, got {other:?}"),
    }
}

#[test]
fn dynamic_resource_skips_type_check() {
    // `Document<Resource>` is the open-set fallback; pre-pass must NOT
    // flag arbitrary types as mismatches.
    let json = r#"{
        "data": {"type": "anything", "id": "1", "attributes": {}}
    }"#;
    let doc: Document<Resource> = Document::from_str(json).unwrap();
    assert_eq!(doc.into_single().unwrap().resource_type(), "anything");
}

// ----- MalformedRelationship -----

#[test]
fn malformed_relationship_data_is_string() {
    let json = r#"{
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {"title": "Hi"},
            "relationships": {
                "author": {"data": "9"}
            }
        }
    }"#;
    let err = Document::<Article>::from_str(json).unwrap_err();
    match err {
        Error::MalformedRelationship {
            name,
            location,
            reason,
        } => {
            assert_eq!(name, "author");
            assert_eq!(location, "data");
            assert!(reason.contains("string"), "reason: {reason}");
        }
        other => panic!("expected MalformedRelationship, got {other:?}"),
    }
}

#[test]
fn malformed_relationship_value_is_not_an_object() {
    let json = r#"{
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {"title": "Hi"},
            "relationships": {"author": "not-an-object"}
        }
    }"#;
    let err = Document::<Article>::from_str(json).unwrap_err();
    match err {
        Error::MalformedRelationship { name, reason, .. } => {
            assert_eq!(name, "author");
            assert!(reason.contains("must be an object"), "reason: {reason}");
        }
        other => panic!("expected MalformedRelationship, got {other:?}"),
    }
}

#[test]
fn relationship_without_data_member_is_allowed() {
    // JSON:API allows links/meta-only relationship objects.
    let json = r#"{
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {"title": "Hi"},
            "relationships": {
                "author": {"links": {"related": "/articles/1/author"}, "data": {"type": "people", "id": "9"}}
            }
        }
    }"#;
    let doc: Document<Article> = Document::from_str(json).unwrap();
    assert_eq!(doc.into_single().unwrap().id, "1");
}

// ----- Pre-pass is non-invasive -----

#[test]
fn errors_document_still_parses() {
    let json = r#"{"errors": [{"status": "404", "title": "Not Found"}]}"#;
    let doc: Document<Article> = Document::from_str(json).unwrap();
    assert!(matches!(doc, Document::Errors { .. }));
}

#[test]
fn meta_document_still_parses() {
    let json = r#"{"meta": {"total": 0}}"#;
    let doc: Document<Article> = Document::from_str(json).unwrap();
    assert!(matches!(doc, Document::Meta { .. }));
}

#[test]
fn invalid_json_surfaces_as_json_error() {
    // Unstructured serde_json failures still come through — pre-pass only
    // adds typed errors for things it can detect structurally.
    let err = Document::<Article>::from_str("not json").unwrap_err();
    assert!(matches!(err, Error::Json(_)));
}
