use jsonapi_core::Relationship;

use crate::article::ArticleResource;
use crate::author::AuthorResource;

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "comments", case = "camelCase")]
pub struct CommentResource {
    #[jsonapi(id)]
    pub id: String,
    pub body: String,
    pub created_at: String,
    #[jsonapi(relationship)]
    pub author: Relationship<AuthorResource>,
    #[jsonapi(relationship)]
    pub article: Relationship<ArticleResource>,
}
