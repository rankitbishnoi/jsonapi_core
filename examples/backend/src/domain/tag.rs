#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Tag {
    pub id: String,
    pub name: String,
}
