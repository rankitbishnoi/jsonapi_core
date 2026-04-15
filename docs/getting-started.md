# Your First Resource

This chapter walks through the smallest end-to-end use of `jsonapi_core`: define a
resource, serialize it, parse it back, and look up a related resource.

## Define a resource

A JSON:API resource is a Rust struct with `#[derive(JsonApi)]`. Mark the id field with
`#[jsonapi(id)]` and declare the resource's wire `type` on the struct.

```rust
use jsonapi_core::{JsonApi, Relationship};

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
```

Unannotated fields like `title` and `body` are serialized as JSON:API
**attributes**. Fields tagged `#[jsonapi(relationship, type = "...")]` are serialized
as **relationships**. Both rules are detailed in the
[Derive Macro Reference](./derive-macro-reference.md).

## Serialize a resource

Wrap the struct in a `Document` and let serde do the rest. The derive macro produces
the JSON:API envelope (`type`, `id`, `attributes`, `relationships`).

```rust
use jsonapi_core::{
    Document, Identity, PrimaryData, Relationship, RelationshipData, ResourceIdentifier,
};

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

let doc: Document<Article> = Document::Data {
    data: PrimaryData::Single(Box::new(article)),
    included: vec![],
    meta: None,
    jsonapi: None,
    links: None,
};

let json = serde_json::to_string_pretty(&doc).unwrap();
println!("{json}");
```

The output looks like:

```json
{
  "data": {
    "type": "articles",
    "id": "1",
    "attributes": {
      "title": "JSON:API paints my bikeshed!",
      "body": "The shortest article. Ever."
    },
    "relationships": {
      "author": {
        "data": { "type": "people", "id": "9" }
      }
    }
  }
}
```

## Deserialize a response

Parse a server response with `Document<Resource>`. `Resource` is the dynamic fallback
that handles **mixed types** in the `included` array — something a single typed
`Document<T>` cannot do because the array would have to be uniformly `T`.

```rust
use jsonapi_core::{Document, PrimaryData, Resource, ResourceObject};

let json = r#"{
    "data": {
        "type": "articles", "id": "1",
        "attributes": {"title": "Hello JSON:API"},
        "relationships": {
            "author": {"data": {"type": "people", "id": "9"}}
        }
    },
    "included": [{
        "type": "people", "id": "9",
        "attributes": {"name": "Dan Gebhardt"}
    }]
}"#;

let doc: Document<Resource> = serde_json::from_str(json).unwrap();
```

## Look up an included resource

`Document::registry()` builds a typed lookup table from the `included` array. Use
`get_by_id` to deserialize a stored resource into a concrete struct.

```rust
let registry = doc.registry().unwrap();

// Typed lookup — deserializes the stored Value into a Person
let author: Person = registry.get_by_id("people", "9").unwrap();
assert_eq!(author.name, "Dan Gebhardt");
```

The registry is a typed bridge: the `included` array is stored as raw
`serde_json::Value`s, and `get_by_id::<Person>` deserializes the matching entry on
demand.

## Where next

- For a deeper look at `Document`, `Resource`, and `PrimaryData`, see
  [Documents and Resources](./documents-and-resources.md).
- For the full set of derive attributes (`case`, `meta`, `links`, `lid`, `skip`,
  `rename`), see [Defining Resources](./defining-resources.md).
- For ad-hoc lookups, recursive resolution, and cycle detection, see the
  [Registry and Resolver](./registry-and-resolver.md) chapter.
