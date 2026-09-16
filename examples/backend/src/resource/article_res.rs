use jsonapi_core::{Relationship, ResourceIdentifier};

use crate::domain::Article;
use crate::resource::{AuthorResource, CommentResource, TagResource};

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles", case = "camelCase")]
pub struct ArticleResource {
    #[jsonapi(id)]
    pub id: String,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    #[jsonapi(relationship, type = "authors")]
    pub author: Relationship<AuthorResource>,
    #[jsonapi(relationship, type = "tags")]
    pub tags: Relationship<TagResource>,
    #[jsonapi(relationship, type = "comments")]
    pub comments: Relationship<CommentResource>,
}

impl ArticleResource {
    pub fn from_parts(a: Article, tag_ids: &[String], comment_ids: &[String]) -> Self {
        Self {
            id: a.id,
            title: a.title,
            body: a.body,
            created_at: a.created_at,
            updated_at: a.updated_at,
            author: Relationship::to_one_id("authors", a.author_id),
            tags: Relationship::to_many(
                tag_ids
                    .iter()
                    .map(|id| ResourceIdentifier::new("tags", id.clone())),
            ),
            comments: Relationship::to_many(
                comment_ids
                    .iter()
                    .map(|id| ResourceIdentifier::new("comments", id.clone())),
            ),
        }
    }
}
