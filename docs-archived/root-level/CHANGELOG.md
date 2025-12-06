# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-11-25

### Added

**Core Features:**
- Database-driven configuration with SQLite
- Plugin system with 5 plugins:
  - Docker Monitor (container health, resource monitoring)
  - Updates Manager (OS package monitoring)
  - System Health Monitor (CPU, memory, disk metrics)
  - Weather Monitor (OpenWeatherMap integration - optional)
  - Speed Test Monitor (Ookla speedtest - optional)
- Task execution engine (local + remote SSH)
- Cron-based task scheduler
- Notification system (Gotify + ntfy.sh support)
- SSH-based remote command execution

**Web UI (HTMX + Askama):**
- Server management page (`/servers`)
- Plugin configuration page (`/plugins`)
- Task management page (`/tasks`)
- Notification settings page (`/settings/notifications`)
- Dark/light theme toggle
- Mobile-responsive design with Nord theme
- Real-time task execution via "Run Now" button
- SSH connection testing from UI

**API:**
- REST API for all resources
- Health check endpoint (`/api/v1/health`)
- Server CRUD endpoints (`/api/v1/servers`)
- Plugin management endpoints (`/api/v1/plugins`)
- Task management endpoints (`/api/v1/tasks`)
- Notification backend endpoints (`/api/v1/notifications`)

**Infrastructure:**
- Docker multi-arch builds (AMD64, ARM64)
- GitHub Actions CI/CD pipeline
- Docker Compose deployment
- Healthcheck support
- Multi-stage Dockerfile with caching

**Documentation:**
- Comprehensive README with quick start
- Detailed QUICKSTART guide
- Database schema documentation
- Docker monitoring setup guide
- Development plan and roadmap
- AI context document for future development

### Technical Details

**Backend:**
- Axum 0.8 web framework
- SQLite database with sqlx
- Tokio async runtime
- Tower middleware (sessions, CORS, tracing)
- async-ssh2-tokio for SSH operations

**Frontend:**
- HTMX 2.0.3 for interactivity
- Askama 0.12 for server-side templates
- Alpine.js 3.14.1 for client-side state
- Custom CSS with Nord color scheme

**Database:**
- 6 migrations for schema evolution
- Tables: servers, plugins, tasks, task_history, notification_backends, settings
- Foreign key constraints
- Indexes for performance

### Design Decisions

- **Database as Source of Truth**: All configuration stored in database, managed via UI
- **Hybrid Docker Monitoring**: Local via Bollard API, remote via SSH
- **Simple Over Complex**: Manual configuration preferred over auto-detection
- **Plugin Architecture**: Easy to add new monitoring capabilities
- **HTMX for UI**: Lightweight, progressive enhancement approach

### Known Limitations

- No user authentication (planned for v1.1.0)
- Single notification backend per type (Gotify/ntfy)
- Docker socket requires manual GID setup for local monitoring
- No HTTPS support (use reverse proxy)

### Upgrade Notes

This is the first stable release. No upgrade path needed.

### Docker Images

Available on GitHub Container Registry:
- `ghcr.io/jsprague84/svrctlrs:latest` (stable)
- `ghcr.io/jsprague84/svrctlrs:develop` (bleeding edge)
- `ghcr.io/jsprague84/svrctlrs:v1.0.0` (specific version)

Supported architectures:
- linux/amd64
- linux/arm64

### Contributors

- jsprague84 - Initial development
- Claude (Anthropic) - AI development assistant

---

## [Unreleased]

### Planned for v1.1.0
- User authentication and authorization
- Role-based access control (Admin, Viewer)
- Session management
- Login/logout functionality
- User management UI

### Planned for v1.2.0
- Dashboard with real-time metrics
- Charts and visualizations
- Activity feed
- Metrics collection and storage

### Planned for v1.3.0
- Task dependencies and chains
- Enhanced task history
- Task templates
- Manual task execution with parameters

See [DEVELOPMENT_PLAN.md](DEVELOPMENT_PLAN.md) for detailed roadmap.

---

[1.0.0]: https://github.com/jsprague84/svrctlrs/releases/tag/v1.0.0
[Unreleased]: https://github.com/jsprague84/svrctlrs/compare/v1.0.0...HEAD

