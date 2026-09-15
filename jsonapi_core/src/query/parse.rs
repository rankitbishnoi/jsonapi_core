//! Server-side parsing of JSON:API request query parameters into a [`Query`].

use std::collections::BTreeMap;

use percent_encoding::percent_decode_str;

use crate::FieldsetConfig;

/// A single `sort` field with its direction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SortField {
    /// The field name (without the leading `-`).
    pub field: String,
    /// Whether the sort is descending (leading `-` in the token).
    pub descending: bool,
}

/// A parsed JSON:API request query.
///
/// Filter and page values are kept as generic maps — JSON:API leaves their
/// semantics to the server. Unknown top-level parameters are ignored.
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use jsonapi_core::Query;
/// let q = Query::from_query_string("?sort=-created&page[size]=25")?;
/// assert!(q.sort[0].descending);
/// assert_eq!(q.page.get("size").map(String::as_str), Some("25"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    /// Parsed `sort` fields, in request order.
    pub sort: Vec<SortField>,
    /// Parsed `include` dot-paths, in request order.
    pub include: Vec<String>,
    /// Parsed `fields[type]` sparse-fieldset configuration.
    pub fields: FieldsetConfig,
    /// Generic `page[...]` parameters (strategy left to the server).
    pub page: BTreeMap<String, String>,
    /// Generic `filter[...]` parameters; repeated keys accumulate.
    pub filter: BTreeMap<String, Vec<String>>,
}

impl Query {
    /// Parse from already-decoded key/value pairs (e.g. an axum/actix param map).
    ///
    /// - Unknown top-level parameters (anything other than `sort`, `include`,
    ///   `fields[...]`, `page[...]`, and `filter[...]`) are silently ignored.
    /// - `filter[key]` repeated across multiple pairs accumulates all values
    ///   into a `Vec` in encounter order.
    /// - `page[key]` repeated across multiple pairs is **last-wins**: only the
    ///   final value is retained (the `page` map is `BTreeMap<String, String>`).
    #[must_use = "parsing result should be used"]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> crate::Result<Query> {
        let mut query = Query::default();
        for (key, value) in pairs {
            let key = *key;
            let value = *value;
            match key {
                "sort" => {
                    for token in value.split(',') {
                        let (field, descending) = match token.strip_prefix('-') {
                            Some(rest) => (rest, true),
                            None => (token, false),
                        };
                        if field.is_empty() {
                            return Err(crate::Error::QueryParse {
                                param: "sort".into(),
                                reason: "empty sort field".into(),
                            });
                        }
                        query.sort.push(SortField {
                            field: field.to_string(),
                            descending,
                        });
                    }
                }
                "include" => {
                    for path in value.split(',') {
                        if path.is_empty() {
                            return Err(crate::Error::QueryParse {
                                param: "include".into(),
                                reason: "empty include path".into(),
                            });
                        }
                        query.include.push(path.to_string());
                    }
                }
                _ if key.starts_with("fields[") => {
                    let type_name = bracket_inner(key, "fields")?;
                    let names: Vec<&str> = value.split(',').filter(|s| !s.is_empty()).collect();
                    // FieldsetConfig::fields is a consuming builder; take-and-replace avoids a clone.
                    query.fields = std::mem::take(&mut query.fields).fields(&type_name, &names);
                }
                _ if key.starts_with("page[") => {
                    let inner = bracket_inner(key, "page")?;
                    query.page.insert(inner, value.to_string());
                }
                _ if key.starts_with("filter[") => {
                    let inner = bracket_inner(key, "filter")?;
                    query
                        .filter
                        .entry(inner)
                        .or_default()
                        .push(value.to_string());
                }
                _ => { /* unknown top-level param: ignore */ }
            }
        }
        Ok(query)
    }

    /// Parse from a raw query string (`?a=b&c=d`), percent-decoding keys and values.
    ///
    /// - A leading `?` is stripped if present; passing an empty string or `"?"`
    ///   returns an empty [`Query`] without error.
    /// - Both keys and values are percent-decoded. `+` is **not** treated as a
    ///   space (use `%20` for spaces).
    /// - All semantics of [`Query::from_pairs`] apply after decoding.
    #[must_use = "parsing result should be used"]
    pub fn from_query_string(query: &str) -> crate::Result<Query> {
        let trimmed = query.strip_prefix('?').unwrap_or(query);
        if trimmed.is_empty() {
            return Ok(Query::default());
        }
        let mut decoded: Vec<(String, String)> = Vec::new();
        for pair in trimmed.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (raw_key, raw_val) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
            let key = decode(raw_key)?;
            let val = decode(raw_val)?;
            decoded.push((key, val));
        }
        let pairs: Vec<(&str, &str)> = decoded
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        Query::from_pairs(&pairs)
    }
}

/// Extract `inner` from a `prefix[inner]` key. Errors if the closing `]` is absent or the inner key is empty.
fn bracket_inner(key: &str, prefix: &str) -> crate::Result<String> {
    let after = &key[prefix.len() + 1..];
    match after.strip_suffix(']') {
        Some(inner) if !inner.is_empty() => Ok(inner.to_string()),
        _ => Err(crate::Error::QueryParse {
            param: key.to_string(),
            reason: format!("malformed `{prefix}[...]` parameter"),
        }),
    }
}

/// Percent-decode a query component to UTF-8. `+` is NOT treated as space.
fn decode(s: &str) -> crate::Result<String> {
    percent_decode_str(s)
        .decode_utf8()
        .map(|cow| cow.into_owned())
        .map_err(|e| crate::Error::QueryParse {
            param: s.to_string(),
            reason: format!("invalid percent-encoding: {e}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sort_with_descending() {
        let q = Query::from_pairs(&[("sort", "-created,title")]).unwrap();
        assert_eq!(q.sort.len(), 2);
        assert_eq!(
            q.sort[0],
            SortField {
                field: "created".into(),
                descending: true
            }
        );
        assert_eq!(
            q.sort[1],
            SortField {
                field: "title".into(),
                descending: false
            }
        );
    }

    #[test]
    fn empty_sort_token_is_error() {
        let err = Query::from_pairs(&[("sort", "a,,b")]).unwrap_err();
        assert!(matches!(err, crate::Error::QueryParse { .. }));
    }

    #[test]
    fn parses_include_paths() {
        let q = Query::from_pairs(&[("include", "author,comments.author")]).unwrap();
        assert_eq!(
            q.include,
            vec!["author".to_string(), "comments.author".to_string()]
        );
    }

    #[test]
    fn parses_fields_into_fieldset_config() {
        let q = Query::from_pairs(&[("fields[articles]", "title,body")]).unwrap();
        assert!(q.fields.has_type("articles"));
        assert!(q.fields.is_included("articles", "title"));
        assert!(q.fields.is_included("articles", "body"));
        assert!(!q.fields.is_included("articles", "secret"));
    }

    #[test]
    fn parses_page_into_generic_map() {
        let q = Query::from_pairs(&[("page[size]", "25"), ("page[after]", "abc")]).unwrap();
        assert_eq!(q.page.get("size").map(String::as_str), Some("25"));
        assert_eq!(q.page.get("after").map(String::as_str), Some("abc"));
    }

    #[test]
    fn parses_filter_accumulating_repeated_keys() {
        let q = Query::from_pairs(&[("filter[tag]", "a"), ("filter[tag]", "b")]).unwrap();
        assert_eq!(
            q.filter.get("tag").unwrap(),
            &vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn bracket_param_without_close_is_error() {
        let err = Query::from_pairs(&[("filter[tag", "a")]).unwrap_err();
        assert!(matches!(err, crate::Error::QueryParse { .. }));
    }

    #[test]
    fn unknown_top_level_param_is_ignored() {
        let q = Query::from_pairs(&[("unknown", "x"), ("sort", "title")]).unwrap();
        assert_eq!(q.sort.len(), 1);
    }

    #[test]
    fn from_query_string_matches_from_pairs_and_decodes() {
        let q = Query::from_query_string("?sort=-created&filter[name]=john%20doe").unwrap();
        assert_eq!(
            q.sort[0],
            SortField {
                field: "created".into(),
                descending: true
            }
        );
        assert_eq!(q.filter.get("name").unwrap(), &vec!["john doe".to_string()]);
    }

    #[test]
    fn from_query_string_empty_is_default() {
        let q = Query::from_query_string("").unwrap();
        assert_eq!(q, Query::default());
    }

    #[test]
    fn page_duplicate_key_is_last_wins_but_filter_accumulates() {
        let q = Query::from_pairs(&[
            ("page[size]", "10"),
            ("page[size]", "20"),
            ("filter[t]", "a"),
            ("filter[t]", "b"),
        ])
        .unwrap();
        assert_eq!(q.page.get("size").map(String::as_str), Some("20"));
        assert_eq!(
            q.filter.get("t").unwrap(),
            &vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn from_query_string_rejects_invalid_percent_encoding() {
        // %FF decodes to a lone 0xFF byte, which is not valid UTF-8.
        let err = Query::from_query_string("?sort=%FF").unwrap_err();
        assert!(matches!(err, crate::Error::QueryParse { .. }));
    }
}
