//! Use Document<Resource> for open-set handling and the recursive resolver.
//!
//! Run: `cargo run -p jsonapi_core --example dynamic_resource`

use jsonapi_core::{Document, PrimaryData, ResolveConfig, Resource, ResourceObject};

fn main() {
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
                }
            }
        },
        "included": [
            {
                "type": "people",
                "id": "9",
                "attributes": {
                    "first-name": "Dan",
                    "last-name": "Gebhardt"
                }
            }
        ]
    }"#;

    // Parse without knowing the schema at compile time
    let doc: Document<Resource> = serde_json::from_str(json).unwrap();
    let registry = doc.registry().unwrap();

    if let Document::Data {
        data: PrimaryData::Single(article),
        ..
    } = &doc
    {
        println!("Type: {}", article.resource_type());
        println!("Title: {}", article.attributes["title"]);
    }

    // Resolve into flattened output (kitsu-core style)
    let value: serde_json::Value = serde_json::to_value(&doc).unwrap();
    let flat = registry.resolve(&value["data"], &ResolveConfig::default());
    println!("\nFlattened:");
    println!("{}", serde_json::to_string_pretty(&flat).unwrap());
}
