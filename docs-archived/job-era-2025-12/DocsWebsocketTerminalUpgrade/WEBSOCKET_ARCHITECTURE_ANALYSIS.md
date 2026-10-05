# WebSocket Architecture Analysis for SvrCtlRS

**Date**: 2024-12-01
**Context**: Evaluating WebSocket addition to existing HTMX/Axum application

## Executive Summary

Adding WebSocket capability to the current application is **low-risk and non-disruptive**. WebSockets can coexist with the existing HTMX architecture without requiring migration of current functionality. Selective use of WebSockets for real-time features (terminal, job status) while maintaining HTMX for CRUD operations is the recommended hybrid approach.

## Current Architecture Overview

### Existing Communication Patterns

```
┌─────────────┐
│   Browser   │
│  (HTMX)     │
└──────┬──────┘
       │ HTTP Requests (GET/POST/DELETE)
       │ - Form submissions
       │ - Navigation
       │ - Polling (every 5s for auto-refresh)
       ▼
┌─────────────┐
│  Axum 0.8   │
│   Server    │
└─────────────┘
       │
       ▼
   ┌────────┐
   │ SQLite │
   └────────┘
```

**Strengths**:
- Simple, stateless request/response model
- Easy to reason about and debug
- Works with standard HTTP caching
- No persistent connections to manage

**Current Limitations**:
- Polling for updates wastes bandwidth
- No true real-time updates
- Cannot push server-initiated events
- Interactive features (like terminal) not possible

## Adding WebSocket Support

### Minimal Integration Example

```rust
// In server/src/main.rs or routes module
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};

async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket))
}

async fn handle_socket(mut socket: WebSocket) {
    // WebSocket logic here
}

// Add to router alongside existing routes
let app = Router::new()
    // Existing HTMX routes
    .route("/", get(index))
    .route("/servers", get(servers_list))
    .route("/servers", post(create_server))
    // NEW: WebSocket routes (don't interfere with above)
    .route("/ws/terminal", get(websocket_handler))
    .route("/ws/job-status", get(job_status_ws_handler))
    .with_state(state);
```

### Key Points:

1. **No Interference**: WebSocket routes are just additional endpoints
2. **Same Server**: Runs on same port (8080), no separate WebSocket server needed
3. **Shared State**: WebSockets access same `AppState` as HTTP handlers
4. **Independent**: Existing HTMX routes unchanged

## Architectural Implications

### 1. Server Configuration

**No Additional Complexity**:
```rust
// BEFORE (current)
let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
axum::serve(listener, app).await?;

// AFTER (with WebSockets) - IDENTICAL
let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
axum::serve(listener, app).await?;  // No change needed!
```

Axum's HTTP server (built on Hyper) natively supports WebSocket upgrades via the `Upgrade` header. No separate configuration required.

### 2. Nginx/Reverse Proxy Considerations

**Minor Configuration Addition**:
```nginx
# Existing proxy config remains unchanged
location / {
    proxy_pass http://localhost:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
}

# Add WebSocket-specific config for WS routes
location /ws/ {
    proxy_pass http://localhost:8080;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_read_timeout 86400;  # 24 hours for long-lived connections
}
```

**Note**: If not using a reverse proxy, no changes needed.

### 3. Resource Management

**Connection Limits**:
```rust
// Recommended: Track active WebSocket connections
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    // Existing state
    db: DatabasePool,

    // NEW: WebSocket connection tracking
    ws_connections: Arc<RwLock<HashMap<String, WebSocketConnection>>>,
    max_ws_connections_per_user: usize,
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, StatusCode> {
    let connections = state.ws_connections.read().await;

    // Limit connections per user
    let user_ws_count = connections.values()
        .filter(|c| c.user_id == session.user_id)
        .count();

    if user_ws_count >= state.max_ws_connections_per_user {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    Ok(ws.on_upgrade(|socket| handle_socket(socket, state, session)))
}
```

**Memory Implications**:
- Each WebSocket: ~4-8KB overhead (buffering)
- 100 concurrent connections: ~400-800KB
- Negligible compared to typical server RAM

### 4. Docker Deployment

**No Changes Required**:
```dockerfile
# Existing Dockerfile works as-is
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/server /usr/local/bin/
EXPOSE 8080
CMD ["server"]
```

Same port, same configuration, no additional exposure needed.

## Migration Decision Matrix

### What Should Use WebSockets?

| Feature | Current Method | Should Migrate? | Reason |
|---------|---------------|-----------------|---------|
| **Terminal emulation** | N/A (new) | ✅ WebSocket | Real-time bidirectional required |
| **Job status updates** | HTMX polling (5s) | ✅ WebSocket | Server-push more efficient |
| **Live logs** | N/A (new) | ✅ WebSocket | Streaming data |
| **Form submissions** | HTMX POST | ❌ Keep HTTP | Stateless, one-shot operations |
| **Navigation** | HTMX GET | ❌ Keep HTTP | Caching benefits, simple |
| **List displays** | HTMX GET | ❌ Keep HTTP | Works well with current approach |
| **Auto-refresh lists** | HTMX polling | ⚠️ Optional | WebSocket better, but polling acceptable |
| **CRUD operations** | HTMX POST/DELETE | ❌ Keep HTTP | RESTful pattern preferred |

### Recommendation: **Hybrid Approach**

**Use WebSockets for**:
1. Terminal sessions (new feature)
2. Real-time job execution status
3. Live log tailing (future feature)
4. System metrics dashboard (future feature)
5. Notification streaming (future feature)

**Keep HTMX/HTTP for**:
1. All CRUD operations (servers, templates, schedules)
2. Form submissions
3. Page navigation
4. Static content delivery
5. One-time data fetches

## HTMX + WebSocket Integration Patterns

### Pattern 1: HTMX for UI, WebSocket for Data

```html
<!-- HTMX handles UI updates -->
<div id="job-status-{{ job.id }}"
     hx-get="/jobs/{{ job.id }}/status"
     hx-trigger="ws-update from:body">
    Status: {{ job.status }}
</div>

<script>
// WebSocket pushes events, triggers HTMX refresh
const ws = new WebSocket('ws://localhost:8080/ws/job-updates');
ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    // Trigger HTMX to refresh specific element
    htmx.trigger(`#job-status-${data.job_id}`, 'ws-update');
};
</script>
```

**Pros**: Leverages HTMX for rendering, WebSocket for notifications
**Cons**: Slightly complex interaction

### Pattern 2: WebSocket Only for Specialized Components

```html
<!-- Regular HTMX components -->
<div hx-get="/servers" hx-trigger="every 5s">...</div>

<!-- WebSocket-only for terminal -->
<div id="terminal" data-ws-endpoint="/ws/terminal"></div>
<script src="/static/js/terminal.js"></script>
```

**Pros**: Clear separation of concerns
**Cons**: None
**Recommendation**: ⭐ **Use this pattern**

### Pattern 3: Replace Polling with WebSocket Push

**Before (HTMX polling)**:
```html
<div id="job-list"
     hx-get="/jobs/running"
     hx-trigger="every 5s"
     hx-swap="innerHTML">
    <!-- Job list here -->
</div>
```

**After (WebSocket push + HTMX render)**:
```html
<div id="job-list"
     hx-get="/jobs/running"
     hx-trigger="job-update from:body"
     hx-swap="innerHTML">
    <!-- Job list here -->
</div>

<script>
const jobWs = new WebSocket('ws://localhost:8080/ws/jobs');
jobWs.onmessage = () => {
    htmx.trigger('#job-list', 'job-update');
};
</script>
```

**Pros**: More efficient than polling
**Cons**: Adds WebSocket management complexity
**Recommendation**: ⚠️ **Optional** - only if polling becomes a performance issue

## HTMX Native WebSocket Support

HTMX has built-in WebSocket support via `hx-ws`:

```html
<!-- HTMX can handle WebSockets natively -->
<div hx-ws="connect:/ws/jobs">
    <div hx-ws="send">
        <button>Refresh Jobs</button>
    </div>

    <!-- Messages from WebSocket render here -->
    <div id="job-list"></div>
</div>
```

**When to use**:
- Simple request/response over WebSocket
- HTMX-driven WebSocket interactions

**When NOT to use**:
- Terminal emulation (need full control)
- Binary data
- Complex bidirectional protocols

## Performance Implications

### Bandwidth Comparison

**Current (HTMX Polling)**:
```
Request every 5 seconds:
- Request headers: ~500 bytes
- Response headers: ~300 bytes
- Response body: ~2KB (job list)
= 2.8KB × 12 per minute = 33.6KB/min per user
```

**WebSocket (Push Updates)**:
```
Initial handshake: ~500 bytes (one-time)
Updates only when changes occur:
- Update message: ~100 bytes per event
= If 3 updates/min: 300 bytes/min per user
```

**Savings**: ~99% reduction in bandwidth for real-time updates

### CPU Implications

**HTTP Polling**:
- New connection per request
- TLS handshake (if HTTPS)
- Full request parsing
- Response serialization

**WebSocket**:
- One connection maintained
- Minimal frame parsing
- Direct message push

**Impact**: Negligible CPU difference for small user counts (<100 concurrent). Both are efficient.

## Security Considerations

### 1. Authentication

```rust
async fn terminal_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    // Reuse existing session middleware
    Extension(user): Extension<User>,
) -> impl IntoResponse {
    // User already authenticated by middleware
    ws.on_upgrade(|socket| handle_terminal(socket, state, user))
}
```

**Key Point**: Use same authentication as HTTP routes (sessions, JWT, etc.)

### 2. Authorization

```rust
async fn handle_terminal(socket: WebSocket, state: AppState, user: User) {
    // Check user has permission to access this server
    let msg = socket.recv().await;
    if let Some(Ok(Message::Text(json))) = msg {
        let cmd: TerminalCommand = serde_json::from_str(&json)?;

        // IMPORTANT: Verify user can access this server
        if !state.db.user_can_access_server(user.id, cmd.server_id).await? {
            socket.send(Message::Close(None)).await.ok();
            return;
        }

        // Proceed with command execution
    }
}
```

### 3. Rate Limiting

```rust
// Limit WebSocket messages per connection
use governor::{Quota, RateLimiter};

async fn handle_terminal(socket: WebSocket, state: AppState, user: User) {
    let limiter = RateLimiter::direct(Quota::per_second(
        nonzero!(10u32)  // Max 10 commands/second
    ));

    while let Some(msg) = socket.recv().await {
        if limiter.check().is_err() {
            socket.send(Message::Text("Rate limit exceeded")).await.ok();
            continue;
        }

        // Process message
    }
}
```

## Operational Considerations

### 1. Monitoring

```rust
// Track WebSocket metrics
struct WebSocketMetrics {
    active_connections: AtomicUsize,
    total_messages_sent: AtomicU64,
    total_messages_received: AtomicU64,
    connection_errors: AtomicU64,
}

// Expose via metrics endpoint
async fn metrics(State(state): State<AppState>) -> String {
    let ws_metrics = &state.ws_metrics;
    format!(
        "ws_active_connections {}\n\
         ws_messages_sent_total {}\n\
         ws_messages_received_total {}\n\
         ws_connection_errors_total {}\n",
        ws_metrics.active_connections.load(Ordering::Relaxed),
        ws_metrics.total_messages_sent.load(Ordering::Relaxed),
        ws_metrics.total_messages_received.load(Ordering::Relaxed),
        ws_metrics.connection_errors.load(Ordering::Relaxed),
    )
}
```

### 2. Graceful Shutdown

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let app = create_app().await?;
    let listener = TcpListener::bind("0.0.0.0:8080").await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    // Wait for SIGTERM
    tokio::signal::ctrl_c().await.ok();

    // Give WebSocket connections time to close gracefully
    info!("Shutting down, waiting for WebSocket connections to close...");
    tokio::time::sleep(Duration::from_secs(5)).await;
}
```

WebSocket connections will receive close frames and can clean up properly.

### 3. Health Checks

```rust
async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let ws_count = state.ws_connections.read().await.len();

    if ws_count > state.max_total_ws_connections {
        return (StatusCode::SERVICE_UNAVAILABLE, "Too many WebSocket connections");
    }

    (StatusCode::OK, "healthy")
}
```

## Migration Strategy

### Phase 1: Add WebSocket Infrastructure (Week 1)

1. Add WebSocket dependencies to `Cargo.toml`
2. Create WebSocket handler structure
3. Add connection tracking to `AppState`
4. Implement authentication/authorization
5. Add monitoring metrics

**No user-facing changes yet**

### Phase 2: Terminal Feature (Week 2)

1. Implement terminal WebSocket endpoint
2. Add xterm.js to frontend
3. Create terminal modal component
4. Test with command execution

**First WebSocket feature live**

### Phase 3: Optional Enhancements (Week 3+)

1. Job status WebSocket updates (replace polling)
2. Live log streaming
3. Real-time notifications

**Progressive enhancement**

## Recommendation

### ✅ **Proceed with WebSocket Addition**

**Reasons**:
1. **Low Risk**: Adds capability without changing existing functionality
2. **No Configuration Overhead**: Works with current Axum setup
3. **Selective Use**: Use where it makes sense (terminal, real-time updates)
4. **Keep HTMX**: Maintain HTMX for CRUD operations (plays to its strengths)
5. **Future-Proof**: Enables real-time features as application grows

### 🎯 **Hybrid Architecture (Recommended)**

```
┌─────────────────────────────────────────┐
│           Browser                       │
├─────────────────┬───────────────────────┤
│  HTMX           │  WebSocket           │
│  - Forms        │  - Terminal          │
│  - Navigation   │  - Job status        │
│  - CRUD ops     │  - Live logs         │
└────────┬────────┴──────────┬────────────┘
         │                   │
         ▼                   ▼
    ┌────────────────────────────┐
    │    Axum Server (8080)      │
    │  - HTTP Routes             │
    │  - WebSocket Routes        │
    └────────────────────────────┘
```

**Benefits**:
- Use right tool for each job
- Gradual migration path
- Low complexity
- Maximum flexibility

## Conclusion

**Adding WebSocket support does NOT**:
- Require changing existing HTTP routes
- Complicate server configuration
- Interfere with HTMX functionality
- Require separate infrastructure

**Adding WebSocket support DOES**:
- Enable real-time features (terminal, live updates)
- Reduce bandwidth for status monitoring
- Improve user experience for interactive features
- Provide foundation for future enhancements

**Bottom Line**: Add WebSockets for terminal feature, keep HTMX for everything else. Best of both worlds with minimal complexity.

---

**Recommendation**: ✅ **Approve WebSocket addition**
**Migration Scope**: Selective (terminal + optional job status)
**Risk Level**: Low
**Complexity**: Low (well-integrated with Axum)
