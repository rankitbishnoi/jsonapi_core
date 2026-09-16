#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Author {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: String,
}
