pub mod conv;

pub use jsonapi_showcase_resources::{
    ArticlePatchResource, ArticleResource, AuthorResource, CommentResource, NewArticleResource,
    TagResource,
};

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
