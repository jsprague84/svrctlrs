//! Tags API endpoints
//!
//! Provides CRUD operations for managing server tags.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{queries::tags, CreateTag, UpdateTag};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create tags router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_tags).post(create_tag))
        .route("/{id}", get(get_tag).put(update_tag).delete(delete_tag))
}

/// List all tags with server counts
#[instrument(skip(state))]
async fn list_tags(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let tags_with_counts = tags::get_tags_with_counts(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to list tags");
        ApiError::internal_error(format!("Failed to list tags: {}", e))
    })?;

    let tags: Vec<_> = tags_with_counts
        .into_iter()
        .map(|tc| {
            json!({
                "id": tc.tag.id,
                "name": tc.tag.name,
                "color": tc.tag.color,
                "description": tc.tag.description,
                "server_count": tc.server_count,
                "created_at": tc.tag.created_at
            })
        })
        .collect();

    Ok(Json(json!({ "tags": tags })))
}

/// Get tag by ID
#[instrument(skip(state))]
async fn get_tag(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let tag = tags::get_tag(&state.pool, id).await.map_err(|e| {
        error!(error = %e, id = id, "Failed to get tag");
        ApiError::not_found("Tag")
    })?;

    Ok(Json(json!({
        "id": tag.id,
        "name": tag.name,
        "color": tag.color,
        "description": tag.description,
        "created_at": tag.created_at
    })))
}

/// Create tag input
#[derive(Debug, Deserialize)]
struct CreateTagInput {
    name: String,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// Create a new tag
#[instrument(skip(state, input))]
async fn create_tag(
    State(state): State<AppState>,
    Json(input): Json<CreateTagInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, "Creating tag");

    let create = CreateTag {
        name: input.name,
        color: input.color,
        description: input.description,
    };

    let id = tags::create_tag(&state.pool, &create).await.map_err(|e| {
        error!(error = %e, "Failed to create tag");
        ApiError::internal_error(format!("Failed to create tag: {}", e))
    })?;

    info!(id = id, "Tag created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Tag created successfully"
        })),
    ))
}

/// Update tag input
#[derive(Debug, Deserialize)]
struct UpdateTagInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// Update an existing tag
#[instrument(skip(state, input))]
async fn update_tag(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateTagInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating tag");

    // Verify tag exists
    tags::get_tag(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Tag"))?;

    let update = UpdateTag {
        name: input.name,
        color: input.color,
        description: input.description,
    };

    tags::update_tag(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update tag");
            ApiError::internal_error(format!("Failed to update tag: {}", e))
        })?;

    info!(id = id, "Tag updated successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Tag updated successfully"
    })))
}

/// Delete a tag
#[instrument(skip(state))]
async fn delete_tag(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting tag");

    tags::delete_tag(&state.pool, id).await.map_err(|e| {
        error!(error = %e, id = id, "Failed to delete tag");
        ApiError::internal_error(format!("Failed to delete tag: {}", e))
    })?;

    info!(id = id, "Tag deleted successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Tag deleted successfully"
    })))
}
