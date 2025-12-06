# Next Session Quick Start

**Previous Session:** 468 → 302 errors (166 fixed, 35% reduction)
**Target This Session:** 302 → 0 errors (100% completion)

## Start Here

```bash
cd /home/jsprague/Development/svrctlrs

# Check current status
cargo check --package server 2>&1 | grep "^error" | wc -l
# Expected: ~302 errors

# Read full progress report
cat RESTRUCTURE_PROGRESS.md
```

## Priority Order (Do These First)

### 1. Fix Old References (12 errors - EASIEST)

**Search for old references:**
```bash
grep -rn "queries::tasks" server/src/
grep -rn "models::AuthType" server/src/
```

**Files to edit:**
- `server/src/routes/ui/dashboard.rs`
- `server/src/routes/api.rs`
- `server/src/state.rs`

**Replace:**
- `queries::tasks::` → `queries::job_schedules::`
- `models::AuthType` → Remove (use credentials table instead)

### 2. Fix .ok_or_else() Patterns (10 errors - QUICK)

**Find them:**
```bash
grep -n "ok_or_else" server/src/routes/ui/notifications.rs
grep -n "ok_or_else" server/src/routes/ui/job_runs.rs
```

**Pattern to fix:**
```rust
// REMOVE the .ok_or_else() block, change error! to warn! in map_err
// See RESTRUCTURE_PROGRESS.md line 343 for detailed example
```

### 3. Add Missing Function (4 errors - 5 MINUTES)

**File:** `database/src/queries/job_runs.rs`

**Add:**
```rust
pub async fn list_job_runs_paginated(
    pool: &Pool<Sqlite>,
    limit: i64,
    offset: i64,
) -> Result<Vec<JobRun>> {
    sqlx::query_as::<_, JobRun>(
        "SELECT * FROM job_runs ORDER BY started_at DESC LIMIT ? OFFSET ?"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
```

### 4. Fix ServerDisplay (7 errors - 10 MINUTES)

**File:** `server/src/templates.rs`

**Add missing fields:**
```rust
pub struct ServerDisplay {
    pub id: i64,
    pub name: String,
    pub host: String,           // ADD THIS
    pub port: i32,              // ADD THIS
    pub username: String,       // ADD THIS
    pub status: String,
    pub tags: Vec<String>,
}

// Update From<Server>:
impl From<Server> for ServerDisplay {
    fn from(server: Server) -> Self {
        Self {
            id: server.id,
            name: server.name,
            host: server.hostname,     // Map hostname → host
            port: server.port,
            username: server.ssh_user,  // Map ssh_user → username
            status: server.status.unwrap_or_else(|| "unknown".to_string()),
            tags: Vec::new(), // TODO: Load from server_tags
        }
    }
}
```

### 5. Fix Other Display Structs (20 errors - 30 MINUTES)

**JobTemplateDisplay - Add:**
- `target_type: String`
- `target_tags: Vec<String>`

**NotificationPolicyDisplay - Add:**
- `scope_type: String`

**JobScheduleFormTemplate - Fix:**
- Change field name or update template to match

**JobTemplateFormTemplate - Add:**
- `command_templates: Vec<CommandTemplate>`

## After Quick Wins (Expected: ~276 errors)

**Run:**
```bash
cargo check --package server 2>&1 | grep "^error\[" | sort | uniq -c | sort -rn | head -20
```

**This will show remaining error categories. Then tackle them systematically.**

## Useful Commands

```bash
# Find specific error types
cargo check --package server 2>&1 | grep "mismatched types" | wc -l
cargo check --package server 2>&1 | grep "no field" | wc -l

# Search for patterns in code
rg "\.ok_or_else" server/src/routes/ui/
rg "queries::tasks" server/src/

# Check specific file
cargo check --package server 2>&1 | grep "notifications.rs" | head -20
```

## Success Indicators

After each fix batch, run:
```bash
cargo check --package server 2>&1 | grep "^error" | wc -l
```

**Milestones:**
- [ ] 302 → 276 (Quick wins done)
- [ ] 276 → 249 (Template structs fixed)
- [ ] 249 → 200 (Type mismatches fixed)
- [ ] 200 → 0 (Final cleanup)

## When You Hit 0 Errors

```bash
# Full build
cargo build --workspace --release

# Run tests
cargo test --workspace

# Run migration
cargo run --bin server
# Check for migration success in logs

# Open UI
# http://localhost:8080
```

## If You Get Stuck

1. Check RESTRUCTURE_PROGRESS.md for detailed examples
2. Check RESTRUCTURE_COMPLETE.md for architecture reference
3. Check QUICK_START.md for user workflow
4. Use Context7 to look up latest Rust/Askama patterns

## Key Files Reference

```
Configuration:
  database/migrations/011_complete_restructure.sql  # Schema
  config/example.toml                               # Settings

Core:
  core/src/executor.rs                              # Job execution
  scheduler/src/lib.rs                              # Scheduling
  database/src/notification_service.rs              # Notifications

Routes (fix these):
  server/src/routes/ui/notifications.rs             # 7 .ok_or_else()
  server/src/routes/ui/job_runs.rs                  # 4 .ok_or_else()
  server/src/routes/ui/dashboard.rs                 # old tasks refs
  server/src/routes/api.rs                          # old tasks refs
  server/src/state.rs                               # old tasks refs

Templates (fix these):
  server/src/templates.rs                           # Display struct fields
  server/templates/                                 # HTML templates

Query Modules:
  database/src/queries/                             # Add missing functions
```

## Estimated Time Budget

- Quick wins (1-3): 45 minutes
- Display struct fixes (4-5): 30 minutes
- Type mismatches: 1-2 hours
- Final cleanup: 1 hour
- **Total: 3-4 hours to completion**

Good luck! The hard work is done - this is just cleanup! 🚀
