use sqlx::SqlitePool;

use crate::domain::Article;

#[derive(Debug, Clone, Copy)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy)]
pub enum ArticleSort {
    CreatedAt(SortDir),
    Title(SortDir),
}

impl ArticleSort {
    fn order_clause(self) -> &'static str {
        match self {
            ArticleSort::CreatedAt(SortDir::Asc) => "created_at ASC, id ASC",
            ArticleSort::CreatedAt(SortDir::Desc) => "created_at DESC, id DESC",
            ArticleSort::Title(SortDir::Asc) => "title ASC, id ASC",
            ArticleSort::Title(SortDir::Desc) => "title DESC, id DESC",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArticleQuery {
    pub limit: i64,
    pub offset: i64,
    pub sort: Vec<ArticleSort>,
    pub author_id: Option<String>,
}

impl Default for ArticleQuery {
    fn default() -> Self {
        Self {
            limit: 10,
            offset: 0,
            sort: vec![ArticleSort::CreatedAt(SortDir::Asc)],
            author_id: None,
        }
    }
}

pub async fn count(pool: &SqlitePool, author_id: Option<&str>) -> Result<i64, sqlx::Error> {
    match author_id {
        Some(id) => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM articles WHERE author_id = ?")
                .bind(id)
                .fetch_one(pool)
                .await
        }
        None => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM articles")
                .fetch_one(pool)
                .await
        }
    }
}

pub async fn list(pool: &SqlitePool, q: &ArticleQuery) -> Result<Vec<Article>, sqlx::Error> {
    let order = if q.sort.is_empty() {
        "created_at ASC, id ASC".to_owned()
    } else {
        q.sort
            .iter()
            .map(|s| s.order_clause())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let sql = match &q.author_id {
        Some(_) => format!(
            "SELECT id, title, body, author_id, created_at, updated_at \
             FROM articles WHERE author_id = ? ORDER BY {order} LIMIT ? OFFSET ?"
        ),
        None => format!(
            "SELECT id, title, body, author_id, created_at, updated_at \
             FROM articles ORDER BY {order} LIMIT ? OFFSET ?"
        ),
    };

    let mut query = sqlx::query_as::<_, Article>(&sql);
    if let Some(id) = &q.author_id {
        query = query.bind(id);
    }
    query = query.bind(q.limit).bind(q.offset);
    query.fetch_all(pool).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Article, sqlx::Error> {
    sqlx::query_as::<_, Article>(
        "SELECT id, title, body, author_id, created_at, updated_at \
         FROM articles WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn list_after(
    pool: &SqlitePool,
    after: Option<&str>,
    limit: i64,
) -> Result<Vec<Article>, sqlx::Error> {
    match after {
        Some(cursor) => {
            sqlx::query_as::<_, Article>(
                "SELECT id, title, body, author_id, created_at, updated_at \
             FROM articles WHERE id > ? ORDER BY id ASC LIMIT ?",
            )
            .bind(cursor)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, Article>(
                "SELECT id, title, body, author_id, created_at, updated_at \
             FROM articles ORDER BY id ASC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}
