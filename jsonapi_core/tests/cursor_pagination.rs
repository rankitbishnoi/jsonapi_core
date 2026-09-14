//! Integration tests for the cursor-pagination profile.

use jsonapi_core::{CURSOR_PAGINATION_PROFILE, CursorLinks, CursorPage, Query};

#[test]
fn parse_then_build_next_link_round_trip() {
    let q =
        Query::from_query_string("?page[size]=15&page[after]=cursor42&filter[kind]=news").unwrap();
    let cp = CursorPage::from_query(&q).unwrap();
    assert_eq!(cp.size, Some(15));
    assert_eq!(cp.after.as_deref(), Some("cursor42"));

    let links = CursorLinks::new("/articles")
        .preserve(&[("filter[kind]", "news")])
        .size(cp.size.unwrap())
        .build(true, None, Some("cursor99"), None);

    let next = match links.get("next").unwrap() {
        jsonapi_core::Link::String(s) => s.clone(),
        _ => panic!("expected string link"),
    };
    assert!(next.contains("page[after]=cursor99"));
    assert!(next.contains("page[size]=15"));
    assert!(next.contains("filter[kind]=news"));
}

#[test]
fn profile_uri_is_stable() {
    assert_eq!(
        CURSOR_PAGINATION_PROFILE,
        "https://jsonapi.org/profiles/ethanresnick/cursor-pagination"
    );
}
