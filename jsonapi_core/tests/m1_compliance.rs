//! Integration tests for M1: 1.1 Compliance Helpers.
//!
//! Tests the public API surface as an external consumer would use it.

use jsonapi_core::{MemberNameKind, negotiate_accept, validate_content_type, validate_member_name};

// --- Member-name validation through public API ---

#[test]
fn member_name_standard_via_public_api() {
    let kind = validate_member_name("title").unwrap();
    assert_eq!(kind, MemberNameKind::Standard);
}

#[test]
fn member_name_at_member_via_public_api() {
    let kind = validate_member_name("@ext:comments").unwrap();
    assert_eq!(
        kind,
        MemberNameKind::AtMember {
            namespace: "ext".into(),
            member: "comments".into(),
        }
    );
}

// --- Content-Type validation through public API ---

#[test]
fn content_type_valid_bare() {
    let mt = validate_content_type("application/vnd.api+json").unwrap();
    assert_eq!(mt.to_header_value(), "application/vnd.api+json");
}

#[test]
fn content_type_with_extensions_round_trips() {
    let header = "application/vnd.api+json; ext=\"https://example.com/ext1\"; profile=\"https://example.com/p1\"";
    let mt = validate_content_type(header).unwrap();
    let reparsed = validate_content_type(&mt.to_header_value()).unwrap();
    assert_eq!(mt, reparsed);
}

#[test]
fn content_type_rejects_charset() {
    assert!(validate_content_type("application/vnd.api+json; charset=utf-8").is_err());
}

// --- Accept negotiation through public API ---

#[test]
fn negotiate_typical_browser_accept() {
    // Browser sends mixed Accept, server supports JSON:API
    let mt = negotiate_accept("text/html, application/vnd.api+json, */*", &[], &[]).unwrap();
    assert_eq!(mt.to_header_value(), "application/vnd.api+json");
}

#[test]
fn negotiate_client_with_extensions() {
    let mt = negotiate_accept(
        "application/vnd.api+json; ext=\"https://example.com/ext1\"",
        &["https://example.com/ext1", "https://example.com/ext2"],
        &[],
    )
    .unwrap();
    assert_eq!(mt.ext.len(), 2);
}

#[test]
fn negotiate_406_all_invalid() {
    let err = negotiate_accept("application/vnd.api+json; charset=utf-8", &[], &[]).unwrap_err();
    assert!(err.to_string().contains("unsupported parameters"));
}

// --- End-to-end: parse from header, validate, build response ---

#[test]
fn end_to_end_server_flow() {
    // 1. Client sends Content-Type with extension
    let client_ct =
        validate_content_type("application/vnd.api+json; ext=\"https://example.com/atomic\"")
            .unwrap();
    assert_eq!(client_ct.ext, vec!["https://example.com/atomic"]);

    // 2. Server validates Accept
    let response_mt = negotiate_accept(
        "application/vnd.api+json",
        &["https://example.com/atomic"],
        &[],
    )
    .unwrap();

    // 3. Server builds response Content-Type
    let response_header = response_mt.to_header_value();
    assert!(response_header.starts_with("application/vnd.api+json"));
}
