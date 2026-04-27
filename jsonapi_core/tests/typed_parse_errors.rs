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

// ============================================================
// MissingAttribute — deferred-variants plan
// ============================================================

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "books")]
struct Book {
    #[jsonapi(id)]
    id: String,
    title: String,
    subtitle: Option<String>,
}

#[test]
fn from_str_surfaces_missing_required_attribute_on_primary_single() {
    let json = r#"{
        "data": {
            "type": "books",
            "id": "1",
            "attributes": { "subtitle": "tagline only" }
        }
    }"#;
    let err = Document::<Book>::from_str(json).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::MissingAttribute {
                resource_type: "books",
                attribute: "title",
                location,
            } if location == "data"
        ),
        "expected MissingAttribute, got: {err:?}",
    );
}

#[test]
fn from_str_surfaces_missing_required_attribute_on_primary_collection() {
    let json = r#"{
        "data": [
            { "type": "books", "id": "1", "attributes": { "title": "First" } },
            { "type": "books", "id": "2", "attributes": { "subtitle": "no title" } }
        ]
    }"#;
    let err = Document::<Book>::from_str(json).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::MissingAttribute {
                resource_type: "books",
                attribute: "title",
                location,
            } if location == "data[1]"
        ),
        "expected MissingAttribute at data[1], got: {err:?}",
    );
}

#[test]
fn from_str_optional_attribute_omission_is_not_an_error() {
    let json = r#"{
        "data": {
            "type": "books",
            "id": "1",
            "attributes": { "title": "Hello" }
        }
    }"#;
    let doc = Document::<Book>::from_str(json).expect("parse");
    let book = doc.into_single().expect("single");
    assert_eq!(book.title, "Hello");
    assert_eq!(book.subtitle, None);
}

#[test]
fn from_str_optional_attribute_explicit_null_is_not_an_error() {
    let json = r#"{
        "data": {
            "type": "books",
            "id": "1",
            "attributes": { "title": "Hello", "subtitle": null }
        }
    }"#;
    let doc = Document::<Book>::from_str(json).expect("parse");
    let book = doc.into_single().expect("single");
    assert_eq!(book.subtitle, None);
}

#[test]
fn from_str_required_attribute_check_skipped_for_dynamic_resource() {
    let json = r#"{
        "data": {
            "type": "books",
            "id": "1",
            "attributes": { "subtitle": "no title" }
        }
    }"#;
    let doc = Document::<Resource>::from_str(json).expect("dynamic Resource accepts any shape");
    let res = doc.into_single().expect("single");
    assert_eq!(res.resource_type(), "books");
}

// ============================================================
// IncludedRefMissing — deferred-variants plan
// ============================================================

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "comments")]
struct Comment {
    #[jsonapi(id)]
    id: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "articles_with_rels")]
struct ArticleWithRels {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
    #[jsonapi(relationship, type = "comments")]
    comments: Relationship<Comment>,
}

#[test]
fn from_str_surfaces_included_ref_missing_on_primary_to_one() {
    let json = r#"{
        "data": {
            "type": "articles_with_rels",
            "id": "1",
            "attributes": { "title": "Hello" },
            "relationships": {
                "author":   { "data": { "type": "people", "id": "9" } },
                "comments": { "data": [] }
            }
        },
        "included": [
            { "type": "people", "id": "1", "attributes": { "name": "Other" } }
        ]
    }"#;
    let err = Document::<ArticleWithRels>::from_str(json).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::IncludedRefMissing { name, type_, id, location }
                if name == "author" && type_ == "people" && id == "9"
                && location == "data.relationships.author"
        ),
        "got: {err:?}",
    );
}

#[test]
fn from_str_surfaces_included_ref_missing_on_primary_to_many() {
    let json = r#"{
        "data": {
            "type": "articles_with_rels",
            "id": "1",
            "attributes": { "title": "Hello" },
            "relationships": {
                "author":   { "data": null },
                "comments": { "data": [
                    { "type": "comments", "id": "1" },
                    { "type": "comments", "id": "9" },
                    { "type": "comments", "id": "2" }
                ]}
            }
        },
        "included": [
            { "type": "comments", "id": "1", "attributes": { "body": "a" } },
            { "type": "comments", "id": "2", "attributes": { "body": "c" } }
        ]
    }"#;
    let err = Document::<ArticleWithRels>::from_str(json).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::IncludedRefMissing { name, type_, id, location }
                if name == "comments" && type_ == "comments" && id == "9"
                && location == "data.relationships.comments"
        ),
        "got: {err:?}",
    );
}

#[test]
fn from_str_does_not_fire_included_ref_missing_for_transitive_references() {
    // IncludedRefMissing is intentionally primary-data-only. References
    // *inside* an included resource are not validated against the included
    // set, because partial-include APIs (Drupal etc.) routinely return
    // included resources whose own relationships point to non-included
    // resources the consumer didn't ask for.
    let json = r#"{
        "data": {
            "type": "articles_with_rels",
            "id": "1",
            "attributes": { "title": "Hello" },
            "relationships": {
                "author":   { "data": { "type": "people", "id": "1" } },
                "comments": { "data": [] }
            }
        },
        "included": [
            {
                "type": "people",
                "id": "1",
                "attributes": { "name": "Dan" },
                "relationships": {
                    "organization": { "data": { "type": "orgs", "id": "42" } }
                }
            }
        ]
    }"#;
    Document::<ArticleWithRels>::from_str(json)
        .expect("transitive included references must not fire IncludedRefMissing");
}

#[test]
fn from_str_lid_only_relationship_skipped() {
    let json = r#"{
        "data": {
            "type": "articles_with_rels",
            "id": "1",
            "attributes": { "title": "Hello" },
            "relationships": {
                "author":   { "data": { "type": "people", "lid": "tmp-1" } },
                "comments": { "data": [] }
            }
        },
        "included": [
            { "type": "people", "id": "1", "attributes": { "name": "Dan" } }
        ]
    }"#;
    // Pre-pass must not fire IncludedRefMissing for lid-only references.
    let result = Document::<ArticleWithRels>::from_str(json);
    if let Err(err) = &result {
        assert!(
            !matches!(err, Error::IncludedRefMissing { .. }),
            "lid-only references must not trigger IncludedRefMissing, got: {err:?}",
        );
    }
}

#[test]
fn from_str_relationship_with_null_data_is_not_an_error() {
    let json = r#"{
        "data": {
            "type": "articles_with_rels",
            "id": "1",
            "attributes": { "title": "Hello" },
            "relationships": {
                "author":   { "data": null },
                "comments": { "data": [] }
            }
        }
    }"#;
    Document::<ArticleWithRels>::from_str(json).expect("parse");
}

#[test]
fn from_str_error_precedence_order_type_mismatch_first() {
    // Payload broken three ways: wrong primary type, missing required
    // attribute, broken included ref. TypeMismatch must fire first.
    let json = r#"{
        "data": {
            "type": "novels",
            "id": "1",
            "attributes": {},
            "relationships": {
                "author":   { "data": { "type": "people", "id": "missing" } },
                "comments": { "data": [] }
            }
        },
        "included": [
            { "type": "people", "id": "1", "attributes": { "name": "x" } }
        ]
    }"#;
    let err = Document::<ArticleWithRels>::from_str(json).unwrap_err();
    assert!(
        matches!(&err, Error::TypeMismatch { .. }),
        "expected TypeMismatch first, got: {err:?}",
    );
}

#[test]
fn from_str_error_precedence_order_missing_attribute_before_included_ref() {
    // Type matches, but attribute is missing AND a relationship ref is
    // missing. MissingAttribute must fire first.
    let json = r#"{
        "data": {
            "type": "articles_with_rels",
            "id": "1",
            "attributes": {},
            "relationships": {
                "author":   { "data": { "type": "people", "id": "missing" } },
                "comments": { "data": [] }
            }
        },
        "included": [
            { "type": "people", "id": "1", "attributes": { "name": "x" } }
        ]
    }"#;
    let err = Document::<ArticleWithRels>::from_str(json).unwrap_err();
    assert!(
        matches!(&err, Error::MissingAttribute { .. }),
        "expected MissingAttribute before IncludedRefMissing, got: {err:?}",
    );
}

#[test]
fn from_str_present_reference_is_not_an_error() {
    let json = r#"{
        "data": {
            "type": "articles_with_rels",
            "id": "1",
            "attributes": { "title": "Hello" },
            "relationships": {
                "author":   { "data": { "type": "people", "id": "1" } },
                "comments": { "data": [
                    { "type": "comments", "id": "10" }
                ]}
            }
        },
        "included": [
            { "type": "people",   "id": "1",  "attributes": { "name": "Dan" } },
            { "type": "comments", "id": "10", "attributes": { "body": "hi" } }
        ]
    }"#;
    Document::<ArticleWithRels>::from_str(json).expect("parse");
}

#[test]
fn from_str_surfaces_included_ref_missing_on_primary_collection() {
    let json = r#"{
        "data": [
            {
                "type": "articles_with_rels",
                "id": "1",
                "attributes": { "title": "first" },
                "relationships": {
                    "author":   { "data": null },
                    "comments": { "data": [] }
                }
            },
            {
                "type": "articles_with_rels",
                "id": "2",
                "attributes": { "title": "second" },
                "relationships": {
                    "author":   { "data": { "type": "people", "id": "missing" } },
                    "comments": { "data": [] }
                }
            }
        ],
        "included": [
            { "type": "people", "id": "1", "attributes": { "name": "Dan" } }
        ]
    }"#;
    let err = Document::<ArticleWithRels>::from_str(json).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::IncludedRefMissing { location, .. }
                if location == "data[1].relationships.author"
        ),
        "got: {err:?}",
    );
}
