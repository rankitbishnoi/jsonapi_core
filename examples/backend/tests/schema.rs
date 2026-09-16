#[path = "support.rs"]
mod support;

use jsonapi_showcase_backend::state::AppState;

#[tokio::test]
async fn deleting_author_cascades_to_articles() {
    let state = AppState::in_memory().await.unwrap();
    sqlx::query("INSERT INTO authors (id, name, email, created_at) VALUES ('a1','A','a@x','t')")
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO articles (id, title, body, author_id, created_at, updated_at) VALUES ('x','T','B','a1','t','t')")
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM authors WHERE id='a1'")
        .execute(&state.pool)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM articles")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "article should cascade-delete with its author");
}
