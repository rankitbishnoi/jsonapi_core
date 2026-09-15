//! Request-URI-aware pagination links (G7).
//!
//! Thin binding over [`jsonapi_core::PaginationLinks`]: it takes the request
//! [`Uri`], uses its path as the link base, and preserves every non-`page` query
//! parameter (`sort`/`filter`/`fields`/`include`, and any custom params) so the
//! generated `first`/`prev`/`next`/`last`/`self` links keep the client's other
//! query state. The pagination math itself lives in `jsonapi_core`.
//!
//! The handler supplies the [`PageStrategy`] it actually applied (the effective
//! offset/limit or number/size — the library never touches the datastore) and an
//! optional total count.

use http::Uri;
use percent_encoding::percent_decode_str;

use jsonapi_core::{Links, PageStrategy, PaginationLinks};

/// Build top-level pagination [`Links`] for a list response from the request
/// `uri`, the `strategy` the handler paginated with, and an optional `total`.
///
/// The link base is `uri.path()`; every non-`page[...]` query parameter is
/// decoded and preserved into each generated link. See
/// [`jsonapi_core::PaginationLinks`] for which links are emitted (and when
/// `prev`/`next`/`last` are omitted).
///
/// ```no_run
/// use http::Uri;
/// use jsonapi_axum::pagination_links;
/// use jsonapi_core::PageStrategy;
///
/// let uri: Uri = "/articles?sort=-created&page[number]=2&page[size]=10".parse().unwrap();
/// let links = pagination_links(&uri, PageStrategy::PageNumber { number: 2, size: 10 }, Some(35));
/// assert!(links.contains("next"));
/// ```
#[must_use]
pub fn pagination_links(uri: &Uri, strategy: PageStrategy, total: Option<u64>) -> Links {
    let preserved = preserved_params(uri.query().unwrap_or(""));
    let preserved_refs: Vec<(&str, &str)> = preserved
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let mut builder = PaginationLinks::new(uri.path(), strategy).preserve(&preserved_refs);
    if let Some(total) = total {
        builder = builder.total(total);
    }
    builder.build()
}

/// Decode a raw query string into `(key, value)` pairs, dropping every
/// `page[...]` parameter (the builder re-emits those per link). Keys and values
/// are percent-decoded so the core builder — which re-encodes via
/// `QueryBuilder` — does not double-encode them.
fn preserved_params(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (raw_key, raw_value) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
            let key = decode(raw_key);
            if key.starts_with("page[") {
                return None;
            }
            Some((key, decode(raw_value)))
        })
        .collect()
}

fn decode(raw: &str) -> String {
    percent_decode_str(raw).decode_utf8_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link_str(links: &Links, rel: &str) -> String {
        match links.get(rel).unwrap() {
            jsonapi_core::Link::String(s) => s.clone(),
            _ => panic!("expected string link for {rel}"),
        }
    }

    #[test]
    fn preserves_non_page_params_and_drops_page_params() {
        let uri: Uri = "/articles?sort=-created&page%5Bnumber%5D=2&page%5Bsize%5D=10&filter%5Bstatus%5D=published"
            .parse()
            .unwrap();
        let links = pagination_links(
            &uri,
            PageStrategy::PageNumber { number: 2, size: 10 },
            Some(35),
        );

        let next = link_str(&links, "next");
        // Base path preserved, page params rebuilt, non-page params carried over.
        assert!(next.starts_with("/articles?"), "{next}");
        assert!(next.contains("sort=-created"), "{next}");
        assert!(next.contains("filter[status]=published"), "{next}");
        assert!(next.contains("page[number]=3"), "{next}");
        assert!(next.contains("page[size]=10"), "{next}");
        // The client's original page params must not leak in duplicated.
        assert!(!next.contains("page[number]=2"), "{next}");
    }

    #[test]
    fn offset_strategy_over_request_uri() {
        let uri: Uri = "/articles?page[offset]=0&page[limit]=10".parse().unwrap();
        let links = pagination_links(&uri, PageStrategy::Offset { offset: 0, limit: 10 }, Some(25));
        assert!(!links.contains("prev"), "first page omits prev");
        assert!(link_str(&links, "next").contains("page[offset]=10"));
        // last offset = floor((25-1)/10)*10 = 20.
        assert!(link_str(&links, "last").contains("page[offset]=20"));
    }

    #[test]
    fn no_query_string_still_builds_links() {
        let uri: Uri = "/articles".parse().unwrap();
        let links = pagination_links(&uri, PageStrategy::Offset { offset: 0, limit: 5 }, None);
        assert!(link_str(&links, "self").starts_with("/articles?page[offset]=0"));
    }
}
