use jsonapi_core::{Identity, Relationship, RelationshipData, ResourceIdentifier};

use crate::domain::Comment;
use crate::resource::{ArticleResource, AuthorResource};

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

impl From<Comment> for CommentResource {
    fn from(c: Comment) -> Self {
        let author = Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            r#type: "authors".to_string(),
            identity: Identity::Id(c.author_id),
            meta: None,
        })));
        let article = Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            r#type: "articles".to_string(),
            identity: Identity::Id(c.article_id),
            meta: None,
        })));
        Self {
            id: c.id,
            body: c.body,
            created_at: c.created_at,
            author,
            article,
        }
    }
}
