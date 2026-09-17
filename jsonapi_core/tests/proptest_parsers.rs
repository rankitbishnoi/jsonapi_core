//! Property-based coverage for the parsers: member-name validation, the query
//! builder <-> parser round-trip, and media-type parsing.
//!
//! These complement the example-based unit tests with two kinds of invariant:
//! *robustness* (never panic on arbitrary, untrusted input) and *round-trip*
//! (building/formatting then parsing reproduces the input). Both are hard to
//! cover exhaustively with hand-written cases.

use jsonapi_core::{
    FieldsetConfig, JsonApiMediaType, MemberNameKind, Query, QueryBuilder, SortField,
    negotiate_accept, validate_content_type, validate_member_name,
};
use proptest::prelude::*;

/// A safe query token (member/type/path name): leading letter, then
/// letters/digits/underscore. Free of the query-string separators (`,`, `&`,
/// `=`, `[`, `]`) so it survives the builder/parser unchanged.
fn arb_token() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,7}".prop_map(String::from)
}

/// A filter/page *value*: any short printable-ASCII run. The builder fully
/// percent-encodes values, so even reserved characters round-trip.
fn arb_value() -> impl Strategy<Value = String> {
    "[ -~]{1,8}".prop_map(String::from)
}

/// A URI-ish `ext`/`profile` token: no whitespace (the parser splits on it), no
/// `"` (the quote delimiter), no `;`/`,` (parameter/entry separators).
fn arb_uri_token() -> impl Strategy<Value = String> {
    "[A-Za-z0-9:/._~-]{1,10}".prop_map(String::from)
}

// ---------------------------------------------------------------------------
// Member-name validation
// ---------------------------------------------------------------------------

proptest! {
    /// Validation runs on untrusted wire input, so it must never panic —
    /// whatever bytes arrive.
    #[test]
    fn member_name_validation_never_panics(s in "(?s).*") {
        let _ = validate_member_name(&s);
    }

    /// Names drawn from the always-legal charset (leading letter, then
    /// letters/digits, no `:`/`@`) classify as a standard member name.
    #[test]
    fn valid_charset_names_are_standard(name in "[a-zA-Z][a-zA-Z0-9]{0,14}") {
        prop_assert!(matches!(
            validate_member_name(&name),
            Ok(MemberNameKind::Standard)
        ));
    }

    /// `@namespace:member` (both halves legal standard names) classifies as an
    /// `AtMember` split into its namespace and member.
    #[test]
    fn at_member_names_split_into_namespace_and_member(
        ns in "[a-z][a-z0-9]{0,7}",
        member in "[a-z][a-z0-9]{0,7}",
    ) {
        let name = format!("@{ns}:{member}");
        match validate_member_name(&name) {
            Ok(MemberNameKind::AtMember { namespace, member: m }) => {
                prop_assert_eq!(namespace, ns);
                prop_assert_eq!(m, member);
            }
            other => prop_assert!(false, "expected AtMember, got {:?}", other),
        }
    }
}

// ---------------------------------------------------------------------------
// Query builder <-> parser round-trip
// ---------------------------------------------------------------------------

proptest! {
    /// Whatever a `QueryBuilder` emits, `Query::from_query_string` parses back to
    /// the same logical query — exercising bracket formatting, comma joining, and
    /// RFC 3986 percent-encode/decode in one shot.
    ///
    /// Generators use distinct keys/types (via maps) because the parser's
    /// per-type / per-key accumulation is only unambiguous for distinct keys.
    #[test]
    fn query_builder_round_trips_through_parse(
        sorts in prop::collection::vec((arb_token(), any::<bool>()), 0..4),
        includes in prop::collection::vec(arb_token(), 0..4),
        fields in prop::collection::btree_map(
            arb_token(),
            prop::collection::vec(arb_token(), 1..4),
            0..3,
        ),
        pages in prop::collection::btree_map(arb_token(), arb_value(), 0..3),
        filters in prop::collection::btree_map(arb_token(), arb_value(), 0..3),
    ) {
        // --- build the query string ---
        let mut builder = QueryBuilder::new();
        if !sorts.is_empty() {
            let tokens: Vec<String> = sorts
                .iter()
                .map(|(field, desc)| if *desc { format!("-{field}") } else { field.clone() })
                .collect();
            let refs: Vec<&str> = tokens.iter().map(String::as_str).collect();
            builder = builder.sort(&refs);
        }
        if !includes.is_empty() {
            let refs: Vec<&str> = includes.iter().map(String::as_str).collect();
            builder = builder.include(&refs);
        }
        for (type_name, field_names) in &fields {
            let refs: Vec<&str> = field_names.iter().map(String::as_str).collect();
            builder = builder.fields(type_name, &refs);
        }
        for (key, value) in &filters {
            builder = builder.filter(key, value);
        }
        for (key, value) in &pages {
            builder = builder.page(key, value);
        }
        let query_string = builder.build();

        // --- the query we expect to parse back out ---
        let mut expected_fields = FieldsetConfig::new();
        for (type_name, field_names) in &fields {
            let refs: Vec<&str> = field_names.iter().map(String::as_str).collect();
            expected_fields = expected_fields.fields(type_name, &refs);
        }
        let expected = Query {
            sort: sorts
                .iter()
                .map(|(field, desc)| SortField { field: field.clone(), descending: *desc })
                .collect(),
            include: includes.clone(),
            fields: expected_fields,
            page: pages.clone(),
            filter: filters
                .iter()
                .map(|(key, value)| (key.clone(), vec![value.clone()]))
                .collect(),
        };

        let parsed = Query::from_query_string(&query_string)
            .expect("a builder-produced query string must parse");
        prop_assert_eq!(parsed, expected);
    }
}

// ---------------------------------------------------------------------------
// Media-type parsing
// ---------------------------------------------------------------------------

proptest! {
    /// The Content-Type / Accept parsers run on untrusted headers and must never
    /// panic on arbitrary input.
    #[test]
    fn media_type_parsing_never_panics(s in "(?s).*") {
        let _ = validate_content_type(&s);
        let _ = negotiate_accept(&s, &[], &[]);
        let _ = JsonApiMediaType::parse(&s);
    }

    /// A well-formed `application/vnd.api+json` header carrying `ext`/`profile`
    /// URIs parses to exactly those lists, and formatting then re-parsing is the
    /// identity.
    #[test]
    fn media_type_ext_profile_round_trips(
        ext in prop::collection::vec(arb_uri_token(), 0..3),
        profile in prop::collection::vec(arb_uri_token(), 0..3),
    ) {
        let mut header = String::from("application/vnd.api+json");
        if !ext.is_empty() {
            header.push_str(&format!("; ext=\"{}\"", ext.join(" ")));
        }
        if !profile.is_empty() {
            header.push_str(&format!("; profile=\"{}\"", profile.join(" ")));
        }

        let mt = JsonApiMediaType::parse(&header).expect("well-formed header must parse");
        prop_assert_eq!(&mt.ext, &ext);
        prop_assert_eq!(&mt.profile, &profile);

        let reparsed = JsonApiMediaType::parse(&mt.to_header_value())
            .expect("formatted media type must re-parse");
        prop_assert_eq!(reparsed, mt);
    }
}
