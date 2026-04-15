//! Atomic Operations extension — build a mixed-type payload, round-trip it,
//! validate lid references.
//!
//! Run with: `cargo run --example atomic_operations --features atomic-ops`

#[cfg(not(feature = "atomic-ops"))]
fn main() {
    eprintln!("This example requires the `atomic-ops` feature.");
    eprintln!("Run with: cargo run --example atomic_operations --features atomic-ops");
    std::process::exit(1);
}

#[cfg(feature = "atomic-ops")]
fn main() {
    use std::collections::BTreeMap;

    use jsonapi_core::{
        Identity, JsonApiMediaType, PrimaryData, Resource,
        atomic::{ATOMIC_EXT_URI, AtomicOperation, AtomicRequest, OperationRef, OperationTarget},
    };

    // Build three ops: add person (lid "p1"), add article (lid "a1"), then
    // update the article's `author` relationship — demonstrating lid refs.
    let person = Resource {
        type_: "people".into(),
        id: None,
        lid: Some("p1".into()),
        attributes: serde_json::json!({"name": "Dan Gebhardt"}),
        relationships: BTreeMap::new(),
        links: None,
        meta: None,
    };

    let mut article_rels = BTreeMap::new();
    article_rels.insert(
        "author".into(),
        jsonapi_core::RelationshipData::ToOne(Some(jsonapi_core::ResourceIdentifier {
            type_: "people".into(),
            identity: Identity::Lid("p1".into()),
            meta: None,
        })),
    );
    let article = Resource {
        type_: "articles".into(),
        id: None,
        lid: Some("a1".into()),
        attributes: serde_json::json!({"title": "Hello JSON:API"}),
        relationships: article_rels,
        links: None,
        meta: None,
    };

    // Op 1: add the person (introduces lid "p1").
    // Op 2: add the article with default target (introduces lid "a1").
    // Op 3: update the article's author relationship — ref.lid "a1" introduced above.
    let req = AtomicRequest {
        operations: vec![
            AtomicOperation::Add {
                target: OperationTarget::default(),
                data: PrimaryData::Single(Box::new(person)),
            },
            AtomicOperation::Add {
                target: OperationTarget::default(),
                data: PrimaryData::Single(Box::new(article)),
            },
            AtomicOperation::Update {
                target: OperationTarget {
                    r#ref: Some(OperationRef {
                        type_: "articles".into(),
                        identity: Identity::Lid("a1".into()),
                        relationship: Some("author".into()),
                    }),
                    href: None,
                },
                data: PrimaryData::Single(Box::new(Resource {
                    type_: "people".into(),
                    id: None,
                    lid: Some("p1".into()),
                    attributes: serde_json::json!({}),
                    relationships: BTreeMap::new(),
                    links: None,
                    meta: None,
                })),
            },
        ],
    };

    // Pre-flight validation.
    req.validate_lid_refs().expect("lid refs should validate");

    // Serialize.
    let body = serde_json::to_string_pretty(&req).unwrap();
    println!("Request body:\n{body}\n");

    // Build the Content-Type header.
    let mt = JsonApiMediaType::with_ext([ATOMIC_EXT_URI]);
    println!("Content-Type: {}", mt.to_header_value());

    // Round-trip.
    let parsed: AtomicRequest = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed, req);
    println!("\nRound-trip OK ({} operations)", parsed.operations.len());
}
