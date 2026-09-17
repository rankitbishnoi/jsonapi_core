#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Comment {
    pub id: String,
    pub article_id: String,
    pub author_id: String,
    pub body: String,
    pub created_at: String,
}
