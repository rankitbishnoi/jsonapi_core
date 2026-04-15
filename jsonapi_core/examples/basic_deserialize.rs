//! Deserialize a JSON:API response and use the Registry for relationship lookups.
//!
//! Run: `cargo run -p jsonapi_core --example basic_deserialize`

use jsonapi_core::{Document, JsonApi, PrimaryData, Relationship, Resource, ResourceObject};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
    #[jsonapi(relationship, type = "comments")]
    comments: Vec<Relationship<Comment>>,
}

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
}

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "comments")]
struct Comment {
    #[jsonapi(id)]
    id: String,
    body: String,
}

fn main() {
    let json = r#"{
        "data": {
            "type": "articles",
            "id": "1",
            "attributes": {
                "title": "JSON:API paints my bikeshed!",
                "body": "The shortest article. Ever."
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
            }
        },
        "included": [
            {
                "type": "people",
                "id": "9",
                "attributes": {"name": "Dan Gebhardt"}
            },
            {
                "type": "comments",
                "id": "5",
                "attributes": {"body": "First!"}
            },
            {
                "type": "comments",
                "id": "12",
                "attributes": {"body": "I like XML better"}
            }
        ]
    }"#;

    // Deserialize as Document<Resource> to handle mixed included types
    let doc: Document<Resource> = serde_json::from_str(json).unwrap();
    let registry = doc.registry().unwrap();

    if let Document::Data {
        data: PrimaryData::Single(article),
        ..
    } = &doc
    {
        println!(
            "Article: {} (id={})",
            article.attributes["title"].as_str().unwrap(),
            article.resource_id().unwrap()
        );

        // Typed lookup — deserializes stored Value into Person
        let author: Person = registry.get_by_id("people", "9").unwrap();
        println!("Author: {}", author.name);

        // Look up each comment
        let comment5: Comment = registry.get_by_id("comments", "5").unwrap();
        let comment12: Comment = registry.get_by_id("comments", "12").unwrap();
        println!("Comment 5: {}", comment5.body);
        println!("Comment 12: {}", comment12.body);
    }
}
