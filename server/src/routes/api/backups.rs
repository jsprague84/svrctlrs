//! Backups of the database (ROADMAP 24): the status About shows, and "Back up now".
//!
//! ```text
//! GET  /backups   where copies go, the schedule, the last attempt and the copies (any token)
//! POST /backups   write a copy now (signed in, or a full-admin token)
//! ```
//!
//! There is no download and no restore here: a copy holds password hashes and every
//! encrypted secret, and a restore replaces everything. `svrctl restore` does that, next to
//! the database (docs/DEPLOYMENT.md, "Backups").

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use chrono::Utc;
use serde_json::json;
use svrctlrs_api::backups::{BackupCreated, BackupFile, BackupRun, BackupStatus};
use svrctlrs_database::backup;
use svrctlrs_database::models::NewAuditEntry;
use svrctlrs_database::queries::backup_runs;
use tracing::{info, instrument};
use utoipa_axum::{router::OpenApiRouter, routes};

use super::ApiError;
use crate::backups;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(status, create))
}

fn run_view(r: svrctlrs_database::models::BackupRun) -> BackupRun {
    BackupRun {
        id: r.id,
        trigger: r.trigger,
        started_at: r.started_at,
        finished_at: r.finished_at,
        file_name: r.file_name,
        size_bytes: r.size_bytes,
        removed: r.removed,
        error: r.error,
    }
}

fn file_view(f: backup::BackupFile) -> BackupFile {
    BackupFile {
        name: f.name,
        size_bytes: f.bytes,
        created_at: f.created_at,
    }
}

/// Where copies go, when the next one runs, the last attempt and the copies that exist.
#[utoipa::path(get, path = "/", tag = "backups",
    responses((status = 200, body = BackupStatus)))]
#[instrument(skip(state))]
async fn status(
    State(state): State<AppState>,
    CurrentUser(_me): CurrentUser,
) -> Result<Json<BackupStatus>, ApiError> {
    let keep = backups::keep(&state.pool).await;
    let hour = backups::hour(&state.pool).await;
    let last_nightly = backup_runs::latest(&state.pool, Some("nightly")).await?;
    let next_run =
        (keep > 0).then(|| backups::next_run(Utc::now(), hour, last_nightly.map(|r| r.started_at)));
    let files = backup::list(state.backups.dir())?
        .into_iter()
        .map(file_view)
        .collect();
    Ok(Json(BackupStatus {
        dir: state.backups.dir().display().to_string(),
        keep,
        hour: i64::from(hour),
        next_run,
        key_source: state.backups.key_source().to_string(),
        running: state.backups.is_running(),
        last_run: backup_runs::latest(&state.pool, None).await?.map(run_view),
        last_success: backup_runs::latest_success(&state.pool)
            .await?
            .map(run_view),
        files,
    }))
}

/// Write a copy of the database now. Older copies beyond `backup.keep` go once it is in
/// place.
#[utoipa::path(post, path = "/", tag = "backups",
    responses((status = 201, body = BackupCreated), (status = 409, body = svrctlrs_api::ErrorBody)))]
#[instrument(skip(state, headers))]
async fn create(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<BackupCreated>), ApiError> {
    let created = backups::run(&state.pool, &state.backups, "manual").await?;
    info!(file = %created.file.name, bytes = created.file.bytes, "Backup made on request");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("backup.created")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .details(json!({
                "file": created.file.name,
                "bytes": created.file.bytes,
                "removed": created.removed,
            })),
    );
    Ok((
        StatusCode::CREATED,
        Json(BackupCreated {
            file: file_view(created.file),
            removed: created.removed,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use crate::test_support::app::AppHarness;
    use axum::http::StatusCode;
    use serde_json::json;

    #[tokio::test]
    async fn back_up_now_writes_a_copy_and_the_status_shows_it() {
        let h = AppHarness::new().await;
        let (status, body) = h.owner("GET", "/api/v1/backups", None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["keep"], 14);
        assert_eq!(body["hour"], 3);
        assert!(body["next_run"].is_string());
        assert!(body["last_run"].is_null());
        assert_eq!(body["files"], json!([]));

        let (status, body) = h.owner("POST", "/api/v1/backups", None).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let name = body["file"]["name"].as_str().unwrap().to_string();
        assert!(body["file"]["size_bytes"].as_u64().unwrap() > 0);

        let (_, body) = h.owner("GET", "/api/v1/backups", None).await;
        assert_eq!(body["files"][0]["name"], name.as_str());
        assert_eq!(body["last_run"]["trigger"], "manual");
        assert_eq!(body["last_success"]["file_name"], name.as_str());
        assert!(body["last_run"]["error"].is_null());
        assert!(!body["running"].as_bool().unwrap());

        // Audited
        let pool = h.state.pool.clone();
        assert!(
            crate::test_support::app::eventually(|| {
                let pool = pool.clone();
                async move {
                    svrctlrs_database::queries::audit::list(&pool, 50, None)
                        .await
                        .unwrap()
                        .iter()
                        .any(|e| e.action == "backup.created")
                }
            })
            .await
        );
    }

    #[tokio::test]
    async fn nightly_off_has_no_next_run_and_bad_settings_are_refused() {
        let h = AppHarness::new().await;
        for (value, ok) in [("0", true), ("366", false), ("-1", false), ("x", false)] {
            let (status, body) = h
                .owner(
                    "PUT",
                    "/api/v1/settings/backup.keep",
                    Some(json!({ "value": value })),
                )
                .await;
            let want = if ok {
                StatusCode::OK
            } else {
                StatusCode::BAD_REQUEST
            };
            assert_eq!(status, want, "keep {value}: {body}");
        }
        for (value, ok) in [("23", true), ("24", false)] {
            let (status, body) = h
                .owner(
                    "PUT",
                    "/api/v1/settings/backup.hour",
                    Some(json!({ "value": value })),
                )
                .await;
            let want = if ok {
                StatusCode::OK
            } else {
                StatusCode::BAD_REQUEST
            };
            assert_eq!(status, want, "hour {value}: {body}");
        }
        let (_, body) = h.owner("GET", "/api/v1/backups", None).await;
        assert_eq!(body["keep"], 0);
        assert_eq!(body["hour"], 23);
        assert!(body["next_run"].is_null());
    }

    #[tokio::test]
    async fn any_token_reads_the_status_and_only_full_admin_backs_up() {
        let h = AppHarness::new().await;
        for preset in ["read_only", "servers_keys", "full_admin"] {
            let (_, token) = h.token(preset, preset).await;
            let (status, body) = h.with(&token, "GET", "/api/v1/backups", None).await;
            assert_eq!(status, StatusCode::OK, "{preset}: {body}");
            let (status, body) = h.with(&token, "POST", "/api/v1/backups", None).await;
            let want = if preset == "full_admin" {
                StatusCode::CREATED
            } else {
                StatusCode::FORBIDDEN
            };
            assert_eq!(status, want, "{preset}: {body}");
        }
    }
}
