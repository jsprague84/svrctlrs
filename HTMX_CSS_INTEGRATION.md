# HTMX + Askama with Your Existing CSS Theme

## ✅ Your Theme is Perfect for HTMX

Your current CSS theme (Nord-inspired, light/dark mode) will work **identically** with HTMX + Askama.

### What Stays the Same

1. **All CSS classes** - `.card`, `.btn-primary`, `.badge-success`, etc.
2. **CSS variables** - `--bg-primary`, `--accent-primary`, etc.
3. **Light/dark mode** - `data-theme="light"` or `data-theme="dark"`
4. **Responsive layout** - Mobile sidebar toggle
5. **Fixed header + sidebar** - Exact same structure

### What Changes

**Before (Dioxus):**
```rust
rsx! {
    div { class: "card",
        h2 { "Server Status" }
        p { "Active" }
    }
}
```

**After (Askama):**
```html
<div class="card">
    <h2>Server Status</h2>
    <p>Active</p>
</div>
```

**That's it!** Same HTML, same classes, same styling.

## Mobile + Desktop Responsive Design

### Your Current Responsive CSS (Already Perfect)

```css
@media (max-width: 1024px) {
    .sidebar {
        transform: translateX(-100%);
        transition: transform 0.3s ease;
    }

    .sidebar.open {
        transform: translateX(0);
    }

    .main-content {
        margin-left: 0;
    }
}
```

### Adding Mobile Menu Toggle with Alpine.js

Alpine.js (15KB) adds client-side interactivity for things like:
- Mobile menu toggle
- Dropdowns
- Modals
- Theme switcher

**Example: Mobile Sidebar Toggle**

```html
<!-- base.html -->
<body x-data="{ sidebarOpen: false, theme: 'dark' }">
    <div class="app-container" :data-theme="theme">
        <!-- Header with mobile menu button -->
        <header class="header">
            <!-- Mobile menu button (only visible on mobile) -->
            <button @click="sidebarOpen = !sidebarOpen" 
                    class="btn-secondary mobile-menu-btn">
                ☰
            </button>
            
            <h1>SvrCtlRS</h1>
            
            <!-- Theme toggle -->
            <button @click="theme = theme === 'light' ? 'dark' : 'light'"
                    class="btn-secondary">
                <span x-show="theme === 'light'">🌙</span>
                <span x-show="theme === 'dark'">☀️</span>
            </button>
        </header>
        
        <!-- Sidebar (toggles on mobile) -->
        <aside class="sidebar" :class="{ 'open': sidebarOpen }">
            <nav>
                <a href="/" class="nav-link">Dashboard</a>
                <a href="/servers" class="nav-link">Servers</a>
                <a href="/tasks" class="nav-link">Tasks</a>
                <a href="/plugins" class="nav-link">Plugins</a>
                <a href="/metrics" class="nav-link">Metrics</a>
            </nav>
        </aside>
        
        <!-- Main content -->
        <main class="main-content">
            {% block content %}{% endblock %}
        </main>
    </div>
</body>
```

**Additional mobile CSS:**
```css
.mobile-menu-btn {
    display: none;
}

@media (max-width: 1024px) {
    .mobile-menu-btn {
        display: block;
    }
    
    /* Overlay when sidebar is open */
    .sidebar.open::before {
        content: '';
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.5);
        z-index: -1;
    }
}
```

## Expanding Functionality: Configuration Interface

### 1. Server Configuration Forms

**Template: `templates/pages/server_config.html`**

```html
{% extends "base.html" %}

{% block content %}
<div class="card">
    <h2>Server Configuration: {{ server.name }}</h2>
    
    <!-- Tabs for different config sections -->
    <div x-data="{ tab: 'general' }" class="tabs">
        <button @click="tab = 'general'" 
                :class="{ 'active': tab === 'general' }"
                class="btn-secondary">
            General
        </button>
        <button @click="tab = 'env'" 
                :class="{ 'active': tab === 'env' }"
                class="btn-secondary">
            Environment
        </button>
        <button @click="tab = 'ssh'" 
                :class="{ 'active': tab === 'ssh' }"
                class="btn-secondary">
            SSH Keys
        </button>
        <button @click="tab = 'monitoring'" 
                :class="{ 'active': tab === 'monitoring' }"
                class="btn-secondary">
            Monitoring
        </button>
    </div>
    
    <!-- General Settings -->
    <div x-show="tab === 'general'" class="tab-content">
        <form hx-put="/servers/{{ server.id }}/config"
              hx-target="#config-status"
              hx-swap="innerHTML">
            <label>
                Server Name:
                <input type="text" name="name" value="{{ server.name }}" required>
            </label>
            
            <label>
                Host:
                <input type="text" name="host" value="{{ server.host }}" required>
            </label>
            
            <label>
                Port:
                <input type="number" name="port" value="{{ server.port|default(22) }}">
            </label>
            
            <label>
                Username:
                <input type="text" name="username" value="{{ server.username|default('root') }}">
            </label>
            
            <button type="submit" class="btn-primary">Save</button>
        </form>
    </div>
    
    <!-- Environment Variables -->
    <div x-show="tab === 'env'" class="tab-content">
        <div id="env-vars-list">
            {% include "components/env_vars_list.html" %}
        </div>
        
        <form hx-post="/servers/{{ server.id }}/env"
              hx-target="#env-vars-list"
              hx-swap="innerHTML">
            <label>
                Key:
                <input type="text" name="key" placeholder="DATABASE_URL" required>
            </label>
            
            <label>
                Value:
                <input type="text" name="value" placeholder="postgres://..." required>
            </label>
            
            <button type="submit" class="btn-primary">Add Variable</button>
        </form>
    </div>
    
    <!-- SSH Keys -->
    <div x-show="tab === 'ssh'" class="tab-content">
        <form hx-post="/servers/{{ server.id }}/ssh-key"
              hx-target="#ssh-status"
              hx-swap="innerHTML">
            <label>
                SSH Private Key:
                <textarea name="private_key" rows="10" placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"></textarea>
            </label>
            
            <label>
                SSH Public Key:
                <textarea name="public_key" rows="3" placeholder="ssh-rsa AAAA..."></textarea>
            </label>
            
            <button type="submit" class="btn-primary">Save Keys</button>
        </form>
        
        <div id="ssh-status"></div>
    </div>
    
    <!-- Monitoring Settings -->
    <div x-show="tab === 'monitoring'" class="tab-content">
        <form hx-put="/servers/{{ server.id }}/monitoring"
              hx-target="#monitoring-status"
              hx-swap="innerHTML">
            <label>
                <input type="checkbox" name="enable_metrics" 
                       {% if server.enable_metrics %}checked{% endif %}>
                Enable Performance Metrics
            </label>
            
            <label>
                Metrics Interval (seconds):
                <input type="number" name="metrics_interval" 
                       value="{{ server.metrics_interval|default(60) }}">
            </label>
            
            <label>
                <input type="checkbox" name="enable_disk_monitoring" 
                       {% if server.enable_disk_monitoring %}checked{% endif %}>
                Enable Disk Usage Monitoring
            </label>
            
            <label>
                Disk Alert Threshold (%):
                <input type="number" name="disk_alert_threshold" 
                       value="{{ server.disk_alert_threshold|default(85) }}">
            </label>
            
            <button type="submit" class="btn-primary">Save Settings</button>
        </form>
        
        <div id="monitoring-status"></div>
    </div>
    
    <div id="config-status"></div>
</div>
{% endblock %}
```

### 2. Historical Data: Performance Metrics

**Template: `templates/pages/metrics.html`**

```html
{% extends "base.html" %}

{% block content %}
<div class="card">
    <h2>Performance Metrics: {{ server.name }}</h2>
    
    <!-- Time range selector -->
    <div class="metrics-controls">
        <select hx-get="/servers/{{ server.id }}/metrics"
                hx-target="#metrics-charts"
                hx-swap="innerHTML"
                name="range">
            <option value="1h">Last Hour</option>
            <option value="24h" selected>Last 24 Hours</option>
            <option value="7d">Last 7 Days</option>
            <option value="30d">Last 30 Days</option>
        </select>
        
        <!-- Auto-refresh toggle -->
        <label>
            <input type="checkbox" x-model="autoRefresh"> Auto-refresh
        </label>
    </div>
    
    <!-- Metrics charts (with auto-refresh) -->
    <div id="metrics-charts"
         hx-get="/servers/{{ server.id }}/metrics?range=24h"
         hx-trigger="load, every 30s[autoRefresh]"
         hx-swap="innerHTML">
        {% include "components/metrics_charts.html" %}
    </div>
</div>

<!-- Current Stats (real-time polling) -->
<div class="card">
    <h3>Current Status</h3>
    
    <div id="current-stats"
         hx-get="/servers/{{ server.id }}/stats"
         hx-trigger="every 5s"
         hx-swap="innerHTML">
        {% include "components/current_stats.html" %}
    </div>
</div>
{% endblock %}
```

**Component: `templates/components/metrics_charts.html`**

```html
<!-- Using Chart.js for visualization -->
<div class="metrics-grid">
    <!-- CPU Usage Chart -->
    <div class="metric-card">
        <h4>CPU Usage</h4>
        <canvas id="cpu-chart"></canvas>
        <script>
            // Chart.js code to render CPU data
            const cpuData = {{ cpu_metrics|tojson }};
            new Chart(document.getElementById('cpu-chart'), {
                type: 'line',
                data: {
                    labels: cpuData.map(d => d.timestamp),
                    datasets: [{
                        label: 'CPU %',
                        data: cpuData.map(d => d.value),
                        borderColor: 'var(--accent-primary)',
                        backgroundColor: 'var(--accent-primary)',
                        tension: 0.4
                    }]
                },
                options: {
                    responsive: true,
                    scales: {
                        y: { beginAtZero: true, max: 100 }
                    }
                }
            });
        </script>
    </div>
    
    <!-- Memory Usage Chart -->
    <div class="metric-card">
        <h4>Memory Usage</h4>
        <canvas id="memory-chart"></canvas>
        <script>
            const memoryData = {{ memory_metrics|tojson }};
            new Chart(document.getElementById('memory-chart'), {
                type: 'line',
                data: {
                    labels: memoryData.map(d => d.timestamp),
                    datasets: [{
                        label: 'Memory %',
                        data: memoryData.map(d => d.value),
                        borderColor: 'var(--accent-success)',
                        backgroundColor: 'var(--accent-success)',
                        tension: 0.4
                    }]
                },
                options: {
                    responsive: true,
                    scales: {
                        y: { beginAtZero: true, max: 100 }
                    }
                }
            });
        </script>
    </div>
    
    <!-- Disk Usage Chart -->
    <div class="metric-card">
        <h4>Disk Usage</h4>
        <canvas id="disk-chart"></canvas>
        <script>
            const diskData = {{ disk_metrics|tojson }};
            new Chart(document.getElementById('disk-chart'), {
                type: 'line',
                data: {
                    labels: diskData.map(d => d.timestamp),
                    datasets: [{
                        label: 'Disk %',
                        data: diskData.map(d => d.value),
                        borderColor: 'var(--accent-warning)',
                        backgroundColor: 'var(--accent-warning)',
                        tension: 0.4
                    }]
                },
                options: {
                    responsive: true,
                    scales: {
                        y: { beginAtZero: true, max: 100 }
                    }
                }
            });
        </script>
    </div>
    
    <!-- Network I/O Chart -->
    <div class="metric-card">
        <h4>Network I/O</h4>
        <canvas id="network-chart"></canvas>
        <script>
            const networkData = {{ network_metrics|tojson }};
            new Chart(document.getElementById('network-chart'), {
                type: 'line',
                data: {
                    labels: networkData.map(d => d.timestamp),
                    datasets: [
                        {
                            label: 'In (MB/s)',
                            data: networkData.map(d => d.in_mbps),
                            borderColor: 'var(--accent-info)',
                            backgroundColor: 'var(--accent-info)',
                            tension: 0.4
                        },
                        {
                            label: 'Out (MB/s)',
                            data: networkData.map(d => d.out_mbps),
                            borderColor: 'var(--accent-error)',
                            backgroundColor: 'var(--accent-error)',
                            tension: 0.4
                        }
                    ]
                },
                options: {
                    responsive: true,
                    scales: {
                        y: { beginAtZero: true }
                    }
                }
            });
        </script>
    </div>
</div>

<style>
.metrics-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
    gap: 1rem;
    margin-top: 1rem;
}

.metric-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1rem;
}

@media (max-width: 768px) {
    .metrics-grid {
        grid-template-columns: 1fr;
    }
}
</style>
```

### 3. Database Schema for Historical Data

**Migration: `database/migrations/003_metrics.sql`**

```sql
-- Server metrics history
CREATE TABLE IF NOT EXISTS server_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    metric_type TEXT NOT NULL, -- 'cpu', 'memory', 'disk', 'network'
    value REAL NOT NULL,
    metadata TEXT, -- JSON for additional data
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX idx_server_metrics_server_time 
    ON server_metrics(server_id, metric_type, timestamp DESC);

-- Disk usage history
CREATE TABLE IF NOT EXISTS disk_usage_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    mount_point TEXT NOT NULL,
    total_bytes INTEGER NOT NULL,
    used_bytes INTEGER NOT NULL,
    available_bytes INTEGER NOT NULL,
    usage_percent REAL NOT NULL,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX idx_disk_usage_server_time 
    ON disk_usage_history(server_id, timestamp DESC);

-- Performance snapshots (aggregated hourly)
CREATE TABLE IF NOT EXISTS performance_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    cpu_avg REAL,
    cpu_max REAL,
    memory_avg REAL,
    memory_max REAL,
    disk_io_read_mb REAL,
    disk_io_write_mb REAL,
    network_in_mb REAL,
    network_out_mb REAL,
    hour_start DATETIME NOT NULL,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX idx_performance_snapshots_server_time 
    ON performance_snapshots(server_id, hour_start DESC);
```

### 4. Backend Routes for Metrics

**`server/src/routes/metrics.rs`**

```rust
use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use askama::Template;
use serde::Deserialize;

#[derive(Template)]
#[template(path = "pages/metrics.html")]
pub struct MetricsPageTemplate {
    pub user: Option<User>,
    pub server: Server,
}

#[derive(Template)]
#[template(path = "components/metrics_charts.html")]
pub struct MetricsChartsTemplate {
    pub cpu_metrics: Vec<MetricPoint>,
    pub memory_metrics: Vec<MetricPoint>,
    pub disk_metrics: Vec<MetricPoint>,
    pub network_metrics: Vec<NetworkMetricPoint>,
}

#[derive(Serialize, Deserialize)]
pub struct MetricPoint {
    pub timestamp: String,
    pub value: f64,
}

#[derive(Serialize, Deserialize)]
pub struct NetworkMetricPoint {
    pub timestamp: String,
    pub in_mbps: f64,
    pub out_mbps: f64,
}

#[derive(Deserialize)]
pub struct MetricsQuery {
    pub range: Option<String>, // "1h", "24h", "7d", "30d"
}

pub async fn metrics_page(
    State(state): State<AppState>,
    session: Session,
    Path(server_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let user = get_user_from_session(&session).await?;
    let server = state.database.get_server(server_id).await?;
    
    let template = MetricsPageTemplate { user, server };
    Ok(Html(template.render()?))
}

pub async fn metrics_charts(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Query(query): Query<MetricsQuery>,
) -> Result<Html<String>, AppError> {
    let range = query.range.as_deref().unwrap_or("24h");
    
    // Fetch metrics from database
    let cpu_metrics = state.database
        .get_metrics(server_id, "cpu", range).await?;
    let memory_metrics = state.database
        .get_metrics(server_id, "memory", range).await?;
    let disk_metrics = state.database
        .get_metrics(server_id, "disk", range).await?;
    let network_metrics = state.database
        .get_network_metrics(server_id, range).await?;
    
    let template = MetricsChartsTemplate {
        cpu_metrics,
        memory_metrics,
        disk_metrics,
        network_metrics,
    };
    
    Ok(Html(template.render()?))
}

pub async fn current_stats(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    // Fetch latest metrics (real-time)
    let stats = state.get_current_server_stats(server_id).await?;
    
    let template = CurrentStatsTemplate { stats };
    Ok(Html(template.render()?))
}
```

## Mobile Optimization

### Responsive Metrics Grid

```css
/* Desktop: 2x2 grid */
.metrics-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 1rem;
}

/* Tablet: 1 column */
@media (max-width: 1024px) {
    .metrics-grid {
        grid-template-columns: 1fr;
    }
}

/* Mobile: Stack everything */
@media (max-width: 768px) {
    .main-content {
        padding: 1rem;
    }
    
    .card {
        padding: 1rem;
    }
    
    /* Make tables scrollable */
    .table-container {
        overflow-x: auto;
    }
    
    /* Smaller charts on mobile */
    canvas {
        max-height: 200px;
    }
}
```

### Touch-Friendly Buttons

```css
/* Larger tap targets for mobile */
@media (max-width: 768px) {
    .btn {
        padding: 12px 20px;
        font-size: 1rem;
    }
    
    .nav-link {
        padding: 16px 24px;
    }
    
    /* Prevent zoom on input focus (iOS) */
    input, textarea, select {
        font-size: 16px;
    }
}
```

## Summary: HTMX + Askama Can Do Everything

| Feature | HTMX + Askama | Status |
|---------|---------------|--------|
| **Your CSS theme** | ✅ Works identically | Same classes, same styling |
| **Light/dark mode** | ✅ Works with Alpine.js | Theme toggle button |
| **Mobile responsive** | ✅ Your CSS already handles it | Sidebar toggle with Alpine.js |
| **Desktop layout** | ✅ Fixed header + sidebar | Same structure |
| **Configuration forms** | ✅ HTMX form submissions | Add/edit/delete servers |
| **Environment variables** | ✅ HTMX CRUD operations | Key-value management |
| **SSH key management** | ✅ Forms with textareas | Upload/store keys |
| **Performance metrics** | ✅ Chart.js + HTMX polling | Real-time + historical |
| **Disk usage history** | ✅ SQLite + Chart.js | Time-series data |
| **Real-time updates** | ✅ HTMX polling/SSE | Auto-refresh stats |
| **Interactive UI** | ✅ Alpine.js for client-side | Tabs, dropdowns, modals |

## JavaScript Bundle Size

- **HTMX**: 14KB
- **Alpine.js**: 15KB
- **Chart.js**: 60KB (for metrics visualization)
- **Total**: ~89KB

vs React: ~200KB+ (before your app code)

## Next Steps

Ready to start? I can:

1. ✅ **Migrate now** - Start with Phase 1-3 (setup, base templates, routes)
2. ✅ **Add metrics system** - Database schema + routes + templates
3. ✅ **Mobile optimization** - Ensure perfect mobile experience

**Your CSS theme is perfect and will work beautifully with HTMX + Askama!**

Want me to begin the migration?

