//! Domain-to-resource conversions for the showcase backend.
//!
//! These impls live here rather than in the `jsonapi-showcase-resources` crate
//! because of the orphan rule: a `From<X> for Y` impl is only legal in a crate
//! that owns at least one of `X` or `Y`. The domain types (`Author`, `Article`,
//! `Comment`, `Tag`) are local to the backend, and the resource types
//! (`AuthorResource`, etc.) are foreign (defined in `jsonapi-showcase-resources`),
//! so the backend is the only valid home for these conversions. The same
//! constraint applies to `article_resource_from_parts`, which is a free function
//! here because an inherent `impl ArticleResource` block is not legal on a
//! foreign type.

use jsonapi_core::{Relationship, ResourceIdentifier};

use crate::domain::{Article, Author, Comment, Tag};
use crate::resource::{ArticleResource, AuthorResource, CommentResource, TagResource};

impl From<Author> for AuthorResource {
    fn from(a: Author) -> Self {
        Self {
            id: a.id,
            name: a.name,
            email: a.email,
            created_at: a.created_at,
        }
    }
}

impl From<Tag> for TagResource {
    fn from(t: Tag) -> Self {
        Self {
            id: t.id,
            name: t.name,
        }
    }
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

/// Build an [`ArticleResource`] from a domain [`Article`] plus its related ids.
pub fn article_resource_from_parts(
    a: Article,
    tag_ids: &[String],
    comment_ids: &[String],
) -> ArticleResource {
    ArticleResource {
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
