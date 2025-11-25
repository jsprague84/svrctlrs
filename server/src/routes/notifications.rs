use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use tracing::{error, info, instrument};

use crate::state::AppState;
use svrctlrs_database::models::notification::{
    CreateNotificationBackend, NotificationBackend, UpdateNotificationBackend,
};
use svrctlrs_database::queries;

/// Create notification backend API router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_backends).post(create_backend))
        .route("/:id", get(get_backend).put(update_backend).delete(delete_backend))
}

/// List all notification backends
#[instrument(skip(state))]
async fn list_backends(
    State(state): State<AppState>,
) -> Result<Json<Vec<NotificationBackend>>, (StatusCode, String)> {
    let db = state.db().await;
    let backends = queries::notifications::list_notification_backends(db.pool())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list notification backends");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    Ok(Json(backends))
}

/// Get a notification backend by ID
#[instrument(skip(state))]
async fn get_backend(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db().await;
    let backend = queries::notifications::get_notification_backend(db.pool(), id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get notification backend");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    Ok(Json(backend))
}

/// Create a new notification backend
#[instrument(skip(state, create_backend_input))]
async fn create_backend(
    State(state): State<AppState>,
    Json(create_backend_input): Json<CreateNotificationBackend>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(name = %create_backend_input.name, backend_type = %create_backend_input.backend_type, "Creating notification backend");
    let db = state.db().await;
    
    let id = queries::notifications::create_notification_backend(db.pool(), &create_backend_input)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create notification backend");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    
    // Return created backend
    let backend = queries::notifications::get_notification_backend(db.pool(), id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get created notification backend");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    
    Ok((StatusCode::CREATED, Json(backend)))
}

/// Update a notification backend
#[instrument(skip(state, update_backend_input))]
async fn update_backend(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(update_backend_input): Json<UpdateNotificationBackend>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(backend_id = id, "Updating notification backend");
    let db = state.db().await;
    
    queries::notifications::update_notification_backend(db.pool(), id, &update_backend_input)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update notification backend");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    
    // Return updated backend
    let backend = queries::notifications::get_notification_backend(db.pool(), id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get updated notification backend");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    
    Ok(Json(backend))
}

/// Delete a notification backend
#[instrument(skip(state))]
async fn delete_backend(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(backend_id = id, "Deleting notification backend");
    let db = state.db().await;
    
    queries::notifications::delete_notification_backend(db.pool(), id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete notification backend");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    
    Ok(StatusCode::NO_CONTENT)
}

