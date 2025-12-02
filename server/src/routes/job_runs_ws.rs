//! WebSocket handler for job run updates
//!
//! Provides real-time updates to the job runs page via WebSocket,
//! replacing the previous polling mechanism for improved efficiency.

use askama::Template;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::any,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use svrctlrs_database::queries::job_runs as queries;
use tracing::{debug, error, info, warn};

use crate::{
    state::{AppState, JobRunUpdate},
    templates::JobRunListTemplate,
};

/// WebSocket message sent to clients
#[derive(Debug, Serialize)]
struct WsMessage {
    #[serde(rename = "type")]
    msg_type: String,
    /// HTML content to swap into the DOM (for list updates)
    html: Option<String>,
    /// Job run ID (for individual updates)
    job_run_id: Option<i64>,
    /// Status string (for individual updates)
    status: Option<String>,
}

/// WebSocket message received from clients
#[derive(Debug, Deserialize)]
struct WsRequest {
    #[serde(rename = "type")]
    request_type: String,
    /// Current page number for pagination
    page: Option<usize>,
    /// Items per page
    per_page: Option<usize>,
}

/// Create job runs WebSocket router
pub fn routes() -> Router<AppState> {
    Router::new().route("/ws/job-runs", any(job_runs_ws_handler))
}

/// WebSocket upgrade handler for job runs
async fn job_runs_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("Job runs WebSocket connection request");
    ws.on_upgrade(move |socket| handle_job_runs_socket(socket, state))
}

/// Handle the job runs WebSocket connection
async fn handle_job_runs_socket(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    info!("Job runs WebSocket connected");

    // Subscribe to job run updates
    let mut rx = state.subscribe_job_run_updates();

    // Default pagination
    let mut current_page: usize = 1;
    let mut per_page: usize = 50;

    // Send initial list
    if let Ok(html) = render_job_run_list(&state, current_page, per_page).await {
        let msg = WsMessage {
            msg_type: "list".to_string(),
            html: Some(html),
            job_run_id: None,
            status: None,
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                return;
            }
        }
    }

    loop {
        tokio::select! {
            // Handle broadcast updates from the server
            update = rx.recv() => {
                match update {
                    Ok(JobRunUpdate::StatusChanged { job_run_id, status }) => {
                        debug!(job_run_id, status = %status, "Broadcasting status change");
                        let msg = WsMessage {
                            msg_type: "status_changed".to_string(),
                            html: None,
                            job_run_id: Some(job_run_id),
                            status: Some(status),
                        };
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(JobRunUpdate::Created { job_run_id }) => {
                        debug!(job_run_id, "Broadcasting new job run");
                        // Send full list refresh for new job runs
                        if let Ok(html) = render_job_run_list(&state, current_page, per_page).await {
                            let msg = WsMessage {
                                msg_type: "list".to_string(),
                                html: Some(html),
                                job_run_id: Some(job_run_id),
                                status: None,
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(JobRunUpdate::RefreshAll) => {
                        debug!("Broadcasting full refresh");
                        if let Ok(html) = render_job_run_list(&state, current_page, per_page).await {
                            let msg = WsMessage {
                                msg_type: "list".to_string(),
                                html: Some(html),
                                job_run_id: None,
                                status: None,
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        warn!("WebSocket client lagged behind by {} messages", count);
                        // Send full refresh to catch up
                        if let Ok(html) = render_job_run_list(&state, current_page, per_page).await {
                            let msg = WsMessage {
                                msg_type: "list".to_string(),
                                html: Some(html),
                                job_run_id: None,
                                status: None,
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        warn!("Broadcast channel closed");
                        break;
                    }
                }
            }

            // Handle WebSocket messages from the client
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(req) = serde_json::from_str::<WsRequest>(&text) {
                            match req.request_type.as_str() {
                                "refresh" => {
                                    // Client requested a refresh
                                    if let Some(page) = req.page {
                                        current_page = page.max(1);
                                    }
                                    if let Some(pp) = req.per_page {
                                        per_page = pp.clamp(10, 100);
                                    }

                                    if let Ok(html) = render_job_run_list(&state, current_page, per_page).await {
                                        let msg = WsMessage {
                                            msg_type: "list".to_string(),
                                            html: Some(html),
                                            job_run_id: None,
                                            status: None,
                                        };
                                        if let Ok(json) = serde_json::to_string(&msg) {
                                            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                                "page" => {
                                    // Client changed page
                                    if let Some(page) = req.page {
                                        current_page = page.max(1);
                                    }
                                    if let Some(pp) = req.per_page {
                                        per_page = pp.clamp(10, 100);
                                    }

                                    if let Ok(html) = render_job_run_list(&state, current_page, per_page).await {
                                        let msg = WsMessage {
                                            msg_type: "list".to_string(),
                                            html: Some(html),
                                            job_run_id: None,
                                            status: None,
                                        };
                                        if let Ok(json) = serde_json::to_string(&msg) {
                                            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                                "ping" => {
                                    let pong = serde_json::json!({"type": "pong"});
                                    if let Ok(json) = serde_json::to_string(&pong) {
                                        if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                                _ => {
                                    debug!("Unknown WebSocket request type: {}", req.request_type);
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("Job runs WebSocket closed by client");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if ws_sender.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        error!("Job runs WebSocket error: {}", e);
                        break;
                    }
                    None => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    info!("Job runs WebSocket connection ended");
}

/// Render the job run list HTML
async fn render_job_run_list(
    state: &AppState,
    page: usize,
    per_page: usize,
) -> Result<String, String> {
    let offset = ((page.max(1) - 1) * per_page) as i64;

    // Get total count for pagination
    let total_count = queries::count_job_runs(&state.pool)
        .await
        .map_err(|e| format!("Failed to count job runs: {}", e))? as usize;

    let total_pages = total_count.div_ceil(per_page);

    // Get job runs with names
    let job_runs = queries::list_job_runs_with_names(&state.pool, per_page as i64, offset)
        .await
        .map_err(|e| format!("Failed to fetch job runs: {}", e))?;

    let template = JobRunListTemplate {
        job_runs: job_runs.into_iter().map(Into::into).collect(),
        current_page: page.max(1),
        total_pages: total_pages.max(1),
        per_page,
    };

    template
        .render()
        .map_err(|e| format!("Failed to render template: {}", e))
}
