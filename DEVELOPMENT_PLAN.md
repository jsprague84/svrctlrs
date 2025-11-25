# SvrCtlRS Development Plan

## Current Status ✅

### Completed Features
1. **Database-Backed Configuration**
   - ✅ SQLite database with migrations
   - ✅ Server management (CRUD via UI and API)
   - ✅ Plugin configuration with schedule support
   - ✅ Notification backend management
   - ✅ Task management (auto-created from plugin configs)

2. **Web UI (HTMX + Askama)**
   - ✅ Dashboard with stats
   - ✅ Server management pages
   - ✅ Plugin configuration pages
   - ✅ Task listing with "Run Now" button
   - ✅ Notification settings pages
   - ✅ SSH connection testing
   - ✅ Mobile-responsive design with Nord theme
   - ✅ Dark/light mode toggle

3. **REST API**
   - ✅ `/api/v1/health` - Health check
   - ✅ `/api/v1/servers` - Server CRUD
   - ✅ `/api/v1/plugins` - Plugin management
   - ✅ `/api/v1/notifications` - Notification backends
   - ✅ `/api/v1/tasks` - Task management

4. **Infrastructure**
   - ✅ Docker multi-arch builds (AMD64, ARM64)
   - ✅ GitHub Actions CI/CD (main and develop branches)
   - ✅ Docker healthcheck working correctly
   - ✅ Docker Compose deployment

---

## Phase 1: Core Functionality (Priority: HIGH)

### 1.1 Authentication & Authorization
**Status:** Placeholder only  
**Priority:** 🔴 Critical

**Tasks:**
- [ ] Implement user authentication with `tower-sessions`
  - Use session-based auth (already have `tower-sessions` dependency)
  - Store sessions in SQLite (use `tower-sessions` SQLite store)
  - Hash passwords with `argon2` or `bcrypt`
- [ ] Create user management UI
  - Login page (already exists as placeholder)
  - User registration/management page
  - Password reset functionality
- [ ] Add authentication middleware
  - Protect admin routes (servers, plugins, settings)
  - Allow public access to health/status endpoints
  - Use Axum's `Extension` pattern for current user
- [ ] Implement role-based access control (RBAC)
  - Admin role (full access)
  - Viewer role (read-only)
  - Store roles in `users` table

**Reference:** See Context7 Axum middleware examples for session management

**Database Schema:**
```sql
-- Already defined in DATABASE_SCHEMA.md
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    email TEXT,
    role TEXT NOT NULL DEFAULT 'viewer', -- admin, viewer
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_login_at DATETIME
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    expires_at DATETIME NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
```

---

### 1.2 Task Execution System
**Status:** Placeholder only  
**Priority:** 🔴 Critical

**Tasks:**
- [ ] Implement actual SSH connection testing
  - Use `openssh` or `ssh2` crate for SSH connections
  - Test connection with timeout
  - Return detailed error messages
  - Update `server_test_connection` in `ui_routes.rs`
- [ ] Implement task execution engine
  - Execute plugin tasks via plugin trait
  - Pass server context and config to plugins
  - Capture stdout/stderr from tasks
  - Record execution results in `task_history` table
  - Update `task_run_now` in `ui_routes.rs`
- [ ] Wire scheduler to database tasks
  - Load tasks from database on startup
  - Register tasks with scheduler
  - Update `next_run_at` after execution
  - Increment `run_count`
- [ ] Add task history UI
  - Show recent task executions
  - Display success/failure status
  - Show execution logs
  - Filter by plugin, server, date range

**Dependencies:**
```toml
ssh2 = "0.9"  # or openssh = "0.10"
```

---

### 1.3 Plugin System Enhancement
**Status:** Basic structure exists  
**Priority:** 🟡 High

**Tasks:**
- [ ] Implement plugin execution context
  - Pass server list to plugins
  - Pass plugin config to plugins
  - Pass notification manager to plugins
- [ ] Add plugin metrics collection
  - Store metrics in `metrics` table
  - Display metrics on dashboard
  - Create metrics API endpoint
- [ ] Add plugin-specific pages
  - Weather plugin: Show current weather, forecasts
  - Docker plugin: Show container status, images
  - Updates plugin: Show available updates
  - Health plugin: Show system health metrics
- [ ] Implement notification integration
  - Send notifications on task failure
  - Send notifications on threshold breach
  - Use configured notification backends

---

## Phase 2: User Experience (Priority: MEDIUM)

### 2.1 Form Validation & Error Handling
**Status:** Basic validation only  
**Priority:** 🟡 High

**Tasks:**
- [ ] Enable HTML5 form validation
  - Set `htmx.config.reportValidityOfForms = true`
  - Add validation attributes to inputs
  - Show inline validation errors
- [ ] Improve error messages
  - Show user-friendly error messages
  - Add toast notifications for success/error
  - Use HTMX `hx-swap-oob` for error display
- [ ] Add form field validation
  - Validate cron expressions
  - Validate SSH connection details
  - Validate API keys and URLs
  - Show validation feedback in real-time

**Reference:** See Context7 HTMX validation best practices

---

### 2.2 Progressive Enhancement
**Status:** Partial  
**Priority:** 🟢 Medium

**Tasks:**
- [ ] Add `hx-boost` to navigation links
  - Boost all internal links for SPA-like experience
  - Maintain browser history
  - Improve perceived performance
- [ ] Add loading indicators
  - Show spinner during AJAX requests
  - Use `hx-indicator` attribute
  - Add CSS for loading states
- [ ] Implement optimistic UI updates
  - Update UI immediately on action
  - Revert on error
  - Use HTMX `hx-swap` strategies

---

### 2.3 Dashboard Enhancements
**Status:** Basic stats only  
**Priority:** 🟢 Medium

**Tasks:**
- [ ] Add real-time metrics
  - Server uptime
  - Task success rate
  - Recent task executions
  - Plugin status
- [ ] Add charts and graphs
  - Use Chart.js or similar
  - Show task execution history
  - Show server health trends
  - Show metric trends over time
- [ ] Add activity feed
  - Recent task executions
  - Recent server changes
  - Recent configuration changes

---

## Phase 3: Advanced Features (Priority: LOW)

### 3.1 Webhook System
**Status:** Routes exist, no implementation  
**Priority:** 🟢 Medium

**Tasks:**
- [ ] Implement webhook authentication
  - Token-based authentication
  - Store webhook tokens in database
  - Validate tokens on webhook requests
- [ ] Implement webhook handlers
  - Docker health check webhook
  - Docker cleanup webhook
  - Updates check webhook
  - Custom task trigger webhook
- [ ] Add webhook UI
  - List configured webhooks
  - Create/edit/delete webhooks
  - Test webhook endpoints
  - Show webhook execution history

---

### 3.2 Notification System
**Status:** Database schema exists  
**Priority:** 🟢 Medium

**Tasks:**
- [ ] Implement Gotify integration
  - Send messages to Gotify server
  - Support priority levels
  - Handle connection errors
- [ ] Implement ntfy.sh integration
  - Send messages to ntfy.sh
  - Support topics and priorities
  - Handle authentication
- [ ] Add notification templates
  - Customize notification messages
  - Include task details, server info
  - Support variables/placeholders
- [ ] Add notification rules
  - Notify on task failure
  - Notify on threshold breach
  - Notify on server offline
  - Configure notification frequency

---

### 3.3 Audit Logging
**Status:** Schema defined, not implemented  
**Priority:** 🟢 Low

**Tasks:**
- [ ] Implement audit logging
  - Log all configuration changes
  - Log all task executions
  - Log all user actions
  - Store in `audit_logs` table
- [ ] Add audit log UI
  - View audit logs
  - Filter by user, action, date
  - Export audit logs

---

## Phase 4: Performance & Scalability (Priority: LOW)

### 4.1 Caching
**Status:** Not implemented  
**Priority:** 🟢 Low

**Tasks:**
- [ ] Add Redis caching layer
  - Cache server status
  - Cache plugin metrics
  - Cache dashboard stats
  - Set appropriate TTLs
- [ ] Implement cache invalidation
  - Invalidate on data changes
  - Use cache-aside pattern

---

### 4.2 Background Jobs
**Status:** Scheduler exists  
**Priority:** 🟢 Low

**Tasks:**
- [ ] Implement job queue
  - Use `tokio` channels or external queue
  - Queue long-running tasks
  - Process jobs in background
- [ ] Add job status tracking
  - Show job progress
  - Show job results
  - Cancel running jobs

---

## Phase 5: Testing & Documentation (Priority: ONGOING)

### 5.1 Testing
**Status:** No tests  
**Priority:** 🟡 High

**Tasks:**
- [ ] Add unit tests
  - Test database queries
  - Test plugin logic
  - Test utility functions
- [ ] Add integration tests
  - Test API endpoints
  - Test UI routes
  - Test middleware
  - Use `axum-test` crate
- [ ] Add end-to-end tests
  - Test complete workflows
  - Test with real database
  - Test with Docker

---

### 5.2 Documentation
**Status:** Basic docs exist  
**Priority:** 🟡 High

**Tasks:**
- [ ] Update API documentation
  - Document all endpoints
  - Add request/response examples
  - Use OpenAPI/Swagger
- [ ] Create user guide
  - Getting started guide
  - Configuration guide
  - Plugin guide
  - Troubleshooting guide
- [ ] Create developer guide
  - Architecture overview
  - Plugin development guide
  - Contributing guide

---

## Technology Stack

### Backend
- **Framework:** Axum 0.8 (async Rust web framework)
- **Database:** SQLite with `sqlx` (migrations, compile-time checked queries)
- **Scheduler:** Custom cron-based scheduler with `cron` crate
- **SSH:** `ssh2` or `openssh` crate (to be implemented)
- **Authentication:** `tower-sessions` + `argon2`/`bcrypt`

### Frontend
- **Templating:** Askama (compile-time Jinja-like templates)
- **Interactivity:** HTMX 2.0.3 (HTML-over-the-wire)
- **Client-side JS:** Alpine.js 3.14.1 (minimal, for theme toggle and mobile menu)
- **Styling:** Custom CSS with Nord theme

### Infrastructure
- **Container:** Docker (multi-arch: AMD64, ARM64)
- **CI/CD:** GitHub Actions
- **Deployment:** Docker Compose

---

## Best Practices to Follow

### Axum Best Practices
1. **Middleware Ordering:** Apply layers from bottom to top (onion model)
2. **State Management:** Use `Extension` for request-scoped data, `State` for app-wide data
3. **Error Handling:** Use `HandleErrorLayer` for fallible middleware
4. **Tracing:** Use `TraceLayer` for structured logging
5. **CORS:** Configure `CorsLayer` for API endpoints

### HTMX Best Practices
1. **Progressive Enhancement:** Ensure forms work without JavaScript
2. **Validation:** Enable `htmx.config.reportValidityOfForms`
3. **Loading States:** Use `hx-indicator` for user feedback
4. **Error Handling:** Use `hx-swap-oob` for inline errors
5. **Accessibility:** Use semantic HTML, ARIA attributes

### Database Best Practices
1. **Migrations:** Use `sqlx-cli` for schema evolution
2. **Transactions:** Use transactions for multi-step operations
3. **Indexes:** Add indexes for frequently queried columns
4. **Foreign Keys:** Enable foreign key constraints
5. **Backups:** Implement regular database backups

---

## Next Immediate Steps

1. **Implement Authentication** (Phase 1.1)
   - This is blocking other features
   - Required for production deployment
   - Start with basic username/password auth

2. **Implement Task Execution** (Phase 1.2)
   - Core functionality of the application
   - Currently just placeholders
   - Required for the app to be useful

3. **Add Form Validation** (Phase 2.1)
   - Improves user experience
   - Prevents invalid data entry
   - Quick win with high impact

4. **Add Tests** (Phase 5.1)
   - Prevent regressions
   - Enable confident refactoring
   - Start with critical paths

---

## Resources

- **Axum Documentation:** https://docs.rs/axum/latest/axum/
- **HTMX Documentation:** https://htmx.org/docs/
- **Askama Documentation:** https://djc.github.io/askama/
- **SQLx Documentation:** https://docs.rs/sqlx/latest/sqlx/
- **Tower Sessions:** https://docs.rs/tower-sessions/latest/tower_sessions/

---

## Notes

- All database schema is already defined in `DATABASE_SCHEMA.md`
- All database models are in `database/src/models/`
- All database queries are in `database/src/queries/`
- UI templates are in `server/templates/`
- Static assets are in `server/static/`
- API routes are in `server/src/routes/`
- UI routes are in `server/src/ui_routes.rs`

