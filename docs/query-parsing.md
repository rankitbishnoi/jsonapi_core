# Query Parsing

`Query` parses JSON:API request query parameters on the server — the inverse of
`QueryBuilder`.

```rust
use jsonapi_core::Query;

let q = Query::from_query_string(
    "?include=author,comments&sort=-created,title&page[size]=10&filter[status]=published",
)?;

assert_eq!(q.include, ["author", "comments"]);
assert!(q.sort[0].descending);          // "-created"
assert_eq!(q.sort[0].field, "created");
assert_eq!(q.page.get("size").map(String::as_str), Some("10"));
assert_eq!(q.filter.get("status").unwrap(), &vec!["published".to_string()]);
# Ok::<(), jsonapi_core::Error>(())
```

## Inputs

- `Query::from_pairs(&[(&str, &str)])` — for framework param maps (axum/actix), which give
  you already-decoded key/value pairs.
- `Query::from_query_string("?a=b&c=d")` — percent-decodes and splits a raw query string.

## What is parsed

| Parameter | Field | Notes |
|---|---|---|
| `sort` | `Vec<SortField>` | comma-separated; leading `-` = descending |
| `include` | `Vec<String>` | comma-separated dot-paths |
| `fields[type]` | `FieldsetConfig` | reuses the sparse-fieldset type |
| `page[...]` | `BTreeMap<String, String>` | generic — no strategy assumed |
| `filter[...]` | `BTreeMap<String, Vec<String>>` | generic; repeated keys accumulate |

Filter and page semantics are intentionally left to your application — JSON:API does not
define them. Unknown top-level parameters are ignored. Repeated `filter[k]` keys accumulate
into a `Vec`; repeated `page[k]` keys are last-wins.

## Validating include paths

Parsing does not validate include paths. Compose with `TypeRegistry`:

```rust
# use jsonapi_core::{Query, TypeRegistry, TypeInfo};
let q = Query::from_pairs(&[("include", "author")])?;
let mut reg = TypeRegistry::new();
reg.register_info(TypeInfo::new("articles", &["title", "author"], &[("author", "people")]));
let paths: Vec<&str> = q.include.iter().map(String::as_str).collect();
reg.validate_include_paths("articles", &paths)?;
# Ok::<(), jsonapi_core::Error>(())
```

A malformed parameter (e.g. an empty `sort` token, or `filter[tag` without a closing
bracket) returns `Error::QueryParse`.
