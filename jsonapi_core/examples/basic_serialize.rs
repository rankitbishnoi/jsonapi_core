//! Serialize a typed struct to a JSON:API document.
//!
//! Run: `cargo run -p jsonapi_core --example basic_serialize`

use jsonapi_core::{
    Document, Identity, JsonApi, PrimaryData, Relationship, RelationshipData, ResourceIdentifier,
};

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
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

fn main() {
    // Build a Person resource (will go in `included`)
    let _person = Person {
        id: "9".into(),
        name: "Dan Gebhardt".into(),
    };

    // Build an Article with a to-one relationship to the person
    let article = Article {
        id: "1".into(),
        title: "JSON:API paints my bikeshed!".into(),
        body: "The shortest article. Ever.".into(),
        author: Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            type_: "people".into(),
            identity: Identity::Id("9".into()),
            meta: None,
        }))),
    };

    // Wrap in a Document
    let doc: Document<Article> = Document::Data {
        data: PrimaryData::Single(Box::new(article)),
        included: vec![], // Person would go in included with a heterogeneous type enum
        meta: None,
        jsonapi: None,
        links: None,
    };

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&doc).unwrap();
    println!("{json}");
}
