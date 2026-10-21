//! Live interactive sessions of the calling user: list them (to offer "resume" after a
//! reload), close one for good, and choose how long they are kept with nobody attached.

use std::time::Duration;

use axum::{
    extract::{Path, State},
    Json,
};
use svrctlrs_api::sessions::{
    SessionClosed, SessionKeep, SessionKeepUpdate, SessionList, SessionSummary,
};
use svrctlrs_database::queries::{persistent_shells, user_settings};
use tracing::{instrument, warn};

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The `user_settings` key holding the user's keep time in seconds; not one of the free-form
/// preference documents, since the server reads it.
const KEEP_KEY: &str = "session_keep";

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_sessions))
        .routes(routes!(set_keep))
        .routes(routes!(close_session))
}

/// How long `user_id`'s shells are kept with nobody attached, and what they chose (none: the
/// server's default). An unreadable setting reads as no choice.
pub(crate) async fn keep_time(state: &AppState, user_id: i64) -> (Duration, Option<u64>) {
    let chosen = match user_settings::get(&state.pool, user_id, KEEP_KEY).await {
        Ok(row) => row.and_then(|r| r.value.trim().parse::<u64>().ok()),
        Err(e) => {
            warn!(user_id, error = %e, "Could not read the keep time; using the default");
            None
        }
    };
    (state.sessions.ttl_for(chosen), chosen)
}

/// The caller's live shells, attached or waiting to be resumed, and how long they are kept.
/// Kept shells (tmux) from before a restart are listed too, as `waiting`: attaching to one
/// reconnects it.
#[utoipa::path(get, path = "/", tag = "sessions",
    responses((status = 200, body = SessionList)))]
#[instrument(skip(state))]
async fn list_sessions(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
) -> Result<Json<SessionList>, ApiError> {
    let mut sessions: Vec<SessionSummary> = state
        .sessions
        .list_for_user(me)
        .await
        .into_iter()
        .map(|s| SessionSummary {
            id: s.id,
            server_id: s.server_id,
            server_name: s.server_name,
            created_at: s.created_at,
            attachments: s.attachments,
            latest_seq: s.latest_seq,
            recorded: s.recorded,
            persistent: s.persistent,
            waiting: false,
        })
        .collect();
    let kept = persistent_shells::list_for_user(&state.pool, me).await?;
    let mut names = std::collections::HashMap::new();
    for row in kept {
        if sessions.iter().any(|s| s.id == row.id) {
            continue;
        }
        if let std::collections::hash_map::Entry::Vacant(slot) = names.entry(row.server_id) {
            let name = svrctlrs_database::queries::servers::get_server(&state.pool, row.server_id)
                .await
                .map(|s| s.name)
                .unwrap_or_default();
            slot.insert(name);
        }
        sessions.push(SessionSummary {
            id: row.id,
            server_id: row.server_id,
            server_name: names[&row.server_id].clone(),
            created_at: row.created_at,
            attachments: 0,
            latest_seq: 0,
            recorded: false,
            persistent: true,
            waiting: true,
        });
    }
    sessions.sort_by_key(|s| s.created_at);
    let (ttl, chosen) = keep_time(&state, me).await;
    Ok(Json(SessionList {
        sessions,
        detach_ttl_secs: ttl.as_secs(),
        detach_ttl_max_secs: state.sessions.max_ttl().as_secs(),
        detach_ttl_choices: state.sessions.ttl_choices(),
        detach_ttl_chosen: chosen.is_some(),
    }))
}

/// Choose how long your shells are kept with nobody attached: one of `detach_ttl_choices`
/// from `GET /sessions`, or null for the server's default. Shells already waiting are kept
/// that long from now.
#[utoipa::path(put, path = "/keep", tag = "sessions", request_body = SessionKeepUpdate,
    responses((status = 200, body = SessionKeep), (status = 400, body = svrctlrs_api::ErrorBody)))]
#[instrument(skip(state))]
async fn set_keep(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Json(input): Json<SessionKeepUpdate>,
) -> Result<Json<SessionKeep>, ApiError> {
    match input.detach_ttl_secs {
        None => user_settings::delete(&state.pool, me, KEEP_KEY).await?,
        Some(secs) => {
            let choices = state.sessions.ttl_choices();
            if !choices.contains(&secs) {
                return Err(ApiError::bad_request(format!(
                    "detach_ttl_secs must be one of {choices:?} (or null for the default)"
                )));
            }
            user_settings::put(&state.pool, me, KEEP_KEY, &secs.to_string()).await?;
        }
    }
    let (ttl, chosen) = keep_time(&state, me).await;
    state.sessions.retime_detached(me, ttl).await;
    Ok(Json(SessionKeep {
        detach_ttl_secs: ttl.as_secs(),
        detach_ttl_chosen: chosen.is_some(),
    }))
}

/// End the shell behind a session, attached or not (a kept shell waiting after a restart
/// too: its tmux session is ended on the server).
#[utoipa::path(delete, path = "/{id}", tag = "sessions",
    params(("id" = String, Path, description = "Session id")),
    responses(
        (status = 200, body = SessionClosed),
        (status = 404, body = svrctlrs_api::ErrorBody),
    ))]
#[instrument(skip(state))]
async fn close_session(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<SessionClosed>, ApiError> {
    if !state.sessions.close_session(&id, me).await {
        if persistent_shells::get_for_user(&state.pool, &id, me)
            .await?
            .is_none()
        {
            return Err(ApiError::not_found("Session"));
        }
        crate::persistent_shells::discard(&state, &id).await;
    }
    Ok(Json(SessionClosed {
        id,
        message: "Session closed".to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::SessionRegistry;

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

    #[tokio::test]
    async fn the_keep_time_is_the_users_choice_capped_or_the_default() {
        let (mut st, _dir) = state().await;
        st.sessions = SessionRegistry::new(Duration::from_secs(900), Duration::from_secs(3600), 16);
        assert_eq!(keep_time(&st, 1).await, (Duration::from_secs(900), None));

        // A choice above the maximum is refused; one within it sticks
        let refused = set_keep(
            State(st.clone()),
            CurrentUser(1),
            Json(SessionKeepUpdate {
                detach_ttl_secs: Some(28800),
            }),
        )
        .await;
        assert!(refused.is_err());
        let Json(kept) = set_keep(
            State(st.clone()),
            CurrentUser(1),
            Json(SessionKeepUpdate {
                detach_ttl_secs: Some(3600),
            }),
        )
        .await
        .unwrap();
        assert_eq!(kept.detach_ttl_secs, 3600);
        assert!(kept.detach_ttl_chosen);
        assert_eq!(keep_time(&st, 1).await.0, Duration::from_secs(3600));

        // A lower maximum later caps a choice already saved
        st.sessions = SessionRegistry::new(Duration::from_secs(900), Duration::from_secs(1800), 16);
        assert_eq!(keep_time(&st, 1).await.0, Duration::from_secs(1800));

        // Null goes back to the default
        let Json(back) = set_keep(
            State(st.clone()),
            CurrentUser(1),
            Json(SessionKeepUpdate {
                detach_ttl_secs: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(back.detach_ttl_secs, 900);
        assert!(!back.detach_ttl_chosen);
    }

    #[tokio::test]
    async fn the_list_shows_kept_shells_waiting_after_a_restart_and_only_ones_own() {
        let (st, _dir) = state().await;
        let server = crate::test_support::add_server(&st.pool, "web", 22, "u", None, None).await;
        persistent_shells::insert(&st.pool, "kept-1", 1, server, "svrctlrs-kept-1")
            .await
            .unwrap();
        let Json(list) = list_sessions(State(st.clone()), CurrentUser(1))
            .await
            .unwrap();
        assert_eq!(list.sessions.len(), 1);
        let s = &list.sessions[0];
        assert_eq!(s.id, "kept-1");
        assert_eq!(s.server_name, "web");
        assert!(s.persistent && s.waiting);
        assert_eq!(list.detach_ttl_choices, st.sessions.ttl_choices());

        // Someone else's kept shell is neither listed nor closable
        let Json(theirs) = list_sessions(State(st.clone()), CurrentUser(2))
            .await
            .unwrap();
        assert!(theirs.sessions.is_empty());
        assert!(
            close_session(State(st.clone()), CurrentUser(2), Path("kept-1".into()))
                .await
                .is_err()
        );
    }
}
