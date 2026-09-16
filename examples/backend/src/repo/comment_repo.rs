use sqlx::SqlitePool;

use crate::domain::Comment;

pub async fn ids_for_article(
    pool: &SqlitePool,
    article_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM comments WHERE article_id = ? ORDER BY id")
        .bind(article_id)
        .fetch_all(pool)
        .await
}

pub async fn by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<Comment>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, article_id, author_id, body, created_at FROM comments WHERE id IN ({placeholders})"
    );
    let mut query = sqlx::query_as::<_, Comment>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    query.fetch_all(pool).await
}
