#[path = "support.rs"]
mod support;

use jsonapi_showcase_backend::repo::seed::seed;
use jsonapi_showcase_backend::state::AppState;

#[tokio::test]
async fn seed_is_idempotent_and_populates_articles() {
    let state = AppState::in_memory().await.unwrap();
    seed(&state.pool).await.unwrap();
    seed(&state.pool).await.unwrap(); // second run must not duplicate
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM articles")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 12);
}
