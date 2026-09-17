use jsonapi_core::{Field, Relationship};

use crate::author::AuthorResource;
use crate::comment::CommentResource;
use crate::tag::TagResource;

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
#[derive(Debug, Clone, jsonapi_core::JsonApi, validator::Validate)]
#[jsonapi(type = "articles", case = "camelCase")]
pub struct NewArticleResource {
    #[jsonapi(id)]
    pub id: Option<String>,
    #[validate(length(min = 1, max = 200, message = "title must be 1..=200 chars"))]
    pub title: String,
    #[validate(length(min = 1, message = "body must not be empty"))]
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
