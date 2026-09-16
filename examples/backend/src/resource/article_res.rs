use jsonapi_core::{Identity, Relationship, RelationshipData, ResourceIdentifier};

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

impl ArticleResource {
    pub fn from_parts(a: Article, tag_ids: &[String], comment_ids: &[String]) -> Self {
        let author = Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            r#type: "authors".to_string(),
            identity: Identity::Id(a.author_id),
            meta: None,
        })));

        let tags = Relationship::new(RelationshipData::ToMany(
            tag_ids
                .iter()
                .map(|id| ResourceIdentifier {
                    r#type: "tags".to_string(),
                    identity: Identity::Id(id.clone()),
                    meta: None,
                })
                .collect(),
        ));

        let comments = Relationship::new(RelationshipData::ToMany(
            comment_ids
                .iter()
                .map(|id| ResourceIdentifier {
                    r#type: "comments".to_string(),
                    identity: Identity::Id(id.clone()),
                    meta: None,
                })
                .collect(),
        ));

        Self {
            id: a.id,
            title: a.title,
            body: a.body,
            created_at: a.created_at,
            updated_at: a.updated_at,
            author,
            tags,
            comments,
        }
    }
}
