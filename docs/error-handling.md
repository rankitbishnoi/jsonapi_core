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
    RegistryLookup { r#type: String, id: String },
    NullRelationship,
    RelationshipCardinalityMismatch { expected: Cardinality }, // Cardinality is ToOne | ToMany
    LidNotIndexed,
    MediaTypeMismatch { expected: String, got: String },
    UnsupportedMediaTypeParam { param: String },
    MediaTypeParse(String),
    NoAcceptableMediaType,
    AllMediaTypesUnsupportedParams,
    Structure(String),
    QueryParse { param: String, reason: String },
    InvalidIncludePath { path: String, segment: String, type_name: String },
    InvalidAtomicOperation { index: usize, reason: String },
    UnexpectedDocumentShape { expected: &'static str, found: &'static str },
    TypeMismatch { expected: &'static str, got: String, location: String },
    MalformedRelationship { name: String, location: String, reason: String },
    MissingAttribute { resource_type: &'static str, attribute: &'static str, location: String },
    FullLinkageViolation { r#type: String, id: String, location: String },
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
| `QueryParse` | `Query::from_query_string` / `from_pairs` hit a malformed parameter (carries the offending `param`). |
| `InvalidIncludePath` | `TypeRegistry::validate_include_paths` couldn't resolve a hop. |
| `InvalidAtomicOperation` | `AtomicRequest::validate_lid_refs` failed (Atomic Ops extension). |
| `UnexpectedDocumentShape` | Caller used an accessor like `Document::into_single` on a document of the wrong shape. |
| `TypeMismatch` | `Document::from_*` pre-pass: wire `data.type` ≠ `#[jsonapi(type = "...")]`. |
| `MalformedRelationship` | `Document::from_*` pre-pass: relationship value isn't an object, or its `data` is structurally invalid. |
| `MissingAttribute` | `Document::from_*` pre-pass: a required (non-`Option`, non-`Vec`) attribute is absent on the primary resource. |
| `FullLinkageViolation` | `Document::from_*` pre-pass: a resource in the wire `included` array is referenced by no resource identifier object in the document (JSON:API full-linkage rule). |

### Typed parse errors

`Document::from_str`, `Document::from_slice`, and `Document::from_value` run a
structural pre-pass over the wire payload before delegating to
`serde_json::from_value`. The pre-pass surfaces four typed errors instead of
opaque `serde_json::Error` strings, so consumers can map upstream-format
failures to specific HTTP statuses.

| Variant | Fires when | Location format |
| --- | --- | --- |
| `TypeMismatch` | Wire `data.type` doesn't match the consumer's `#[jsonapi(type = "...")]`. | `"data"` or `"data[N]"` |
| `MalformedRelationship` | Relationship value isn't a JSON object, or its `data` member is neither null/object/array. | `"data"` or `"data[N]"` |
| `MissingAttribute` | A non-`Option`, non-`Vec` attribute on the consumer's struct is absent from the wire `attributes` block on the primary resource. | `"data"` or `"data[N]"` |
| `FullLinkageViolation` | A resource in the wire `included` array is referenced by no resource identifier object elsewhere in the document. | `"included[N]"` |

**Pre-pass walk order** (first error wins):

1. `TypeMismatch` (primary data)
2. `MissingAttribute` (primary data)
3. `MalformedRelationship` (primary data)
4. `FullLinkageViolation` (whole document)

**`FullLinkageViolation` enforces the spec's full-linkage rule.** Every resource
in a non-empty `included` array must be referenced by at least one resource
identifier object in the same document — whether in the primary data's
relationship linkage or in another resource's. An included resource that nothing
references is unreachable and signals a server-side compound-document defect.

This is *not* the inverse: a relationship may reference a resource that is not in
`included`. Partial-include responses — e.g. `?include=comments` returns the
comments in `included` while `author` linkage points at a resource the client did
not request — are valid and parse cleanly. The check only flags unreferenced
*included* resources.

**References that use only `lid`** (no `id`) are tracked as `lid` linkage; an
included resource identified by `lid` is matched against it. (`lid`-only
identifiers are atomic-operation client-local references, resolved at request
execution time.)

**`Document<Resource>` semantics.** When the primary type `P` is the dynamic
`Resource`, the type check and required-attribute check are skipped (open-set
primary type). The relationship walk and full-linkage check still run, so
`Document::<Resource>::from_str` validates compound-document linkage on documents
with arbitrary primary shapes.

**Empty-set behaviour.** When the wire payload's `included` array is absent or
empty, `FullLinkageViolation` does not fire — there is no compound document to
validate.

### Mapping to HTTP status codes

A natural translation table for a handler:

| Error variant | Suggested HTTP status |
|---------------|-----------------------|
| `MediaTypeMismatch`, `UnsupportedMediaTypeParam` | 415 Unsupported Media Type |
| `NoAcceptableMediaType`, `AllMediaTypesUnsupportedParams` | 406 Not Acceptable |
| `Structure`, `InvalidMemberName`, `QueryParse`, `InvalidIncludePath`, `InvalidAtomicOperation` | 400 Bad Request |
| `Json` (during request parsing) | 400 Bad Request |
| `RegistryLookup`, `NullRelationship`, `RelationshipCardinalityMismatch`, `LidNotIndexed` | Internal — these reflect *your* code's assumptions about the document |
| `TypeMismatch`, `MalformedRelationship`, `FullLinkageViolation` | 502 Bad Gateway — upstream payload structurally wrong |
| `MissingAttribute` | 422 Unprocessable Entity — consumer's schema requires a field upstream omitted |
| `UnexpectedDocumentShape` | Internal — caller used the wrong accessor |

`MissingAttribute` deserves `422 Unprocessable Entity` because the failure is
a contract mismatch (consumer expected a field upstream considers optional);
the other three pre-pass typed errors are `502 Bad Gateway` because the wire
payload itself is broken.

This table is written from the consumer's perspective — parsing an *upstream*
response, where a structural defect is the upstream server's fault (`502`). The
`jsonapi_http` crate's `status_for`, by contrast, parses *incoming request*
bodies, where the same defect is the *client's* fault; it therefore maps
`FullLinkageViolation` (and the other structural pre-pass errors) to `400 Bad
Request`. Pick the status that matches where the document came from.

```rust
use jsonapi_core::Error;

fn map_to_http_status(err: &Error) -> u16 {
    match err {
        Error::TypeMismatch { .. }
        | Error::MalformedRelationship { .. }
        | Error::FullLinkageViolation { .. } => 502,
        Error::MissingAttribute { .. } => 422,
        Error::Json(_) => 400,
        _ => 500,
    }
}
```

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

Use the builder methods to populate only the fields you care about:

```rust
use jsonapi_core::{ApiError, ErrorSource};

let err = ApiError::new()
    .with_status("422")
    .with_title("Validation failed")
    .with_detail("`title` is required")
    .with_source(ErrorSource::pointer("/data/attributes/title"));
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
    let status_str = match err {
        Error::MediaTypeMismatch { .. } | Error::UnsupportedMediaTypeParam { .. } => "415",
        Error::NoAcceptableMediaType | Error::AllMediaTypesUnsupportedParams => "406",
        _ => "400",
    };
    let api_err = ApiError::new()
        .with_status(status_str)
        .with_title("Request rejected")
        .with_detail(err.to_string());
    let status: u16 = status_str.parse().unwrap_or(400);
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

ApiError::new()
    .with_status("422")
    .with_source(ErrorSource::pointer("/data/attributes/email"))
    .with_detail("must be a valid email address");
```

Clients can use the pointer to attach the message to a specific form field —
JSON:API's standard mechanism for field-level validation feedback.
