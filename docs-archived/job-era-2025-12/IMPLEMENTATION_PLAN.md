# SvrCtlRS Implementation Plan

**Date:** November 28, 2025  
**Current Status:** ✅ Code compiles, architecture complete, UI partially implemented  
**Next Phase:** Security hardening + feature completion

---

## 📊 Current State Assessment

### ✅ **Completed (100%)**

1. **Database Schema** - 18 tables, migration 011 complete
2. **Core Models** - All database models implemented
3. **Database Queries** - All CRUD operations implemented
4. **Job Execution Engine** - `core/src/executor.rs` (1,045 lines)
5. **Scheduler** - Database-driven cron scheduler
6. **Notification System** - Gotify + ntfy with policies
7. **Route Structure** - Modular UI routes (12 modules)
8. **Display Pattern** - Template-friendly models implemented
9. **Technology Stack** - Axum 0.8, HTMX 2.0.4, Askama 0.14 (all latest)

### ⚠️ **In Progress (50-90%)**

1. **UI Forms** - Basic CRUD works, advanced forms incomplete
2. **SSH Integration** - Connection pooling works, testing incomplete
3. **Job Execution** - Engine ready, UI integration incomplete
4. **Display Models** - Core models done, JOINs needed for counts

### ❌ **Not Started (0%)**

1. **Authentication** - No session management or login
2. **CSRF Protection** - No middleware
3. **Security Hardening** - No token masking or constant-time comparison
4. **Unit Tests** - 0% coverage
5. **Advanced Reporting** - No daily/weekly summaries

---

## 🎯 Implementation Priorities

### Phase 1: Security Foundation (Week 1-2) 🔴 **CRITICAL**

#### 1.1 Authentication System

**Priority:** P0 (Blocking production)  
**Effort:** 3-4 days  
**Dependencies:** None

**Implementation:**

```rust
// server/Cargo.toml
[dependencies]
tower-sessions = "0.13"
tower-sessions-sqlx-store = "0.14"
argon2 = "0.5"

// server/src/auth.rs
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use tower_sessions::{Session, SessionManagerLayer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserRole {
    Admin,
    Operator,
    Viewer,
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string();
    Ok(password_hash)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

// Middleware
pub async fn require_auth(
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if session.get::<User>("user").await?.is_some() {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
```

**Database Migration:**

```sql
-- Add to new migration: 012_add_authentication.sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin', 'operator', 'viewer')),
    enabled BOOLEAN NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create default admin user (password: admin - CHANGE IN PRODUCTION!)
INSERT INTO users (username, email, password_hash, role) VALUES
    ('admin', 'admin@localhost', '$argon2id$v=19$m=19456,t=2,p=1$...', 'admin');
```

**Routes to Protect:**

```rust
// server/src/main.rs
use tower_sessions::{SessionManagerLayer, sqlx_store::SqliteStore};

let session_store = SqliteStore::new(pool.clone());
let session_layer = SessionManagerLayer::new(session_store)
    .with_secure(false) // Set to true in production with HTTPS
    .with_same_site(tower_sessions::cookie::SameSite::Lax);

let app = Router::new()
    // Public routes
    .route("/auth/login", post(login))
    .route("/auth/logout", post(logout))
    // Protected routes
    .nest("/", protected_routes())
    .layer(session_layer);

fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard))
        .route("/servers", get(servers_page))
        .route("/jobs", get(jobs_page))
        // ... all other routes
        .layer(middleware::from_fn(require_auth))
}
```

#### 1.2 CSRF Protection

**Priority:** P0 (Critical security)  
**Effort:** 1 day  
**Dependencies:** None

```rust
// server/Cargo.toml
[dependencies]
tower-http = { version = "0.6", features = ["csrf"] }

// server/src/main.rs
use tower_http::csrf::{CsrfLayer, CsrfToken};

let app = Router::new()
    .route("/", get(root))
    .layer(CsrfLayer::new());

// In templates
<form hx-post="/servers">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
    <!-- ... -->
</form>

// In handlers
async fn create_server(
    State(state): State<AppState>,
    CsrfToken(token): CsrfToken,
    Form(input): Form<CreateServerInput>,
) -> Result<Html<String>, AppError> {
    // CSRF token is automatically validated by middleware
    // ...
}
```

#### 1.3 Token Masking

**Priority:** P1 (High security)  
**Effort:** 1 day  
**Dependencies:** None

```rust
// core/src/notifications.rs
pub fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}***{}", &token[..3], &token[token.len()-3..])
    }
}

// Usage in logging
tracing::info!(
    "Sending notification to {} with token {}",
    channel.name,
    mask_token(&channel.token)
);
```

#### 1.4 Constant-Time Token Comparison

**Priority:** P1 (High security)  
**Effort:** 1 day  
**Dependencies:** None

```rust
// server/Cargo.toml
[dependencies]
subtle = "2.6"

// server/src/routes/webhooks.rs
use subtle::ConstantTimeEq;

fn verify_webhook_token(provided: &str, expected: &str) -> bool {
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

// Usage
if !verify_webhook_token(&provided_token, &expected_token) {
    return Err(StatusCode::UNAUTHORIZED);
}
```

---

### Phase 2: Feature Completion (Week 3-4) 🟡 **HIGH**

#### 2.1 Complete Job Type Creation UI

**Priority:** P2 (Key extensibility feature)  
**Effort:** 2-3 days  
**Dependencies:** None

**Files to Create/Update:**

1. `server/templates/components/job_type_form.html` - Full form with all fields
2. `server/templates/components/command_template_form.html` - Command template builder
3. `server/src/routes/ui/job_types.rs` - Form handlers

**Form Features:**

```html
<!-- server/templates/components/job_type_form.html -->
<form hx-post="/job-types" hx-target="#job-type-list" hx-swap="outerHTML">
    <!-- Basic Info -->
    <input name="name" placeholder="storage_management" required>
    <input name="display_name" placeholder="Storage Management" required>
    <textarea name="description"></textarea>
    
    <!-- Visual -->
    <input name="icon" placeholder="database" list="icon-list">
    <input name="color" type="color" value="#88C0D0">
    
    <!-- Capabilities (Multi-select) -->
    <div class="checkbox-group">
        <label><input type="checkbox" name="requires_capabilities[]" value="docker"> Docker</label>
        <label><input type="checkbox" name="requires_capabilities[]" value="zfs"> ZFS</label>
        <label><input type="checkbox" name="requires_capabilities[]" value="lvm"> LVM</label>
    </div>
    
    <!-- Parameter Schema Builder (JSON editor or form builder) -->
    <div x-data="parameterBuilder()">
        <button type="button" @click="addParameter">Add Parameter</button>
        <template x-for="(param, index) in parameters" :key="index">
            <div class="parameter-row">
                <input x-model="param.name" placeholder="Parameter name">
                <select x-model="param.type">
                    <option value="string">String</option>
                    <option value="integer">Integer</option>
                    <option value="boolean">Boolean</option>
                    <option value="array">Array</option>
                </select>
                <input x-model="param.default" placeholder="Default value">
                <label><input type="checkbox" x-model="param.required"> Required</label>
                <button type="button" @click="removeParameter(index)">Remove</button>
            </div>
        </template>
        <input type="hidden" name="parameter_schema" :value="JSON.stringify(parameters)">
    </div>
    
    <button type="submit">Create Job Type</button>
</form>

<script>
function parameterBuilder() {
    return {
        parameters: [],
        addParameter() {
            this.parameters.push({
                name: '',
                type: 'string',
                default: '',
                required: false
            });
        },
        removeParameter(index) {
            this.parameters.splice(index, 1);
        }
    };
}
</script>
```

#### 2.2 Complete Command Template Creation UI

**Priority:** P2 (Key extensibility feature)  
**Effort:** 2-3 days  
**Dependencies:** Job Type UI

```html
<!-- server/templates/components/command_template_form.html -->
<form hx-post="/command-templates" hx-target="#command-template-list">
    <!-- Job Type Selection -->
    <select name="job_type_id" required>
        {% for jt in job_types %}
        <option value="{{ jt.id }}">{{ jt.display_name }}</option>
        {% endfor %}
    </select>
    
    <!-- Basic Info -->
    <input name="name" placeholder="zpool_status" required>
    <input name="display_name" placeholder="ZFS Pool Status" required>
    <textarea name="description"></textarea>
    
    <!-- Command with Variable Guide -->
    <div class="command-editor">
        <label>Command Template</label>
        <textarea name="command" placeholder="zpool status {{pool_name}}" rows="5"></textarea>
        <div class="help-text">
            Use {{variable_name}} for substitution. Example: docker ps --filter 'status={{status}}'
        </div>
    </div>
    
    <!-- OS Filter -->
    <div class="os-filter">
        <label>Operating Systems (leave empty for all)</label>
        <div class="checkbox-group">
            <label><input type="checkbox" name="os_filter[]" value="ubuntu"> Ubuntu</label>
            <label><input type="checkbox" name="os_filter[]" value="debian"> Debian</label>
            <label><input type="checkbox" name="os_filter[]" value="fedora"> Fedora</label>
            <label><input type="checkbox" name="os_filter[]" value="rhel"> RHEL</label>
            <label><input type="checkbox" name="os_filter[]" value="arch"> Arch</label>
        </div>
    </div>
    
    <!-- Required Capabilities -->
    <div class="capabilities">
        <label>Required Capabilities</label>
        <div class="checkbox-group">
            <label><input type="checkbox" name="required_capabilities[]" value="docker"> Docker</label>
            <label><input type="checkbox" name="required_capabilities[]" value="zfs"> ZFS</label>
            <label><input type="checkbox" name="required_capabilities[]" value="lvm"> LVM</label>
            <label><input type="checkbox" name="required_capabilities[]" value="systemd"> Systemd</label>
        </div>
    </div>
    
    <!-- Execution Settings -->
    <input type="number" name="timeout_seconds" value="300" min="1">
    <input name="working_directory" placeholder="/opt/scripts">
    
    <!-- Environment Variables -->
    <div x-data="envBuilder()">
        <label>Environment Variables</label>
        <button type="button" @click="addEnv">Add Variable</button>
        <template x-for="(env, index) in envs" :key="index">
            <div class="env-row">
                <input x-model="env.key" placeholder="VAR_NAME">
                <input x-model="env.value" placeholder="value">
                <button type="button" @click="removeEnv(index)">Remove</button>
            </div>
        </template>
        <input type="hidden" name="environment" :value="JSON.stringify(Object.fromEntries(envs.map(e => [e.key, e.value])))">
    </div>
    
    <!-- Notification Defaults -->
    <label><input type="checkbox" name="notify_on_success"> Notify on success</label>
    <label><input type="checkbox" name="notify_on_failure" checked> Notify on failure</label>
    
    <button type="submit">Create Command Template</button>
</form>
```

#### 2.3 Implement SSH Connection Testing

**Priority:** P2 (Important for server setup)  
**Effort:** 1-2 days  
**Dependencies:** None

```rust
// server/src/routes/ui/servers.rs

async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let server = queries::servers::get_server(&state.pool, id).await?;
    
    // Get credential if specified
    let credential = if let Some(cred_id) = server.credential_id {
        Some(queries::credentials::get_credential(&state.pool, cred_id).await?)
    } else {
        None
    };
    
    // Test SSH connection
    let result = match test_ssh_connection(&server, credential.as_ref()).await {
        Ok(info) => {
            // Update server status
            queries::servers::update_server_status(
                &state.pool,
                id,
                "online",
                None,
                Some(chrono::Utc::now()),
            ).await?;
            
            format!(
                "<div class='alert alert-success'>
                    ✅ Connection successful!
                    <br>OS: {}
                    <br>Uptime: {}
                </div>",
                info.os_info, info.uptime
            )
        }
        Err(e) => {
            // Update server status
            queries::servers::update_server_status(
                &state.pool,
                id,
                "offline",
                Some(e.to_string()),
                None,
            ).await?;
            
            format!(
                "<div class='alert alert-error'>
                    ❌ Connection failed: {}
                </div>",
                e
            )
        }
    };
    
    Ok(Html(result))
}

async fn test_ssh_connection(
    server: &Server,
    credential: Option<&Credential>,
) -> Result<ConnectionInfo> {
    if server.is_local {
        return Ok(ConnectionInfo {
            os_info: std::fs::read_to_string("/etc/os-release")?,
            uptime: std::fs::read_to_string("/proc/uptime")?,
        });
    }
    
    // SSH connection
    let hostname = server.hostname.as_ref().ok_or("Missing hostname")?;
    let username = server.username.as_ref().ok_or("Missing username")?;
    
    let mut session = openssh::Session::connect(
        format!("{}@{}:{}", username, hostname, server.port),
        openssh::KnownHosts::Accept,
    ).await?;
    
    // Test basic command
    let os_info = session.command("cat /etc/os-release").output().await?;
    let uptime = session.command("uptime").output().await?;
    
    Ok(ConnectionInfo {
        os_info: String::from_utf8_lossy(&os_info.stdout).to_string(),
        uptime: String::from_utf8_lossy(&uptime.stdout).to_string(),
    })
}
```

#### 2.4 Implement Capability Detection

**Priority:** P2 (Important for job routing)  
**Effort:** 2-3 days  
**Dependencies:** SSH connection testing

```rust
// server/src/routes/ui/servers.rs

async fn detect_capabilities(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let server = queries::servers::get_server(&state.pool, id).await?;
    let credential = if let Some(cred_id) = server.credential_id {
        Some(queries::credentials::get_credential(&state.pool, cred_id).await?)
    } else {
        None
    };
    
    let capabilities = detect_server_capabilities(&server, credential.as_ref()).await?;
    
    // Update server record
    queries::servers::update_server_capabilities(
        &state.pool,
        id,
        &capabilities,
    ).await?;
    
    // Update server_capabilities table
    for (capability, available) in capabilities.iter() {
        queries::servers::upsert_capability(
            &state.pool,
            id,
            capability,
            *available,
            None, // version
        ).await?;
    }
    
    let html = format!(
        "<div class='alert alert-success'>
            ✅ Detected {} capabilities
            <ul>
                {}
            </ul>
        </div>",
        capabilities.len(),
        capabilities.iter()
            .map(|(cap, avail)| format!(
                "<li>{}: {}</li>",
                cap,
                if *avail { "✅" } else { "❌" }
            ))
            .collect::<Vec<_>>()
            .join("")
    );
    
    Ok(Html(html))
}

async fn detect_server_capabilities(
    server: &Server,
    credential: Option<&Credential>,
) -> Result<HashMap<String, bool>> {
    let mut capabilities = HashMap::new();
    
    if server.is_local {
        // Local detection
        capabilities.insert("docker".to_string(), which::which("docker").is_ok());
        capabilities.insert("systemd".to_string(), which::which("systemctl").is_ok());
        capabilities.insert("apt".to_string(), which::which("apt").is_ok());
        capabilities.insert("dnf".to_string(), which::which("dnf").is_ok());
        capabilities.insert("pacman".to_string(), which::which("pacman").is_ok());
    } else {
        // Remote detection via SSH
        let hostname = server.hostname.as_ref().ok_or("Missing hostname")?;
        let username = server.username.as_ref().ok_or("Missing username")?;
        
        let mut session = openssh::Session::connect(
            format!("{}@{}:{}", username, hostname, server.port),
            openssh::KnownHosts::Accept,
        ).await?;
        
        // Detection script
        let script = r#"
#!/bin/bash
echo "__OS_RELEASE__"
cat /etc/os-release 2>/dev/null || echo "unknown"

echo "__DOCKER__"
command -v docker >/dev/null 2>&1 && echo "1" || echo "0"

echo "__SYSTEMD__"
command -v systemctl >/dev/null 2>&1 && echo "1" || echo "0"

echo "__APT__"
command -v apt >/dev/null 2>&1 && echo "1" || echo "0"

echo "__DNF__"
command -v dnf >/dev/null 2>&1 && echo "1" || echo "0"

echo "__PACMAN__"
command -v pacman >/dev/null 2>&1 && echo "1" || echo "0"

echo "__ZFS__"
command -v zpool >/dev/null 2>&1 && echo "1" || echo "0"

echo "__LVM__"
command -v lvm >/dev/null 2>&1 && echo "1" || echo "0"
"#;
        
        let output = session.command("bash").arg("-s").stdin(script).output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Parse output
        for line in stdout.lines() {
            if line.starts_with("__") {
                let parts: Vec<&str> = line.split("__").collect();
                if parts.len() >= 2 {
                    let key = parts[1].to_lowercase();
                    // Next line contains the value
                }
            }
        }
        
        // Simplified parsing (actual implementation would be more robust)
        capabilities.insert("docker".to_string(), stdout.contains("__DOCKER__\n1"));
        capabilities.insert("systemd".to_string(), stdout.contains("__SYSTEMD__\n1"));
        capabilities.insert("apt".to_string(), stdout.contains("__APT__\n1"));
        capabilities.insert("dnf".to_string(), stdout.contains("__DNF__\n1"));
        capabilities.insert("pacman".to_string(), stdout.contains("__PACMAN__\n1"));
        capabilities.insert("zfs".to_string(), stdout.contains("__ZFS__\n1"));
        capabilities.insert("lvm".to_string(), stdout.contains("__LVM__\n1"));
    }
    
    Ok(capabilities)
}
```

---

### Phase 3: Database Optimization (Week 5) 🟢 **MEDIUM**

#### 3.1 Add JOINs to Display Models

**Priority:** P3 (Performance + UX)  
**Effort:** 2-3 days  
**Dependencies:** None

**Current Issue:** Many display models have `// TODO: Join from X table` comments

**Solution:** Update database queries to use JOINs

```rust
// database/src/queries/job_templates.rs

pub async fn list_job_templates_with_details(
    pool: &SqlitePool,
) -> Result<Vec<JobTemplateWithDetails>> {
    sqlx::query_as!(
        JobTemplateWithDetails,
        r#"
        SELECT 
            jt.id,
            jt.name,
            jt.display_name,
            jt.description,
            jt.job_type_id,
            jtype.name as job_type_name,
            jt.is_composite,
            jt.command_template_id,
            ct.name as command_template_name,
            jt.timeout_seconds,
            jt.retry_count,
            jt.created_at,
            jt.updated_at,
            (SELECT COUNT(*) FROM job_template_steps WHERE job_template_id = jt.id) as step_count,
            (SELECT COUNT(*) FROM job_schedules WHERE job_template_id = jt.id) as schedule_count
        FROM job_templates jt
        LEFT JOIN job_types jtype ON jt.job_type_id = jtype.id
        LEFT JOIN command_templates ct ON jt.command_template_id = ct.id
        ORDER BY jt.created_at DESC
        "#
    )
    .fetch_all(pool)
    .await
}
```

**Files to Update:**
- `database/src/queries/job_templates.rs`
- `database/src/queries/job_schedules.rs`
- `database/src/queries/job_runs.rs`
- `database/src/queries/servers.rs`
- `database/src/queries/credentials.rs`

---

### Phase 4: Testing & CI/CD (Week 6) 🔵 **LOW**

#### 4.1 Add Unit Tests

**Priority:** P4 (Quality)  
**Effort:** 5-7 days  
**Target:** 70% coverage

```rust
// database/src/queries/job_types.rs

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_create_job_type() {
        let pool = setup_test_db().await;
        
        let input = CreateJobType {
            name: "test_type".to_string(),
            display_name: "Test Type".to_string(),
            description: Some("Test description".to_string()),
            requires_capabilities: Some(vec!["docker".to_string()]),
            enabled: true,
            ..Default::default()
        };
        
        let id = create_job_type(&pool, &input).await.unwrap();
        assert!(id > 0);
        
        let job_type = get_job_type(&pool, id).await.unwrap();
        assert_eq!(job_type.name, "test_type");
        assert_eq!(job_type.display_name, "Test Type");
    }

    #[tokio::test]
    async fn test_list_job_types() {
        let pool = setup_test_db().await;
        
        // Create multiple job types
        for i in 0..5 {
            let input = CreateJobType {
                name: format!("type_{}", i),
                display_name: format!("Type {}", i),
                ..Default::default()
            };
            create_job_type(&pool, &input).await.unwrap();
        }
        
        let job_types = list_job_types(&pool).await.unwrap();
        assert_eq!(job_types.len(), 5);
    }
}
```

#### 4.2 Improve CI/CD

**Priority:** P4 (Quality)  
**Effort:** 1 day

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [ develop, main ]
  pull_request:
    branches: [ develop, main ]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      
      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Cache cargo index
        uses: actions/cache@v4
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Cache cargo build
        uses: actions/cache@v4
        with:
          path: target
          key: ${{ runner.os }}-cargo-build-target-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Check formatting
        run: cargo fmt -- --check
      
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings
      
      - name: Run tests
        run: cargo test --workspace --all-features
      
      - name: Security audit
        run: |
          cargo install cargo-audit
          cargo audit
      
      - name: Build
        run: cargo build --release --workspace
```

---

## 📝 TODO Summary

**From Code Analysis (41 TODOs found):**

### Critical (P0-P1)
- [ ] Implement authentication (auth.rs)
- [ ] Add CSRF protection
- [ ] Implement token masking
- [ ] Add constant-time token comparison

### High Priority (P2)
- [ ] Complete job type creation UI
- [ ] Complete command template creation UI
- [ ] Implement SSH connection testing (servers.rs)
- [ ] Implement capability detection (servers.rs)
- [ ] Add database JOINs for display models (9 TODOs)

### Medium Priority (P3)
- [ ] Implement job execution engine integration (job_schedules.rs)
- [ ] Add notification test functionality (notifications.rs)
- [ ] Implement actual job cancellation (job_runs.rs)

### Low Priority (P4)
- [ ] Fix deprecated routes (plugins_old.rs, tasks_old.rs)
- [ ] Add proper host key checking (ssh.rs)
- [ ] Fetch actual metrics from database (api.rs)

---

## 🚀 Quick Start Guide

### For Security Implementation
1. Start with authentication (biggest impact)
2. Add CSRF protection (quick win)
3. Implement token masking (logging safety)
4. Add constant-time comparison (webhook security)

### For Feature Completion
1. Job type creation UI (enables extensibility)
2. Command template UI (completes job system)
3. SSH testing (improves server setup UX)
4. Capability detection (enables smart job routing)

### For Quality
1. Add unit tests for database queries
2. Add integration tests for UI routes
3. Improve CI/CD with clippy + audit
4. Add code coverage reporting

---

## 📊 Success Metrics

### Week 2 (Security Complete)
- ✅ Authentication working with session management
- ✅ CSRF protection active on all forms
- ✅ Token masking in all logs
- ✅ Constant-time comparison for webhooks

### Week 4 (Features Complete)
- ✅ Job type creation via UI
- ✅ Command template creation via UI
- ✅ SSH connection testing working
- ✅ Capability detection working
- ✅ All display model JOINs implemented

### Week 6 (Quality Complete)
- ✅ 70%+ test coverage
- ✅ CI/CD with clippy + audit
- ✅ All deprecation warnings fixed
- ✅ Documentation updated

---

**Next Action:** Choose a phase to start implementing. Recommend starting with Phase 1 (Security) as it's blocking production use.

