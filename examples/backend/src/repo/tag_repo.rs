use std::collections::BTreeMap;

use sqlx::SqlitePool;

use crate::domain::Tag;

pub async fn ids_for_article(
    pool: &SqlitePool,
    article_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT tag_id FROM article_tags WHERE article_id = ? ORDER BY tag_id")
        .bind(article_id)
        .fetch_all(pool)
        .await
}

pub async fn ids_for_articles(
    pool: &SqlitePool,
    article_ids: &[String],
) -> Result<BTreeMap<String, Vec<String>>, sqlx::Error> {
    if article_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let placeholders = std::iter::repeat_n("?", article_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT article_id, tag_id FROM article_tags WHERE article_id IN ({placeholders}) ORDER BY tag_id"
    );
    let mut query = sqlx::query_as::<_, (String, String)>(&sql);
    for id in article_ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(pool).await?;
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (article_id, tag_id) in rows {
        map.entry(article_id).or_default().push(tag_id);
    }
    Ok(map)
}

pub async fn by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<Tag>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT id, name FROM tags WHERE id IN ({placeholders})");
    let mut query = sqlx::query_as::<_, Tag>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    query.fetch_all(pool).await
}
