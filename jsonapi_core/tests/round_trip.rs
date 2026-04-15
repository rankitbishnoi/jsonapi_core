use std::collections::BTreeMap;

use jsonapi_core::{
    Document, Identity, PrimaryData, Registry, Relationship, RelationshipData, Resource,
    ResourceIdentifier, ResourceObject,
};

/// Spec example: single resource with relationships and included.
#[test]
fn test_spec_compound_document() {
    let json = r#"{
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {
                "title": "JSON:API paints my bikeshed!"
            },
            "relationships": {
                "author": {
                    "data": {"type": "people", "id": "9"}
                },
                "comments": {
                    "data": [
                        {"type": "comments", "id": "5"},
                        {"type": "comments", "id": "12"}
                    ]
                }
            },
            "links": {
                "self": "http://example.com/articles/1"
            }
        },
        "included": [
            {
                "type": "people",
                "id": "9",
                "attributes": {
                    "first-name": "Dan",
                    "last-name": "Gebhardt",
                    "twitter": "dgeb"
                },
                "links": {
                    "self": "http://example.com/people/9"
                }
            },
            {
                "type": "comments",
                "id": "5",
                "attributes": {
                    "body": "First!"
                },
                "links": {
                    "self": "http://example.com/comments/5"
                }
            },
            {
                "type": "comments",
                "id": "12",
                "attributes": {
                    "body": "I like XML better"
                },
                "links": {
                    "self": "http://example.com/comments/12"
                }
            }
        ]
    }"#;

    let doc: Document<Resource> = serde_json::from_str(json).unwrap();

    match &doc {
        Document::Data {
            data,
            included,
            links,
            ..
        } => {
            // Primary data
            let article = match data {
                PrimaryData::Single(r) => r.as_ref(),
                _ => panic!("expected single resource"),
            };
            assert_eq!(article.resource_type(), "articles");
            assert_eq!(article.resource_id(), Some("1"));
            assert_eq!(article.attributes["title"], "JSON:API paints my bikeshed!");

            // Relationships
            assert!(article.relationships.contains_key("author"));
            assert!(article.relationships.contains_key("comments"));
            match &article.relationships["comments"] {
                RelationshipData::ToMany(rids) => assert_eq!(rids.len(), 2),
                _ => panic!("expected to-many"),
            }

            // Included
            assert_eq!(included.len(), 3);

            // Registry lookup
            let registry = Registry::from_included(included).unwrap();
            let dan: Resource = registry.get_by_id("people", "9").unwrap();
            assert_eq!(dan.attributes["first-name"], "Dan");

            let comment: Resource = registry.get_by_id("comments", "5").unwrap();
            assert_eq!(comment.attributes["body"], "First!");

            // Suppress unused variable warning
            let _ = links;
        }
        _ => panic!("expected Document::Data"),
    }

    // Round-trip
    let serialized = serde_json::to_string(&doc).unwrap();
    let _reparsed: Document<Resource> = serde_json::from_str(&serialized).unwrap();
}

/// Error document.
#[test]
fn test_error_document() {
    let json = r#"{
        "errors": [
            {
                "status": "422",
                "source": {"pointer": "/data/attributes/first-name"},
                "title": "Invalid Attribute",
                "detail": "First name must contain at least two characters."
            }
        ],
        "jsonapi": {"version": "1.1"}
    }"#;

    let doc: Document<Resource> = serde_json::from_str(json).unwrap();
    match &doc {
        Document::Errors {
            errors, jsonapi, ..
        } => {
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].status.as_deref(), Some("422"));
            assert_eq!(
                errors[0].source.as_ref().unwrap().pointer.as_deref(),
                Some("/data/attributes/first-name")
            );
            assert_eq!(jsonapi.as_ref().unwrap().version.as_deref(), Some("1.1"));
        }
        _ => panic!("expected Document::Errors"),
    }

    let serialized = serde_json::to_string(&doc).unwrap();
    let _reparsed: Document<Resource> = serde_json::from_str(&serialized).unwrap();
}

/// Meta-only document.
#[test]
fn test_meta_only_document() {
    let json = r#"{"meta":{"total-pages":13}}"#;
    let doc: Document<Resource> = serde_json::from_str(json).unwrap();
    match &doc {
        Document::Meta { meta, .. } => {
            assert_eq!(meta["total-pages"], 13);
        }
        _ => panic!("expected Document::Meta"),
    }
}

/// data + errors = rejected (strict mode).
#[test]
fn test_reject_data_and_errors() {
    let json = r#"{"data":null,"errors":[]}"#;
    let result: Result<Document<Resource>, _> = serde_json::from_str(json);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("must not contain both")
    );
}

/// Empty document = rejected.
#[test]
fn test_reject_empty_document() {
    let json = "{}";
    let result: Result<Document<Resource>, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

/// lid-only resource identifier.
#[test]
fn test_lid_only_identifier() {
    let json = r#"{"type":"articles","lid":"temp-1"}"#;
    let rid: ResourceIdentifier = serde_json::from_str(json).unwrap();
    assert_eq!(rid.identity, Identity::Lid("temp-1".into()));
    assert_eq!(serde_json::to_string(&rid).unwrap(), json);
}

/// Document with data: null (empty to-one).
#[test]
fn test_null_data_document() {
    let json = r#"{"data":null}"#;
    let doc: Document<Resource> = serde_json::from_str(json).unwrap();
    match doc {
        Document::Data {
            data: PrimaryData::Null,
            included,
            ..
        } => assert!(included.is_empty()),
        _ => panic!("expected Data with Null"),
    }
}

/// Registry get via typed Relationship.
#[test]
fn test_registry_get_via_relationship() {
    let included = vec![Resource {
        type_: "people".into(),
        id: Some("9".into()),
        lid: None,
        attributes: serde_json::json!({"name": "Dan"}),
        relationships: BTreeMap::new(),
        links: None,
        meta: None,
    }];

    let registry = Registry::from_included(&included).unwrap();
    let rel: Relationship<Resource> =
        Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            type_: "people".into(),
            identity: Identity::Id("9".into()),
            meta: None,
        })));

    let person: Resource = registry.get(&rel).unwrap();
    assert_eq!(person.attributes["name"], "Dan");
}
