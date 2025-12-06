# SvrCtlRS Development Plan

> **Note:** For comprehensive feature catalog, see [FUTURE_FEATURES.md](./FUTURE_FEATURES.md) and [docs/features/](./docs/features/)

## v1.0.0 - Released ✅

### Completed Features

**Core Infrastructure:**
- ✅ Database-backed configuration (SQLite + migrations)
- ✅ Plugin system with 5 plugins (Docker, Updates, Health, Weather, Speedtest)
- ✅ Task execution engine (local + remote SSH)
- ✅ Cron-based scheduler
- ✅ Notification system (Gotify + ntfy.sh)

**Web UI (HTMX + Askama):**
- ✅ Server management (`/servers`)
- ✅ Plugin configuration (`/plugins`)
- ✅ Task management (`/tasks`)
- ✅ Notification settings (`/settings/notifications`)
- ✅ Dark/light theme toggle
- ✅ Mobile responsive design

**Deployment:**
- ✅ Docker multi-arch builds (AMD64, ARM64)
- ✅ GitHub Actions CI/CD
- ✅ Docker Compose ready
- ✅ Healthcheck support

---

## v1.1.0 - Authentication & Security (Next Release)

**Priority: 🔴 Critical**

### Goals
Add user authentication and authorization to secure the application.

### Tasks

1. **User Authentication**
   - [ ] Implement login/logout with `tower-sessions`
   - [ ] Password hashing with `argon2`
   - [ ] Session management (SQLite store)
   - [ ] Login page UI
   - [ ] Protected routes middleware

2. **User Management**
   - [ ] User registration (admin-only)
   - [ ] User list/edit/delete UI
   - [ ] Password reset functionality
   - [ ] Email verification (optional)

3. **Authorization**
   - [ ] Role-based access control (Admin, Viewer)
   - [ ] Protect admin routes (servers, plugins, settings)
   - [ ] Allow public access to health endpoints
   - [ ] Audit logging for user actions

**Database Migration:**
```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    email TEXT,
    role TEXT NOT NULL DEFAULT 'viewer',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_login_at DATETIME
);
```

**Estimated Time:** 2-3 weeks

---

## v1.2.0 - Dashboard & Metrics

**Priority: 🟡 High**

### Goals
Create a comprehensive dashboard with real-time metrics and visualizations.

### Tasks

1. **Dashboard Page**
   - [ ] Overview stats (servers, tasks, plugins)
   - [ ] Recent task executions
   - [ ] System health summary
   - [ ] Quick actions (run tasks, add servers)

2. **Metrics Collection**
   - [ ] Store metrics in `metrics` table
   - [ ] Collect metrics from plugins
   - [ ] Time-series data storage
   - [ ] Metrics retention policy

3. **Visualizations**
   - [ ] Charts for task execution history
   - [ ] Graphs for system health trends
   - [ ] Server uptime visualization
   - [ ] Plugin performance metrics

4. **Activity Feed**
   - [ ] Recent task executions
   - [ ] Configuration changes
   - [ ] System events
   - [ ] Real-time updates (WebSocket or SSE)

**Dependencies:**
- Chart.js or similar for visualizations
- Consider WebSocket for real-time updates

**Estimated Time:** 2-3 weeks

---

## v1.3.0 - Enhanced Task System

**Priority: 🟡 High**

### Goals
Improve task execution with dependencies, chains, and better history.

### Tasks

1. **Task Dependencies**
   - [ ] Define task dependencies in database
   - [ ] Execute tasks in order
   - [ ] Handle dependency failures
   - [ ] Visualize task chains

2. **Task History Enhancements**
   - [ ] Detailed execution logs
   - [ ] Filter by plugin, server, date
   - [ ] Export task history (CSV, JSON)
   - [ ] Task execution statistics

3. **Task Templates**
   - [ ] Create reusable task templates
   - [ ] Template variables
   - [ ] Clone tasks from templates

4. **Manual Task Execution**
   - [ ] Run tasks with custom parameters
   - [ ] Override schedule for one-off runs
   - [ ] Task execution queue

**Estimated Time:** 1-2 weeks

---

## v2.0.0 - Advanced Features

**Priority: 🟢 Medium**

### Goals
Add advanced features for power users and enterprise deployments.

### Tasks

1. **Webhook System**
   - [ ] Incoming webhooks for external triggers
   - [ ] Outgoing webhooks for events
   - [ ] Webhook authentication (tokens)
   - [ ] Webhook UI and testing

2. **API Enhancements**
   - [ ] OpenAPI/Swagger documentation
   - [ ] API authentication (tokens)
   - [ ] Rate limiting
   - [ ] API versioning

3. **Plugin Enhancements**
   - [ ] Kubernetes plugin
   - [ ] Prometheus exporter plugin
   - [ ] Custom script plugin
   - [ ] Plugin marketplace (future)

4. **Multi-Tenancy**
   - [ ] Organization support
   - [ ] Per-org servers and tasks
   - [ ] Per-org notification settings
   - [ ] Billing integration (optional)

**Estimated Time:** 3-4 weeks

---

## v2.1.0 - Performance & Scalability

**Priority: 🟢 Low**

### Goals
Optimize performance for large deployments.

### Tasks

1. **Caching**
   - [ ] Redis caching layer
   - [ ] Cache server status
   - [ ] Cache dashboard stats
   - [ ] Cache invalidation strategy

2. **Background Jobs**
   - [ ] Job queue (Redis or in-memory)
   - [ ] Async task execution
   - [ ] Job status tracking
   - [ ] Job cancellation

3. **Database Optimization**
   - [ ] Add indexes for common queries
   - [ ] Query optimization
   - [ ] Connection pooling tuning
   - [ ] Database cleanup jobs

4. **Monitoring**
   - [ ] Prometheus metrics export
   - [ ] Grafana dashboards
   - [ ] Health check improvements
   - [ ] Performance profiling

**Estimated Time:** 2-3 weeks

---

## Future Ideas (Backlog)

### High Priority
- [ ] Container log viewer (Docker plugin)
- [ ] Container exec/terminal (Docker plugin)
- [ ] Automated image updates (Docker plugin)
- [ ] Email notifications (in addition to Gotify/ntfy)
- [ ] Slack/Discord/Teams integrations
- [ ] Mobile app (React Native or Flutter)

### Medium Priority
- [ ] Backup/restore functionality
- [ ] Configuration export/import
- [ ] Multi-language support (i18n)
- [ ] Custom themes/branding
- [ ] Plugin development SDK
- [ ] WASM plugins (sandboxed)

### Low Priority
- [ ] AI-powered anomaly detection
- [ ] Predictive maintenance
- [ ] Cost optimization recommendations
- [ ] Compliance reporting
- [ ] SLA monitoring

---

## Technology Stack

### Backend
- **Framework:** Axum 0.8
- **Database:** SQLite with sqlx
- **Scheduler:** Custom cron-based
- **SSH:** async-ssh2-tokio
- **Auth:** tower-sessions + argon2

### Frontend
- **Templates:** Askama 0.12
- **Interactivity:** HTMX 2.0.3
- **Client JS:** Alpine.js 3.14.1
- **Styling:** Custom CSS (Nord theme)

### Infrastructure
- **Container:** Docker (multi-arch)
- **CI/CD:** GitHub Actions
- **Deployment:** Docker Compose

---

## Development Guidelines

### Code Style
1. Use `tracing` for logging (`info!`, `warn!`, `error!`, `debug!`)
2. Use `anyhow::Result` for error handling
3. Add `#[instrument]` for function tracing
4. Write tests for new features
5. Document public APIs

### Git Workflow
1. `main` branch for stable releases
2. `develop` branch for active development
3. Feature branches for new features
4. Tag releases with semantic versioning

### Release Process
1. Update version in all `Cargo.toml` files
2. Update `CHANGELOG.md`
3. Create git tag (`v1.0.0`)
4. Push tag to trigger GitHub Actions
5. Publish release notes on GitHub

---

## Resources

- **Documentation:** See `AI_CONTEXT.md` for detailed context
- **Database Schema:** See `DATABASE_SCHEMA.md`
- **Docker Setup:** See `DOCKER_MONITORING.md`
- **Quick Start:** See `QUICKSTART.md`
- **API Docs:** Coming in v2.0.0

---

## Contributing

We welcome contributions! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

See `AI_CONTEXT.md` for development guidelines.

---

**Last Updated:** 2025-11-25 (v1.0.0 release)
**Maintainer:** jsprague84
