#![cfg(feature = "atomic-ops")]

use std::collections::BTreeMap;

use jsonapi_core::{
    Identity, JsonApiMediaType, PrimaryData, Resource,
    atomic::{
        ATOMIC_EXT_URI, AtomicOperation, AtomicRequest, AtomicResponse, AtomicResult, OperationRef,
        OperationTarget,
    },
    negotiate_accept,
};

fn make_resource(type_: &str, id: Option<&str>, lid: Option<&str>) -> Resource {
    Resource {
        type_: type_.into(),
        id: id.map(str::to_owned),
        lid: lid.map(str::to_owned),
        attributes: serde_json::json!({}),
        relationships: BTreeMap::new(),
        links: None,
        meta: None,
    }
}

#[test]
fn spec_add_resource_round_trip() {
    let wire = r#"{
        "atomic:operations": [
            {
                "op": "add",
                "data": {
                    "type": "articles",
                    "lid": "local-1",
                    "attributes": {"title": "Ember Hamster"}
                }
            }
        ]
    }"#;
    let req: AtomicRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.operations.len(), 1);
    let re: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert_eq!(re["atomic:operations"][0]["op"], "add");
    assert_eq!(re["atomic:operations"][0]["data"]["lid"], "local-1");
}

#[test]
fn spec_update_resource_round_trip() {
    let wire = r#"{
        "atomic:operations": [
            {
                "op": "update",
                "ref": {"type": "articles", "id": "1"},
                "data": {
                    "type": "articles",
                    "id": "1",
                    "attributes": {"title": "Ember Hamster Rename"}
                }
            }
        ]
    }"#;
    let req: AtomicRequest = serde_json::from_str(wire).unwrap();
    match &req.operations[0] {
        AtomicOperation::Update { target, data } => {
            assert_eq!(target.r#ref.as_ref().unwrap().type_, "articles");
            assert!(matches!(data, PrimaryData::Single(_)));
        }
        _ => panic!("expected Update"),
    }
    let re: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert_eq!(re["atomic:operations"][0]["op"], "update");
}

#[test]
fn spec_remove_resource_round_trip() {
    let wire = r#"{
        "atomic:operations": [
            {
                "op": "remove",
                "ref": {"type": "articles", "id": "13"}
            }
        ]
    }"#;
    let req: AtomicRequest = serde_json::from_str(wire).unwrap();
    assert!(matches!(req.operations[0], AtomicOperation::Remove { .. }));
    let re: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert!(re["atomic:operations"][0].get("data").is_none());
}

#[test]
fn spec_add_to_to_many_round_trip() {
    let wire = r#"{
        "atomic:operations": [
            {
                "op": "add",
                "ref": {"type": "articles", "id": "1", "relationship": "tags"},
                "data": [
                    {"type": "tags", "id": "2"},
                    {"type": "tags", "id": "3"}
                ]
            }
        ]
    }"#;
    let req: AtomicRequest = serde_json::from_str(wire).unwrap();
    match &req.operations[0] {
        AtomicOperation::Add { target, data } => {
            assert_eq!(
                target.r#ref.as_ref().unwrap().relationship.as_deref(),
                Some("tags")
            );
            match data {
                PrimaryData::Many(v) => assert_eq!(v.len(), 2),
                _ => panic!("expected Many"),
            }
        }
        _ => panic!("expected Add"),
    }
}

#[test]
fn spec_replace_to_one_round_trip() {
    let wire = r#"{
        "atomic:operations": [
            {
                "op": "update",
                "ref": {"type": "articles", "id": "1", "relationship": "author"},
                "data": {"type": "people", "id": "9"}
            }
        ]
    }"#;
    let req: AtomicRequest = serde_json::from_str(wire).unwrap();
    let re: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert_eq!(re["atomic:operations"][0]["data"]["id"], "9");
}

#[test]
fn spec_lid_reference_round_trip() {
    let wire = r#"{
        "atomic:operations": [
            {
                "op": "add",
                "data": {
                    "type": "people",
                    "lid": "author-1",
                    "attributes": {"name": "Dan"}
                }
            },
            {
                "op": "add",
                "data": {
                    "type": "articles",
                    "lid": "article-1",
                    "attributes": {"title": "Hello"},
                    "relationships": {
                        "author": {"data": {"type": "people", "lid": "author-1"}}
                    }
                }
            }
        ]
    }"#;
    let req: AtomicRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.operations.len(), 2);
    req.validate_lid_refs().unwrap();
}

#[test]
fn mixed_type_payload_round_trips() {
    let req = AtomicRequest {
        operations: vec![
            AtomicOperation::Add {
                target: OperationTarget::default(),
                data: PrimaryData::Single(Box::new(make_resource("people", None, Some("p1")))),
            },
            AtomicOperation::Add {
                target: OperationTarget::default(),
                data: PrimaryData::Single(Box::new(make_resource("articles", None, Some("a1")))),
            },
        ],
    };
    let json = serde_json::to_value(&req).unwrap();
    let round: AtomicRequest = serde_json::from_value(json).unwrap();
    assert_eq!(round, req);
}

#[test]
fn atomic_ext_propagates_through_negotiate_accept() {
    let mt = negotiate_accept(
        "application/vnd.api+json; ext=\"https://jsonapi.org/ext/atomic\"",
        &[ATOMIC_EXT_URI],
        &[],
    )
    .unwrap();
    assert!(mt.ext.iter().any(|e| e == ATOMIC_EXT_URI));

    let header = mt.to_header_value();
    let parsed = JsonApiMediaType::parse(&header).unwrap();
    assert!(parsed.ext.iter().any(|e| e == ATOMIC_EXT_URI));
}

#[test]
fn response_with_mixed_results_round_trips() {
    let wire = r#"{
        "atomic:results": [
            {"data": {"type": "articles", "id": "1", "attributes": {"title": "Hi"}}},
            {},
            {"data": null}
        ]
    }"#;
    let resp: AtomicResponse = serde_json::from_str(wire).unwrap();
    assert_eq!(resp.results.len(), 3);
    assert!(resp.results[1] == AtomicResult::default());
    // {"data": null} — serde's Option absorbs the JSON null, yielding None
    assert!(resp.results[2].data.is_none());
}

#[test]
fn validate_lid_refs_catches_dangling_ref() {
    let req = AtomicRequest {
        operations: vec![AtomicOperation::Update {
            target: OperationTarget {
                r#ref: Some(OperationRef {
                    type_: "people".into(),
                    identity: Identity::Lid("ghost".into()),
                    relationship: None,
                }),
                href: None,
            },
            data: PrimaryData::Single(Box::new(make_resource("people", None, Some("ghost")))),
        }],
    };
    assert!(req.validate_lid_refs().is_err());
}
