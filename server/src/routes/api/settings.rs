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
use tracing::{info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create settings router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(list_settings).post(create_setting).put(update_settings),
        )
        .route("/{key}", get(get_setting).put(update_setting))
}

/// Settings that are write-only through the API: reads say whether they are set, never what.
/// Values the server reads as numbers are checked on the way in, so a typo cannot quietly
/// fall back to a default.
fn check_value(key: &str, value: &str) -> Result<(), ApiError> {
    if key == "recording.retention_days"
        && !value
            .trim()
            .parse::<i64>()
            .is_ok_and(|d| (0..=3650).contains(&d))
    {
        return Err(ApiError::bad_request(
            "Keep recordings for 0 to 3650 days (0 keeps them until you delete them)",
        ));
    }
    if key == "approval.channel" && !matches!(value, "push" | "webhook" | "matrix" | "none") {
        return Err(ApiError::bad_request(
            "The approval channel is push, webhook, matrix or none",
        ));
    }
    if matches!(
        key,
        "approval.link_base" | "approval.matrix_homeserver" | "approval.webhook_url"
    ) && !value.is_empty()
        && !(value.starts_with("https://") || value.starts_with("http://"))
    {
        return Err(ApiError::bad_request(format!(
            "{key} must start with http:// or https://"
        )));
    }
    Ok(())
}

fn is_secret(key: &str) -> bool {
    // A webhook URL usually carries its own secret in the path
    matches!(
        key,
        "notification.token" | "approval.matrix_token" | "approval.webhook_url"
    )
}

fn setting_json(s: &svrctlrs_database::models::Setting) -> serde_json::Value {
    let secret = is_secret(&s.key);
    json!({
        "key": s.key,
        "value": if secret { "" } else { s.value.as_str() },
        "secret": secret,
        "is_set": !s.value.is_empty(),
        "value_type": s.value_type,
        "description": s.description,
        "updated_at": s.updated_at
    })
}

/// List all settings
#[instrument(skip(state))]
async fn list_settings(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let all_settings = settings::list_settings(&state.pool).await?;

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

        categories
            .entry(category)
            .or_default()
            .push(setting_json(&setting));
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
) -> Result<impl IntoResponse, ApiError> {
    let setting = settings::get_setting(&state.pool, &key).await?;

    Ok(Json(setting_json(&setting)))
}

/// Batch update input: `{"values": {"notification.url": "…", …}}`
#[derive(Debug, Deserialize)]
struct UpdateSettingsInput {
    values: std::collections::BTreeMap<String, String>,
}

/// Update several settings at once (upsert each). A secret key with an empty value is left
/// untouched, so a form can resend everything without knowing the stored token.
#[instrument(skip(state, input), fields(keys = input.values.len()))]
async fn update_settings(
    State(state): State<AppState>,
    Json(input): Json<UpdateSettingsInput>,
) -> Result<impl IntoResponse, ApiError> {
    // All or nothing: refuse before writing any
    for (key, value) in &input.values {
        if key.trim().is_empty() {
            return Err(ApiError::bad_request("Setting key cannot be empty"));
        }
        check_value(key, value)?;
    }
    let mut written = Vec::new();
    for (key, value) in &input.values {
        if is_secret(key) && value.is_empty() {
            continue;
        }
        settings::set_setting(&state.pool, key, value).await?;
        written.push(key.clone());
    }
    info!(count = written.len(), "Settings updated");
    Ok(Json(
        json!({ "updated": written, "message": "Settings updated" }),
    ))
}

/// Create setting input
#[derive(Debug, Deserialize)]
struct CreateSettingInput {
    key: String,
    value: String,
    #[serde(default = "default_value_type")]
    value_type: String,
    description: Option<String>,
}

fn default_value_type() -> String {
    "string".to_string()
}

/// Create a new setting
#[instrument(skip(state, input))]
async fn create_setting(
    State(state): State<AppState>,
    Json(input): Json<CreateSettingInput>,
) -> Result<impl IntoResponse, ApiError> {
    info!(key = %input.key, "Creating setting");

    // Validate key is not empty
    if input.key.trim().is_empty() {
        return Err(ApiError::bad_request("Setting key cannot be empty"));
    }

    // Validate value_type
    let valid_types = ["string", "number", "boolean", "json"];
    if !valid_types.contains(&input.value_type.as_str()) {
        return Err(ApiError::bad_request(
            "value_type must be one of: string, number, boolean, json",
        ));
    }

    // Validate value matches type
    match input.value_type.as_str() {
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
        "json" if serde_json::from_str::<serde_json::Value>(&input.value).is_err() => {
            return Err(ApiError::bad_request("JSON setting must be valid JSON"));
        }
        _ => {}
    }

    // Check if setting already exists
    if settings::get_setting(&state.pool, &input.key).await.is_ok() {
        return Err(ApiError::bad_request(
            "Setting with this key already exists",
        ));
    }

    settings::create_setting(
        &state.pool,
        &input.key,
        &input.value,
        &input.value_type,
        input.description.as_deref(),
    )
    .await?;

    info!(key = %input.key, "Setting created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "key": input.key,
            "message": "Setting created successfully"
        })),
    ))
}

/// Update setting input
#[derive(Debug, Deserialize)]
struct UpdateSettingInput {
    value: String,
}

/// Update (or create) a setting value — upsert behavior
#[instrument(skip(state, input))]
async fn update_setting(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(input): Json<UpdateSettingInput>,
) -> Result<impl IntoResponse, ApiError> {
    info!(key = %key, "Updating setting");
    check_value(&key, &input.value)?;

    // Use set_setting (upsert) — creates if missing, updates if exists
    settings::set_setting(&state.pool, &key, &input.value).await?;

    info!(key = %key, "Setting updated successfully");

    Ok(Json(json!({
        "key": key,
        "message": "Setting updated successfully"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_must_be_a_number_of_days() {
        for ok in ["0", "30", " 365 ", "3650"] {
            assert!(check_value("recording.retention_days", ok).is_ok(), "{ok}");
        }
        for bad in ["", "-1", "3651", "thirty", "1.5"] {
            let e = check_value("recording.retention_days", bad).unwrap_err();
            assert_eq!(e.status, axum::http::StatusCode::BAD_REQUEST, "{bad}");
        }
        assert!(check_value("notification.url", "anything").is_ok());
    }
}
