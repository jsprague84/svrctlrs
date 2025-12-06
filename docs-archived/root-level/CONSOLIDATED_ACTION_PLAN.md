# SvrCtlRS Consolidated Action Plan

**Date**: November 26, 2025  
**Combined Assessment Grade**: B+ (85/100)  
**Status**: Production-Ready with Critical Security Gaps

---

## Executive Summary

Both internal and external assessments agree:
- ✅ **Excellent architecture** - Plugin system, clean code, modern stack
- ✅ **Superior to weatherust** - Simpler deployment, better UX
- 🔴 **Critical security gaps** - Must fix before production use
- 🟡 **UI needs organization** - Working but needs polish
- 🟢 **Strong foundation** - Ready for expansion

**Consensus**: Fix security issues (2 weeks) → Production-ready

---

## Critical Priorities (MUST FIX - Week 1-2)

### 🔴 Priority 1: Security Hardening

#### 1.1 Add Constant-Time Token Comparison
**Risk**: Timing attack vulnerability in webhook authentication  
**File**: `server/src/routes/webhooks.rs`

```rust
// Add to Cargo.toml
[dependencies]
subtle = "2.6"

// In webhooks.rs
use subtle::ConstantTimeEq;

fn verify_webhook_token(provided: &str, expected: &str) -> bool {
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}
```

#### 1.2 Implement Token Masking for Logs
**Risk**: API tokens exposed in debug logs  
**File**: `core/src/notifications.rs`

```rust
pub fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}***{}", &token[..3], &token[token.len()-3..])
    }
}

// Use in logging
tracing::info!("Using token: {}", mask_token(&token));
```

#### 1.3 Add CSRF Protection
**Risk**: Cross-site request forgery  
**File**: `server/src/main.rs`

```rust
use tower_http::csrf::CsrfLayer;

let app = Router::new()
    // ... routes
    .layer(CsrfLayer::new());
```

#### 1.4 Implement Real Authentication
**Risk**: No access control currently  
**File**: `server/src/routes/auth.rs`

**Current**: Stubbed/TODO  
**Required**: 
- Session-based auth with tower-sessions ✅ (already have)
- Login/logout functionality
- Password hashing (argon2)
- Optional: 2FA support

#### 1.5 Add Transaction Support
**Risk**: Data inconsistency on failures  
**File**: `database/src/lib.rs`

```rust
pub async fn with_transaction<F, T>(pool: &PgPool, f: F) -> Result<T>
where
    F: FnOnce(&mut Transaction<'_, Postgres>) -> BoxFuture<'_, Result<T>>,
{
    let mut tx = pool.begin().await?;
    let result = f(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}
```

---

### 🔴 Priority 2: Fix Broken Features

#### 2.1 Fix ntfy.sh Notifications (CRITICAL)
**Status**: Currently broken (403 errors were fixed, but needs verification)  
**File**: `core/src/notifications.rs`

**Actions**:
- ✅ Token authentication implemented
- ✅ Basic auth implemented
- ⚠️ Need to verify working with user's setup
- Add retry logic with exponential backoff

#### 2.2 Add Status Reports
**Current**: Plugins only alert on problems  
**Required**: Periodic "all systems OK" summaries

```rust
// In plugin config
pub struct PluginConfig {
    pub send_summary: bool, // ✅ Already implemented
    pub summary_schedule: String, // Add this
}
```

#### 2.3 Fix UI Task Grouping
**Status**: Backend works, frontend not displaying  
**File**: `server/templates/components/task_list.html`

**Debug**: Logs show grouping works, template rendering issue
- Check HTMX swap behavior
- Verify template variable passing
- Test with simple HTML first

#### 2.4 Fix Inline Schedule Editing
**Status**: Auto-refresh conflicts with editing  
**File**: `server/templates/pages/tasks.html`

**Solution**: Already implemented htmx:beforeSwap handler, needs testing

---

## High Priority (Week 3-4)

### 🟡 Priority 3: UI Organization

#### 3.1 Split ui_routes.rs (1042 lines)
**Current**: Single massive file  
**Target**: Modular structure

```
server/src/routes/ui/
├── mod.rs          # Router assembly
├── dashboard.rs    # Dashboard routes
├── servers.rs      # Server management
├── tasks.rs        # Task management
├── plugins.rs      # Plugin configuration
├── settings.rs     # Settings
└── auth.rs         # Authentication
```

#### 3.2 Build Real Monitoring Dashboard
**Current**: Basic counts  
**Required**:
- Real-time metrics (CPU, memory, disk)
- Task execution history chart
- Recent activity feed
- Health indicators
- Docker container status cards

**Technology**: Use HTMX + SSE (Server-Sent Events) for real-time updates

#### 3.3 Complete Plugin Configuration UI
**Status**: Partially implemented  
**Issues**: Save/load logic incomplete for some plugins

**Files to fix**:
- `server/src/ui_routes.rs` - plugin_config_save handler
- `server/templates/components/plugin_config_form.html`

#### 3.4 Add Server Health Indicators
**Current**: Just connection test  
**Required**:
- Real-time connection status
- Last seen timestamp
- Response time metrics
- Visual health indicators (🟢🟡🔴)

---

## Medium Priority (Week 5-8)

### 🟢 Priority 4: Feature Completeness

#### 4.1 Webhook API Implementation
**Status**: Partially implemented  
**Required**:
- Task triggering via webhooks
- Webhook authentication (with constant-time comparison)
- Webhook event history
- Webhook retry logic

#### 4.2 Task Dependencies/Workflows
**Current**: Independent tasks only  
**Required**:
```rust
pub struct Task {
    // ... existing fields
    pub depends_on: Vec<i64>, // Task IDs that must complete first
    pub on_failure: FailureAction, // Continue, Stop, Retry
    pub on_success: Vec<i64>, // Tasks to trigger on success
}
```

#### 4.3 Metrics Visualization
**Technology**: Chart.js or similar  
**Required**:
- Time-series charts for system metrics
- Task execution duration trends
- Success/failure rates
- Plugin performance metrics

#### 4.4 Audit Logging
**Required**:
```sql
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY,
    user_id INTEGER,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id INTEGER,
    details TEXT,
    ip_address TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

#### 4.5 SSH Key Management UI
**Current**: File-based only  
**Required**:
- Upload SSH keys via UI
- Store keys securely
- Associate keys with servers
- Key rotation support

#### 4.6 Server Grouping/Tagging
**Required**:
```rust
pub struct ServerGroup {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub servers: Vec<i64>,
}
```

---

## Testing & Reliability (Week 9-10)

### 🔵 Priority 5: Testing Infrastructure

#### 5.1 Unit Tests
**Current**: None  
**Target**: 70% coverage

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_execution() {
        // Test plugin logic
    }

    #[test]
    fn test_token_masking() {
        assert_eq!(mask_token("abc123def"), "abc***def");
    }
}
```

#### 5.2 Integration Tests
**Technology**: `axum-test` crate

```rust
#[tokio::test]
async fn test_server_crud() {
    let app = create_test_app().await;
    
    // Test create server
    let response = app.post("/servers")
        .json(&create_server_input)
        .send()
        .await;
    
    assert_eq!(response.status(), StatusCode::OK);
}
```

#### 5.3 CI/CD Improvements
**Add to GitHub Actions**:
```yaml
- name: Run Clippy
  run: cargo clippy -- -D warnings

- name: Security Audit
  run: cargo audit

- name: Run Tests
  run: cargo test --all-features

- name: Check Coverage
  run: cargo tarpaulin --out Xml
```

---

## Advanced Features (Week 11-12)

### 🟣 Priority 6: Real-Time & Advanced

#### 6.1 Replace Polling with SSE
**Current**: 5-second HTMX polling  
**Target**: Server-Sent Events

```rust
use axum::response::sse::{Event, Sse};

async fn task_events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = state.event_bus.subscribe();
    Sse::new(stream)
}
```

#### 6.2 File-Based Secret Management
**Current**: Environment variables only  
**Required**: Support Docker secrets, Kubernetes secrets

```rust
fn get_secret(key_var: &str, file_var: &str) -> Option<String> {
    env::var(key_var).ok().or_else(|| {
        env::var(file_var).ok()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|s| s.trim().to_string())
    })
}
```

#### 6.3 Retry Logic with Exponential Backoff
**Port from weatherust**:

```rust
pub async fn retry_async<F, Fut, T, E>(
    operation: F,
    max_retries: u32,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut delay = initial_delay;
    for attempt in 0..max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt == max_retries - 1 => return Err(e),
            Err(_) => {
                tokio::time::sleep(delay).await;
                delay = delay.mul_f32(1.5 + rand::random::<f32>() * 0.5); // Jitter
            }
        }
    }
    unreachable!()
}
```

#### 6.4 Notification Metrics
**Track**:
- Notifications sent per backend
- Success/failure rates
- Average delivery time
- Error types

```rust
pub struct NotificationMetrics {
    pub sent: Counter,
    pub failed: Counter,
    pub duration: Histogram,
}
```

#### 6.5 Multi-User Support with RBAC
**Required**:
```rust
pub enum Role {
    Admin,      // Full access
    Operator,   // Can run tasks, view all
    Viewer,     // Read-only access
}

pub struct Permission {
    pub resource: String, // "servers", "tasks", "plugins"
    pub action: String,   // "read", "write", "execute"
}
```

---

## Technology Upgrades

### Immediate
- ✅ Axum 0.8.x - Current
- ✅ HTMX 2.0.3 - Current  
- ⚠️ **Askama 0.12 → 0.14** - Upgrade recommended

### Add Dependencies
```toml
[dependencies]
subtle = "2.6"              # Constant-time comparison
argon2 = "0.5"             # Password hashing
tower-http = { version = "0.6", features = ["csrf", "compression"] }
axum-test = "15.0"         # Testing
prometheus = "0.13"        # Metrics
```

---

## Migration Path from Weatherust

### Phase 1: Parallel Deployment (Week 1-2)
1. Fix security issues in SvrCtlRS
2. Deploy SvrCtlRS alongside weatherust
3. Route 10% of traffic to SvrCtlRS (canary)
4. Monitor for issues

### Phase 2: Gradual Migration (Week 3-4)
1. Increase SvrCtlRS traffic to 50%
2. Verify all features working
3. Monitor metrics and logs
4. Fix any issues discovered

### Phase 3: Full Migration (Week 5-6)
1. Route 100% traffic to SvrCtlRS
2. Keep weatherust as backup for 1 month
3. Document any differences
4. Train users on new UI

### Phase 4: Decommission (Week 7-8)
1. Archive weatherust configuration
2. Stop weatherust containers
3. Document lessons learned
4. Celebrate! 🎉

---

## Success Metrics

### Week 2 (Security Complete)
- ✅ All security issues resolved
- ✅ CSRF protection active
- ✅ Token masking implemented
- ✅ Authentication working
- ✅ Constant-time comparison in use

### Week 4 (UI Polished)
- ✅ ui_routes.rs split into modules
- ✅ Dashboard shows real metrics
- ✅ Task grouping displays correctly
- ✅ Inline editing works
- ✅ Server health indicators visible

### Week 8 (Feature Complete)
- ✅ Webhook API functional
- ✅ Task dependencies working
- ✅ Audit logging active
- ✅ SSH key management via UI
- ✅ Server grouping implemented

### Week 10 (Production Ready)
- ✅ 70%+ test coverage
- ✅ CI/CD with clippy + audit
- ✅ Load tested
- ✅ Documentation complete
- ✅ Monitoring in place

### Week 12 (Enterprise Grade)
- ✅ SSE real-time updates
- ✅ Multi-user RBAC
- ✅ Prometheus metrics
- ✅ File-based secrets
- ✅ Retry logic throughout

---

## Files Requiring Immediate Attention

### Critical (This Week)
1. `server/src/routes/webhooks.rs` - Add constant-time comparison
2. `core/src/notifications.rs` - Add token masking, verify ntfy fix
3. `server/src/routes/auth.rs` - Implement authentication
4. `database/src/lib.rs` - Add transaction support
5. `server/src/main.rs` - Add CSRF middleware

### High Priority (Next Week)
1. `server/src/ui_routes.rs` - Split into modules (1042 lines)
2. `server/templates/pages/dashboard.html` - Build real dashboard
3. `server/templates/components/task_list.html` - Fix grouping display
4. `server/templates/pages/tasks.html` - Verify inline editing fix

### Medium Priority (Week 3-4)
1. `server/src/routes/api.rs` - Expand webhook API
2. `scheduler/src/lib.rs` - Add task dependencies
3. `database/migrations/` - Add audit_log table
4. `plugins/*/src/lib.rs` - Add unit tests

---

## Conclusion

**Current State**: B+ (85/100) - Excellent foundation, critical gaps

**After Security Fixes**: A- (90/100) - Production-ready

**After UI Polish**: A (95/100) - Professional platform

**After Full Roadmap**: A+ (98/100) - Enterprise-grade

**Recommendation**: 
1. ✅ **This week**: Fix security issues
2. ✅ **Next week**: Polish UI
3. ✅ **Week 3**: Start parallel deployment with weatherust
4. ✅ **Week 6**: Full migration to SvrCtlRS
5. ✅ **Week 12**: Enterprise features

SvrCtlRS is **architecturally superior** to weatherust and just needs security hardening and UI polish to be production-ready. The plugin system, single-binary deployment, and modern UI make it the clear path forward.

---

**Assessment By**: Combined Internal + External Review  
**Next Review**: After Week 2 (Security Complete)  
**Target Production Date**: Week 6 (After parallel deployment)

