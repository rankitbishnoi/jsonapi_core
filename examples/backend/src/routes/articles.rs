//! Handlers for the `/articles` resource.
use axum::extract::{Path, State};
use axum::http::Uri;
use axum::response::IntoResponse;
use jsonapi_axum::{
    DocumentBuilder, IntoJsonApiError, JsonApiError, JsonApiQuery, JsonApiResponse, pagination_links,
};
use jsonapi_core::{Link, PageNumberPage, PageStrategy, links};

use crate::error::AppError;
use crate::repo::article_repo::{self, ArticleQuery, ArticleSort, SortDir};
use crate::resource::ArticleResource;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    uri: Uri,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let page = PageNumberPage::from_query(&query)
        .map_err(|e| AppError::BadQuery(e.to_string()).into_json_api_error())?;

    let number = page.number.max(1);
    let size = page.size.unwrap_or(5).clamp(1, 50);

    let opts = ArticleQuery {
        limit: size as i64,
        offset: number.saturating_sub(1).saturating_mul(size).min(i64::MAX as u64) as i64,
        sort: vec![ArticleSort::CreatedAt(SortDir::Asc)],
        author_id: None,
    };

    let rows = article_repo::list(&state.pool, &opts).await?;
    let total = article_repo::count(&state.pool, None).await?;

    let resources: Vec<ArticleResource> = rows
        .into_iter()
        .map(|a| ArticleResource::from_parts(a, &[], &[]))
        .collect();

    let l = pagination_links(
        &uri,
        PageStrategy::PageNumber { number, size },
        Some(total as u64),
    );

    Ok(JsonApiResponse::new(
        DocumentBuilder::collection(resources).links(l).build(),
    ))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, JsonApiError> {
    let article = article_repo::get(&state.pool, &id).await?;
    let self_link = links::resource_self(&state.base_url.0, "articles", &article.id);
    let resource = ArticleResource::from_parts(article, &[], &[]);
    Ok(JsonApiResponse::new(
        DocumentBuilder::single(resource)
            .link("self", Link::String(self_link))
            .build(),
    ))
}
