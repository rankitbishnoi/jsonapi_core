//! Handlers for the `/articles` resource.
use axum::extract::{Path, State};
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use jsonapi_axum::{
    ApiErrorExt, CURSOR_PAGINATION_PROFILE, ClientIdPolicy, CursorLinks, CursorPage,
    DocumentBuilder, JsonApi, JsonApiError, JsonApiQuery, JsonApiQueryValidated, JsonApiResponse,
    OffsetPage, PageNumberPage, SortField, pagination_links_with_base, resolve_includes,
    with_status,
};
use jsonapi_core::{JsonApiMediaType, Link, PageStrategy, Resource, links};

use crate::domain::{ArticlePatch, NewArticle};
use crate::include_resolver::DbIncludeResolver;
use crate::repo::article_repo::{self, ArticleQuery, ArticleSort, SortDir};
use crate::repo::{comment_repo, tag_repo};
use crate::resource::{ArticlePatchResource, ArticleResource, NewArticleResource};
use crate::state::AppState;

/// Returns an ISO-8601-ish sortable timestamp string (UTC) from the system
/// clock. Uses only `std` — no external crates. Since `Duration::as_secs`
/// returns `u64`, all arithmetic is unsigned and post-epoch only.
fn now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let nanos = d.subsec_nanos();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Gregorian calendar conversion (civil date from epoch days, post-1970 only)
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{nanos:09}Z")
}

/// Generate an id from nanoseconds since epoch.
fn mint_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

/// Map `query.sort` fields to the whitelisted [`ArticleSort`] enum, or return a
/// 400 error for any unrecognised field name.
#[allow(clippy::result_large_err)]
fn sorts_from_query(sort: &[SortField]) -> Result<Vec<ArticleSort>, JsonApiError> {
    if sort.is_empty() {
        return Ok(vec![ArticleSort::CreatedAt(SortDir::Asc)]);
    }
    sort.iter()
        .map(|s| {
            let dir = if s.descending {
                SortDir::Desc
            } else {
                SortDir::Asc
            };
            match s.field.as_str() {
                "createdAt" | "created_at" => Ok(ArticleSort::CreatedAt(dir)),
                "title" => Ok(ArticleSort::Title(dir)),
                other => Err(JsonApiError::from_api_error(
                    with_status(StatusCode::BAD_REQUEST)
                        .detail(format!("cannot sort by `{other}`")),
                )),
            }
        })
        .collect()
}

/// Extract the first value of `filter[author]` from the query, if present.
///
/// The `filter` map accumulates repeated values into a `Vec`; we take just the
/// first one since `author` is a single-value filter.
fn author_filter(query: &jsonapi_core::Query) -> Option<String> {
    query.filter.get("author").and_then(|v| v.first()).cloned()
}

pub async fn list(
    State(state): State<AppState>,
    uri: Uri,
    JsonApiQueryValidated { query, .. }: JsonApiQueryValidated<ArticleResource>,
) -> Result<impl IntoResponse, JsonApiError> {
    let page = PageNumberPage::from_query(&query)?;

    let number = page.number.max(1);
    let size = page.size.unwrap_or(5).clamp(1, 50);

    let sort = sorts_from_query(&query.sort)?;
    let author_id = author_filter(&query);

    let opts = ArticleQuery {
        limit: size as i64,
        offset: number
            .saturating_sub(1)
            .saturating_mul(size)
            .min(i64::MAX as u64) as i64,
        sort,
        author_id: author_id.clone(),
    };

    let rows = article_repo::list(&state.pool, &opts).await?;
    let total = article_repo::count(&state.pool, author_id.as_deref()).await?;

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

    Ok(
        JsonApiResponse::new(DocumentBuilder::collection(resources).links(l).build())
            .fields(query.fields),
    )
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
    JsonApiQueryValidated { query, .. }: JsonApiQueryValidated<ArticleResource>,
) -> Result<impl IntoResponse, JsonApiError> {
    let article = article_repo::get(&state.pool, &id).await?;
    let self_link = links::resource_self(&state.base_url.0, "articles", &article.id);

    let tag_ids = tag_repo::ids_for_article(&state.pool, &article.id).await?;
    let comment_ids = comment_repo::ids_for_article(&state.pool, &article.id).await?;

    let resource = ArticleResource::from_parts(article, &tag_ids, &comment_ids);

    // Clone the fieldset config before consuming `query` so we can use the
    // `include` list independently.
    let fieldset = query.fields.clone();

    if query.include.is_empty() {
        return Ok(JsonApiResponse::new(
            DocumentBuilder::single(resource)
                .link("self", Link::String(self_link))
                .build(),
        )
        .fields(fieldset));
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
    )
    .fields(fieldset))
}

pub async fn create(
    State(state): State<AppState>,
    document: JsonApi<NewArticleResource>,
) -> Result<impl IntoResponse, JsonApiError> {
    // Server assigns id when the client omits it; accept a client-supplied one.
    document.check_client_id(ClientIdPolicy::Assign)?;

    let new_res = document
        .0
        .into_single()
        .map_err(|e| JsonApiError::from_core(&e))?;

    let author_id = new_res
        .author
        .first_id()
        .ok_or_else(|| {
            JsonApiError::from_api_error(
                with_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .pointer("/data/relationships/author/data/id")
                    .detail("author relationship is required"),
            )
        })?
        .to_owned();

    let id = new_res.id.unwrap_or_else(mint_id);
    let ts = now();

    let new_article = NewArticle {
        id: Some(id.clone()),
        title: new_res.title,
        body: new_res.body,
        author_id,
    };

    let article = article_repo::create(&state.pool, &new_article, &ts).await?;
    let resource = ArticleResource::from_parts(article, &[], &[]);
    let self_link = links::resource_self(&state.base_url.0, "articles", &id);

    Ok(JsonApiResponse::new(
        DocumentBuilder::single(resource)
            .link("self", Link::String(self_link.clone()))
            .build(),
    )
    .created(self_link))
}

pub async fn patch(
    State(state): State<AppState>,
    Path(id): Path<String>,
    document: JsonApi<ArticlePatchResource>,
) -> Result<impl IntoResponse, JsonApiError> {
    document.require_id(&id)?;

    let patch_res = document
        .0
        .into_single()
        .map_err(|e| JsonApiError::from_core(&e))?;

    // Verify the article exists before patching.
    article_repo::get(&state.pool, &id).await?;

    let patch = ArticlePatch {
        title: patch_res.title,
        body: patch_res.body,
    };

    let ts = now();
    let article = article_repo::patch(&state.pool, &id, &patch, &ts).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &article.id).await?;
    let comment_ids = comment_repo::ids_for_article(&state.pool, &article.id).await?;
    let resource = ArticleResource::from_parts(article, &tag_ids, &comment_ids);
    let self_link = links::resource_self(&state.base_url.0, "articles", &id);

    Ok(JsonApiResponse::new(
        DocumentBuilder::single(resource)
            .link("self", Link::String(self_link))
            .build(),
    ))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, JsonApiError> {
    let deleted = article_repo::delete(&state.pool, &id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(JsonApiError::from_api_error(
            with_status(StatusCode::NOT_FOUND).detail(format!("article `{id}` does not exist")),
        ))
    }
}
