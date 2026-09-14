//! Cursor-pagination profile support (ethanresnick cursor-pagination).

use crate::model::{Link, Links};
use crate::query::{Query, QueryBuilder};

/// The ethanresnick cursor-pagination profile URI.
pub const CURSOR_PAGINATION_PROFILE: &str =
    "https://jsonapi.org/profiles/ethanresnick/cursor-pagination";

/// A typed view over the generic `page` map for the cursor-pagination profile.
///
/// ```
/// use jsonapi_core::{Query, CursorPage};
/// let q = Query::from_query_string("?page[size]=20&page[after]=abc").unwrap();
/// let cp = CursorPage::from_query(&q).unwrap();
/// assert_eq!(cp.size, Some(20));
/// assert_eq!(cp.after.as_deref(), Some("abc"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CursorPage {
    /// `page[size]`.
    pub size: Option<u64>,
    /// `page[after]` cursor.
    pub after: Option<String>,
    /// `page[before]` cursor.
    pub before: Option<String>,
}

impl CursorPage {
    /// Extract the cursor-profile page parameters from a parsed [`Query`].
    #[must_use = "parsing result should be used"]
    pub fn from_query(query: &Query) -> crate::Result<CursorPage> {
        let size = match query.page.get("size") {
            Some(s) => Some(s.parse::<u64>().map_err(|_| crate::Error::QueryParse {
                param: "page[size]".into(),
                reason: "expected a non-negative integer".into(),
            })?),
            None => None,
        };
        Ok(CursorPage {
            size,
            after: query.page.get("after").cloned(),
            before: query.page.get("before").cloned(),
        })
    }
}

/// Builds `first`/`prev`/`next`/`last` pagination [`Link`]s for a cursor-paginated
/// collection, preserving caller-provided query parameters.
///
/// ```
/// use jsonapi_core::CursorLinks;
/// let links = CursorLinks::new("/articles").size(10).build(true, None, Some("cur"), None);
/// assert!(links.contains("next"));
/// ```
#[derive(Debug, Clone)]
pub struct CursorLinks<'a> {
    base_path: &'a str,
    preserved: Vec<(String, String)>,
    size: Option<u64>,
}

impl<'a> CursorLinks<'a> {
    /// Start building links for a collection at `base_path` (e.g. `/articles`).
    #[must_use]
    pub fn new(base_path: &'a str) -> Self {
        Self {
            base_path,
            preserved: Vec::new(),
            size: None,
        }
    }

    /// Carry these (already-decoded) non-`page` query params forward into every
    /// generated link (e.g. the request's `sort`/`filter`/`include`).
    /// Replaces any params set by a prior `preserve` call.
    #[must_use]
    pub fn preserve(mut self, pairs: &[(&str, &str)]) -> Self {
        self.preserved = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self
    }

    /// Set `page[size]` on every generated link.
    #[must_use]
    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Build the links. `first` (bool) emits a cursor-less first-page link;
    /// `prev`/`next`/`last` emit `page[before]`/`page[after]`/`page[before]`
    /// links respectively when `Some`.
    #[must_use]
    pub fn build(
        &self,
        first: bool,
        prev: Option<&str>,
        next: Option<&str>,
        last: Option<&str>,
    ) -> Links {
        let mut links = Links::new();
        if first {
            links
                .0
                .insert("first".to_string(), Some(Link::String(self.url(None))));
        }
        if let Some(cur) = prev {
            links.0.insert(
                "prev".to_string(),
                Some(Link::String(self.url(Some(("before", cur))))),
            );
        }
        if let Some(cur) = next {
            links.0.insert(
                "next".to_string(),
                Some(Link::String(self.url(Some(("after", cur))))),
            );
        }
        if let Some(cur) = last {
            // `last` uses page[before]: the cursor marking the end boundary.
            links.0.insert(
                "last".to_string(),
                Some(Link::String(self.url(Some(("before", cur))))),
            );
        }
        links
    }

    fn url(&self, cursor: Option<(&str, &str)>) -> String {
        let mut qb = QueryBuilder::new();
        for (k, v) in &self.preserved {
            qb = qb.param(k, v);
        }
        if let Some(size) = self.size {
            qb = qb.page("size", &size.to_string());
        }
        if let Some((key, value)) = cursor {
            qb = qb.page(key, value);
        }
        let qs = qb.build();
        if qs.is_empty() {
            self.base_path.to_string()
        } else {
            format!("{}?{}", self.base_path, qs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Query;

    #[test]
    fn profile_constant_is_the_ethanresnick_uri() {
        assert_eq!(
            CURSOR_PAGINATION_PROFILE,
            "https://jsonapi.org/profiles/ethanresnick/cursor-pagination"
        );
    }

    #[test]
    fn cursor_page_from_query_reads_size_after_before() {
        let q = Query::from_pairs(&[
            ("page[size]", "25"),
            ("page[after]", "abc"),
            ("page[before]", "xyz"),
        ])
        .unwrap();
        let cp = CursorPage::from_query(&q).unwrap();
        assert_eq!(cp.size, Some(25));
        assert_eq!(cp.after.as_deref(), Some("abc"));
        assert_eq!(cp.before.as_deref(), Some("xyz"));
    }

    #[test]
    fn cursor_page_bad_size_is_query_parse_error() {
        let q = Query::from_pairs(&[("page[size]", "notanumber")]).unwrap();
        let err = CursorPage::from_query(&q).unwrap_err();
        assert!(matches!(err, crate::Error::QueryParse { .. }));
    }

    #[test]
    fn cursor_links_build_emits_requested_links_with_cursors() {
        let links = CursorLinks::new("/articles")
            .preserve(&[("filter[status]", "published")])
            .size(10)
            .build(true, Some("prevcur"), Some("nextcur"), None);

        assert!(links.contains("first"));
        assert!(links.contains("prev"));
        assert!(links.contains("next"));
        assert!(!links.contains("last"));

        let next_str = match links.get("next").unwrap() {
            crate::Link::String(s) => s.clone(),
            _ => panic!("expected a string link"),
        };
        assert!(next_str.starts_with("/articles?"));
        assert!(next_str.contains("page[after]=nextcur"));
        assert!(next_str.contains("page[size]=10"));
        assert!(next_str.contains("filter[status]=published"));

        let prev_str = match links.get("prev").unwrap() {
            crate::Link::String(s) => s.clone(),
            _ => panic!("expected a string link"),
        };
        assert!(prev_str.contains("page[before]=prevcur"));
    }

    #[test]
    fn cursor_links_first_has_no_cursor() {
        let links = CursorLinks::new("/a").size(5).build(true, None, None, None);
        let first = match links.get("first").unwrap() {
            crate::Link::String(s) => s.clone(),
            _ => panic!(),
        };
        assert!(!first.contains("page[after]"));
        assert!(!first.contains("page[before]"));
        assert!(first.contains("page[size]=5"));
    }

    #[test]
    fn cursor_links_build_emits_last_link_with_before_cursor() {
        let links = CursorLinks::new("/articles")
            .size(10)
            .build(false, None, None, Some("endcur"));

        assert!(links.contains("last"));
        assert!(!links.contains("first"));
        assert!(!links.contains("prev"));
        assert!(!links.contains("next"));

        let last = match links.get("last").unwrap() {
            crate::Link::String(s) => s.clone(),
            _ => panic!("expected a string link"),
        };
        assert!(last.contains("page[before]=endcur"));
        assert!(!last.contains("page[after]"));
        assert!(last.contains("page[size]=10"));
    }
}
