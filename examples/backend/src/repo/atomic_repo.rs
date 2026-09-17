//! SQL helpers for atomic operations — one function per mutation, each
//! accepting a `&mut SqliteConnection` so the caller controls the transaction.

use sqlx::SqliteConnection;

use crate::domain::{Article, Author};

// ── Author mutations ──────────────────────────────────────────────────────────

/// Insert a new author and return the inserted row.
pub async fn create_author(
    conn: &mut SqliteConnection,
    id: &str,
    name: &str,
    email: &str,
    now: &str,
) -> Result<Author, sqlx::Error> {
    sqlx::query("INSERT INTO authors (id, name, email, created_at) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(name)
        .bind(email)
        .bind(now)
        .execute(&mut *conn)
        .await?;

    sqlx::query_as::<_, Author>("SELECT id, name, email, created_at FROM authors WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *conn)
        .await
}

/// Update an author's `name` and/or `email` (both optional) and return the
/// refreshed row. Returns `RowNotFound` when the author does not exist.
pub async fn update_author(
    conn: &mut SqliteConnection,
    id: &str,
    name: Option<&str>,
    email: Option<&str>,
) -> Result<Author, sqlx::Error> {
    if let Some(n) = name {
        sqlx::query("UPDATE authors SET name = ? WHERE id = ?")
            .bind(n)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    if let Some(e) = email {
        sqlx::query("UPDATE authors SET email = ? WHERE id = ?")
            .bind(e)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }

    sqlx::query_as::<_, Author>("SELECT id, name, email, created_at FROM authors WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *conn)
        .await
}

/// Delete an author. Returns `RowNotFound` when the author does not exist.
pub async fn remove_author(conn: &mut SqliteConnection, id: &str) -> Result<(), sqlx::Error> {
    let result = sqlx::query("DELETE FROM authors WHERE id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

// ── Article mutations ─────────────────────────────────────────────────────────

/// Insert a new article and return the inserted row.
pub async fn create_article(
    conn: &mut SqliteConnection,
    id: &str,
    title: &str,
    body: &str,
    author_id: &str,
    now: &str,
) -> Result<Article, sqlx::Error> {
    sqlx::query(
        "INSERT INTO articles (id, title, body, author_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(title)
    .bind(body)
    .bind(author_id)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;

    sqlx::query_as::<_, Article>(
        "SELECT id, title, body, author_id, created_at, updated_at FROM articles WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&mut *conn)
    .await
}

/// Update an article's `title` and/or `body` (both optional), refresh
/// `updated_at`, and return the refreshed row. Returns `RowNotFound` when the
/// article does not exist.
pub async fn update_article(
    conn: &mut SqliteConnection,
    id: &str,
    title: Option<&str>,
    body: Option<&str>,
    now: &str,
) -> Result<Article, sqlx::Error> {
    if let Some(t) = title {
        sqlx::query("UPDATE articles SET title = ?, updated_at = ? WHERE id = ?")
            .bind(t)
            .bind(now)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    if let Some(b) = body {
        sqlx::query("UPDATE articles SET body = ?, updated_at = ? WHERE id = ?")
            .bind(b)
            .bind(now)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }

    sqlx::query_as::<_, Article>(
        "SELECT id, title, body, author_id, created_at, updated_at FROM articles WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&mut *conn)
    .await
}

/// Delete an article. Returns `RowNotFound` when the article does not exist.
pub async fn remove_article(conn: &mut SqliteConnection, id: &str) -> Result<(), sqlx::Error> {
    let result = sqlx::query("DELETE FROM articles WHERE id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}
