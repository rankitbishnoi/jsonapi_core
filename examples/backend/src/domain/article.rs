use jsonapi_core::Field;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Article {
    pub id: String,
    pub title: String,
    pub body: String,
    pub author_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewArticle {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub author_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct ArticlePatch {
    pub title: Field<String>,
    pub body: Field<String>,
}
