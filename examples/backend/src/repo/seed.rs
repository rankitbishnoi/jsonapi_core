use sqlx::SqlitePool;

/// Populate demo data with a deterministic, idempotent fixture.
/// Uses `INSERT OR IGNORE` so running this multiple times is safe.
pub async fn seed(pool: &SqlitePool) -> anyhow::Result<()> {
    // Authors
    for (id, name, email) in [
        ("a1", "Ada Lovelace", "ada@example.com"),
        ("a2", "Alan Turing", "alan@example.com"),
    ] {
        sqlx::query(
            "INSERT OR IGNORE INTO authors (id, name, email, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(email)
        .bind("2026-01-01T00:00:00Z")
        .execute(pool)
        .await?;
    }

    // Tags
    for (id, name) in [("t-rust", "rust"), ("t-web", "web"), ("t-api", "api")] {
        sqlx::query("INSERT OR IGNORE INTO tags (id, name) VALUES (?, ?)")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await?;
    }

    // Articles and their tags
    for i in 1i32..=12 {
        let article_id = format!("art-{i:02}");
        let author_id = if i % 2 != 0 { "a1" } else { "a2" };
        let title = format!("Article {i}");
        let body = format!("Body of article {i}.");
        let ts = format!("2026-02-{i:02}T00:00:00Z");

        sqlx::query(
            "INSERT OR IGNORE INTO articles \
             (id, title, body, author_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&article_id)
        .bind(&title)
        .bind(&body)
        .bind(author_id)
        .bind(&ts)
        .bind(&ts)
        .execute(pool)
        .await?;

        // Every article gets "t-rust"
        sqlx::query(
            "INSERT OR IGNORE INTO article_tags (article_id, tag_id) VALUES (?, ?)",
        )
        .bind(&article_id)
        .bind("t-rust")
        .execute(pool)
        .await?;

        // Even articles also get "t-web"; odd articles get "t-api"
        let second_tag = if i % 2 == 0 { "t-web" } else { "t-api" };
        sqlx::query(
            "INSERT OR IGNORE INTO article_tags (article_id, tag_id) VALUES (?, ?)",
        )
        .bind(&article_id)
        .bind(second_tag)
        .execute(pool)
        .await?;
    }

    // Comments on art-01
    for (id, author_id) in [("c1", "a2"), ("c2", "a1")] {
        let body = format!("Comment {id}");
        sqlx::query(
            "INSERT OR IGNORE INTO comments \
             (id, article_id, author_id, body, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind("art-01")
        .bind(author_id)
        .bind(&body)
        .bind("2026-01-01T00:00:00Z")
        .execute(pool)
        .await?;
    }

    Ok(())
}
