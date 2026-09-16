use sqlx::SqlitePool;

use crate::domain::Author;

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Author, sqlx::Error> {
    sqlx::query_as::<_, Author>(
        "SELECT id, name, email, created_at FROM authors WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<Author>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, name, email, created_at FROM authors WHERE id IN ({placeholders})"
    );
    let mut query = sqlx::query_as::<_, Author>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    query.fetch_all(pool).await
}
