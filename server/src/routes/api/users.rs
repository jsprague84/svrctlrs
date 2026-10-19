//! User accounts API. Every account is an administrator until roles exist (see SECURITY.md),
//! so any user may manage accounts; the only guards are "not yourself" and "not the last one".

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{
    models::{CreateUser, NewAuditEntry, UserSetting},
    queries::{user_settings, users},
};
use tracing::{info, instrument};

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users).post(create_user))
        .route("/me/password", put(change_password))
        .route(
            "/me/preferences/{key}",
            get(get_preference).put(put_preference),
        )
        .route("/{id}", axum::routing::delete(delete_user))
}

fn user_json(
    u: &svrctlrs_database::models::User,
    me: i64,
    last_login_at: Option<chrono::DateTime<chrono::Utc>>,
) -> serde_json::Value {
    json!({
        "id": u.id,
        "username": u.username,
        "is_me": u.id == me,
        "last_login_at": last_login_at,
        "created_at": u.created_at,
        "updated_at": u.updated_at,
    })
}

#[instrument(skip(state))]
async fn list_users(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
) -> Result<impl IntoResponse, ApiError> {
    let list = users::list_users(&state.pool).await?;
    // From the audit log; a missing entry just means no login since the log began
    let last: std::collections::HashMap<i64, chrono::DateTime<chrono::Utc>> =
        svrctlrs_database::queries::audit::last_logins(&state.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();
    let list: Vec<_> = list
        .iter()
        .map(|u| user_json(u, me, last.get(&u.id).copied()))
        .collect();
    Ok(Json(json!({ "users": list })))
}

#[derive(Debug, Deserialize)]
struct CreateUserInput {
    username: String,
    password: String,
}

#[instrument(skip(state, input))]
async fn create_user(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<CreateUserInput>,
) -> Result<impl IntoResponse, ApiError> {
    let username = input.username.trim().to_string();
    if username.is_empty() || username.chars().count() > 64 {
        return Err(ApiError::bad_request("username must be 1–64 characters"));
    }
    if users::get_user_by_username(&state.pool, &username)
        .await?
        .is_some()
    {
        return Err(ApiError::conflict("username already exists"));
    }
    let id = users::create_user(
        &state.pool,
        &CreateUser {
            username: username.clone(),
            password: input.password,
        },
    )
    .await?;
    info!(by = me, id, username = %username, "User created");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("user.created")
            .user(me)
            .client_ip(Some(crate::routes::ui::auth::client_ip(&headers)))
            .target("user", id, &username),
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "message": "User created" })),
    ))
}

#[instrument(skip(state))]
async fn delete_user(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    if id == me {
        return Err(ApiError::bad_request("you cannot delete your own account"));
    }
    let total = users::count_users(&state.pool).await?;
    if total <= 1 {
        return Err(ApiError::bad_request("the last account cannot be deleted"));
    }
    let name = users::get_user(&state.pool, id)
        .await?
        .map(|u| u.username)
        .unwrap_or_default();
    users::delete_user(&state.pool, id).await?;
    info!(by = me, id, "User deleted");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("user.deleted")
            .user(me)
            .client_ip(Some(crate::routes::ui::auth::client_ip(&headers)))
            .target("user", id, &name),
    );
    Ok(Json(json!({ "id": id, "message": "User deleted" })))
}

#[derive(Debug, Deserialize)]
struct ChangePasswordInput {
    current_password: String,
    new_password: String,
}

/// Change the caller's own password; the current password is required so a hijacked session
/// alone cannot lock the owner out.
#[instrument(skip(state, input))]
async fn change_password(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<ChangePasswordInput>,
) -> Result<impl IntoResponse, ApiError> {
    let user = users::get_user(&state.pool, me)
        .await?
        .ok_or_else(|| ApiError::not_found("User"))?;
    let valid = users::verify_password(&input.current_password, &user.password_hash)?;
    if !valid {
        return Err(ApiError::unauthorized("current password is incorrect"));
    }
    users::update_password(&state.pool, me, &input.new_password).await?;
    info!(user_id = me, "Password changed");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("user.password_changed")
            .user(me)
            .client_ip(Some(crate::routes::ui::auth::client_ip(&headers))),
    );
    Ok(Json(json!({ "message": "Password changed" })))
}

/// The preference documents a user can keep on the server. The UI owns their contents.
const PREFERENCE_KEYS: &[&str] = &["terminal"];
/// Far more than any of them needs; a bound so one account cannot fill the database.
const MAX_PREFERENCE_BYTES: usize = 16 * 1024;

fn preference_key(key: &str) -> Result<&str, ApiError> {
    PREFERENCE_KEYS
        .iter()
        .find(|k| **k == key)
        .copied()
        .ok_or_else(|| ApiError::not_found("Preference"))
}

fn preference_json(key: &str, row: Option<&UserSetting>) -> serde_json::Value {
    // A stored value always parsed when it was saved; a row that somehow does not is "unset"
    let value = row.and_then(|r| serde_json::from_str::<serde_json::Value>(&r.value).ok());
    json!({
        "key": key,
        "value": value,
        "updated_at": row.map(|r| r.updated_at),
    })
}

/// The caller's own preference document under `key`; `value` is null until they save one.
#[instrument(skip(state))]
async fn get_preference(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let key = preference_key(&key)?;
    let row = user_settings::get(&state.pool, me, key).await?;
    Ok(Json(preference_json(key, row.as_ref())))
}

#[derive(Debug, Deserialize)]
struct PutPreferenceInput {
    value: serde_json::Value,
}

/// Replace the caller's own preference document under `key` (a JSON object, at most 16 KiB).
#[instrument(skip(state, input))]
async fn put_preference(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Path(key): Path<String>,
    Json(input): Json<PutPreferenceInput>,
) -> Result<impl IntoResponse, ApiError> {
    let key = preference_key(&key)?;
    if !input.value.is_object() {
        return Err(ApiError::bad_request("value must be a JSON object"));
    }
    let text = input.value.to_string();
    if text.len() > MAX_PREFERENCE_BYTES {
        return Err(ApiError::bad_request("value is larger than 16 KiB"));
    }
    let row = user_settings::put(&state.pool, me, key, &text).await?;
    Ok(Json(preference_json(key, Some(&row))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    async fn state() -> (AppState, tempfile::TempDir) {
        crate::test_support::ensure_encryption();
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite:{}", dir.path().join("t.db").display());
        let database = svrctlrs_database::Database::new(&url).await.unwrap();
        database.migrate().await.unwrap();
        let state = AppState::new(database, vec![]).await.unwrap();
        for name in ["ann", "bob"] {
            svrctlrs_database::sqlx::query(
                "INSERT INTO users (username, password_hash) VALUES (?, 'x')",
            )
            .bind(name)
            .execute(&state.pool)
            .await
            .unwrap();
        }
        (state, dir)
    }

    async fn call(
        state: &AppState,
        user: i64,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let app = routes()
            .with_state(state.clone())
            .layer(axum::Extension(CurrentUser(user)));
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        let request = match body {
            Some(b) => request.body(Body::from(b.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    #[tokio::test]
    async fn a_user_saves_and_reads_back_their_own_terminal_preferences() {
        let (s, _dir) = state().await;
        let uri = "/me/preferences/terminal";
        let (status, body) = call(&s, 1, "GET", uri, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["value"], serde_json::Value::Null);

        let prefs = json!({ "fontSize": 12, "cursorStyle": "bar" });
        let (status, body) = call(&s, 1, "PUT", uri, Some(json!({ "value": prefs }))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["value"], prefs);
        assert!(body["updated_at"].is_string());

        let (_, body) = call(&s, 1, "GET", uri, None).await;
        assert_eq!(body["value"], prefs);
        // Another account sees only its own (none yet), and its save does not touch ann's
        let (_, body) = call(&s, 2, "GET", uri, None).await;
        assert_eq!(body["value"], serde_json::Value::Null);
        call(
            &s,
            2,
            "PUT",
            uri,
            Some(json!({ "value": { "fontSize": 20 } })),
        )
        .await;
        let (_, body) = call(&s, 1, "GET", uri, None).await;
        assert_eq!(body["value"], prefs);
    }

    #[tokio::test]
    async fn only_known_keys_objects_and_bounded_sizes_are_accepted() {
        let (s, _dir) = state().await;
        let (status, _) = call(&s, 1, "GET", "/me/preferences/theme", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = call(
            &s,
            1,
            "PUT",
            "/me/preferences/anything",
            Some(json!({ "value": {} })),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let uri = "/me/preferences/terminal";
        for value in [
            json!([1, 2]),
            json!("big"),
            json!(12),
            serde_json::Value::Null,
        ] {
            let (status, _) = call(&s, 1, "PUT", uri, Some(json!({ "value": value }))).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
        }
        let huge = json!({ "value": { "x": "y".repeat(MAX_PREFERENCE_BYTES) } });
        let (status, _) = call(&s, 1, "PUT", uri, Some(huge)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (_, body) = call(&s, 1, "GET", uri, None).await;
        assert_eq!(body["value"], serde_json::Value::Null, "nothing was saved");
    }
}
