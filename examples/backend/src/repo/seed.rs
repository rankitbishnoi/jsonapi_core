use sqlx::SqlitePool;

/// Populate demo data. Real fixture lands in Task 3.
pub async fn seed(_pool: &SqlitePool) -> anyhow::Result<()> {
    Ok(())
}
