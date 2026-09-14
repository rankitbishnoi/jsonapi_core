//! Property-based round-trip tests: for a generated value, serializing to JSON
//! and deserializing back must yield an equal value. Catches serde asymmetries
//! (dropped/renamed members, cardinality confusion) that point tests can miss.

use jsonapi_core::{
    Document, Identity, PrimaryData, RelationshipData, Resource, ResourceIdentifier,
    ResourceRelationship,
};
use proptest::prelude::*;

// A JSON:API member name: leading letter, then letters/digits/underscore.
fn arb_member() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,7}".prop_map(String::from)
}

// Scalar attribute values that round-trip exactly through serde_json. Floats are
// excluded on purpose — f64 text formatting is not guaranteed to be identity.
fn arb_scalar() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        "[ -~]{0,16}".prop_map(serde_json::Value::from),
        any::<bool>().prop_map(serde_json::Value::from),
        any::<i64>().prop_map(serde_json::Value::from),
    ]
}

fn arb_attributes() -> impl Strategy<Value = serde_json::Value> {
    prop::collection::btree_map(arb_member(), arb_scalar(), 0..4)
        .prop_map(|m| serde_json::Value::Object(m.into_iter().collect()))
}

fn arb_identifier() -> impl Strategy<Value = ResourceIdentifier> {
    ("[a-z]{1,8}", "[a-z0-9]{1,6}").prop_map(|(ty, id)| ResourceIdentifier {
        r#type: ty,
        identity: Identity::Id(id),
        meta: None,
    })
}

fn arb_relationship() -> impl Strategy<Value = ResourceRelationship> {
    prop_oneof![
        arb_identifier()
            .prop_map(|rid| ResourceRelationship::new(RelationshipData::ToOne(Some(rid)))),
        Just(ResourceRelationship::new(RelationshipData::ToOne(None))),
        prop::collection::vec(arb_identifier(), 0..3)
            .prop_map(|v| ResourceRelationship::new(RelationshipData::ToMany(v))),
    ]
}

fn arb_resource() -> impl Strategy<Value = Resource> {
    (
        "[a-z]{1,10}",
        prop::option::of("[a-z0-9]{1,6}"),
        arb_attributes(),
        prop::collection::btree_map(arb_member(), arb_relationship(), 0..3),
    )
        .prop_map(|(ty, id, attributes, relationships)| Resource {
            r#type: ty,
            id,
            lid: None,
            attributes,
            relationships,
            links: None,
            meta: None,
        })
}

proptest! {
    #[test]
    fn resource_round_trips_through_json(r in arb_resource()) {
        let json = serde_json::to_string(&r).unwrap();
        let back: Resource = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(r, back);
    }

    #[test]
    fn document_round_trips_through_json(
        primary in arb_resource(),
        included in prop::collection::vec(arb_resource(), 0..3),
    ) {
        let doc: Document<Resource> = Document::Data {
            data: PrimaryData::Single(Box::new(primary)),
            included,
            meta: None,
            jsonapi: None,
            links: None,
        };
        let json = serde_json::to_string(&doc).unwrap();
        let back: Document<Resource> = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(doc, back);
    }
}
