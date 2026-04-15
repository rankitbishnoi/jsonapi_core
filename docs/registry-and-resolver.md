# The Registry and Resolver

The [`Registry`] is the answer to the question: *"a JSON:API document arrived
with twenty resources in `included` — how do I get the one I actually want?"*

It's a lookup table populated from a deserialized `Document`'s `included` array,
keyed by `(type, id)`. It supports four read patterns:

- **`get(&Relationship<T>)`** — type-safe lookup driven by the relationship value.
- **`get_many(&Relationship<T>)`** — same, for to-many relationships.
- **`get_by_id(type, id)`** — explicit lookup by the wire identifiers.
- **`get_all(type)`** — every entry of a given type, deserialized as `T`.

Plus one recursive operation:

- **`resolve(value, &ResolveConfig)`** — kitsu-core-style flattened output with
  cycle detection.

## Building a registry

In the common case, you get a registry from `Document::registry()`:

```rust
use jsonapi_core::{Document, Resource};

let doc: Document<Resource> = serde_json::from_str(json)?;
let registry = doc.registry()?;
```

`registry()` returns `Result<Registry>` because building the registry involves
re-serializing each `included` resource through serde. For `Document::Errors`
and `Document::Meta`, an empty registry is returned.

You can also build a registry directly from any slice that implements
`ResourceObject`:

```rust
use jsonapi_core::Registry;

let included: Vec<Resource> = /* ... */;
let registry = Registry::from_included(&included)?;
```

## Typed lookups by relationship

If you have a derived struct, the relationship field carries the target type as a
phantom — so `get` infers the return type:

```rust
let author: Person = registry.get(&article.author)?;
```

For a to-many relationship:

```rust
let comments: Vec<Comment> = registry.get_many(&article.comments_field)?;
```

`get` and `get_many` both error with:

- `Error::RegistryLookup { type_, id }` — no entry for that identifier
- `Error::NullRelationship` — relationship was `ToOne(None)`
- `Error::RelationshipCardinalityMismatch { expected }` — `get` called on
  to-many or `get_many` called on to-one
- `Error::LidNotIndexed` — relationship references an `lid`, but the registry
  is keyed by server-assigned `id` only

## Lookups by explicit type and id

When you don't have a typed `Relationship<T>` (e.g. you're working with the
dynamic `Resource`):

```rust
let author: Person = registry.get_by_id("people", "9")?;
```

The turbofish form is also handy when the type isn't inferable:

```rust
let author = registry.get_by_id::<Person>("people", "9")?;
```

## Bulk fetches

`get_all` returns every resource in the registry of a given type that
deserializes as `T`. Resources of the same wire type that fail to deserialize
as `T` are silently omitted:

```rust
let everyone: Vec<Person> = registry.get_all("people");
```

This is intentional. The registry stores values as `serde_json::Value` and
can hold the same wire type with different attribute shapes; `get_all` filters
to the ones that match `T`.

## The recursive resolver

`Registry::resolve` produces a flattened, kitsu-core-style representation of a
resource: attributes are hoisted to the top level, relationships are replaced
with the resolved related resource (recursively), and the JSON:API envelope is
stripped.

```rust
use jsonapi_core::{Document, ResolveConfig, Resource};

let doc: Document<Resource> = serde_json::from_str(json)?;
let registry = doc.registry()?;
let value: serde_json::Value = serde_json::to_value(&doc)?;
let data = &value["data"];

let flat = registry.resolve(data, &ResolveConfig::default());
assert_eq!(flat["title"], "Hello");
assert_eq!(flat["author"]["name"], "Dan");
```

Behaviour to be aware of:

- **Cycles** are detected and broken — when a back-reference would re-enter
  an ancestor, the inner copy is left as a bare `{type, id}` identifier.
- **Missing resources** (referenced from `relationships` but absent from
  `included`) are left as bare identifiers.
- **Depth limit**: `ResolveConfig::max_depth` defaults to `10`. Beyond that,
  relationships stop expanding.
- Resource-level `meta` and `links` are preserved on the flattened output;
  relationship-level `meta`/`links` are dropped.

The output is a `serde_json::Value` — useful for handing to a UI layer that
prefers a single tree over JSON:API's normalized form.

## A complete example

```rust
use jsonapi_core::{Document, ResolveConfig, Resource};

let json = r#"{
    "data": {
        "type": "articles", "id": "1",
        "attributes": {"title": "Hello"},
        "relationships": {
            "author": {"data": {"type": "people", "id": "9"}}
        }
    },
    "included": [{
        "type": "people", "id": "9",
        "attributes": {"name": "Dan"}
    }]
}"#;

let doc: Document<Resource> = serde_json::from_str(json).unwrap();
let registry = doc.registry().unwrap();

// (a) Typed lookup:
#[derive(serde::Deserialize)]
struct Person { name: String }

let person: Person = registry.get_by_id("people", "9").unwrap();
assert_eq!(person.name, "Dan");

// (b) Recursive resolve:
let value: serde_json::Value = serde_json::to_value(&doc).unwrap();
let flat = registry.resolve(&value["data"], &ResolveConfig::default());
println!("{}", serde_json::to_string_pretty(&flat).unwrap());
```
