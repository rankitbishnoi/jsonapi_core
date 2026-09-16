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

/// Replace all tags for an article atomically.
pub async fn replace_article_tags(
    pool: &SqlitePool,
    article_id: &str,
    tag_ids: &[String],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM article_tags WHERE article_id = ?")
        .bind(article_id)
        .execute(&mut *tx)
        .await?;
    for tag_id in tag_ids {
        sqlx::query("INSERT OR IGNORE INTO article_tags (article_id, tag_id) VALUES (?, ?)")
            .bind(article_id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}

/// Add tags to an article (idempotent — ignores existing links).
pub async fn add_article_tags(
    pool: &SqlitePool,
    article_id: &str,
    tag_ids: &[String],
) -> Result<(), sqlx::Error> {
    for tag_id in tag_ids {
        sqlx::query("INSERT OR IGNORE INTO article_tags (article_id, tag_id) VALUES (?, ?)")
            .bind(article_id)
            .bind(tag_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Remove specific tags from an article.
pub async fn remove_article_tags(
    pool: &SqlitePool,
    article_id: &str,
    tag_ids: &[String],
) -> Result<(), sqlx::Error> {
    for tag_id in tag_ids {
        sqlx::query("DELETE FROM article_tags WHERE article_id = ? AND tag_id = ?")
            .bind(article_id)
            .bind(tag_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}
