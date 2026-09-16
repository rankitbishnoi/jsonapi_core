//! End-to-end server flow: parse a request query, then build a paginated
//! compound response with DocumentBuilder + CursorLinks against realistic data.

use jsonapi_core::{
    CURSOR_PAGINATION_PROFILE, CursorLinks, CursorPage, Document, DocumentBuilder, PrimaryData,
    Query, Resource,
};

const RICH: &str = include_str!("fixtures/rich_article.json");

#[test]
fn parse_query_then_build_paginated_response() {
    // 1. Incoming request query.
    let q = Query::from_query_string("?include=field_section&page[size]=2&sort=-created").unwrap();
    let cursor = CursorPage::from_query(&q).unwrap();
    assert_eq!(cursor.size, Some(2));

    // 2. Load domain data — reuse the rich fixture's included resources as a "page".
    let source: Document<Resource> = serde_json::from_str(RICH).unwrap();
    let included = match &source {
        Document::Data { included, .. } => included.clone(),
        _ => panic!("fixture is a data document"),
    };
    let page: Vec<Resource> = included.into_iter().take(2).collect();
    assert_eq!(page.len(), 2);

    // 3. Build a paginated compound response carrying the cursor-pagination profile.
    let links = CursorLinks::new("/articles")
        .size(cursor.size.unwrap())
        .first()
        .next("nextcur")
        .links();
    let doc = DocumentBuilder::collection(page)
        .links(links)
        .profile(CURSOR_PAGINATION_PROFILE)
        .build();

    // 4. Assert the emitted document's shape.
    let v = serde_json::to_value(&doc).unwrap();
    assert!(v["data"].is_array());
    assert_eq!(v["jsonapi"]["profile"][0], CURSOR_PAGINATION_PROFILE);
    assert!(v["links"]["first"].is_string());
    assert!(
        v["links"]["next"]
            .as_str()
            .unwrap()
            .contains("page[after]=nextcur"),
        "next link: {}",
        v["links"]["next"]
    );

    // 5. The built document round-trips back into a typed Document.
    let reparsed: Document<Resource> = serde_json::from_str(&v.to_string()).unwrap();
    assert!(matches!(
        reparsed,
        Document::Data {
            data: PrimaryData::Many(_),
            ..
        }
    ));
}
