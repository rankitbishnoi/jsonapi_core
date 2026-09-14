//! JSON:API query string builder.
//!
//! [`QueryBuilder`] produces spec-compliant query strings with bracket encoding
//! for `filter[]`, `fields[]`, and `page[]` parameters, and RFC 3986
//! percent-encoding for values. Commas in sort/include/fields are preserved as
//! delimiters; commas in filter/page/param values are encoded.

use percent_encoding::{AsciiSet, CONTROLS};

/// Percent-encode everything except RFC 3986 unreserved characters:
/// `A-Z a-z 0-9 - _ . ~`
const UNRESERVED_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// A method-chaining query string builder for JSON:API requests.
///
/// Produces spec-compliant query strings with bracket-formatted parameter
/// names and RFC 3986 percent-encoded values.
#[must_use]
#[derive(Debug, Clone)]
pub struct QueryBuilder {
    params: Vec<(String, String, bool)>,
}

impl QueryBuilder {
    /// Create an empty query builder.
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Add a filter parameter: `filter[key]=value`.
    pub fn filter(mut self, key: &str, value: &str) -> Self {
        self.params
            .push((format!("filter[{key}]"), value.into(), false));
        self
    }

    /// Add a sort parameter: `sort=field1,-field2`.
    ///
    /// Descending fields use `-` prefix convention: `&["-created", "title"]`.
    pub fn sort(mut self, fields: &[&str]) -> Self {
        self.params.push(("sort".into(), fields.join(","), true));
        self
    }

    /// Add an include parameter: `include=path1,path2`.
    pub fn include(mut self, paths: &[&str]) -> Self {
        self.params.push(("include".into(), paths.join(","), true));
        self
    }

    /// Add a sparse fieldset parameter: `fields[type]=field1,field2`.
    pub fn fields(mut self, type_: &str, fields: &[&str]) -> Self {
        self.params
            .push((format!("fields[{type_}]"), fields.join(","), true));
        self
    }

    /// Add a pagination parameter: `page[key]=value`.
    pub fn page(mut self, key: &str, value: &str) -> Self {
        self.params
            .push((format!("page[{key}]"), value.into(), false));
        self
    }

    /// Add an arbitrary key=value parameter. No bracket wrapping.
    ///
    /// The key is passed through as-is (caller's responsibility).
    pub fn param(mut self, key: &str, value: &str) -> Self {
        self.params.push((key.into(), value.into(), false));
        self
    }

    /// Build the query string. Returns the string without a leading `?`.
    ///
    /// Returns `""` for an empty builder.
    #[must_use]
    pub fn build(&self) -> String {
        self.params
            .iter()
            .map(|(key, value, preserve_commas)| {
                let encoded_value = if *preserve_commas {
                    value
                        .split(',')
                        .map(|segment| {
                            percent_encoding::utf8_percent_encode(segment, UNRESERVED_ENCODE_SET)
                                .to_string()
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                } else {
                    percent_encoding::utf8_percent_encode(value, UNRESERVED_ENCODE_SET).to_string()
                };
                format!("{key}={encoded_value}")
            })
            .collect::<Vec<_>>()
            .join("&")
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_builder_returns_empty_string() {
        let qs = QueryBuilder::new().build();
        assert_eq!(qs, "");
    }

    #[test]
    fn single_filter() {
        let qs = QueryBuilder::new().filter("name", "john").build();
        assert_eq!(qs, "filter[name]=john");
    }

    #[test]
    fn multiple_filters() {
        let qs = QueryBuilder::new()
            .filter("name", "john")
            .filter("age", "30")
            .build();
        assert_eq!(qs, "filter[name]=john&filter[age]=30");
    }

    #[test]
    fn sort_single_field() {
        let qs = QueryBuilder::new().sort(&["title"]).build();
        assert_eq!(qs, "sort=title");
    }

    #[test]
    fn sort_multiple_with_descending() {
        let qs = QueryBuilder::new().sort(&["-created", "title"]).build();
        assert_eq!(qs, "sort=-created,title");
    }

    #[test]
    fn include_single_path() {
        let qs = QueryBuilder::new().include(&["author"]).build();
        assert_eq!(qs, "include=author");
    }

    #[test]
    fn include_multiple_paths() {
        let qs = QueryBuilder::new()
            .include(&["author", "comments.author"])
            .build();
        assert_eq!(qs, "include=author,comments.author");
    }

    #[test]
    fn fields_sparse_fieldset() {
        let qs = QueryBuilder::new()
            .fields("articles", &["title", "body"])
            .build();
        assert_eq!(qs, "fields[articles]=title,body");
    }

    #[test]
    fn fields_multiple_types() {
        let qs = QueryBuilder::new()
            .fields("articles", &["title"])
            .fields("people", &["name"])
            .build();
        assert_eq!(qs, "fields[articles]=title&fields[people]=name");
    }

    #[test]
    fn page_parameter() {
        let qs = QueryBuilder::new().page("size", "25").build();
        assert_eq!(qs, "page[size]=25");
    }

    #[test]
    fn page_multiple() {
        let qs = QueryBuilder::new()
            .page("size", "25")
            .page("number", "2")
            .build();
        assert_eq!(qs, "page[size]=25&page[number]=2");
    }

    #[test]
    fn raw_param() {
        let qs = QueryBuilder::new().param("custom", "value").build();
        assert_eq!(qs, "custom=value");
    }

    #[test]
    fn raw_param_with_brackets() {
        let qs = QueryBuilder::new().param("stats[total]", "true").build();
        assert_eq!(qs, "stats[total]=true");
    }

    #[test]
    fn value_with_spaces_encoded() {
        let qs = QueryBuilder::new().filter("name", "john doe").build();
        assert_eq!(qs, "filter[name]=john%20doe");
    }

    #[test]
    fn value_with_special_chars_encoded() {
        let qs = QueryBuilder::new().filter("q", "a&b=c").build();
        assert_eq!(qs, "filter[q]=a%26b%3Dc");
    }

    #[test]
    fn unreserved_chars_not_encoded() {
        let qs = QueryBuilder::new().filter("q", "a-b_c.d~e").build();
        assert_eq!(qs, "filter[q]=a-b_c.d~e");
    }

    #[test]
    fn unicode_value_encoded() {
        let qs = QueryBuilder::new().filter("name", "café").build();
        // UTF-8 bytes for é: 0xC3 0xA9
        assert_eq!(qs, "filter[name]=caf%C3%A9");
    }

    #[test]
    fn brackets_in_keys_are_literal() {
        let qs = QueryBuilder::new().filter("name", "x").build();
        assert!(qs.contains("filter[name]"));
        assert!(!qs.contains("%5B"));
        assert!(!qs.contains("%5D"));
    }

    #[test]
    fn method_chaining_preserves_insertion_order() {
        let qs = QueryBuilder::new()
            .include(&["author"])
            .filter("status", "published")
            .sort(&["-created"])
            .page("size", "10")
            .fields("articles", &["title"])
            .build();
        assert_eq!(
            qs,
            "include=author&filter[status]=published&sort=-created&page[size]=10&fields[articles]=title"
        );
    }

    #[test]
    fn commas_in_sort_are_literal() {
        let qs = QueryBuilder::new().sort(&["a", "b", "c"]).build();
        assert_eq!(qs, "sort=a,b,c");
        assert!(!qs.contains("%2C"));
    }

    #[test]
    fn commas_in_filter_values_are_encoded() {
        let qs = QueryBuilder::new().filter("tags", "a,b").build();
        assert_eq!(qs, "filter[tags]=a%2Cb");
    }

    #[test]
    fn commas_in_param_values_are_encoded() {
        let qs = QueryBuilder::new().param("q", "a,b,c").build();
        assert_eq!(qs, "q=a%2Cb%2Cc");
    }
}
