# Documents and Resources

Every JSON:API exchange — request or response — is a **document**. `jsonapi_core`
models the document with the [`Document<T>`] enum and the resource(s) inside it
with the [`PrimaryData<T>`] enum.

## The `Document` enum

A JSON:API document is exactly one of three shapes, and `Document<T>` reflects that
with three variants:

```rust
pub enum Document<T> {
    Data {
        data: PrimaryData<T>,
        included: Vec<T>,
        meta: Option<Meta>,
        jsonapi: Option<JsonApiObject>,
        links: Option<Links>,
    },
    Errors {
        errors: Vec<ApiError>,
        meta: Option<Meta>,
        jsonapi: Option<JsonApiObject>,
        links: Option<Links>,
    },
    Meta {
        meta: Meta,
        jsonapi: Option<JsonApiObject>,
        links: Option<Links>,
    },
}
```

`data` and `errors` are spec-mutually-exclusive — `Document` enforces that at the
type level. The `Meta` variant exists for documents that carry only top-level
meta (e.g. discovery endpoints).

## The `PrimaryData` enum

The `data` member of a JSON:API response can be `null`, a single resource, or an
array. `PrimaryData<T>` models all three:

```rust
pub enum PrimaryData<T> {
    Null,
    Single(Box<T>),
    Many(Vec<T>),
}
```

`Single` is boxed because, in practice, you frequently store collection responses
elsewhere and a `Box<T>` keeps the enum small.

## The two flavours of `T`

`Document<T>` is generic. There are two natural choices for `T`:

| Choice | When to use |
|--------|-------------|
| A struct deriving `JsonApi` (e.g. `Document<Article>`) | The shape is known at compile time and all `included` items share that shape. |
| `Document<Resource>` | You don't know the shape, or `included` mixes types. |

In practice, a typical client uses `Document<Resource>` for response parsing
(because `included` is heterogeneous) and `Document<MyStruct>` for serialization
(because the request body is uniform).

## The `Resource` fallback

`Resource` is the dynamic counterpart to a derived struct. It stores attributes as
a `serde_json::Value` and relationships as a `BTreeMap`:

```rust
pub struct Resource {
    pub type_: String,
    pub id: Option<String>,
    pub lid: Option<String>,
    pub attributes: serde_json::Value,
    pub relationships: BTreeMap<String, RelationshipData>,
    pub links: Option<Links>,
    pub meta: Option<Meta>,
}
```

`Resource` implements `ResourceObject` (the trait `Document<T>` requires), so you
can serialize and deserialize it just like a derived struct:

```rust
use jsonapi_core::{Document, PrimaryData, Resource, ResourceObject};

let json = r#"{"data": {"type": "widgets", "id": "42", "attributes": {"color": "red"}}}"#;
let doc: Document<Resource> = serde_json::from_str(json).unwrap();

if let Document::Data { data: PrimaryData::Single(widget), .. } = &doc {
    assert_eq!(widget.resource_type(), "widgets");
    assert_eq!(widget.attributes["color"], "red");
}
```

## The `ResourceObject` trait

Every typed resource implements `ResourceObject`:

```rust
pub trait ResourceObject: Serialize + for<'de> Deserialize<'de> {
    fn resource_type(&self) -> &str;
    fn resource_id(&self) -> Option<&str>;
    fn resource_lid(&self) -> Option<&str> { None }
    fn field_names() -> &'static [&'static str];
    fn type_info() -> TypeInfo where Self: Sized { /* default panics */ }
}
```

The derive macro generates this impl for you. A hand-written impl is supported and
described in the [Derive Macro Reference](./derive-macro-reference.md).

## Building a successful document

```rust
use jsonapi_core::{Document, PrimaryData};

let doc: Document<MyType> = Document::Data {
    data: PrimaryData::Single(Box::new(my_value)),
    included: vec![],
    meta: None,
    jsonapi: None,
    links: None,
};
```

## Building an error document

```rust
use jsonapi_core::{ApiError, Document, ErrorSource, Resource};

let doc: Document<Resource> = Document::Errors {
    errors: vec![ApiError {
        status: Some("422".into()),
        title: Some("Validation failed".into()),
        detail: Some("`title` is required".into()),
        source: Some(ErrorSource {
            pointer: Some("/data/attributes/title".into()),
            ..Default::default()
        }),
        ..Default::default()
    }],
    meta: None,
    jsonapi: None,
    links: None,
};
```

`ApiError` covers every member from the spec's error object (`id`, `links`,
`status`, `code`, `title`, `detail`, `source`, `meta`) and is `Default`, so you
only have to populate the fields you care about.

## Top-level members

The `meta`, `jsonapi`, and `links` fields on `Document` map to the spec's
top-level members of the same name:

- **`meta`** is `serde_json::Map<String, serde_json::Value>` — arbitrary
  application-specific data.
- **`jsonapi`** is a `JsonApiObject` describing the server's implementation
  (`version`, `ext`, `profile`, `meta`).
- **`links`** is a `Links` map; values are nullable `Link`s, where each `Link`
  is either a bare URL string or a richer `LinkObject` (with `rel`, `title`,
  `hreflang`, `meta`, etc.).

## Cross-references

- The dynamic `included` array is what makes `Document<Resource>` so useful for
  parsing — see [The Registry and Resolver](./registry-and-resolver.md) for how
  to look things up inside it.
- `Document<T>` only round-trips cleanly when `T: ResourceObject + DeserializeOwned`.
  The derive macro provides both impls.
