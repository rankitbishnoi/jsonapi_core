# Relationships and Identifiers

JSON:API expresses relationships as **resource identifiers** — small `{type, id}`
objects that point at the related resource by reference. The full related
resource may be inlined in the document's `included` array, but it doesn't have
to be.

`jsonapi_core` models this with three layered types:

| Type | What it is |
|------|------------|
| [`Identity`] | The id half of an identifier — either a server-assigned `Id(String)` or a JSON:API 1.1 client-local `Lid(String)`. |
| [`ResourceIdentifier`] | A `{type, id}` (or `{type, lid}`) object with optional `meta`. |
| [`RelationshipData`] | The wire shape of a relationship's `data` member: `null`, a single identifier, or an array. |
| [`Relationship<T>`] | The typed wrapper used in derived structs — carries `data`, optional `links`/`meta`, and a phantom target type for type-safe registry lookups. |

## `Identity`

```rust
pub enum Identity {
    Id(String),
    Lid(String),
}
```

`Id` is the server-assigned identifier — what you'll see in 99% of responses.
`Lid` is the client-generated local identifier introduced in 1.1 for
create-time cross-references (see [Atomic Operations](./atomic-operations.md)).

`ResourceIdentifier`'s `Deserialize` impl rejects identifiers that have **both**
`id` and `lid` (per spec) and identifiers that have **neither**.

## `ResourceIdentifier`

```rust
pub struct ResourceIdentifier {
    pub type_: String,
    pub identity: Identity,
    pub meta: Option<Meta>,
}
```

Field name `type_` (with trailing underscore) avoids the Rust keyword; on the wire
this is `"type"`. The custom `Serialize` impl handles the rename.

## `RelationshipData`

```rust
pub enum RelationshipData {
    ToOne(Option<ResourceIdentifier>),  // null or one
    ToMany(Vec<ResourceIdentifier>),    // possibly empty array
}
```

Note the asymmetry: a to-one relationship's data may be `null` (no value), whereas
a to-many's data is always an array (which may be empty).

## `Relationship<T>`

This is what derived structs use for their relationship fields:

```rust
pub struct Relationship<T> {
    pub data: RelationshipData,
    pub links: Option<Links>,
    pub meta: Option<Meta>,
    /* phantom target type */
}
```

The phantom `T` lets you call typed lookups like `registry.get::<Person>(&rel)`
without having to repeat the type at the call site. Construct one with
`Relationship::new(...)`:

```rust
use jsonapi_core::{Identity, Relationship, RelationshipData, ResourceIdentifier};

let author: Relationship<Person> = Relationship::new(
    RelationshipData::ToOne(Some(ResourceIdentifier {
        type_: "people".into(),
        identity: Identity::Id("9".into()),
        meta: None,
    }))
);
```

## To-one vs to-many at the field level

The macro looks at the field type to figure out cardinality:

| Field type | Cardinality |
|------------|-------------|
| `Relationship<T>` | to-one |
| `Vec<Relationship<T>>` | to-many |

A `Vec<Relationship<T>>` collapses on the wire into a single
`{ "data": [ ...identifiers... ] }` block — each `Relationship<T>` in the `Vec`
contributes one identifier. (`links` and `meta` from individual elements are
not preserved when collapsed; use the dynamic `Resource` flow if you need to
preserve them.)

## Looking up the related value

Once you have a deserialized document, the [`Registry`] resolves identifiers
into typed values:

```rust
use jsonapi_core::{Document, PrimaryData, Resource};

let doc: Document<Resource> = serde_json::from_str(json)?;
let registry = doc.registry()?;

// Get the article (dynamic)
if let Document::Data { data: PrimaryData::Single(article), .. } = &doc {
    // The article's relationships are a BTreeMap<String, RelationshipData>
    let author_data = &article.relationships["author"];
    if let RelationshipData::ToOne(Some(rid)) = author_data {
        // Typed lookup by explicit type/id pair
        let author: Person = registry.get_by_id(&rid.type_, match &rid.identity {
            Identity::Id(s) => s,
            Identity::Lid(_) => panic!("unexpected lid in response"),
        })?;
    }
}
```

In the **typed** path (where the article is a `Document<Article>`), the article's
`author` field is already a `Relationship<Person>`, so `registry.get(&article.author)`
works directly:

```rust
let person: Person = registry.get(&article.author)?;
```

See [The Registry and Resolver](./registry-and-resolver.md) for the full set
of registry operations, including `get_many` (to-many), `get_all` (every
included resource of a type), and `resolve` (recursive flattening).
