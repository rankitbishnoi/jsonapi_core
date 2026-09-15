//! Self- and related-link assembly.
//!
//! JSON:API recommends `links.self` on documents and resources and `self` /
//! `related` links on relationships. These pure functions build the URL strings
//! (and convenient [`Links`] maps) from a base URL plus the resource `type`,
//! `id`, and — for relationships — the relationship name. They are
//! framework-agnostic; an adapter supplies the base URL (see the crate's
//! `jsonapi_axum::BaseUrl`, chosen over deriving from `Host`/`X-Forwarded-*` for
//! proxy safety).
//!
//! The resource `id` is percent-encoded as a single path segment; `type` and
//! relationship names are JSON:API member names (already restricted to
//! path-safe characters) and are emitted verbatim.
//!
//! ```
//! use jsonapi_core::links;
//! assert_eq!(links::resource_self("https://api.test", "articles", "1"),
//!            "https://api.test/articles/1");
//! assert_eq!(links::relationship_self("https://api.test", "articles", "1", "author"),
//!            "https://api.test/articles/1/relationships/author");
//! assert_eq!(links::relationship_related("https://api.test", "articles", "1", "author"),
//!            "https://api.test/articles/1/author");
//! ```

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

use crate::model::{Link, Links};

/// Characters percent-encoded inside a single URL path segment: controls plus
/// the characters that would break out of, or be ambiguous within, a segment.
/// RFC 3986 unreserved characters (`A-Z a-z 0-9 - _ . ~`) and other `pchar`s are
/// left readable.
const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Trim a single trailing `/` from a base URL so joins never double the slash.
fn trim_base(base: &str) -> &str {
    base.strip_suffix('/').unwrap_or(base)
}

/// Percent-encode a resource id as one path segment.
fn seg(id: &str) -> String {
    utf8_percent_encode(id, PATH_SEGMENT).to_string()
}

/// Collection self link: `{base}/{type}`.
#[must_use]
pub fn collection_self(base: &str, type_: &str) -> String {
    format!("{}/{}", trim_base(base), type_)
}

/// Resource self link: `{base}/{type}/{id}`.
#[must_use]
pub fn resource_self(base: &str, type_: &str, id: &str) -> String {
    format!("{}/{}/{}", trim_base(base), type_, seg(id))
}

/// Relationship self link: `{base}/{type}/{id}/relationships/{name}` — the
/// endpoint operating on the relationship's *linkage*.
#[must_use]
pub fn relationship_self(base: &str, type_: &str, id: &str, name: &str) -> String {
    format!(
        "{}/{}/{}/relationships/{}",
        trim_base(base),
        type_,
        seg(id),
        name
    )
}

/// Relationship related link: `{base}/{type}/{id}/{name}` — the endpoint
/// returning the *related resource(s)*.
#[must_use]
pub fn relationship_related(base: &str, type_: &str, id: &str, name: &str) -> String {
    format!("{}/{}/{}/{}", trim_base(base), type_, seg(id), name)
}

/// A `{ "self": … }` [`Links`] map for a resource.
#[must_use]
pub fn resource_self_links(base: &str, type_: &str, id: &str) -> Links {
    let mut links = Links::new();
    links.insert("self", Some(Link::String(resource_self(base, type_, id))));
    links
}

/// A `{ "self": … }` [`Links`] map for a collection.
#[must_use]
pub fn collection_self_links(base: &str, type_: &str) -> Links {
    let mut links = Links::new();
    links.insert("self", Some(Link::String(collection_self(base, type_))));
    links
}

/// A `{ "self": …, "related": … }` [`Links`] map for a relationship, suitable
/// for a relationship object's `links` member.
#[must_use]
pub fn relationship_links(base: &str, type_: &str, id: &str, name: &str) -> Links {
    let mut links = Links::new();
    links.insert(
        "self",
        Some(Link::String(relationship_self(base, type_, id, name))),
    );
    links.insert(
        "related",
        Some(Link::String(relationship_related(base, type_, id, name))),
    );
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_and_collection_self_links() {
        assert_eq!(
            collection_self("https://api.test", "articles"),
            "https://api.test/articles"
        );
        assert_eq!(
            resource_self("https://api.test", "articles", "1"),
            "https://api.test/articles/1"
        );
    }

    #[test]
    fn trailing_slash_in_base_is_not_doubled() {
        assert_eq!(
            resource_self("https://api.test/", "articles", "1"),
            "https://api.test/articles/1"
        );
    }

    #[test]
    fn relationship_self_and_related_links() {
        assert_eq!(
            relationship_self("https://api.test", "articles", "1", "author"),
            "https://api.test/articles/1/relationships/author"
        );
        assert_eq!(
            relationship_related("https://api.test", "articles", "1", "author"),
            "https://api.test/articles/1/author"
        );
    }

    #[test]
    fn id_is_percent_encoded_as_a_path_segment() {
        // A slash or space in an id must not break the path structure.
        assert_eq!(
            resource_self("https://api.test", "articles", "a/b c"),
            "https://api.test/articles/a%2Fb%20c"
        );
        // Unreserved characters stay readable.
        assert_eq!(
            resource_self("https://api.test", "articles", "a-b_c.d~e"),
            "https://api.test/articles/a-b_c.d~e"
        );
    }

    #[test]
    fn links_maps_carry_self_and_related() {
        let links = resource_self_links("https://api.test", "articles", "1");
        assert!(matches!(links.get("self"), Some(Link::String(s)) if s == "https://api.test/articles/1"));

        let rel = relationship_links("https://api.test", "articles", "1", "author");
        assert!(rel.get("self").is_some());
        assert!(rel.get("related").is_some());
    }
}
