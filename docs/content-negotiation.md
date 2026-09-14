# Content Negotiation

JSON:API 1.1 has strict rules about HTTP `Content-Type` and `Accept` headers:

- The base media type is `application/vnd.api+json`.
- The only allowed parameters are `ext` (extension URIs) and `profile`
  (profile URIs).
- Any other parameter on a `Content-Type` is a **415 Unsupported Media Type**.
- An `Accept` header where every JSON:API instance carries unknown parameters
  is a **406 Not Acceptable**.

`jsonapi_core::media_type` provides three building blocks for handling all of
this:

- [`JsonApiMediaType`] — the parsed media-type value.
- [`validate_content_type`] — strict parser for incoming `Content-Type` headers.
- [`negotiate_accept`] — picks a media type from an `Accept` header.

## Validating `Content-Type`

```rust
use jsonapi_core::validate_content_type;

let mt = validate_content_type("application/vnd.api+json")?;
assert!(mt.ext.is_empty());
assert!(mt.profile.is_empty());

// With extensions and profiles:
let mt = validate_content_type(
    r#"application/vnd.api+json; ext="https://example.com/ext1"; profile="https://example.com/p1""#,
)?;
assert_eq!(mt.ext, vec!["https://example.com/ext1".to_string()]);
assert_eq!(mt.profile, vec!["https://example.com/p1".to_string()]);

// Unknown parameter → 415:
let err = validate_content_type("application/vnd.api+json; charset=utf-8");
assert!(err.is_err());
```

In a typical web framework:

```rust
match validate_content_type(headers.get("content-type").unwrap_or("")) {
    Ok(_)  => { /* accept the request */ }
    Err(_) => return Response::status(415),
}
```

## Negotiating `Accept`

`negotiate_accept` chooses the response media type. Pass it the client's
`Accept` header and the server's declared extension and profile URIs.

The function parses each JSON:API entry in the `Accept` header, respects `q`
weights (see [Quality weights](#quality-weights-q) below), selects the
highest-weighted acceptable entry, and returns **that entry's** `ext`/`profile`
intersected with the server's capabilities. A bare `application/vnd.api+json`
or a wildcard entry requests a plain response — the server's capabilities are
NOT injected into the result.

```rust
use jsonapi_core::negotiate_accept;

let response = negotiate_accept(
    "application/vnd.api+json, application/json",
    &["https://example.com/ext1"],   // server extensions
    &["https://example.com/p1"],     // server profiles
)?;

println!("Content-Type: {}", response.to_header_value());
// → application/vnd.api+json
```

A client that explicitly requests an extension gets it back (when the server
supports it), but a bare request yields a plain response:

```rust
// Client requests ext1 explicitly → server returns it (server supports it):
let mt = negotiate_accept(
    r#"application/vnd.api+json; ext="https://example.com/ext1""#,
    &["https://example.com/ext1"],
    &[],
)?;
assert_eq!(mt.ext, vec!["https://example.com/ext1".to_string()]);

// Bare accept → plain response regardless of server capabilities:
let mt = negotiate_accept("application/vnd.api+json", &["https://example.com/ext1"], &[])?;
assert!(mt.ext.is_empty());
```

Wildcards also yield a plain response, even when the server has capabilities:

```rust
let response = negotiate_accept("*/*", &[], &[])?;
assert_eq!(response.to_header_value(), "application/vnd.api+json");

let mt = negotiate_accept("*/*", &["https://example.com/ext1"], &[])?;
assert!(mt.ext.is_empty());
```

If every JSON:API instance in `Accept` carries unknown parameters, you get
`Error::AllMediaTypesUnsupportedParams` — translate to **406 Not Acceptable**.

## Quality weights (`q`)

`negotiate_accept` honours RFC 7231 §5.3.1 quality weights:

- A missing or malformed `q` defaults to **1.0**.
- `q=0` (or lower) marks a media type as **not acceptable** — it is excluded
  from selection.
- Among candidates with equal weight, a specific `application/vnd.api+json`
  entry wins over a wildcard, then document order is used as a tiebreaker.

```rust
// Higher q wins — ext1 entry at q=0.9 beats plain at q=0.5:
let mt = negotiate_accept(
    r#"application/vnd.api+json; ext="https://example.com/ext1"; q=0.9, application/vnd.api+json; q=0.5"#,
    &["https://example.com/ext1"],
    &[],
)?;
assert_eq!(mt.ext, vec!["https://example.com/ext1".to_string()]);

// q=0 means not acceptable — returns an error:
let err = negotiate_accept("application/vnd.api+json; q=0", &[], &[]);
assert!(err.is_err());
```

## Constructing media types

`JsonApiMediaType` has constructors for the common cases:

```rust
use jsonapi_core::JsonApiMediaType;

// application/vnd.api+json
let mt = JsonApiMediaType::plain();

// application/vnd.api+json; ext="https://jsonapi.org/ext/atomic"
let mt = JsonApiMediaType::with_ext(["https://jsonapi.org/ext/atomic"]);
```

For full control, build the struct directly and then `to_header_value()`:

```rust
let mt = JsonApiMediaType {
    ext: vec!["https://example.com/ext1".into()],
    profile: vec!["https://example.com/p1".into()],
};
let header = mt.to_header_value();
// → application/vnd.api+json; ext="https://example.com/ext1"; profile="https://example.com/p1"
```

## Compatibility checks

`is_compatible_with` checks whether one media type's `ext` and `profile` URIs
are all present in another's:

```rust
let server = JsonApiMediaType::parse(
    r#"application/vnd.api+json; ext="https://example.com/ext1 https://example.com/ext2""#,
)?;
let client = JsonApiMediaType::parse(
    r#"application/vnd.api+json; ext="https://example.com/ext1""#,
)?;

assert!(client.is_compatible_with(&server));   // client wants ext1 — server has it
assert!(!server.is_compatible_with(&client));  // server has more than client offered
```

## Putting it together

A handler skeleton:

```rust
use jsonapi_core::{JsonApiMediaType, negotiate_accept, validate_content_type};

fn handle_request(headers: &HeaderMap, body: &[u8]) -> Response {
    // 1. Validate Content-Type → 415 on failure.
    if validate_content_type(headers.get("content-type").unwrap_or("")).is_err() {
        return Response::status(415);
    }

    // 2. Negotiate response media type → 406 on failure.
    let response_mt = match negotiate_accept(
        headers.get("accept").unwrap_or("*/*"),
        &[],   // server extensions
        &[],   // server profiles
    ) {
        Ok(mt) => mt,
        Err(_) => return Response::status(406),
    };

    // 3. Process the request, build the response, set Content-Type.
    let body = build_response_body();
    Response::ok()
        .header("content-type", response_mt.to_header_value())
        .body(body)
}
```

For the **Atomic Operations** extension, the wire `Content-Type` must declare
the extension URI — see the [Atomic Operations](./atomic-operations.md) chapter
for the pattern.
