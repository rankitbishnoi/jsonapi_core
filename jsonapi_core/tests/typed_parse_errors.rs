//! Integration tests for typed parse errors (improvement #7).
//!
//! Verifies that `Document::parse` / `from_slice` / `from_value`
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

// ----- Regression: partial-include responses must parse (full-linkage) -----

#[test]
fn from_value_accepts_partial_include_response() {
    // The issue repro: `author` references people:9 which is NOT in `included`,
    // while the requested `comments:5` IS included and referenced. This is a
    // valid partial-include response and must parse.
    let value = serde_json::json!({
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {},
            "relationships": {
                "author": {"data": {"type": "people", "id": "9"}},
                "comments": {"data": [{"type": "comments", "id": "5"}]}
            }
        },
        "included": [{"type": "comments", "id": "5", "attributes": {}}]
    });
    Document::<Resource>::from_value(value)
        .expect("partial-include response must parse under full-linkage");
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
    let doc: Document<Article> = Document::parse(json).unwrap();
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
    let err = Document::<Article>::parse(json).unwrap_err();
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
    let err = Document::<Article>::parse(json).unwrap_err();
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
    let doc: Document<Resource> = Document::parse(json).unwrap();
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
    let err = Document::<Article>::parse(json).unwrap_err();
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
    let err = Document::<Article>::parse(json).unwrap_err();
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
    let doc: Document<Article> = Document::parse(json).unwrap();
    assert_eq!(doc.into_single().unwrap().id, "1");
}

// ----- Pre-pass is non-invasive -----

#[test]
fn errors_document_still_parses() {
    let json = r#"{"errors": [{"status": "404", "title": "Not Found"}]}"#;
    let doc: Document<Article> = Document::parse(json).unwrap();
    assert!(matches!(doc, Document::Errors { .. }));
}

#[test]
fn meta_document_still_parses() {
    let json = r#"{"meta": {"total": 0}}"#;
    let doc: Document<Article> = Document::parse(json).unwrap();
    assert!(matches!(doc, Document::Meta { .. }));
}

#[test]
fn invalid_json_surfaces_as_json_error() {
    // Unstructured serde_json failures still come through — pre-pass only
    // adds typed errors for things it can detect structurally.
    let err = Document::<Article>::parse("not json").unwrap_err();
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
    let err = Document::<Book>::parse(json).unwrap_err();
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
    let err = Document::<Book>::parse(json).unwrap_err();
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
    let doc = Document::<Book>::parse(json).expect("parse");
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
    let doc = Document::<Book>::parse(json).expect("parse");
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
    let doc = Document::<Resource>::parse(json).expect("dynamic Resource accepts any shape");
    let res = doc.into_single().expect("single");
    assert_eq!(res.resource_type(), "books");
}

// ============================================================
// FullLinkageViolation — compound-document full-linkage check
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
fn from_str_accepts_partial_include_response() {
    // Partial include: `author` references people:9 which is NOT in `included`,
    // while the requested comment IS included and referenced. Valid response.
    let json = r#"{
        "data": {
            "type": "articles_with_rels",
            "id": "1",
            "attributes": { "title": "Hello" },
            "relationships": {
                "author":   { "data": { "type": "people", "id": "9" } },
                "comments": { "data": [{ "type": "comments", "id": "5" }] }
            }
        },
        "included": [
            { "type": "comments", "id": "5", "attributes": { "body": "hi" } }
        ]
    }"#;
    Document::<ArticleWithRels>::parse(json)
        .expect("partial-include response must parse under full-linkage");
}

#[test]
fn from_str_surfaces_full_linkage_violation_for_unreferenced_included() {
    // people:1 is in `included` but no relationship anywhere references it.
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
    let err = Document::<ArticleWithRels>::parse(json).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::FullLinkageViolation { r#type: type_, id, location }
                if type_ == "people" && id == "1" && location == "included[0]"
        ),
        "got: {err:?}",
    );
}

#[test]
fn from_str_full_linkage_satisfied_by_transitive_reference() {
    // orgs:42 is referenced only by another included resource (people:1), not
    // by primary data. Full linkage is still satisfied.
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
            },
            { "type": "orgs", "id": "42", "attributes": { "name": "Acme" } }
        ]
    }"#;
    Document::<ArticleWithRels>::parse(json).expect("transitive linkage must satisfy full linkage");
}

#[test]
fn from_str_full_linkage_matches_lid_identified_included() {
    // Included resource identified by `lid` is matched against `lid` linkage.
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
            { "type": "people", "lid": "tmp-1", "attributes": { "name": "Dan" } }
        ]
    }"#;
    Document::<ArticleWithRels>::parse(json)
        .expect("lid-identified included must match lid linkage");
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
    Document::<ArticleWithRels>::parse(json).expect("parse");
}

#[test]
fn from_str_error_precedence_order_type_mismatch_first() {
    // Payload broken three ways: wrong primary type, missing required
    // attribute, unreferenced included resource. TypeMismatch must fire first.
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
    let err = Document::<ArticleWithRels>::parse(json).unwrap_err();
    assert!(
        matches!(&err, Error::TypeMismatch { .. }),
        "expected TypeMismatch first, got: {err:?}",
    );
}

#[test]
fn from_str_error_precedence_order_missing_attribute_before_full_linkage() {
    // Type matches, but the required attribute is missing AND `included` carries
    // an unreferenced resource. MissingAttribute must fire first.
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
    let err = Document::<ArticleWithRels>::parse(json).unwrap_err();
    assert!(
        matches!(&err, Error::MissingAttribute { .. }),
        "expected MissingAttribute before FullLinkageViolation, got: {err:?}",
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
    Document::<ArticleWithRels>::parse(json).expect("parse");
}

#[test]
fn from_str_full_linkage_considers_whole_primary_collection() {
    // people:9 is referenced by the second article, so it is linked. people:1 in
    // `included` is referenced by neither article → full-linkage violation.
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
                    "author":   { "data": { "type": "people", "id": "9" } },
                    "comments": { "data": [] }
                }
            }
        ],
        "included": [
            { "type": "people", "id": "9", "attributes": { "name": "Dan" } },
            { "type": "people", "id": "1", "attributes": { "name": "Nobody" } }
        ]
    }"#;
    let err = Document::<ArticleWithRels>::parse(json).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::FullLinkageViolation { r#type: type_, id, location }
                if type_ == "people" && id == "1" && location == "included[1]"
        ),
        "got: {err:?}",
    );
}

#[test]
fn from_str_full_linkage_fires_for_dynamic_resource_primary() {
    // The type and required-attribute checks are skipped for the open-set
    // `Resource` primary, but the full-linkage check must still run: tags:99 is
    // included yet referenced by nothing.
    let json = r#"{
        "data": {
            "type": "whatever",
            "id": "1",
            "relationships": {
                "author": { "data": { "type": "people", "id": "9" } }
            }
        },
        "included": [
            { "type": "people", "id": "9", "attributes": {} },
            { "type": "tags", "id": "99", "attributes": {} }
        ]
    }"#;
    let err = Document::<Resource>::parse(json).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::FullLinkageViolation { r#type: type_, id, location }
                if type_ == "tags" && id == "99" && location == "included[1]"
        ),
        "got: {err:?}",
    );
}

#[test]
fn from_str_full_linkage_accepts_self_referencing_included() {
    // Characterisation: an included resource whose own linkage points at itself
    // counts as referenced, because references are gathered from included
    // resources' relationships too. people:9 is not referenced by primary data,
    // only by its own `self` relationship — and the document parses. Pinned so a
    // future stricter reachability-from-primary reading is a deliberate change.
    let json = r#"{
        "data": {
            "type": "whatever",
            "id": "1",
            "relationships": {
                "noop": { "data": null }
            }
        },
        "included": [
            {
                "type": "people", "id": "9", "attributes": {},
                "relationships": {
                    "self": { "data": { "type": "people", "id": "9" } }
                }
            }
        ]
    }"#;
    Document::<Resource>::parse(json)
        .expect("self-referencing included resource counts as referenced");
}

#[test]
fn from_str_full_linkage_accepts_island_cycle_unreachable_from_primary() {
    // Characterisation: two included resources reference each other but neither
    // is referenced from primary data. They satisfy the check because linkage is
    // gathered document-wide (not by reachability from primary). Pinned so the
    // lenient behaviour can't drift silently.
    let json = r#"{
        "data": {
            "type": "whatever",
            "id": "1",
            "relationships": {
                "noop": { "data": null }
            }
        },
        "included": [
            {
                "type": "people", "id": "9", "attributes": {},
                "relationships": {
                    "org": { "data": { "type": "orgs", "id": "42" } }
                }
            },
            {
                "type": "orgs", "id": "42", "attributes": {},
                "relationships": {
                    "lead": { "data": { "type": "people", "id": "9" } }
                }
            }
        ]
    }"#;
    Document::<Resource>::parse(json)
        .expect("island cycle satisfies document-wide full linkage");
}

// ----- Single-step parse helpers (parse_single / parse_many) -----

#[test]
fn parse_single_folds_from_slice_and_into_single() {
    let json = br#"{"data":{"type":"people","id":"9","attributes":{"name":"Dan"}}}"#;
    let person = Document::<Person>::parse_single(json).unwrap();
    assert_eq!(person.resource_id(), Some("9"));
    assert_eq!(person.name, "Dan");
}

#[test]
fn parse_many_folds_from_slice_and_into_many() {
    let json = br#"{"data":[
        {"type":"people","id":"1","attributes":{"name":"A"}},
        {"type":"people","id":"2","attributes":{"name":"B"}}
    ]}"#;
    let people = Document::<Person>::parse_many(json).unwrap();
    assert_eq!(people.len(), 2);
    assert_eq!(people[1].resource_id(), Some("2"));
}

#[test]
fn parse_single_errors_on_collection_shape() {
    let json = br#"{"data":[{"type":"people","id":"1","attributes":{"name":"A"}}]}"#;
    let err = Document::<Person>::parse_single(json).unwrap_err();
    assert!(
        matches!(err, Error::UnexpectedDocumentShape { .. }),
        "got: {err:?}",
    );
}

#[test]
fn parse_many_errors_on_single_shape() {
    let json = br#"{"data":{"type":"people","id":"1","attributes":{"name":"A"}}}"#;
    let err = Document::<Person>::parse_many(json).unwrap_err();
    assert!(
        matches!(err, Error::UnexpectedDocumentShape { .. }),
        "got: {err:?}",
    );
}

#[test]
fn parse_single_propagates_invalid_json() {
    let err = Document::<Person>::parse_single(b"not json").unwrap_err();
    assert!(matches!(err, Error::Json(_)), "got: {err:?}");
}
