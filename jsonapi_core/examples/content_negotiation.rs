//! JSON:API 1.1 media-type parsing and content negotiation.
//!
//! Run: `cargo run -p jsonapi_core --example content_negotiation`

use jsonapi_core::{JsonApiMediaType, negotiate_accept, validate_content_type};

fn main() {
    // 1. Validate a Content-Type header (strict: rejects unknown parameters)
    println!("=== Content-Type Validation ===");

    let mt = validate_content_type("application/vnd.api+json").unwrap();
    println!("Plain JSON:API: ext={:?}, profile={:?}", mt.ext, mt.profile);

    let mt = validate_content_type(
        r#"application/vnd.api+json; ext="https://example.com/ext1"; profile="https://example.com/p1""#,
    ).unwrap();
    println!(
        "With ext+profile: ext={:?}, profile={:?}",
        mt.ext, mt.profile
    );

    let err = validate_content_type("application/vnd.api+json; charset=utf-8");
    println!("Unknown param: {}", err.unwrap_err());

    // 2. Negotiate an Accept header (returns server capabilities)
    println!("\n=== Accept Negotiation ===");

    let response = negotiate_accept(
        "application/vnd.api+json, application/json",
        &["https://example.com/ext1"],
        &["https://example.com/profile1"],
    )
    .unwrap();
    println!("Negotiated: {}", response.to_header_value());

    // Wildcard acceptance
    let response = negotiate_accept("*/*", &[], &[]).unwrap();
    println!("Wildcard: {}", response.to_header_value());

    // 3. Compatibility check
    println!("\n=== Compatibility ===");

    let server = JsonApiMediaType::parse(
        r#"application/vnd.api+json; ext="https://example.com/ext1 https://example.com/ext2""#,
    )
    .unwrap();
    let client =
        JsonApiMediaType::parse(r#"application/vnd.api+json; ext="https://example.com/ext1""#)
            .unwrap();
    println!(
        "Client compatible with server? {}",
        client.is_compatible_with(&server)
    );
    println!(
        "Server compatible with client? {}",
        server.is_compatible_with(&client)
    );
}
