//! Cursor-pagination profile support (ethanresnick cursor-pagination).

use crate::model::{Link, Links};
use crate::query::{Query, QueryBuilder};

/// The ethanresnick cursor-pagination profile URI.
pub const CURSOR_PAGINATION_PROFILE: &str =
    "https://jsonapi.org/profiles/ethanresnick/cursor-pagination";

/// A typed view over the generic `page` map for the cursor-pagination profile.
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use jsonapi_core::{Query, CursorPage};
/// let q = Query::from_query_string("?page[size]=20&page[after]=abc")?;
/// let cp = CursorPage::from_query(&q)?;
/// assert_eq!(cp.size, Some(20));
/// assert_eq!(cp.after.as_deref(), Some("abc"));
/// # Ok(())
/// # }
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
/// Set which links to emit fluently, then finish with [`links`](Self::links):
///
/// ```
/// use jsonapi_core::CursorLinks;
/// let links = CursorLinks::new("/articles")
///     .size(10)
///     .first()
///     .next("cur")
///     .links();
/// assert!(links.contains("next"));
/// ```
#[derive(Debug, Clone)]
pub struct CursorLinks<'a> {
    base_path: &'a str,
    preserved: Vec<(String, String)>,
    size: Option<u64>,
    include_first: bool,
    prev_cursor: Option<String>,
    next_cursor: Option<String>,
    last_cursor: Option<String>,
}

impl<'a> CursorLinks<'a> {
    /// Start building links for a collection at `base_path` (e.g. `/articles`).
    #[must_use]
    pub fn new(base_path: &'a str) -> Self {
        Self {
            base_path,
            preserved: Vec::new(),
            size: None,
            include_first: false,
            prev_cursor: None,
            next_cursor: None,
            last_cursor: None,
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

    /// Emit a cursor-less `first` link.
    #[must_use]
    pub fn first(mut self) -> Self {
        self.include_first = true;
        self
    }

    /// Emit a `prev` link at the given `page[before]` cursor.
    #[must_use]
    pub fn prev(mut self, cursor: impl Into<String>) -> Self {
        self.prev_cursor = Some(cursor.into());
        self
    }

    /// Emit a `next` link at the given `page[after]` cursor.
    #[must_use]
    pub fn next(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }

    /// Emit a `last` link at the given `page[before]` end-boundary cursor.
    #[must_use]
    pub fn last(mut self, cursor: impl Into<String>) -> Self {
        self.last_cursor = Some(cursor.into());
        self
    }

    /// Finish and build the configured links.
    #[must_use]
    pub fn links(&self) -> Links {
        let mut links = Links::new();
        if self.include_first {
            links.insert("first", Some(Link::String(self.url(None))));
        }
        if let Some(cur) = self.prev_cursor.as_deref() {
            links.insert("prev", Some(Link::String(self.url(Some(("before", cur))))));
        }
        if let Some(cur) = self.next_cursor.as_deref() {
            links.insert("next", Some(Link::String(self.url(Some(("after", cur))))));
        }
        if let Some(cur) = self.last_cursor.as_deref() {
            // `last` uses page[before]: the cursor marking the end boundary.
            links.insert("last", Some(Link::String(self.url(Some(("before", cur))))));
        }
        links
    }

    /// Build the links from positional arguments.
    ///
    /// Superseded by the fluent [`first`](Self::first)/[`prev`](Self::prev)/
    /// [`next`](Self::next)/[`last`](Self::last) setters + [`links`](Self::links),
    /// which read more clearly at the call site.
    #[deprecated(
        since = "0.5.0",
        note = "use the fluent .first()/.prev()/.next()/.last().links() builder instead"
    )]
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
            links.insert("first", Some(Link::String(self.url(None))));
        }
        if let Some(cur) = prev {
            links.insert("prev", Some(Link::String(self.url(Some(("before", cur))))));
        }
        if let Some(cur) = next {
            links.insert("next", Some(Link::String(self.url(Some(("after", cur))))));
        }
        if let Some(cur) = last {
            // `last` uses page[before]: the cursor marking the end boundary.
            links.insert("last", Some(Link::String(self.url(Some(("before", cur))))));
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

// --- Offset & page-number pagination (G7) -------------------------------

/// A typed view over the generic `page` map for **offset** pagination
/// (`page[offset]` / `page[limit]`). `offset` defaults to `0` when absent;
/// `limit` is left `None` (the server picks a default page size).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OffsetPage {
    /// `page[offset]` (defaults to `0`).
    pub offset: u64,
    /// `page[limit]`, if the client supplied one.
    pub limit: Option<u64>,
}

impl OffsetPage {
    /// Extract the offset-strategy page parameters from a parsed [`Query`].
    ///
    /// # Errors
    /// [`Error::QueryParse`](crate::Error::QueryParse) if `page[offset]` or
    /// `page[limit]` is present but not a non-negative integer.
    pub fn from_query(query: &Query) -> crate::Result<OffsetPage> {
        Ok(OffsetPage {
            offset: parse_page_u64(query, "offset")?.unwrap_or(0),
            limit: parse_page_u64(query, "limit")?,
        })
    }
}

/// A typed view over the generic `page` map for **page-number** pagination
/// (`page[number]` / `page[size]`). `number` defaults to `1` when absent; `size`
/// is left `None` (the server picks a default page size).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageNumberPage {
    /// `page[number]`, 1-based (defaults to `1`).
    pub number: u64,
    /// `page[size]`, if the client supplied one.
    pub size: Option<u64>,
}

impl Default for PageNumberPage {
    fn default() -> Self {
        Self {
            number: 1,
            size: None,
        }
    }
}

impl PageNumberPage {
    /// Extract the page-number-strategy page parameters from a parsed [`Query`].
    ///
    /// # Errors
    /// [`Error::QueryParse`](crate::Error::QueryParse) if `page[number]` or
    /// `page[size]` is present but not a non-negative integer.
    pub fn from_query(query: &Query) -> crate::Result<PageNumberPage> {
        Ok(PageNumberPage {
            number: parse_page_u64(query, "number")?.unwrap_or(1),
            size: parse_page_u64(query, "size")?,
        })
    }
}

/// Parse a `page[key]` value as a `u64`, mapping a bad value to a
/// [`QueryParse`](crate::Error::QueryParse) error naming the full parameter.
fn parse_page_u64(query: &Query, key: &str) -> crate::Result<Option<u64>> {
    match query.page.get(key) {
        Some(raw) => raw
            .parse::<u64>()
            .map(Some)
            .map_err(|_| crate::Error::QueryParse {
                param: format!("page[{key}]"),
                reason: "expected a non-negative integer".into(),
            }),
        None => Ok(None),
    }
}

/// The concrete page a [`PaginationLinks`] builder is centered on: either
/// offset/limit or (1-based) number/size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageStrategy {
    /// `page[offset]` / `page[limit]` pagination.
    Offset {
        /// The current page's `page[offset]`.
        offset: u64,
        /// The page size, `page[limit]`.
        limit: u64,
    },
    /// `page[number]` / `page[size]` pagination (1-based `number`).
    PageNumber {
        /// The current page's `page[number]` (1-based).
        number: u64,
        /// The page size, `page[size]`.
        size: u64,
    },
}

/// Builds top-level `self`/`first`/`prev`/`next`/`last` pagination [`Link`]s for
/// an offset- or page-number-paginated collection, preserving caller-provided
/// (`sort`/`filter`/`fields`/`include`) query parameters.
///
/// `prev` is omitted on the first page; `next`/`last` are omitted at/after the
/// end. With an unknown total, `last` is omitted and `next` is always emitted
/// (the end cannot be known).
///
/// ```
/// use jsonapi_core::{PaginationLinks, PageStrategy};
/// let links = PaginationLinks::new("/articles", PageStrategy::PageNumber { number: 2, size: 10 })
///     .total(35)
///     .build();
/// assert!(links.contains("prev") && links.contains("next") && links.contains("last"));
/// ```
#[derive(Debug, Clone)]
pub struct PaginationLinks<'a> {
    base_path: &'a str,
    preserved: Vec<(String, String)>,
    strategy: PageStrategy,
    total: Option<u64>,
}

impl<'a> PaginationLinks<'a> {
    /// Start building links for a collection at `base_path` (e.g. `/articles`)
    /// centered on `strategy`'s current page.
    #[must_use]
    pub fn new(base_path: &'a str, strategy: PageStrategy) -> Self {
        Self {
            base_path,
            preserved: Vec::new(),
            strategy,
            total: None,
        }
    }

    /// Carry these (already-decoded) non-`page` query params forward into every
    /// generated link (e.g. the request's `sort`/`filter`/`fields`/`include`).
    /// Replaces any params set by a prior `preserve` call.
    #[must_use]
    pub fn preserve(mut self, pairs: &[(&str, &str)]) -> Self {
        self.preserved = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self
    }

    /// Set the total number of resources, enabling `last` and bounding `next`.
    #[must_use]
    pub fn total(mut self, total: u64) -> Self {
        self.total = Some(total);
        self
    }

    /// Build the `self`/`first`/`prev`/`next`/`last` links.
    #[must_use]
    pub fn build(&self) -> Links {
        match self.strategy {
            PageStrategy::Offset { offset, limit } => self.build_offset(offset, limit),
            PageStrategy::PageNumber { number, size } => self.build_page_number(number, size),
        }
    }

    fn build_offset(&self, offset: u64, limit: u64) -> Links {
        let mut links = Links::new();
        let page = |off: u64| vec![("offset", off.to_string()), ("limit", limit.to_string())];

        links.insert("self", Some(Link::String(self.url(&page(offset)))));
        links.insert("first", Some(Link::String(self.url(&page(0)))));

        // A zero limit cannot advance; only self/first are meaningful.
        if limit == 0 {
            return links;
        }

        if offset > 0 {
            let prev = offset.saturating_sub(limit);
            links.insert("prev", Some(Link::String(self.url(&page(prev)))));
        }

        let next_offset = offset.saturating_add(limit);
        let has_next = match self.total {
            Some(total) => next_offset < total,
            None => true,
        };
        if has_next {
            links.insert("next", Some(Link::String(self.url(&page(next_offset)))));
        }

        if let Some(total) = self.total {
            let last_offset = if total == 0 {
                0
            } else {
                ((total - 1) / limit) * limit
            };
            links.insert("last", Some(Link::String(self.url(&page(last_offset)))));
        }

        links
    }

    fn build_page_number(&self, number: u64, size: u64) -> Links {
        let mut links = Links::new();
        let number = number.max(1); // pages are 1-based
        let page = |n: u64| vec![("number", n.to_string()), ("size", size.to_string())];

        links.insert("self", Some(Link::String(self.url(&page(number)))));
        links.insert("first", Some(Link::String(self.url(&page(1)))));

        if size == 0 {
            return links;
        }

        if number > 1 {
            links.insert("prev", Some(Link::String(self.url(&page(number - 1)))));
        }

        let last_number = self.total.map(|total| total.div_ceil(size).max(1));
        let has_next = match last_number {
            Some(last) => number < last,
            None => true,
        };
        if has_next {
            links.insert("next", Some(Link::String(self.url(&page(number + 1)))));
        }

        if let Some(last) = last_number {
            links.insert("last", Some(Link::String(self.url(&page(last)))));
        }

        links
    }

    fn url(&self, page: &[(&str, String)]) -> String {
        let mut qb = QueryBuilder::new();
        for (k, v) in &self.preserved {
            qb = qb.param(k, v);
        }
        for (key, value) in page {
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
            .first()
            .prev("prevcur")
            .next("nextcur")
            .links();

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
        let links = CursorLinks::new("/a").size(5).first().links();
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
            .last("endcur")
            .links();

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

    #[test]
    #[allow(deprecated)]
    fn cursor_links_deprecated_build_matches_fluent() {
        let base = CursorLinks::new("/articles").size(10);
        let positional = base.clone().build(true, None, Some("nextcur"), None);
        let fluent = base.first().next("nextcur").links();
        assert_eq!(positional.get("next"), fluent.get("next"));
        assert_eq!(positional.get("first"), fluent.get("first"));
    }

    // --- Offset & page-number typed views + link builders (G7) ---

    fn link_str(links: &Links, rel: &str) -> String {
        match links.get(rel).unwrap() {
            crate::Link::String(s) => s.clone(),
            _ => panic!("expected a string link for {rel}"),
        }
    }

    #[test]
    fn offset_page_from_query_defaults_offset_to_zero() {
        let q = Query::from_pairs(&[("page[limit]", "10")]).unwrap();
        let op = OffsetPage::from_query(&q).unwrap();
        assert_eq!(op.offset, 0);
        assert_eq!(op.limit, Some(10));
    }

    #[test]
    fn page_number_from_query_defaults_number_to_one() {
        let q = Query::from_pairs(&[("page[size]", "20")]).unwrap();
        let pn = PageNumberPage::from_query(&q).unwrap();
        assert_eq!(pn.number, 1);
        assert_eq!(pn.size, Some(20));
    }

    #[test]
    fn offset_page_bad_value_is_query_parse_error() {
        let q = Query::from_pairs(&[("page[offset]", "nope")]).unwrap();
        let err = OffsetPage::from_query(&q).unwrap_err();
        assert!(
            matches!(err, crate::Error::QueryParse { ref param, .. } if param == "page[offset]")
        );
    }

    #[test]
    fn offset_middle_page_has_all_rels_and_preserves_params() {
        // offset=20, limit=10, total=35 → middle page.
        let links = PaginationLinks::new(
            "/articles",
            PageStrategy::Offset {
                offset: 20,
                limit: 10,
            },
        )
        .preserve(&[("sort", "-created"), ("filter[status]", "published")])
        .total(35)
        .build();

        assert!(links.contains("self"));
        assert!(links.contains("first"));
        assert!(links.contains("prev"));
        assert!(links.contains("next"));
        assert!(links.contains("last"));

        let next = link_str(&links, "next");
        assert!(next.contains("page[offset]=30"), "{next}");
        assert!(next.contains("page[limit]=10"), "{next}");
        assert!(next.contains("sort=-created"), "{next}");
        assert!(next.contains("filter[status]=published"), "{next}");

        assert!(link_str(&links, "prev").contains("page[offset]=10"));
        assert!(link_str(&links, "first").contains("page[offset]=0"));
        // last offset = floor((35-1)/10)*10 = 30.
        assert!(link_str(&links, "last").contains("page[offset]=30"));
        assert!(link_str(&links, "self").contains("page[offset]=20"));
    }

    #[test]
    fn offset_first_page_omits_prev() {
        let links = PaginationLinks::new(
            "/articles",
            PageStrategy::Offset {
                offset: 0,
                limit: 10,
            },
        )
        .total(35)
        .build();
        assert!(!links.contains("prev"));
        assert!(links.contains("next"));
        assert!(links.contains("last"));
    }

    #[test]
    fn offset_last_page_omits_next() {
        // offset=30, limit=10, total=35 → last page (30..35).
        let links = PaginationLinks::new(
            "/articles",
            PageStrategy::Offset {
                offset: 30,
                limit: 10,
            },
        )
        .total(35)
        .build();
        assert!(links.contains("prev"));
        assert!(!links.contains("next"), "at/after end must omit next");
        assert!(links.contains("last"));
    }

    #[test]
    fn unknown_total_omits_last_but_keeps_next() {
        let links = PaginationLinks::new(
            "/articles",
            PageStrategy::Offset {
                offset: 10,
                limit: 10,
            },
        )
        .build();
        assert!(!links.contains("last"), "unknown total → no last");
        assert!(links.contains("next"), "unknown total → next still emitted");
        assert!(links.contains("prev"));
    }

    #[test]
    fn page_number_middle_page_computes_neighbors_and_last() {
        // number=2, size=10, total=35 → last page = ceil(35/10) = 4.
        let links = PaginationLinks::new(
            "/articles",
            PageStrategy::PageNumber {
                number: 2,
                size: 10,
            },
        )
        .total(35)
        .build();
        assert!(link_str(&links, "prev").contains("page[number]=1"));
        assert!(link_str(&links, "next").contains("page[number]=3"));
        assert!(link_str(&links, "last").contains("page[number]=4"));
        assert!(link_str(&links, "first").contains("page[number]=1"));
        assert!(link_str(&links, "self").contains("page[number]=2"));
    }

    #[test]
    fn page_number_last_page_omits_next() {
        let links = PaginationLinks::new(
            "/articles",
            PageStrategy::PageNumber {
                number: 4,
                size: 10,
            },
        )
        .total(35)
        .build();
        assert!(!links.contains("next"));
        assert!(links.contains("prev"));
        assert!(link_str(&links, "last").contains("page[number]=4"));
    }
}
