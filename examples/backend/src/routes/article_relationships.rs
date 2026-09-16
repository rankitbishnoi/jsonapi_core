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

/// Read server-assigned ids from an incoming linkage payload, rejecting any
/// `lid`-only identifier with a 400 — relationship endpoints operate on ids.
fn client_ids(incoming: &[ResourceIdentifier]) -> Result<Vec<String>, JsonApiError> {
    let mut ids = Vec::with_capacity(incoming.len());
    for r in incoming {
        ids.push(r.require_id().map_err(JsonApiError::from)?.to_owned());
    }
    Ok(ids)
}

// ── tags relationship ─────────────────────────────────────────────────────────

pub async fn get_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<RelationshipResponse, JsonApiError> {
    article_repo::get(&state.pool, &id).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    Ok(
        RelationshipResponse::new(RelationshipData::ToMany(ResourceIdentifier::many(
            "tags", tag_ids,
        )))
        .links(l),
    )
}

pub async fn replace_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> Result<RelationshipResponse, JsonApiError> {
    article_repo::get(&state.pool, &id).await?;
    let new_ids = client_ids(&incoming)?;
    tag_repo::replace_article_tags(&state.pool, &id, &new_ids).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    Ok(
        RelationshipResponse::new(RelationshipData::ToMany(ResourceIdentifier::many(
            "tags", tag_ids,
        )))
        .links(l),
    )
}

pub async fn add_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> Result<RelationshipResponse, JsonApiError> {
    article_repo::get(&state.pool, &id).await?;
    let add_ids = client_ids(&incoming)?;
    tag_repo::add_article_tags(&state.pool, &id, &add_ids).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    Ok(
        RelationshipResponse::new(RelationshipData::ToMany(ResourceIdentifier::many(
            "tags", tag_ids,
        )))
        .links(l),
    )
}

pub async fn remove_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> Result<RelationshipResponse, JsonApiError> {
    article_repo::get(&state.pool, &id).await?;
    let rm_ids = client_ids(&incoming)?;
    tag_repo::remove_article_tags(&state.pool, &id, &rm_ids).await?;
    let tag_ids = tag_repo::ids_for_article(&state.pool, &id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    Ok(
        RelationshipResponse::new(RelationshipData::ToMany(ResourceIdentifier::many(
            "tags", tag_ids,
        )))
        .links(l),
    )
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
    let author_id = rid.require_id().map_err(JsonApiError::from)?;
    article_repo::set_author(&state.pool, &id, author_id).await?;
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "author");
    Ok(
        RelationshipResponse::new(RelationshipData::ToOne(Some(ResourceIdentifier::new(
            "authors", author_id,
        ))))
        .links(l),
    )
}
