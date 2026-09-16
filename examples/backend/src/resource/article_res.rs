use jsonapi_core::{Field, Relationship, ResourceIdentifier};

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
    #[jsonapi(relationship)]
    pub author: Relationship<AuthorResource>,
    #[jsonapi(relationship)]
    pub tags: Relationship<TagResource>,
    #[jsonapi(relationship)]
    pub comments: Relationship<CommentResource>,
}

/// Write resource for creating a new article. `id` is optional so the client
/// may omit it and let the server assign one.
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles", case = "camelCase")]
pub struct NewArticleResource {
    #[jsonapi(id)]
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    #[jsonapi(relationship)]
    pub author: Relationship<AuthorResource>,
}

/// Partial-update resource for PATCH. Every attribute is a tri-state [`Field`]
/// so the handler can distinguish "absent" (leave unchanged) from "set".
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles", case = "camelCase")]
pub struct ArticlePatchResource {
    #[jsonapi(id)]
    pub id: String,
    pub title: Field<String>,
    pub body: Field<String>,
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
            tags: Relationship::to_many(ResourceIdentifier::many("tags", tag_ids.iter().cloned())),
            comments: Relationship::to_many(ResourceIdentifier::many(
                "comments",
                comment_ids.iter().cloned(),
            )),
        }
    }
}
