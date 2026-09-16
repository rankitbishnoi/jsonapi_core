use std::future::Future;

use jsonapi_axum::{IncludeResolver, JsonApiError};
use jsonapi_core::Resource;

use crate::repo::{author_repo, comment_repo, tag_repo};
use crate::resource::{AuthorResource, CommentResource, TagResource};
use crate::state::AppState;

pub struct DbIncludeResolver<'a> {
    pub state: &'a AppState,
}

impl IncludeResolver for DbIncludeResolver<'_> {
    type Error = JsonApiError;

    fn load(
        &self,
        type_name: &str,
        ids: &[String],
    ) -> impl Future<Output = Result<Vec<Resource>, Self::Error>> + Send {
        let pool = self.state.pool.clone();
        let type_name = type_name.to_string();
        let ids = ids.to_vec();

        async move {
            match type_name.as_str() {
                "authors" => {
                    let authors = author_repo::by_ids(&pool, &ids).await?;
                    authors
                        .into_iter()
                        .map(|a| {
                            Resource::from_typed(&AuthorResource::from(a))
                                .map_err(|e| JsonApiError::from_core(&e))
                        })
                        .collect()
                }
                "tags" => {
                    let tags = tag_repo::by_ids(&pool, &ids).await?;
                    tags.into_iter()
                        .map(|t| {
                            Resource::from_typed(&TagResource::from(t))
                                .map_err(|e| JsonApiError::from_core(&e))
                        })
                        .collect()
                }
                "comments" => {
                    let comments = comment_repo::by_ids(&pool, &ids).await?;
                    comments
                        .into_iter()
                        .map(|c| {
                            Resource::from_typed(&CommentResource::from(c))
                                .map_err(|e| JsonApiError::from_core(&e))
                        })
                        .collect()
                }
                _ => Ok(vec![]),
            }
        }
    }
}
