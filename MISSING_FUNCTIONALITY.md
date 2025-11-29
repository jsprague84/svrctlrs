# Missing Functionality - SvrCtlRS

## 🔴 Critical - Core Features Not Implemented

### 1. **SSH Connection Testing**
**Status**: Placeholder implemented, returns "Feature coming soon"  
**Location**: `server/src/routes/ui/servers.rs:377-397`  
**Route**: `POST /servers/test`  
**What's needed**:
- Implement actual SSH connection test using `async-ssh2-tokio`
- Test with provided hostname, port, username
- Return success/failure with meaningful error messages
- Support both password and key-based authentication
- Test credential validation

**Related Route**: `POST /servers/{id}/test` (line 400) - also needs implementation

---

### 2. **Job Template Execution ("Run Now")**
**Status**: Route does NOT exist (404)  
**Location**: Template references `/job-templates/{id}/run`  
**Template**: `server/templates/components/job_template_list.html:54`  
**What's needed**:
- Create route `POST /job-templates/{id}/run` in `job_templates.rs`
- Implement handler to:
  - Validate job template exists
  - Create a `job_run` record
  - Trigger job execution via job executor
  - Return job run ID or status
  - Handle both simple and composite jobs
  - Support parameter overrides (server selection, etc.)

---

### 3. **Job Schedule Execution ("Run Now")**
**Status**: Route exists but not fully implemented  
**Location**: `server/src/routes/ui/job_schedules.rs:442`  
**Route**: `POST /job-schedules/{id}/run`  
**What's needed**:
- Complete the `run_job_schedule` function (currently has TODO at line 457)
- Integrate with job execution engine
- Create job_run record
- Execute the associated job template
- Return execution status

---

### 4. **Notification Channel Testing**
**Status**: Route does NOT exist (404)  
**Location**: Template references `/notification-channels/{id}/test`  
**Template**: `server/templates/components/notification_channel_list.html:24`  
**What's needed**:
- Create route `POST /settings/notifications/channels/{id}/test` in `notifications.rs`
- Implement handler to:
  - Send test notification to Gotify or ntfy.sh
  - Validate credentials/tokens
  - Return success/failure with error details
  - Handle authentication (Bearer token, Basic auth)

---

## 🟡 Medium Priority - Data Population

### 5. **Credential Server Count**
**Status**: Hardcoded to 0  
**Location**: `server/src/routes/ui/credentials.rs:118`  
**What's needed**:
- Query `servers` table to count servers using each credential
- Add JOIN or subquery to fetch actual count
- Display in credential list

---

### 6. **Server Credential Name Display**
**Status**: Empty string  
**Location**: `server/src/routes/ui/servers.rs:754`  
**What's needed**:
- JOIN with `credentials` table to fetch credential name
- Display in server list/detail views

---

### 7. **Server Capabilities Display**
**Status**: Empty array  
**Location**: `server/src/routes/ui/servers.rs:775`  
**What's needed**:
- Query `server_capabilities` table
- Display capabilities (docker, apt, dnf, etc.) for each server
- Auto-detect capabilities on server creation/connection

---

### 8. **Server OS Version Extraction**
**Status**: Empty string  
**Location**: `server/src/routes/ui/servers.rs:766`  
**What's needed**:
- Extract OS version from `os_distro` or metadata
- Display in server details

---

## 🟢 Low Priority - Polish & Enhancement

### 9. **Authentication System**
**Status**: Placeholder, always returns None  
**Location**: `server/src/routes/ui/mod.rs:54-56`  
**Routes**: `server/src/routes/ui/auth.rs:32, 41`  
**What's needed**:
- Implement session management with `tower-sessions`
- Add login/logout functionality
- Protect routes with authentication middleware
- Add user management (create, edit, delete users)
- Add role-based access control (RBAC)

---

### 10. **Job Execution Engine Integration**
**Status**: TODO comment in job schedules  
**Location**: `server/src/routes/ui/job_schedules.rs:457`  
**What's needed**:
- Complete integration between UI and job execution engine
- Ensure job runs are properly tracked in `job_runs` table
- Update `server_job_results` and `step_execution_results` tables
- Send notifications based on policies

---

### 11. **Legacy Tasks Page**
**Status**: Unknown - needs investigation  
**Location**: Navigation shows "Tasks" link to `/tasks`  
**What's needed**:
- Determine if this should be removed (marked as "Legacy")
- Or implement if still needed for backward compatibility
- Same for "Plugins" page

---

## 📋 Summary by Priority

### Must Have (Blocking Core Functionality)
1. SSH Connection Testing
2. Job Template Execution
3. Job Schedule Execution  
4. Notification Channel Testing

### Should Have (Improves UX)
5. Credential Server Count
6. Server Credential Name
7. Server Capabilities
8. Server OS Version

### Nice to Have (Future Enhancement)
9. Authentication System
10. Job Execution Engine Integration
11. Legacy Pages Cleanup

---

## 🎯 Recommended Implementation Order

1. **SSH Connection Testing** - Critical for server management
2. **Notification Channel Testing** - Needed to verify notification setup
3. **Job Template Execution** - Core job functionality
4. **Job Schedule Execution** - Complete the scheduler
5. **Data Population** (items 5-8) - Improve data display
6. **Authentication** - Security before production
7. **Cleanup** - Remove or implement legacy pages

---

## 📝 Notes

- All TODO comments have been cataloged
- Routes that return 404 have been identified
- Placeholder implementations have been documented
- Database queries needing JOINs have been noted

**Generated**: 2025-11-29  
**Based on**: Comprehensive UI testing and code review

