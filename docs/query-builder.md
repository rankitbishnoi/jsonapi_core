# The Query Builder

[`QueryBuilder`] produces JSON:API-compliant query strings with correct bracket
encoding for `filter[]`, `fields[]`, and `page[]` parameters and RFC 3986
percent-encoding for values.

## Basic usage

```rust
use jsonapi_core::QueryBuilder;

let qs = QueryBuilder::new()
    .filter("published", "true")
    .sort(&["-created", "title"])
    .include(&["author", "comments"])
    .fields("articles", &["title", "body"])
    .page("number", "1")
    .page("size", "25")
    .build();

println!("?{qs}");
// ?filter[published]=true&sort=-created,title&include=author,comments&fields[articles]=title,body&page[number]=1&page[size]=25
```

`build()` returns a `String` **without** a leading `?`. Prepend it yourself if
you're concatenating onto a base URL.

## Methods

| Method | Output shape |
|--------|--------------|
| `.filter(key, value)` | `filter[key]=value` |
| `.sort(&["a", "-b"])` | `sort=a,-b` |
| `.include(&["a", "b.c"])` | `include=a,b.c` |
| `.fields(type, &[...])` | `fields[type]=a,b` |
| `.page(key, value)` | `page[key]=value` |
| `.param(key, value)` | `key=value` (escape hatch — no bracket wrapping) |

Each method consumes `self` and returns `Self` so you can chain.

## Encoding rules

The builder applies RFC 3986 percent-encoding so that any character outside
`A-Z a-z 0-9 - _ . ~` is escaped. **Commas** are special: they're preserved
as delimiters inside `sort`, `include`, and `fields` values, but encoded
inside `filter`, `page`, and `param` values.

```rust
use jsonapi_core::QueryBuilder;

// Spaces are encoded:
let qs = QueryBuilder::new().filter("search", "hello world").build();
assert!(qs.contains("filter[search]=hello%20world"));

// Commas in `sort` are preserved as delimiters:
let qs = QueryBuilder::new().sort(&["-created", "title"]).build();
assert!(qs.contains("sort=-created,title"));

// Commas in `filter` values are encoded:
let qs = QueryBuilder::new().filter("tags", "rust,api").build();
assert!(qs.contains("filter[tags]=rust%2Capi"));
```

## A compound example

The query equivalent of *"give me published articles, sorted newest first, with
their authors and comments included, only the title/body fields, paginated by
25":*

```rust
use jsonapi_core::QueryBuilder;

let qs = QueryBuilder::new()
    .filter("published", "true")
    .sort(&["-created"])
    .include(&["author", "comments"])
    .fields("articles", &["title", "body"])
    .fields("people", &["name"])
    .page("number", "1")
    .page("size", "25")
    .build();
```

## Custom parameters

For non-standard parameters that aren't in the JSON:API spec (API keys,
implementation-specific filters), use `param`:

```rust
let qs = QueryBuilder::new()
    .param("api_key", "abc123")
    .include(&["author"])
    .build();
// api_key=abc123&include=author
```

The key is passed through as-is — escape it yourself if it could contain
problematic characters.

## A note on parameter ordering

`build()` preserves insertion order. JSON:API doesn't require any specific
ordering, but stable output makes log-grepping and snapshot tests easier.

## Pairing the builder with content negotiation

A typical JSON:API client builds the URL with `QueryBuilder` and then sets
the `Accept` header per [Content Negotiation](./content-negotiation.md):

```rust
use jsonapi_core::{JsonApiMediaType, QueryBuilder};

let url = format!(
    "https://api.example.com/articles?{}",
    QueryBuilder::new().include(&["author"]).build(),
);
let accept = JsonApiMediaType::plain().to_header_value();
// → "application/vnd.api+json"
```
