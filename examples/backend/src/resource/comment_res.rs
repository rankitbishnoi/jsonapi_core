use jsonapi_core::Relationship;

use crate::domain::Comment;
use crate::resource::{ArticleResource, AuthorResource};

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "comments", case = "camelCase")]
pub struct CommentResource {
    #[jsonapi(id)]
    pub id: String,
    pub body: String,
    pub created_at: String,
    #[jsonapi(relationship, type = "authors")]
    pub author: Relationship<AuthorResource>,
    #[jsonapi(relationship, type = "articles")]
    pub article: Relationship<ArticleResource>,
}

impl From<Comment> for CommentResource {
    fn from(c: Comment) -> Self {
        Self {
            id: c.id,
            body: c.body,
            created_at: c.created_at,
            author: Relationship::to_one_id("authors", c.author_id),
            article: Relationship::to_one_id("articles", c.article_id),
        }
    }
}
