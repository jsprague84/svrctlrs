//! The calling user's session recordings: list, details, the asciicast itself (to play or
//! download), delete. Every lookup is scoped to the caller, and another user's recording
//! answers 404 like a missing one (`docs/SECURITY.md`, "Session recordings").

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    routing::get,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::models::NewAuditEntry;
use svrctlrs_database::queries::recordings;
use tracing::{info, instrument};

use super::ApiError;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;
use utoipa_axum::router::OpenApiRouter;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .route("/", get(list))
        .route("/{id}", get(get_one).delete(remove))
        .route("/{id}/cast", get(cast))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default)]
    server_id: Option<i64>,
    #[serde(default)]
    limit: Option<i64>,
    /// The smallest id of the previous page
    #[serde(default)]
    before: Option<i64>,
}

/// Newest first, with the retention and size limit the list explains.
#[instrument(skip(state))]
async fn list(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Query(q): Query<ListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = q.limit.unwrap_or(100);
    let items = recordings::list_for_user(&state.pool, me, q.server_id, limit, q.before).await?;
    Ok(Json(json!({
        "recordings": items,
        "retention_days": crate::recordings::retention_days(&state.pool).await,
        "max_bytes": state.recordings.max_bytes(),
    })))
}

#[instrument(skip(state))]
async fn get_one(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let recording = recordings::get_for_user(&state.pool, id, me).await?;
    Ok(Json(json!({ "recording": recording })))
}

#[derive(Debug, Deserialize)]
struct CastQuery {
    /// Serve as a file download (and audit it) instead of for the player
    #[serde(default)]
    download: bool,
}

/// The recording as plain asciicast v2, decrypted as it streams. A recording still being
/// written returns what has been written so far.
#[instrument(skip(state, headers))]
async fn cast(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<CastQuery>,
) -> Result<Response, ApiError> {
    let recording = recordings::get_for_user(&state.pool, id, me).await?;
    let path = state
        .recordings
        .path_of(recording.user_id, &recording.file_name)
        .filter(|p| p.is_file())
        .ok_or_else(|| ApiError::not_found("Recording file"))?;

    if q.download {
        crate::audit::record(
            &state.pool,
            NewAuditEntry::new("recording.downloaded")
                .user(me)
                .client_ip(Some(client_ip(&headers)))
                .target("recording", recording.id, &recording.server_name),
        );
        info!(recording_id = id, "Recording download");
    }

    let mut response = Response::new(Body::from_stream(crate::recordings::read_cast(path)));
    let out = response.headers_mut();
    out.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-asciicast"),
    );
    // What a terminal showed is as private as the shell itself
    out.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    if q.download {
        let name = format!(
            "{}-{}.cast",
            recording.server_name,
            recording.started_at.format("%Y%m%d-%H%M%S")
        );
        let disposition = format!(
            "attachment; filename*=UTF-8''{}",
            super::files::percent_encode(&name)
        );
        if let Ok(v) = HeaderValue::from_str(&disposition) {
            out.insert(header::CONTENT_DISPOSITION, v);
        }
    }
    Ok(response)
}

/// Delete a finished recording and its file. One still being written is refused: end the
/// session first, so the recorder is not left writing to a deleted file.
#[instrument(skip(state, headers))]
async fn remove(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let recording = recordings::get_for_user(&state.pool, id, me).await?;
    if recording.status == "recording" {
        return Err(ApiError::conflict(
            "This shell is still being recorded; end its session first",
        ));
    }
    let recording = recordings::delete_for_user(&state.pool, id, me).await?;
    state
        .recordings
        .remove_file(recording.user_id, &recording.file_name)
        .await;
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("recording.deleted")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("recording", recording.id, &recording.server_name),
    );
    Ok(Json(json!({ "id": id, "message": "Recording deleted" })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use std::time::Duration;
    use svrctlrs_database::Database;
    use tokio::sync::mpsc;
    use tower::ServiceExt;

    struct Harness {
        state: AppState,
        _dir: tempfile::TempDir,
    }

    async fn harness() -> Harness {
        crate::test_support::ensure_encryption();
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite:{}", dir.path().join("t.db").display());
        let database = Database::new(&url).await.unwrap();
        database.migrate().await.unwrap();
        let mut state = AppState::new(database, vec![]).await.unwrap();
        state.recordings =
            crate::recordings::RecordingStore::new(dir.path().join("recordings"), 1 << 20);
        for name in ["ann", "bob"] {
            svrctlrs_database::sqlx::query(
                "INSERT INTO users (username, password_hash) VALUES (?, 'x')",
            )
            .bind(name)
            .execute(&state.pool)
            .await
            .unwrap();
        }
        crate::test_support::add_server(&state.pool, "web", 22, "ann", None, None).await;
        Harness { state, _dir: dir }
    }

    /// A finished recording of `user` saying `text`
    async fn recorded(h: &Harness, user: i64, text: &str) -> i64 {
        let (tx, _rx) = mpsc::channel(8);
        let live = crate::sessions::LiveSession::new(user, 1, "web".into(), tx);
        let id = crate::recordings::start(&h.state.pool, &h.state.recordings, &live, 80, 24)
            .await
            .unwrap();
        live.push_text(text).await;
        live.mark_closed("Shell exited with status 0");
        // Wait for this recorder only; the store stays open for the next one. Generous: CI runs
        // beside image builds, and every write here is a fsync on a file database.
        for _ in 0..3000 {
            let r = recordings::get_for_user(&h.state.pool, id, user)
                .await
                .unwrap();
            if r.status != "recording" {
                return id;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("recording did not finish");
    }

    async fn call(
        h: &Harness,
        user: i64,
        method: &str,
        uri: &str,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let app = axum::Router::from(routes())
            .with_state(h.state.clone())
            .layer(axum::Extension(CurrentUser(user)));
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 1 << 22).await.unwrap();
        (status, headers, body.to_vec())
    }

    fn json_of(body: &[u8]) -> serde_json::Value {
        serde_json::from_slice(body).unwrap()
    }

    #[tokio::test]
    async fn the_owner_lists_plays_downloads_and_deletes_a_recording() {
        let h = harness().await;
        let id = recorded(&h, 1, "uptime: 3 days").await;

        let (status, _, body) = call(&h, 1, "GET", "/").await;
        assert_eq!(status, StatusCode::OK);
        let list = json_of(&body);
        assert_eq!(list["recordings"][0]["id"], id);
        assert_eq!(list["recordings"][0]["status"], "complete");
        assert!(list["recordings"][0].get("file_name").is_none(), "internal");
        assert_eq!(list["retention_days"], 30);

        let (status, headers, body) = call(&h, 1, "GET", &format!("/{id}/cast")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/x-asciicast");
        assert_eq!(headers[header::CACHE_CONTROL], "private, no-store");
        assert!(headers.get(header::CONTENT_DISPOSITION).is_none());
        let cast = String::from_utf8(body).unwrap();
        let header: serde_json::Value = serde_json::from_str(cast.lines().next().unwrap()).unwrap();
        assert_eq!(header["version"], 2, "header first: {cast}");
        assert!(cast.contains("uptime: 3 days"));

        let (_, headers, _) = call(&h, 1, "GET", &format!("/{id}/cast?download=true")).await;
        let disposition = headers[header::CONTENT_DISPOSITION].to_str().unwrap();
        assert!(
            disposition.starts_with("attachment; filename*=UTF-8''web-"),
            "{disposition}"
        );
        // The audit write is fire-and-forget; wait for it
        let mut downloaded = false;
        for _ in 0..3000 {
            downloaded = svrctlrs_database::queries::audit::list(&h.state.pool, 10, None)
                .await
                .unwrap()
                .iter()
                .any(|a| a.action == "recording.downloaded");
            if downloaded {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(downloaded, "downloads are audited");

        let (status, _, _) = call(&h, 1, "DELETE", &format!("/{id}")).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = call(&h, 1, "GET", &format!("/{id}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn another_users_recording_does_not_exist_for_you() {
        let h = harness().await;
        let anns = recorded(&h, 1, "ann's secret").await;

        let (status, _, body) = call(&h, 2, "GET", "/").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json_of(&body)["recordings"], json!([]));
        for (method, uri) in [
            ("GET", format!("/{anns}")),
            ("GET", format!("/{anns}/cast")),
            ("GET", format!("/{anns}/cast?download=true")),
            ("DELETE", format!("/{anns}")),
        ] {
            let (status, _, body) = call(&h, 2, method, &uri).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
            assert!(!String::from_utf8_lossy(&body).contains("secret"));
        }
        // Still there for Ann
        let (status, _, _) = call(&h, 1, "GET", &format!("/{anns}")).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_recording_still_being_written_cannot_be_deleted() {
        let h = harness().await;
        let (tx, _rx) = mpsc::channel(8);
        let live: Arc<crate::sessions::LiveSession> =
            crate::sessions::LiveSession::new(1, 1, "web".into(), tx);
        let id = crate::recordings::start(&h.state.pool, &h.state.recordings, &live, 80, 24)
            .await
            .unwrap();
        let (status, _, body) = call(&h, 1, "DELETE", &format!("/{id}")).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(String::from_utf8_lossy(&body).contains("end its session"));
        live.mark_closed("done");
    }
}
