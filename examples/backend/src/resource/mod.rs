pub mod article_res;
pub mod author_res;
pub mod comment_res;
pub mod tag_res;

pub use article_res::{ArticlePatchResource, ArticleResource, NewArticleResource};
pub use author_res::AuthorResource;
pub use comment_res::CommentResource;
pub use tag_res::TagResource;

use jsonapi_axum::TypeRegistry;

/// Build a [`TypeRegistry`] populated with all resource types used by this app.
///
/// Registered so that [`jsonapi_axum::JsonApiQueryValidated`] can validate
/// include paths end-to-end (e.g. `comments.author`).
pub fn type_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry
        .register::<ArticleResource>()
        .register::<AuthorResource>()
        .register::<TagResource>()
        .register::<CommentResource>();
    registry
}
