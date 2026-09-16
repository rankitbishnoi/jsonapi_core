//! Handlers for the `/articles` resource.
use axum::extract::{Path, State};
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use jsonapi_axum::{
    ApiErrorExt, CURSOR_PAGINATION_PROFILE, ClientIdPolicy, CursorLinks, CursorPage,
    DocumentBuilder, JsonApi, JsonApiError, JsonApiQuery, JsonApiQueryValidated, JsonApiResponse,
    NegotiatedMediaType, OffsetPage, PageNumberPage, SortField, pagination_links_with_base,
    resolve_includes, with_status,
};
use jsonapi_core::{JsonApiMediaType, Link, PageStrategy, Resource, links};
use validator::Validate;

use crate::domain::{ArticlePatch, NewArticle};
use crate::include_resolver::DbIncludeResolver;
use crate::repo::article_repo::{self, ArticleQuery, ArticleSort, SortDir};
use crate::repo::{comment_repo, tag_repo};
use crate::resource::{ArticlePatchResource, ArticleResource, NewArticleResource};
use crate::state::AppState;
use crate::util::{mint_id, now};

/// Map `query.sort` fields to the whitelisted [`ArticleSort`] enum, or return a
/// 400 error for any unrecognised field name.
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
                        .detail(format!("cannot sort by `{other}`"))
                        .parameter("sort"),
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
    let w = PageNumberPage::from_query(&query)?.resolve(5, 50);

    let sort = sorts_from_query(&query.sort)?;
    let author_id = author_filter(&query);

    let opts = ArticleQuery {
        limit: w.limit as i64,
        offset: w.offset as i64,
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
        PageStrategy::PageNumber {
            number: w.number,
            size: w.limit,
        },
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
    let w = OffsetPage::from_query(&query)?.resolve(5, 50);

    let opts = ArticleQuery {
        limit: w.limit as i64,
        offset: w.offset as i64,
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
        PageStrategy::Offset {
            offset: w.offset,
            limit: w.limit,
        },
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
    NegotiatedMediaType(media): NegotiatedMediaType,
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
        .media_type(media)
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
    .media_type(media)
    .fields(fieldset))
}

pub async fn create(
    State(state): State<AppState>,
    document: JsonApi<NewArticleResource>,
) -> Result<impl IntoResponse, JsonApiError> {
    // Server assigns id when the client omits it; accept a client-supplied one.
    document.check_client_id(ClientIdPolicy::Assign)?;

    let new_res = document.0.into_single()?;

    // Validate before any DB write; aggregate all field errors into one 422 response.
    new_res
        .validate()
        .map_err(|e| JsonApiError::from_validation_errors(&e))?;

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

    let article = article_repo::create(&state.pool, &new_article, &id, &ts).await?;
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

    let patch_res = document.0.into_single()?;

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
