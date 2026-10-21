//! The whole application (`app::router`) on a temporary database with one signed-in user,
//! for tests that go through the real auth layer: tokens, approvals.

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::json;
use std::time::Duration;
use svrctlrs_database::models::CreateUser;
use tower::ServiceExt;
use tower_sessions_sqlx_store::SqliteStore;

use crate::state::AppState;

pub struct AppHarness {
    pub app: Router,
    pub state: AppState,
    /// The owner's (ann, id 1) session id, sent as a session Bearer token
    pub session: String,
    _dir: tempfile::TempDir,
}

impl AppHarness {
    pub async fn new() -> Self {
        Self::with_sign_in(crate::sign_in::SignInConfig::default()).await
    }

    /// The app with these sign-in settings. With password sign-in off, the owner's session is
    /// made in the store directly (as a provider sign-in would leave it).
    pub async fn with_sign_in(sign_in: crate::sign_in::SignInConfig) -> Self {
        super::ensure_encryption();
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite:{}", dir.path().join("t.db").display());
        let database = svrctlrs_database::Database::new(&url).await.unwrap();
        database.migrate().await.unwrap();
        let mut state = AppState::new(database, vec![]).await.unwrap();
        state.backups = crate::backups::Backups::new(dir.path().join("backups"));
        let password_login = sign_in.password_login;
        state.sign_in = std::sync::Arc::new(sign_in);
        let store = SqliteStore::new(state.pool.clone());
        store.migrate().await.unwrap();
        svrctlrs_database::queries::users::create_user(
            &state.pool,
            &CreateUser {
                username: "ann".into(),
                password: "correct horse battery".into(),
            },
        )
        .await
        .unwrap();
        let app = crate::app::router(state.clone(), store.clone(), false);
        let session = if password_login {
            let (status, body) = call(
                &app,
                "POST",
                "/api/v1/auth/login",
                None,
                Some(json!({ "username": "ann", "password": "correct horse battery" })),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            body["session_id"].as_str().unwrap().to_string()
        } else {
            use tower_sessions::SessionStore;
            let mut record = tower_sessions::session::Record {
                id: tower_sessions::session::Id::default(),
                data: std::collections::HashMap::from([
                    ("user_id".to_string(), json!(1)),
                    ("auth_method".to_string(), json!("oidc")),
                ]),
                expiry_date: time::OffsetDateTime::now_utc() + time::Duration::hours(1),
            };
            store.create(&mut record).await.unwrap();
            record.id.to_string()
        };
        Self {
            app,
            state,
            session,
            _dir: dir,
        }
    }

    /// A call as the signed-in owner
    pub async fn owner(
        &self,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        call(&self.app, method, uri, Some(&self.session), body).await
    }

    /// A call with a token (or any Bearer value)
    pub async fn with(
        &self,
        token: &str,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        call(&self.app, method, uri, Some(token), body).await
    }

    /// A new never-expiring token without approvals: (id, secret)
    pub async fn token(&self, name: &str, preset: &str) -> (i64, String) {
        self.token_with(name, preset, false).await
    }

    pub async fn token_with(
        &self,
        name: &str,
        preset: &str,
        require_approval: bool,
    ) -> (i64, String) {
        let (status, body) = self
            .owner(
                "POST",
                "/api/v1/tokens",
                Some(json!({
                    "name": name,
                    "preset": preset,
                    "expires_in_days": null,
                    "require_approval": require_approval,
                })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        (
            body["token"]["id"].as_i64().unwrap(),
            body["secret"].as_str().unwrap().to_string(),
        )
    }
}

/// One request through `app`; the body parsed as JSON (Null when it is not).
pub async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let request = request
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// Poll for something a request started without waiting (audit rows, notifications).
pub async fn eventually<F, Fut>(mut check: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..150 {
        if check().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}
