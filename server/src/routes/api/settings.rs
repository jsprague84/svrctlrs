//! Settings API endpoints
//!
//! Provides access to application settings.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::queries::settings;
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create settings router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_settings))
        .route("/{key}", get(get_setting).put(update_setting))
}

/// List all settings
#[instrument(skip(state))]
async fn list_settings(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let all_settings = settings::list_settings(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to list settings");
        ApiError::internal_error(format!("Failed to list settings: {}", e))
    })?;

    // Group settings by category for better organization
    let mut categories: std::collections::HashMap<String, Vec<serde_json::Value>> =
        std::collections::HashMap::new();

    for setting in all_settings {
        let category = setting
            .key
            .split('.')
            .next()
            .unwrap_or("general")
            .to_string();

        let entry = json!({
            "key": setting.key,
            "value": setting.value,
            "value_type": setting.value_type,
            "description": setting.description,
            "updated_at": setting.updated_at
        });

        categories.entry(category).or_default().push(entry);
    }

    Ok(Json(json!({
        "settings": categories
    })))
}

/// Get setting by key
#[instrument(skip(state))]
async fn get_setting(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let setting = settings::get_setting(&state.pool, &key)
        .await
        .map_err(|e| {
            error!(error = %e, key = %key, "Failed to get setting");
            ApiError::not_found("Setting")
        })?;

    Ok(Json(json!({
        "key": setting.key,
        "value": setting.value,
        "value_type": setting.value_type,
        "description": setting.description,
        "updated_at": setting.updated_at
    })))
}

/// Update setting input
#[derive(Debug, Deserialize)]
struct UpdateSettingInput {
    value: String,
}

/// Update a setting value
#[instrument(skip(state, input))]
async fn update_setting(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(input): Json<UpdateSettingInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(key = %key, "Updating setting");

    // Verify setting exists
    let existing = settings::get_setting(&state.pool, &key)
        .await
        .map_err(|_| ApiError::not_found("Setting"))?;

    // Validate value type
    match existing.value_type.as_str() {
        "boolean" => {
            if input.value != "true" && input.value != "false" {
                return Err(ApiError::bad_request(
                    "Boolean setting must be 'true' or 'false'",
                ));
            }
        }
        "number" => {
            if input.value.parse::<f64>().is_err() {
                return Err(ApiError::bad_request(
                    "Number setting must be a valid number",
                ));
            }
        }
        "json" => {
            if serde_json::from_str::<serde_json::Value>(&input.value).is_err() {
                return Err(ApiError::bad_request("JSON setting must be valid JSON"));
            }
        }
        _ => {}
    }

    let update = svrctlrs_database::UpdateSetting { value: input.value };

    settings::update_setting(&state.pool, &key, &update)
        .await
        .map_err(|e| {
            error!(error = %e, key = %key, "Failed to update setting");
            ApiError::internal_error(format!("Failed to update setting: {}", e))
        })?;

    info!(key = %key, "Setting updated successfully");

    Ok(Json(json!({
        "key": key,
        "message": "Setting updated successfully"
    })))
}
