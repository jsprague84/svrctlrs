//! Settings API endpoints
//!
//! Provides access to application settings.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::json;
use svrctlrs_api::settings::{
    NewSetting, Setting, SettingSaved, SettingValue, SettingsList, SettingsUpdate, SettingsUpdated,
};
use svrctlrs_api::ErrorBody;
use svrctlrs_database::{models::NewAuditEntry, queries::settings};
use tracing::{info, instrument};

use super::ApiError;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Create settings router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_settings, create_setting, update_settings))
        .routes(routes!(get_setting, update_setting))
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
    if key == "backup.keep"
        && !value
            .trim()
            .parse::<i64>()
            .is_ok_and(|k| (0..=crate::backups::MAX_KEEP).contains(&k))
    {
        return Err(ApiError::bad_request(
            "Keep 0 to 365 backups (0 turns nightly backups off)",
        ));
    }
    if key == "backup.hour" && !value.trim().parse::<u32>().is_ok_and(|h| h < 24) {
        return Err(ApiError::bad_request("The backup hour is 0 to 23 (UTC)"));
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

/// A setting's stored value, or None when it has never been set
async fn stored_value(state: &AppState, key: &str) -> Result<Option<String>, ApiError> {
    match settings::get_setting(&state.pool, key).await {
        Ok(s) => Ok(Some(s.value)),
        Err(svrctlrs_core::Error::NotFound(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Record `setting.updated` when a write changed the value: old and new, or for a secret
/// setting only whether it was and is set. A form resending unchanged values records nothing.
fn audit_change(
    state: &AppState,
    me: i64,
    headers: &HeaderMap,
    key: &str,
    old: Option<String>,
    new: &str,
) {
    if old.as_deref() == Some(new) {
        return;
    }
    let details = if is_secret(key) {
        json!({
            "secret": true,
            "was_set": old.as_deref().is_some_and(|v| !v.is_empty()),
            "is_set": !new.is_empty(),
        })
    } else {
        json!({ "old": old, "new": new })
    };
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("setting.updated")
            .user(me)
            .client_ip(Some(client_ip(headers)))
            .target_named("setting", key)
            .details(details),
    );
}

fn setting_view(s: svrctlrs_database::models::Setting) -> Setting {
    let secret = is_secret(&s.key);
    Setting {
        is_set: !s.value.is_empty(),
        value: if secret { String::new() } else { s.value },
        key: s.key,
        secret,
        value_type: s.value_type,
        description: s.description,
        updated_at: s.updated_at,
    }
}

/// All settings, grouped by the part of the key before the first dot
#[utoipa::path(get, path = "/", tag = "settings",
    responses((status = 200, body = SettingsList)))]
#[instrument(skip(state))]
async fn list_settings(State(state): State<AppState>) -> Result<Json<SettingsList>, ApiError> {
    let mut groups = std::collections::BTreeMap::<String, Vec<Setting>>::new();
    for setting in settings::list_settings(&state.pool).await? {
        let category = setting
            .key
            .split('.')
            .next()
            .unwrap_or("general")
            .to_string();
        groups
            .entry(category)
            .or_default()
            .push(setting_view(setting));
    }
    Ok(Json(SettingsList { settings: groups }))
}

/// One setting
#[utoipa::path(get, path = "/{key}", tag = "settings",
    params(("key" = String, Path, description = "e.g. notification.url")),
    responses((status = 200, body = Setting), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn get_setting(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Setting>, ApiError> {
    Ok(Json(setting_view(
        settings::get_setting(&state.pool, &key).await?,
    )))
}

/// Update several settings at once (upsert each). A secret key with an empty value is left
/// untouched, so a form can resend everything without knowing the stored token.
#[utoipa::path(put, path = "/", tag = "settings", request_body = SettingsUpdate,
    responses((status = 200, body = SettingsUpdated), (status = 400, body = ErrorBody)))]
#[instrument(skip(state, headers, input), fields(keys = input.values.len()))]
async fn update_settings(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<SettingsUpdate>,
) -> Result<Json<SettingsUpdated>, ApiError> {
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
        let old = stored_value(&state, key).await?;
        settings::set_setting(&state.pool, key, value).await?;
        audit_change(&state, me, &headers, key, old, value);
        written.push(key.clone());
    }
    info!(count = written.len(), "Settings updated");
    Ok(Json(SettingsUpdated {
        updated: written,
        message: "Settings updated".to_string(),
    }))
}

/// Create a new setting
#[utoipa::path(post, path = "/", tag = "settings", request_body = NewSetting,
    responses((status = 201, body = SettingSaved), (status = 400, body = ErrorBody)))]
#[instrument(skip(state, headers, input))]
async fn create_setting(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<NewSetting>,
) -> Result<(StatusCode, Json<SettingSaved>), ApiError> {
    info!(key = %input.key, "Creating setting");

    // Validate key is not empty
    if input.key.trim().is_empty() {
        return Err(ApiError::bad_request("Setting key cannot be empty"));
    }

    // Validate value_type: the ones the settings table accepts
    let valid_types = ["string", "integer", "boolean", "json"];
    if !valid_types.contains(&input.value_type.as_str()) {
        return Err(ApiError::bad_request(
            "value_type must be one of: string, integer, boolean, json",
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
        "integer" => {
            if input.value.trim().parse::<i64>().is_err() {
                return Err(ApiError::bad_request(
                    "Integer setting must be a whole number",
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
    audit_change(&state, me, &headers, &input.key, None, &input.value);

    info!(key = %input.key, "Setting created successfully");

    Ok((
        StatusCode::CREATED,
        Json(SettingSaved {
            key: input.key,
            message: "Setting created successfully".to_string(),
        }),
    ))
}

/// Update (or create) a setting value — upsert behavior
#[utoipa::path(put, path = "/{key}", tag = "settings", request_body = SettingValue,
    params(("key" = String, Path, description = "e.g. recording.retention_days")),
    responses((status = 200, body = SettingSaved), (status = 400, body = ErrorBody)))]
#[instrument(skip(state, headers, input))]
async fn update_setting(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(input): Json<SettingValue>,
) -> Result<Json<SettingSaved>, ApiError> {
    info!(key = %key, "Updating setting");
    check_value(&key, &input.value)?;

    // Use set_setting (upsert) — creates if missing, updates if exists
    let old = stored_value(&state, &key).await?;
    settings::set_setting(&state.pool, &key, &input.value).await?;
    audit_change(&state, me, &headers, &key, old, &input.value);

    info!(key = %key, "Setting updated successfully");

    Ok(Json(SettingSaved {
        key,
        message: "Setting updated successfully".to_string(),
    }))
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

    /// Every settings write that changes a value leaves `setting.updated` with old and new
    /// (a secret one only whether it is set), naming the token it came in with; a write that
    /// changes nothing leaves no row.
    #[tokio::test]
    async fn setting_changes_are_audited() {
        use crate::test_support::app::{eventually, AppHarness};
        use serde_json::Value;

        let h = AppHarness::new().await;
        let (token_id, secret) = h.token("agent", "full_admin").await;

        let (status, body) = h
            .with(
                &secret,
                "PUT",
                "/api/v1/settings/backup.keep",
                Some(json!({ "value": "30" })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, body) = h
            .owner(
                "PUT",
                "/api/v1/settings/backup.keep",
                Some(json!({ "value": "14" })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        // Unchanged values and an empty secret (left untouched) record nothing; the token does
        let (status, body) = h
            .owner(
                "PUT",
                "/api/v1/settings",
                Some(json!({ "values": {
                    "backup.keep": "14",
                    "notification.token": "",
                    "approval.matrix_token": "s3cret-value",
                } })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        // A refused write records nothing either
        let (status, _) = h
            .owner(
                "PUT",
                "/api/v1/settings/backup.keep",
                Some(json!({ "value": "-1" })),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, body) = h
            .owner(
                "POST",
                "/api/v1/settings",
                Some(json!({ "key": "ui.motd", "value": "hi", "value_type": "string" })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");

        let pool = h.state.pool.clone();
        let rows = || async {
            svrctlrs_database::queries::audit::list(&pool, 50, None)
                .await
                .unwrap()
                .into_iter()
                .filter(|e| e.action == "setting.updated")
                .collect::<Vec<_>>()
        };
        assert!(eventually(|| async { rows().await.len() >= 4 }).await);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let rows = rows().await;
        assert_eq!(rows.len(), 4, "{rows:?}");
        // Each row is written by its own task (`audit::record`), so requests close together may
        // land in either order: find each by its setting and details
        let row = |key: &str, details: Value| {
            rows.iter()
                .find(|r| {
                    r.target_name.as_deref() == Some(key)
                        && r.details
                            .as_deref()
                            .and_then(|d| serde_json::from_str::<Value>(d).ok())
                            .as_ref()
                            == Some(&details)
                })
                .unwrap_or_else(|| panic!("no {key} row with {details}: {rows:?}"))
        };

        let by_token = row("backup.keep", json!({ "old": "14", "new": "30" }));
        assert_eq!(by_token.target_type.as_deref(), Some("setting"));
        assert_eq!(by_token.token_id, Some(token_id));
        assert_eq!(by_token.token_name.as_deref(), Some("agent"));
        assert_eq!(by_token.user_id, Some(1));

        let by_owner = row("backup.keep", json!({ "old": "30", "new": "14" }));
        assert_eq!(by_owner.token_id, None);

        let matrix = row(
            "approval.matrix_token",
            json!({ "secret": true, "was_set": false, "is_set": true }),
        );
        assert!(!matrix.details.as_deref().unwrap().contains("s3cret"));

        row("ui.motd", json!({ "old": null, "new": "hi" }));
    }
}
