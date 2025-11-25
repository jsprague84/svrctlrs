# HTMX + Askama Migration Plan

## Why HTMX + Askama?

**Perfect for SvrCtlRS because:**
- ✅ 100% Rust - No TypeScript, no npm, no build tools
- ✅ Interactive forms - HTMX handles AJAX submissions
- ✅ Real-time updates - HTMX polling/SSE for monitoring
- ✅ Tiny bundle - ~14KB vs ~200KB+ (React)
- ✅ Simple deployment - Single binary
- ✅ No WASM issues - Pure HTML/CSS/JS
- ✅ Perfect for CRUD apps - Which is what you're building

**vs React:**
- React: Two languages, duplicate types, npm, webpack, larger bundle
- HTMX: One language, one build tool, simpler everything

## Phase 1: Setup Dependencies (30 minutes)

### 1. Update `server/Cargo.toml`

```toml
[dependencies]
# Remove ALL Dioxus dependencies
# dioxus = ...
# dioxus-router = ...
# dioxus-fullstack = ...
# dioxus-ssr = ...
# dioxus-server = ...
# dioxus-cli-config = ...

# Add templating and HTMX support
askama = "0.12"
askama_axum = "0.4"

# Keep existing Axum dependencies (already have these)
axum = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }
tokio = { workspace = true }

# Add session management for auth
tower-sessions = "0.13"
tower-sessions-sqlx-store = { version = "0.13", features = ["sqlite"] }

# Keep all your existing dependencies
svrctlrs-core = { workspace = true }
svrctlrs-scheduler = { workspace = true }
# ... etc
```

### 2. Create templates directory

```bash
mkdir -p server/templates
mkdir -p server/templates/components
mkdir -p server/templates/pages
```

### 3. Add static assets

```bash
mkdir -p server/static/css
mkdir -p server/static/js
```

Download HTMX (14KB):
```bash
curl -o server/static/js/htmx.min.js https://unpkg.com/htmx.org@2.0.3/dist/htmx.min.js
```

Optional - Alpine.js for client-side interactions (15KB):
```bash
curl -o server/static/js/alpine.min.js https://unpkg.com/alpinejs@3.14.1/dist/cdn.min.js
```

## Phase 2: Base Templates (1 hour)

### `server/templates/base.html`

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}SvrCtlRS{% endblock %}</title>
    
    <!-- Your existing CSS or new styles -->
    <link rel="stylesheet" href="/static/css/styles.css">
    
    <!-- HTMX -->
    <script src="/static/js/htmx.min.js"></script>
    
    <!-- Optional: Alpine.js for dropdowns, modals, etc. -->
    <script defer src="/static/js/alpine.min.js"></script>
    
    {% block head %}{% endblock %}
</head>
<body>
    <nav>
        <a href="/">Dashboard</a>
        <a href="/servers">Servers</a>
        <a href="/tasks">Tasks</a>
        <a href="/plugins">Plugins</a>
        {% if user %}
        <a href="/auth/logout">Logout ({{ user.username }})</a>
        {% else %}
        <a href="/auth/login">Login</a>
        {% endif %}
    </nav>
    
    <main>
        {% block content %}{% endblock %}
    </main>
    
    <!-- Toast notifications -->
    <div id="toast" class="toast"></div>
</body>
</html>
```

### `server/src/templates.rs`

```rust
use askama::Template;
use serde::{Deserialize, Serialize};

// Base template with user context
#[derive(Template)]
#[template(path = "base.html")]
pub struct BaseTemplate {
    pub user: Option<User>,
}

// Dashboard
#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardTemplate {
    pub user: Option<User>,
    pub stats: DashboardStats,
}

// Server list
#[derive(Template)]
#[template(path = "pages/servers.html")]
pub struct ServersTemplate {
    pub user: Option<User>,
    pub servers: Vec<Server>,
}

// Server form (partial for HTMX)
#[derive(Template)]
#[template(path = "components/server_form.html")]
pub struct ServerFormTemplate {
    pub server: Option<Server>, // None = create, Some = edit
    pub error: Option<String>,
}

// Server list (partial for HTMX)
#[derive(Template)]
#[template(path = "components/server_list.html")]
pub struct ServerListTemplate {
    pub servers: Vec<Server>,
}

#[derive(Serialize, Deserialize)]
pub struct Server {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub ssh_key: Option<String>,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
}

#[derive(Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_servers: usize,
    pub active_tasks: usize,
    pub enabled_plugins: usize,
}
```

## Phase 3: Routes (2 hours)

### `server/src/routes/ui.rs`

```rust
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post, delete, put},
    Form, Router,
};
use askama::Template;
use tower_sessions::Session;

use crate::{AppState, templates::*};

pub fn ui_routes() -> Router<AppState> {
    Router::new()
        // Pages
        .route("/", get(dashboard_page))
        .route("/servers", get(servers_page))
        .route("/tasks", get(tasks_page))
        .route("/plugins", get(plugins_page))
        
        // Server CRUD (HTMX endpoints)
        .route("/servers/new", get(server_form).post(create_server))
        .route("/servers/:id", get(server_detail).put(update_server).delete(delete_server))
        .route("/servers/:id/edit", get(edit_server_form))
        
        // Auth
        .route("/auth/login", get(login_page).post(login))
        .route("/auth/logout", post(logout))
        
        // Static files
        .nest_service("/static", tower_http::services::ServeDir::new("server/static"))
}

// Dashboard
async fn dashboard_page(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let user = get_user_from_session(&session).await?;
    
    let stats = DashboardStats {
        total_servers: state.database.count_servers().await?,
        active_tasks: state.scheduler.active_tasks_count().await,
        enabled_plugins: state.plugins.read().await.len(),
    };
    
    let template = DashboardTemplate { user, stats };
    Ok(Html(template.render()?))
}

// Servers page
async fn servers_page(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let user = get_user_from_session(&session).await?;
    let servers = state.database.get_all_servers().await?;
    
    let template = ServersTemplate { user, servers };
    Ok(Html(template.render()?))
}

// Server form (HTMX partial)
async fn server_form() -> Result<Html<String>, AppError> {
    let template = ServerFormTemplate { server: None, error: None };
    Ok(Html(template.render()?))
}

// Create server (HTMX endpoint)
async fn create_server(
    State(state): State<AppState>,
    Form(input): Form<CreateServerInput>,
) -> Result<Html<String>, AppError> {
    // Validate
    if input.name.is_empty() || input.host.is_empty() {
        let template = ServerFormTemplate {
            server: None,
            error: Some("Name and host are required".to_string()),
        };
        return Ok(Html(template.render()?));
    }
    
    // Create server
    state.database.create_server(input).await?;
    
    // Return updated server list (HTMX will swap this in)
    let servers = state.database.get_all_servers().await?;
    let template = ServerListTemplate { servers };
    Ok(Html(template.render()?))
}

// Delete server (HTMX endpoint)
async fn delete_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    state.database.delete_server(id).await?;
    
    // Return empty response (HTMX will remove the element)
    Ok(Html(""))
}

#[derive(Deserialize)]
pub struct CreateServerInput {
    pub name: String,
    pub host: String,
    pub ssh_key: Option<String>,
}
```

## Phase 4: Templates (3 hours)

### `server/templates/pages/servers.html`

```html
{% extends "base.html" %}

{% block title %}Servers - SvrCtlRS{% endblock %}

{% block content %}
<div class="container">
    <h1>Servers</h1>
    
    <!-- Add server button -->
    <button hx-get="/servers/new" 
            hx-target="#server-form" 
            hx-swap="innerHTML">
        Add Server
    </button>
    
    <!-- Server form (loaded via HTMX) -->
    <div id="server-form"></div>
    
    <!-- Server list -->
    <div id="server-list">
        {% include "components/server_list.html" %}
    </div>
</div>
{% endblock %}
```

### `server/templates/components/server_form.html`

```html
<div class="card">
    <h2>{% if server %}Edit{% else %}Add{% endif %} Server</h2>
    
    {% if error %}
    <div class="error">{{ error }}</div>
    {% endif %}
    
    <form {% if server %}
          hx-put="/servers/{{ server.id }}"
          {% else %}
          hx-post="/servers/new"
          {% endif %}
          hx-target="#server-list"
          hx-swap="innerHTML">
        
        <label>
            Name:
            <input type="text" name="name" 
                   value="{% if server %}{{ server.name }}{% endif %}" 
                   required>
        </label>
        
        <label>
            Host:
            <input type="text" name="host" 
                   value="{% if server %}{{ server.host }}{% endif %}" 
                   required>
        </label>
        
        <label>
            SSH Key (optional):
            <textarea name="ssh_key">{% if server %}{{ server.ssh_key|default("") }}{% endif %}</textarea>
        </label>
        
        <button type="submit">Save</button>
        <button type="button" onclick="this.closest('#server-form').innerHTML = ''">
            Cancel
        </button>
    </form>
</div>
```

### `server/templates/components/server_list.html`

```html
<div class="server-grid">
    {% for server in servers %}
    <div class="server-card" id="server-{{ server.id }}">
        <h3>{{ server.name }}</h3>
        <p>{{ server.host }}</p>
        
        <div class="actions">
            <button hx-get="/servers/{{ server.id }}/edit"
                    hx-target="#server-form"
                    hx-swap="innerHTML">
                Edit
            </button>
            
            <button hx-delete="/servers/{{ server.id }}"
                    hx-target="#server-{{ server.id }}"
                    hx-swap="outerHTML"
                    hx-confirm="Delete {{ server.name }}?">
                Delete
            </button>
        </div>
    </div>
    {% endfor %}
</div>
```

## Phase 5: Authentication (2 hours)

### `server/templates/pages/login.html`

```html
{% extends "base.html" %}

{% block title %}Login - SvrCtlRS{% endblock %}

{% block content %}
<div class="login-container">
    <h1>Login</h1>
    
    {% if error %}
    <div class="error">{{ error }}</div>
    {% endif %}
    
    <form hx-post="/auth/login" hx-target="body">
        <label>
            Username:
            <input type="text" name="username" required autofocus>
        </label>
        
        <label>
            Password:
            <input type="password" name="password" required>
        </label>
        
        <button type="submit">Login</button>
    </form>
</div>
{% endblock %}
```

### `server/src/routes/auth.rs`

```rust
use axum::{
    extract::State,
    response::{Html, Redirect},
    Form,
};
use tower_sessions::Session;
use askama::Template;

#[derive(Template)]
#[template(path = "pages/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

pub async fn login_page() -> Html<String> {
    let template = LoginTemplate { error: None };
    Html(template.render().unwrap())
}

pub async fn login(
    State(state): State<AppState>,
    session: Session,
    Form(creds): Form<LoginForm>,
) -> Result<Redirect, Html<String>> {
    // Authenticate user
    match state.database.authenticate(&creds.username, &creds.password).await {
        Ok(user) => {
            // Store user ID in session
            session.insert("user_id", user.id).await.unwrap();
            Ok(Redirect::to("/"))
        }
        Err(_) => {
            let template = LoginTemplate {
                error: Some("Invalid username or password".to_string()),
            };
            Err(Html(template.render().unwrap()))
        }
    }
}

pub async fn logout(session: Session) -> Redirect {
    session.delete().await.unwrap();
    Redirect::to("/auth/login")
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}
```

## Phase 6: Real-Time Updates (1 hour)

### HTMX Polling for Task Status

```html
<!-- templates/components/task_status.html -->
<div hx-get="/tasks/{{ task.id }}/status" 
     hx-trigger="every 2s" 
     hx-swap="outerHTML">
    <div class="task-status">
        <span class="status-{{ task.status }}">{{ task.status }}</span>
        <span>{{ task.progress }}%</span>
    </div>
</div>
```

### Server-Sent Events (SSE) for Live Logs

```rust
// routes/tasks.rs
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;

pub async fn task_logs_stream(
    State(state): State<AppState>,
    Path(task_id): Path<i64>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = state.scheduler.subscribe_to_task_logs(task_id);
    
    Sse::new(stream.map(|log| {
        Ok(Event::default().data(log))
    }))
}
```

```html
<!-- templates/pages/task_detail.html -->
<div hx-ext="sse" sse-connect="/tasks/{{ task.id }}/logs">
    <div sse-swap="message" hx-swap="beforeend">
        <!-- Logs will be appended here -->
    </div>
</div>
```

## Phase 7: Update main.rs (30 minutes)

```rust
// server/src/main.rs
use axum::{Router, middleware};
use tower_http::services::ServeDir;
use tower_sessions::{SessionManagerLayer, SqliteStore};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... (existing setup)
    
    // Session store for auth
    let session_store = SqliteStore::new(state.database.pool.clone());
    session_store.migrate().await?;
    
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false); // Set true in production with HTTPS
    
    // Build router
    let app = Router::new()
        // API routes (keep existing)
        .nest("/api", routes::api_routes(state.clone()))
        
        // UI routes (new HTMX routes)
        .merge(routes::ui::ui_routes())
        
        // Middleware
        .layer(session_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state);
    
    // Start server
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    info!(addr = %args.addr, "Server listening");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}
```

## Phase 8: Styling (2 hours)

### `server/static/css/styles.css`

```css
/* Modern, clean styles */
:root {
    --primary: #3b82f6;
    --danger: #ef4444;
    --success: #10b981;
    --bg: #ffffff;
    --surface: #f9fafb;
    --text: #111827;
    --border: #e5e7eb;
}

* {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
}

body {
    font-family: system-ui, -apple-system, sans-serif;
    background: var(--bg);
    color: var(--text);
}

nav {
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    padding: 1rem 2rem;
    display: flex;
    gap: 2rem;
}

nav a {
    color: var(--text);
    text-decoration: none;
    font-weight: 500;
}

nav a:hover {
    color: var(--primary);
}

main {
    max-width: 1200px;
    margin: 2rem auto;
    padding: 0 2rem;
}

.card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1.5rem;
    margin-bottom: 1rem;
}

button {
    background: var(--primary);
    color: white;
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
}

button:hover {
    opacity: 0.9;
}

input, textarea {
    width: 100%;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    font-family: inherit;
}

.server-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 1rem;
}

.error {
    background: #fee2e2;
    color: #991b1b;
    padding: 1rem;
    border-radius: 4px;
    margin-bottom: 1rem;
}

/* HTMX loading states */
.htmx-request {
    opacity: 0.5;
    pointer-events: none;
}
```

## Timeline Summary

| Phase | Time | Description |
|-------|------|-------------|
| 1. Setup | 30 min | Update Cargo.toml, download HTMX |
| 2. Base templates | 1 hour | Create base.html, templates.rs |
| 3. Routes | 2 hours | Implement UI routes |
| 4. Templates | 3 hours | Create page/component templates |
| 5. Auth | 2 hours | Login/logout with sessions |
| 6. Real-time | 1 hour | Polling and SSE |
| 7. main.rs | 30 min | Update main entry point |
| 8. Styling | 2 hours | CSS and polish |
| **Total** | **12 hours** | **~1.5 days** |

## Benefits Over React

1. **Simpler**: One language, one build tool
2. **Faster**: No npm install, no webpack
3. **Smaller**: 14KB vs 200KB+
4. **Type-safe**: Rust templates catch errors at compile time
5. **Single binary**: No separate frontend/backend
6. **Better DX**: Change template, refresh browser (with cargo-watch)

## Next Steps

Want me to:
1. ✅ **Start the migration now** (I can do phases 1-3 immediately)
2. ✅ Create a proof-of-concept for one feature (e.g., server management)
3. ❌ Stick with React instead

**I strongly recommend HTMX + Askama for your use case!**

