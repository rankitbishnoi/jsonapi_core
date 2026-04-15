//! Build JSON:API query strings with QueryBuilder.
//!
//! Run: `cargo run -p jsonapi_core --example query_builder`

use jsonapi_core::QueryBuilder;

fn main() {
    // Compound query: filter + sort + include + sparse fieldsets + pagination
    let qs = QueryBuilder::new()
        .filter("author", "dan")
        .filter("published", "true")
        .sort(&["-created", "title"])
        .include(&["author", "comments", "comments.author"])
        .fields("articles", &["title", "body"])
        .fields("people", &["name"])
        .page("number", "1")
        .page("size", "25")
        .build();

    println!("Compound query:");
    println!("  ?{qs}");
    println!();

    // Simple query: just a filter
    let qs = QueryBuilder::new().filter("search", "hello world").build();

    println!("Filter with spaces (percent-encoded):");
    println!("  ?{qs}");
    println!();

    // Arbitrary parameter via param() escape hatch
    let qs = QueryBuilder::new()
        .param("api_key", "abc123")
        .include(&["author"])
        .build();

    println!("Custom parameter:");
    println!("  ?{qs}");
}
