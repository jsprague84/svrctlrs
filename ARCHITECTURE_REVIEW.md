# SvrCtlRS Architecture Review & Restructuring Plan

## Current Structure Analysis

### ✅ What's Working Well

1. **Workspace Organization**
   - Clear separation of concerns (core, server, plugins, scheduler, database)
   - Plugin architecture is modular and extensible
   - Workspace dependencies are well-managed

2. **Plugin System**
   - Each plugin is a separate crate
   - Easy to add/remove plugins
   - Good isolation

3. **Core Library**
   - Shared types and traits
   - Notification system
   - Remote execution

### ⚠️ Areas for Improvement

1. **Server Module Structure**
   ```
   server/src/
   ├── bin/svrctl.rs          # CLI tool
   ├── routes/                # API routes (good)
   ├── config.rs              # Config (good)
   ├── main.rs                # Entry point (good)
   ├── routes.rs              # Route aggregator (could be better)
   ├── state.rs               # App state (good)
   ├── templates.rs           # Template structs (could be organized better)
   └── ui_routes.rs           # UI routes (could be organized better)
   ```

2. **Documentation Scattered**
   - Root level: README.md, CLAUDE.md, DOCKER_WORKFLOW.md, etc.
   - docs/ directory: Some old docs
   - No clear hierarchy

3. **Configuration**
   - Multiple config files at root (config.toml, config.example.toml, docker-vm-config.toml)
   - Could be organized better

4. **Static Assets**
   - Currently flat structure in server/static/
   - Could benefit from better organization as it grows

## Recommended Structure

### Option 1: Minimal Changes (Recommended for Now)

Keep the current workspace structure but reorganize the server module:

```
svrctlrs/
├── crates/                     # Rename from root-level dirs
│   ├── core/                   # Shared library
│   ├── database/               # Database layer
│   ├── scheduler/              # Task scheduler
│   └── server/                 # Main application
│       ├── src/
│       │   ├── api/            # API layer (rename from routes/)
│       │   │   ├── mod.rs
│       │   │   ├── v1/         # Versioned API
│       │   │   │   ├── mod.rs
│       │   │   │   ├── servers.rs
│       │   │   │   ├── tasks.rs
│       │   │   │   └── plugins.rs
│       │   │   └── webhooks.rs
│       │   ├── ui/             # UI layer
│       │   │   ├── mod.rs
│       │   │   ├── routes.rs   # Route definitions
│       │   │   ├── handlers/   # Route handlers
│       │   │   │   ├── mod.rs
│       │   │   │   ├── dashboard.rs
│       │   │   │   ├── servers.rs
│       │   │   │   ├── tasks.rs
│       │   │   │   ├── plugins.rs
│       │   │   │   └── auth.rs
│       │   │   └── templates/  # Template structs
│       │   │       ├── mod.rs
│       │   │       ├── pages.rs
│       │   │       └── components.rs
│       │   ├── bin/
│       │   │   └── svrctl.rs
│       │   ├── config.rs
│       │   ├── main.rs
│       │   └── state.rs
│       ├── static/             # Static assets
│       │   ├── css/
│       │   ├── js/
│       │   └── images/         # Future: Add images
│       └── templates/          # Askama templates
│           ├── base.html
│           ├── components/
│           └── pages/
├── plugins/                    # Plugin crates
│   ├── docker/
│   ├── health/
│   ├── updates/
│   ├── weather/
│   └── speedtest/
├── config/                     # Configuration files
│   ├── example.toml
│   ├── docker-vm.toml
│   └── production.toml
├── docs/                       # Documentation
│   ├── README.md               # Docs index
│   ├── development/
│   │   ├── setup.md
│   │   ├── plugins.md
│   │   └── testing.md
│   ├── deployment/
│   │   ├── docker.md
│   │   └── docker-vm.md
│   └── architecture/
│       ├── overview.md
│       └── plugins.md
├── scripts/                    # Utility scripts
│   ├── dev.sh                  # Development helpers
│   └── deploy.sh               # Deployment helpers
├── .github/
│   └── workflows/
├── Cargo.toml                  # Workspace root
├── Cargo.lock
├── README.md                   # Project overview
├── CLAUDE.md                   # AI development guide
├── Dockerfile
└── docker-compose.yml
```

### Option 2: Full Restructure (Future Consideration)

More opinionated structure for larger projects:

```
svrctlrs/
├── crates/
│   ├── svrctlrs-core/         # Explicit naming
│   ├── svrctlrs-database/
│   ├── svrctlrs-scheduler/
│   ├── svrctlrs-server/
│   └── svrctlrs-cli/          # Separate CLI crate
├── plugins/
│   ├── svrctlrs-plugin-docker/
│   ├── svrctlrs-plugin-health/
│   └── ...
├── web/                        # Future: Separate web assets
│   ├── src/
│   ├── public/
│   └── package.json
├── config/
├── docs/
├── scripts/
└── ...
```

## Immediate Recommendations

### Phase 1: Quick Wins (Do Now)

1. **Consolidate Documentation**
   ```bash
   # Move all docs to docs/
   mv DOCKER_WORKFLOW.md docs/deployment/docker.md
   mv DOCKER_VM_SETUP.md docs/deployment/docker-vm.md
   mv PROJECT_STATUS.md docs/status.md
   
   # Keep at root:
   # - README.md (project overview)
   # - CLAUDE.md (AI development guide)
   # - Cargo.toml (workspace)
   ```

2. **Organize Config Files**
   ```bash
   mkdir -p config
   mv config.example.toml config/example.toml
   mv docker-vm-config.toml config/docker-vm.toml
   # Keep config.toml at root (gitignored)
   ```

3. **Clean Up Empty/Unused Directories**
   ```bash
   # Remove empty scripts/ if unused
   # Remove old docs/ content
   ```

4. **Reorganize Server Module**
   - Split `ui_routes.rs` into separate handler files
   - Split `templates.rs` into logical modules
   - Create `api/v1/` for versioned API endpoints

### Phase 2: Server Module Refactor (Next Sprint)

1. **Create Handler Modules**
   ```rust
   // server/src/ui/handlers/dashboard.rs
   pub async fn dashboard_page(...) -> Result<Html<String>, AppError> { }
   
   // server/src/ui/handlers/servers.rs
   pub async fn servers_page(...) -> Result<Html<String>, AppError> { }
   pub async fn server_create(...) -> Result<Html<String>, AppError> { }
   // etc.
   ```

2. **Organize Templates**
   ```rust
   // server/src/ui/templates/pages.rs
   pub struct DashboardTemplate { }
   pub struct ServersTemplate { }
   
   // server/src/ui/templates/components.rs
   pub struct ServerListTemplate { }
   pub struct ServerFormTemplate { }
   ```

3. **Version API**
   ```rust
   // server/src/api/v1/servers.rs
   pub fn routes() -> Router<AppState> { }
   
   // server/src/api/v1/mod.rs
   pub fn routes() -> Router<AppState> {
       Router::new()
           .nest("/servers", servers::routes())
           .nest("/tasks", tasks::routes())
           .nest("/plugins", plugins::routes())
   }
   ```

### Phase 3: Advanced (Future)

1. **Separate CLI Crate**
   - Move `svrctl` to its own crate
   - Share code via `svrctlrs-core`

2. **Add Integration Tests**
   ```
   server/
   ├── src/
   ├── tests/
   │   ├── api/
   │   ├── ui/
   │   └── integration/
   └── Cargo.toml
   ```

3. **Add Examples**
   ```
   examples/
   ├── custom-plugin.rs
   ├── notification-setup.rs
   └── remote-execution.rs
   ```

## Benefits of Restructuring

### Immediate (Phase 1)
- ✅ Cleaner root directory
- ✅ Better documentation organization
- ✅ Easier to find configuration files
- ✅ Professional appearance

### Medium-term (Phase 2)
- ✅ Easier to navigate server code
- ✅ Better separation of concerns
- ✅ Easier to test individual components
- ✅ Clearer API versioning strategy
- ✅ Easier onboarding for new developers

### Long-term (Phase 3)
- ✅ Modular architecture
- ✅ Easy to extract components
- ✅ Better testability
- ✅ Scalable codebase

## Migration Strategy

### Step 1: Documentation (5 minutes)
```bash
mkdir -p docs/deployment
mv DOCKER_WORKFLOW.md docs/deployment/docker.md
mv DOCKER_VM_SETUP.md docs/deployment/docker-vm.md
mv PROJECT_STATUS.md docs/status.md
rm -rf docs/architecture docs/development docs/planning docs/reference
rm docs/CI-CD.md docs/PROGRESS.md
git add -A
git commit -m "docs: consolidate documentation structure"
```

### Step 2: Config Files (2 minutes)
```bash
mkdir -p config
mv config.example.toml config/example.toml
mv docker-vm-config.toml config/docker-vm.toml
git add -A
git commit -m "refactor: organize config files in config/ directory"
```

### Step 3: Server Module (30-60 minutes)
- Create new module structure
- Move code to new locations
- Update imports
- Test compilation
- Commit incrementally

## Recommendation

**Start with Phase 1 (Quick Wins) immediately:**
1. Takes < 10 minutes
2. No code changes, just file moves
3. Immediate improvement in organization
4. No risk of breaking anything

**Plan Phase 2 for next development session:**
1. Requires code refactoring
2. Better done when not actively debugging
3. Can be done incrementally
4. Each step can be tested

**Phase 3 is optional:**
1. Only if project grows significantly
2. Good for team collaboration
3. Overkill for solo development

## Current Assessment

**Overall Grade: B+**

**Strengths:**
- ✅ Good workspace structure
- ✅ Clean plugin architecture
- ✅ Proper separation of concerns
- ✅ Well-organized Cargo workspace

**Improvements Needed:**
- ⚠️ Documentation scattered
- ⚠️ Config files at root
- ⚠️ Server module could be more modular
- ⚠️ Old/unused docs lingering

**Verdict:** The current structure is **good enough for production** but would benefit from Phase 1 cleanup immediately and Phase 2 refactoring when time permits.

