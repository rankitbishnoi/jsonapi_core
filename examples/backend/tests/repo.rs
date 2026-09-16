#[path = "support.rs"]
mod support;

use jsonapi_showcase_backend::repo::article_repo::{self, ArticleQuery, ArticleSort, SortDir};
use jsonapi_showcase_backend::repo::seed::seed;
use jsonapi_showcase_backend::state::AppState;

#[tokio::test]
async fn list_paginates_and_sorts_by_title_desc() {
    let state = AppState::in_memory().await.unwrap();
    seed(&state.pool).await.unwrap();
    let q = ArticleQuery {
        limit: 5,
        offset: 0,
        sort: vec![ArticleSort::Title(SortDir::Desc)],
        author_id: None,
    };
    let page = article_repo::list(&state.pool, &q).await.unwrap();
    assert_eq!(page.len(), 5);
    assert_eq!(page[0].title, "Article 9"); // lexicographic desc among "Article N"
}

#[tokio::test]
async fn get_missing_article_is_row_not_found() {
    let state = AppState::in_memory().await.unwrap();
    seed(&state.pool).await.unwrap();
    let err = article_repo::get(&state.pool, "nope").await.unwrap_err();
    assert!(matches!(err, sqlx::Error::RowNotFound));
}

#[tokio::test]
async fn filter_by_author_counts_correctly() {
    let state = AppState::in_memory().await.unwrap();
    seed(&state.pool).await.unwrap();
    let n = article_repo::count(&state.pool, Some("a1")).await.unwrap();
    assert_eq!(n, 6); // odd-numbered articles belong to a1
}
