# Error Handling

`jsonapi_core` uses a single `Error` enum (`jsonapi_core::Error`) for every
fallible operation in the crate, plus a `Result<T>` alias for
`std::result::Result<T, Error>`.

There are **two distinct kinds** of error to keep separate in your head:

1. **Crate-level errors** (`jsonapi_core::Error`) — programmer/protocol
   violations: a malformed media type, a missing registry entry, a media-type
   mismatch.
2. **Application-level errors** (`jsonapi_core::ApiError`) — the JSON:API
   "error object" you put inside `Document::Errors` to send to a client.

This chapter covers both.

## `jsonapi_core::Error`

```rust
#[non_exhaustive]
pub enum Error {
    Json(serde_json::Error),
    InvalidMemberName { name: String, reason: String },
    RegistryLookup { type_: String, id: String },
    NullRelationship,
    RelationshipCardinalityMismatch { expected: &'static str },
    LidNotIndexed,
    MediaTypeMismatch { expected: String, got: String },
    UnsupportedMediaTypeParam { param: String },
    MediaTypeParse(String),
    NoAcceptableMediaType,
    AllMediaTypesUnsupportedParams,
    Structure(String),
    InvalidIncludePath { path: String, segment: String, type_name: String },
    InvalidAtomicOperation { index: usize, reason: String },
}
```

It implements `std::error::Error` (via `thiserror`) and `From<serde_json::Error>`,
so the `?` operator works when interleaving with serde calls. It is
`#[non_exhaustive]` — when matching, include a wildcard arm.

### When each variant fires

| Variant | Comes from |
|---------|------------|
| `Json` | Any failure in the underlying serde_json calls. |
| `InvalidMemberName` | `validate_member_name` and the derive macro's compile-time check (where it's emitted as a compile error, not a runtime `Error`). |
| `RegistryLookup` | `Registry::get*` couldn't find a `(type, id)`. |
| `NullRelationship` | `Registry::get` was called on a `ToOne(None)`. |
| `RelationshipCardinalityMismatch` | `get` on to-many or `get_many` on to-one. |
| `LidNotIndexed` | Relationship references an `lid`; the registry only indexes `id`. |
| `MediaTypeMismatch` | Header base type isn't `application/vnd.api+json`. |
| `UnsupportedMediaTypeParam` | `validate_content_type` saw a non-`ext`/non-`profile` parameter. |
| `MediaTypeParse` | Syntactically broken media-type header. |
| `NoAcceptableMediaType` | `negotiate_accept` found nothing JSON:API in the Accept header. |
| `AllMediaTypesUnsupportedParams` | All JSON:API entries in Accept had unsupported params (406). |
| `Structure` | Document structure rule violated (e.g. data + errors both present). |
| `InvalidIncludePath` | `TypeRegistry::validate_include_paths` couldn't resolve a hop. |
| `InvalidAtomicOperation` | `AtomicRequest::validate_lid_refs` failed (Atomic Ops extension). |

### Mapping to HTTP status codes

A natural translation table for a handler:

| Error variant | Suggested HTTP status |
|---------------|-----------------------|
| `MediaTypeMismatch`, `UnsupportedMediaTypeParam` | 415 Unsupported Media Type |
| `NoAcceptableMediaType`, `AllMediaTypesUnsupportedParams` | 406 Not Acceptable |
| `Structure`, `InvalidMemberName`, `InvalidIncludePath`, `InvalidAtomicOperation` | 400 Bad Request |
| `Json` (during request parsing) | 400 Bad Request |
| `RegistryLookup`, `NullRelationship`, `RelationshipCardinalityMismatch`, `LidNotIndexed` | Internal — these reflect *your* code's assumptions about the document |

## `ApiError` — the wire error object

`ApiError` mirrors the spec's [error object](https://jsonapi.org/format/#error-objects):

```rust
pub struct ApiError {
    pub id: Option<String>,
    pub links: Option<ErrorLinks>,
    pub status: Option<String>,
    pub code: Option<String>,
    pub title: Option<String>,
    pub detail: Option<String>,
    pub source: Option<ErrorSource>,
    pub meta: Option<Meta>,
}
```

It derives `Default`, so you typically build one with `..Default::default()`:

```rust
use jsonapi_core::{ApiError, ErrorSource};

let err = ApiError {
    status: Some("422".into()),
    title: Some("Validation failed".into()),
    detail: Some("`title` is required".into()),
    source: Some(ErrorSource {
        pointer: Some("/data/attributes/title".into()),
        ..Default::default()
    }),
    ..Default::default()
};
```

Wrap one or more in `Document::Errors`:

```rust
use jsonapi_core::{Document, Resource};

let doc: Document<Resource> = Document::Errors {
    errors: vec![err],
    meta: None,
    jsonapi: None,
    links: None,
};
let body = serde_json::to_string(&doc)?;
```

## A typical translation layer

A small helper that converts a crate-level `Error` into an outbound `ApiError`
and HTTP status:

```rust
use jsonapi_core::{ApiError, Error};

fn into_api_error(err: &Error) -> (u16, ApiError) {
    let api_err = ApiError {
        status: Some(match err {
            Error::MediaTypeMismatch { .. } | Error::UnsupportedMediaTypeParam { .. } => "415",
            Error::NoAcceptableMediaType | Error::AllMediaTypesUnsupportedParams => "406",
            _ => "400",
        }.into()),
        title: Some("Request rejected".into()),
        detail: Some(err.to_string()),
        ..Default::default()
    };
    let status: u16 = api_err.status.as_deref().unwrap_or("400").parse().unwrap_or(400);
    (status, api_err)
}
```

The shape is up to you — JSON:API only requires that an error response use the
`errors` member; everything inside is optional.

## Tip: surface JSON pointers

When you can name the offending field, use `ErrorSource::pointer` (an
[RFC 6901](https://datatracker.ietf.org/doc/html/rfc6901) JSON pointer):

```rust
use jsonapi_core::{ApiError, ErrorSource};

ApiError {
    status: Some("422".into()),
    source: Some(ErrorSource {
        pointer: Some("/data/attributes/email".into()),
        ..Default::default()
    }),
    detail: Some("must be a valid email address".into()),
    ..Default::default()
};
```

Clients can use the pointer to attach the message to a specific form field —
JSON:API's standard mechanism for field-level validation feedback.
