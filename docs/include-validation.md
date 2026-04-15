# Include Path Validation

JSON:API clients ask for compound documents via the `include` query parameter:
`?include=author,comments,comments.author`. Each path is a dot-separated chain
of relationship names. A server typically wants to reject paths that don't
exist on the requested type **before** doing any database work.

`TypeRegistry` is the static type-graph that `jsonapi_core` uses for that
validation.

## What's in the registry

`TypeInfo` holds compile-time metadata about a resource type:

```rust
pub struct TypeInfo {
    pub type_name: &'static str,
    pub field_names: &'static [&'static str],
    pub relationships: &'static [(&'static str, &'static str)],
}
```

The derive macro generates a `TypeInfo` for every type it sees. The
`relationships` array pairs each relationship name with its **target** type
string — that's what makes graph traversal possible.

## Registering types

Build a `TypeRegistry` once at startup, register every type you want to validate
against, and use it for the lifetime of the application:

```rust
use jsonapi_core::TypeRegistry;

let mut registry = TypeRegistry::new();
registry
    .register::<Article>()
    .register::<Person>()
    .register::<Comment>();
```

`register::<T>` calls `T::type_info()` (the derive-generated impl) and stores it
under `type_info().type_name`. You can also register a `TypeInfo` directly:

```rust
registry.register_info(jsonapi_core::TypeInfo::new(
    "tags",
    &["name", "color"],
    &[],
));
```

## Validating include paths

```rust
let result = registry.validate_include_paths(
    "articles",
    &["author", "comments.author", "comments.author.organization"],
);
```

The walker resolves each path one segment at a time, hopping through the
relationships graph:

- `author` on `articles` → exists, target is `people` → done.
- `comments.author` on `articles` → `comments` exists with target `comments` →
  `author` exists on `comments` with target `people` → done.
- `comments.author.organization` → if `organization` is not a relationship on
  `people`, validation fails with `Error::InvalidIncludePath { path, segment, type_name }`.

Empty paths are skipped. Terminal segments don't require their target type to be
registered — only intermediate hops do. So the pattern of registering the
"top-level" types and not bothering with leaf-only types works fine.

## A complete example

```rust
use jsonapi_core::{JsonApi, Relationship, TypeRegistry};

#[derive(JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
    #[jsonapi(relationship, type = "comments")]
    comments: Vec<Relationship<Comment>>,
}

#[derive(JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
}

#[derive(JsonApi)]
#[jsonapi(type = "comments")]
struct Comment {
    #[jsonapi(id)]
    id: String,
    body: String,
    #[jsonapi(relationship, type = "people")]
    author: Relationship<Person>,
}

let mut registry = TypeRegistry::new();
registry
    .register::<Article>()
    .register::<Person>()
    .register::<Comment>();

assert!(registry.validate_include_paths("articles", &["author"]).is_ok());
assert!(registry.validate_include_paths("articles", &["comments.author"]).is_ok());

// Not a relationship on articles → error
assert!(registry.validate_include_paths("articles", &["editor"]).is_err());
```

## Reading the error

```rust
match registry.validate_include_paths("articles", &["author.posts"]) {
    Ok(()) => { /* proceed */ }
    Err(jsonapi_core::Error::InvalidIncludePath { path, segment, type_name }) => {
        // 400 Bad Request, body:
        //   "invalid include path 'author.posts': relationship 'posts' not found on type 'people'"
        eprintln!("invalid include path '{path}': '{segment}' not found on '{type_name}'");
    }
    Err(e) => eprintln!("{e}"),
}
```

## When the type isn't registered

If you call `validate_include_paths` for a `root_type` that hasn't been
registered, the first path returns
`Error::InvalidIncludePath { type_name: <root>, segment: <first> }`. Register
the root type before validating against it.
