//! Framework-free JSON:API wire types shared by the showcase backend and the
//! Leptos WASM frontend. Depends only on `jsonapi_core` — compiles to native
//! and `wasm32-unknown-unknown`.

pub mod article;
pub mod author;
pub mod comment;
pub mod tag;

pub use article::{ArticlePatchResource, ArticleResource, NewArticleResource};
pub use author::AuthorResource;
pub use comment::CommentResource;
pub use tag::TagResource;
