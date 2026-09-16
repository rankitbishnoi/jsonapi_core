use jsonapi_showcase_backend::domain::Author;
use jsonapi_showcase_backend::resource::AuthorResource;

#[test]
fn author_serializes_with_camelcase_and_type() {
    let res = AuthorResource::from(Author {
        id: "a1".into(),
        name: "Ada".into(),
        email: "ada@example.com".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
    });
    let doc = jsonapi_axum::DocumentBuilder::single(res).build();
    let value = serde_json::to_value(&doc).unwrap();
    assert_eq!(value["data"]["type"], "authors");
    assert_eq!(value["data"]["id"], "a1");
    assert_eq!(
        value["data"]["attributes"]["createdAt"],
        "2026-01-01T00:00:00Z"
    );
    assert!(value["data"]["attributes"].get("created_at").is_none());
}

#[test]
fn article_serializes_to_many_tags_linkage() {
    use jsonapi_showcase_backend::domain::Article;
    use jsonapi_showcase_backend::resource::ArticleResource;
    let a = Article {
        id: "art-01".into(),
        title: "T".into(),
        body: "B".into(),
        author_id: "a1".into(),
        created_at: "t".into(),
        updated_at: "t".into(),
    };
    let res = ArticleResource::from_parts(
        a,
        &["t-rust".to_string(), "t-web".to_string()],
        &["c1".to_string()],
    );
    let value = serde_json::to_value(jsonapi_axum::DocumentBuilder::single(res).build()).unwrap();
    assert_eq!(value["data"]["relationships"]["author"]["data"]["id"], "a1");
    assert_eq!(
        value["data"]["relationships"]["author"]["data"]["type"],
        "authors"
    );
    let tags = value["data"]["relationships"]["tags"]["data"]
        .as_array()
        .unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0]["type"], "tags");
    assert_eq!(value["data"]["attributes"]["updatedAt"], "t");
    assert!(value["data"]["attributes"].get("updated_at").is_none());
    let comments = value["data"]["relationships"]["comments"]["data"]
        .as_array()
        .unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["type"], "comments");
    assert_eq!(comments[0]["id"], "c1");
}

#[test]
fn comment_serializes_to_one_relationships() {
    use jsonapi_showcase_backend::domain::Comment;
    use jsonapi_showcase_backend::resource::CommentResource;
    let c = Comment {
        id: "c1".into(),
        article_id: "art-01".into(),
        author_id: "a2".into(),
        body: "Hi".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
    };
    let value = serde_json::to_value(
        jsonapi_axum::DocumentBuilder::single(CommentResource::from(c)).build(),
    )
    .unwrap();
    assert_eq!(value["data"]["type"], "comments");
    assert_eq!(
        value["data"]["relationships"]["author"]["data"]["type"],
        "authors"
    );
    assert_eq!(value["data"]["relationships"]["author"]["data"]["id"], "a2");
    assert_eq!(
        value["data"]["relationships"]["article"]["data"]["type"],
        "articles"
    );
    assert_eq!(
        value["data"]["relationships"]["article"]["data"]["id"],
        "art-01"
    );
    assert_eq!(
        value["data"]["attributes"]["createdAt"],
        "2026-01-01T00:00:00Z"
    );
}
