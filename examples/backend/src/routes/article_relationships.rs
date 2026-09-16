//! Handlers for `/articles/{id}/relationships/tags` and
//! `/articles/{id}/relationships/author`.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use jsonapi_axum::{
    ApiErrorExt, JsonApiError, JsonApiToMany, JsonApiToOne, RelationshipResponse, with_status,
};
use jsonapi_core::{RelationshipData, ResourceIdentifier, links};

use crate::repo::{article_repo, tag_repo};
use crate::state::AppState;

// ── helpers ──────────────────────────────────────────────────────────────────

fn tag_identifiers(ids: &[String]) -> Vec<ResourceIdentifier> {
    ids.iter()
        .map(|id| ResourceIdentifier::new("tags", id.as_str()))
        .collect()
}

// ── tags relationship ─────────────────────────────────────────────────────────

pub async fn get_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<RelationshipResponse, JsonApiError> {
    article_repo::get(&state.pool, &id).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    Ok(RelationshipResponse::new(RelationshipData::ToMany(tag_identifiers(&tag_ids))).links(l))
}

pub async fn replace_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> Result<RelationshipResponse, JsonApiError> {
    article_repo::get(&state.pool, &id).await?;
    let new_ids: Vec<String> = incoming
        .iter()
        .filter_map(|r| r.identity.as_id().map(str::to_owned))
        .collect();
    tag_repo::replace_article_tags(&state.pool, &id, &new_ids).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    Ok(RelationshipResponse::new(RelationshipData::ToMany(tag_identifiers(&tag_ids))).links(l))
}

pub async fn add_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> Result<RelationshipResponse, JsonApiError> {
    article_repo::get(&state.pool, &id).await?;
    let add_ids: Vec<String> = incoming
        .iter()
        .filter_map(|r| r.identity.as_id().map(str::to_owned))
        .collect();
    tag_repo::add_article_tags(&state.pool, &id, &add_ids).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    Ok(RelationshipResponse::new(RelationshipData::ToMany(tag_identifiers(&tag_ids))).links(l))
}

pub async fn remove_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> Result<RelationshipResponse, JsonApiError> {
    article_repo::get(&state.pool, &id).await?;
    let rm_ids: Vec<String> = incoming
        .iter()
        .filter_map(|r| r.identity.as_id().map(str::to_owned))
        .collect();
    tag_repo::remove_article_tags(&state.pool, &id, &rm_ids).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    Ok(RelationshipResponse::new(RelationshipData::ToMany(tag_identifiers(&tag_ids))).links(l))
}

// ── author relationship ───────────────────────────────────────────────────────

pub async fn get_author(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<RelationshipResponse, JsonApiError> {
    let author_id = article_repo::author_id_of(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "author");
    Ok(
        RelationshipResponse::new(RelationshipData::ToOne(Some(ResourceIdentifier::new(
            "authors", &author_id,
        ))))
        .links(l),
    )
}

pub async fn set_author(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToOne(incoming): JsonApiToOne,
) -> Result<RelationshipResponse, JsonApiError> {
    let rid = incoming.ok_or_else(|| {
        JsonApiError::from_api_error(
            with_status(StatusCode::BAD_REQUEST).detail("author linkage required"),
        )
    })?;
    let author_id = rid.identity.as_id().ok_or_else(|| {
        JsonApiError::from_api_error(
            with_status(StatusCode::BAD_REQUEST).detail("author id must be a server-assigned id"),
        )
    })?;
    article_repo::set_author(&state.pool, &id, author_id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "author");
    Ok(
        RelationshipResponse::new(RelationshipData::ToOne(Some(ResourceIdentifier::new(
            "authors", author_id,
        ))))
        .links(l),
    )
}
