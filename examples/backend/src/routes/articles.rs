//! Handlers for the `/articles` resource.
use axum::extract::{Path, State};
use axum::http::Uri;
use axum::response::IntoResponse;
use jsonapi_axum::{
    CURSOR_PAGINATION_PROFILE, CursorLinks, CursorPage, DocumentBuilder, JsonApiError,
    JsonApiQuery, JsonApiResponse, OffsetPage, PageNumberPage, pagination_links_with_base,
    resolve_includes,
};
use jsonapi_core::{JsonApiMediaType, Link, PageStrategy, Resource, links};

use crate::include_resolver::DbIncludeResolver;
use crate::repo::article_repo::{self, ArticleQuery, ArticleSort, SortDir};
use crate::repo::{comment_repo, tag_repo};
use crate::resource::ArticleResource;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    uri: Uri,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let page = PageNumberPage::from_query(&query)?;

    let number = page.number.max(1);
    let size = page.size.unwrap_or(5).clamp(1, 50);

    let opts = ArticleQuery {
        limit: size as i64,
        offset: number
            .saturating_sub(1)
            .saturating_mul(size)
            .min(i64::MAX as u64) as i64,
        sort: vec![ArticleSort::CreatedAt(SortDir::Asc)],
        author_id: None,
    };

    let rows = article_repo::list(&state.pool, &opts).await?;
    let total = article_repo::count(&state.pool, None).await?;

    let resources: Vec<ArticleResource> = rows
        .into_iter()
        .map(|a| ArticleResource::from_parts(a, &[], &[]))
        .collect();

    let l = pagination_links_with_base(
        &state.base_url.0,
        &uri,
        PageStrategy::PageNumber { number, size },
        Some(total as u64),
    );

    Ok(JsonApiResponse::new(
        DocumentBuilder::collection(resources).links(l).build(),
    ))
}

pub async fn list_offset(
    State(state): State<AppState>,
    uri: Uri,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let page = OffsetPage::from_query(&query)?;

    let offset = page.offset;
    let limit = page.limit.unwrap_or(5).clamp(1, 50);

    let opts = ArticleQuery {
        limit: limit as i64,
        offset: offset.min(i64::MAX as u64) as i64,
        sort: vec![ArticleSort::CreatedAt(SortDir::Asc)],
        author_id: None,
    };

    let rows = article_repo::list(&state.pool, &opts).await?;
    let total = article_repo::count(&state.pool, None).await?;

    let resources: Vec<ArticleResource> = rows
        .into_iter()
        .map(|a| ArticleResource::from_parts(a, &[], &[]))
        .collect();

    let l = pagination_links_with_base(
        &state.base_url.0,
        &uri,
        PageStrategy::Offset { offset, limit },
        Some(total as u64),
    );

    Ok(JsonApiResponse::new(
        DocumentBuilder::collection(resources).links(l).build(),
    ))
}

pub async fn list_cursor(
    State(state): State<AppState>,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let page = CursorPage::from_query(&query)?;

    let size = page.size.unwrap_or(5).clamp(1, 50);

    let rows = article_repo::list_after(&state.pool, page.after.as_deref(), size as i64).await?;

    // Only advertise a `next` link when a full page came back; a short page is
    // the end of the dataset.
    let next_cursor = if rows.len() as u64 == size {
        rows.last().map(|a| a.id.clone())
    } else {
        None
    };

    let resources: Vec<ArticleResource> = rows
        .into_iter()
        .map(|a| ArticleResource::from_parts(a, &[], &[]))
        .collect();

    let mut cursor_links = CursorLinks::new("/articles/cursor").size(size).first();
    if let Some(cursor) = next_cursor {
        cursor_links = cursor_links.next(cursor);
    }
    let l = cursor_links.links();

    let media_type = JsonApiMediaType::with_profile([CURSOR_PAGINATION_PROFILE]);

    Ok(
        JsonApiResponse::new(DocumentBuilder::collection(resources).links(l).build())
            .media_type(media_type),
    )
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let article = article_repo::get(&state.pool, &id).await?;
    let self_link = links::resource_self(&state.base_url.0, "articles", &article.id);

    let tag_ids = tag_repo::ids_for_article(&state.pool, &article.id).await?;
    let comment_ids = comment_repo::ids_for_article(&state.pool, &article.id).await?;

    let resource = ArticleResource::from_parts(article, &tag_ids, &comment_ids);

    if query.include.is_empty() {
        return Ok(JsonApiResponse::new(
            DocumentBuilder::single(resource)
                .link("self", Link::String(self_link))
                .build(),
        ));
    }

    let primary = Resource::from_typed(&resource).map_err(|e| JsonApiError::from_core(&e))?;
    let paths: Vec<&str> = query.include.iter().map(String::as_str).collect();
    let resolver = DbIncludeResolver { state: &state };
    let included = resolve_includes(&[primary], &paths, &resolver).await?;

    Ok(JsonApiResponse::new(
        DocumentBuilder::single(resource)
            .link("self", Link::String(self_link))
            .include_many(included)
            .build(),
    ))
}
