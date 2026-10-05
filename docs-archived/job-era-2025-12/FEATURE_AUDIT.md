# Feature Audit: Database Schema vs UI Implementation

**Date**: 2025-11-30
**Migration**: 012 (latest)
**Purpose**: Identify database features not fully exposed in UI

## ✅ Fully Implemented Features

### 1. Core Entity Management
- **Servers** - Full CRUD with credential assignment, tags
- **Credentials** - SSH keys, API tokens, passwords
- **Tags** - Organization with colors
- **Job Types** - Built-in job categories
- **Job Templates** - Reusable job definitions
- **Job Schedules** - Cron-based scheduling
- **Job Runs** - Execution history viewing

### 2. Notifications
- **Notification Channels** - Gotify, ntfy configuration
- **Notification Policies** - Basic policy creation
- **Auto-notification** - Scheduler integration (✅ just implemented)

## ⚠️ Partially Implemented Features

### 3. Command Templates
**Schema Support:**
- Variable substitution `{{variable}}`
- OS filtering (distro-specific commands)
- Capability requirements (docker, apt, etc.)
- Output parsing configuration
- Environment variables
- Working directory
- Timeout overrides

**UI Status:**
- ✅ Basic CRUD in job types view
- ❌ **Missing**: Parameter schema UI (migration 012)
- ❌ **Missing**: OS filter UI
- ❌ **Missing**: Output parser configuration
- ❌ **Missing**: Environment variables UI

### 4. Job Templates
**Schema Support:**
- Simple jobs (single command)
- Composite jobs (multi-step workflows via `job_template_steps`)
- Variable substitution
- Retry configuration (count + delay)
- Notification policy assignment

**UI Status:**
- ✅ Basic template creation
- ⚠️ **Partial**: Composite jobs UI (steps management unclear)
- ❌ **Missing**: Variables UI for template substitution
- ❌ **Missing**: Retry configuration UI

### 5. Notification Policies
**Schema Support:**
- Multi-channel assignment (via `notification_policy_channels`)
- Priority overrides per channel
- Filtering: job_type, server, tags
- Throttling: `min_severity`, `max_per_hour`
- Message templates (title + body)

**UI Status:**
- ✅ Basic policy creation
- ✅ Trigger conditions (on_success, on_failure, on_timeout)
- ❌ **Missing**: Multiple channels per policy
- ❌ **Missing**: Priority override UI
- ❌ **Missing**: Filtering UI (job_type, server, tags)
- ❌ **Missing**: Throttling/severity UI
- ❌ **Missing**: Message template UI

### 6. Server Capabilities
**Schema Support:**
- Auto-detection (docker, systemd, package managers)
- Version tracking
- Capability-based job filtering

**UI Status:**
- ❌ **Missing**: Capability viewing UI
- ❌ **Missing**: Manual capability management
- ❌ **Missing**: Capability detection trigger

## ❌ Not Implemented in UI

### 7. Composite Job Step Results
**Schema**: `step_execution_results` table
- Tracks execution of each step in multi-step jobs
- Per-step status, timing, output

**UI**: ❌ No step-level detail view

### 8. Server Job Results (Multi-Server)
**Schema**: `server_job_results` table
- Reserved for future multi-server job support
- Per-server results for jobs run on multiple servers

**UI**: ❌ Not implemented (schema says "currently unused")

### 9. Notification Log/Audit Trail
**Schema**: `notification_log` table
- Audit trail of all sent notifications
- Retry tracking
- Success/failure status

**UI**: ❌ No notification history view

### 10. Settings Management
**Schema**: `settings` table with defaults
- App name, version
- Scheduler config
- Job retention
- SSH timeouts
- UI preferences

**UI**: ⚠️ Settings page exists but functionality unclear

### 11. Advanced Job Schedule Features
**Schema Support:**
- Timeout overrides
- Retry overrides
- Notification overrides

**UI Status:**
- ❌ **Missing**: Override UI in schedule form

## 🔍 Database Features to Investigate

### Migration 012: Parameter Schema
**Added**: `parameter_schema` column to `command_templates`

**Purpose**: Define parameters for command templates with:
- Type (string, number, boolean, select)
- Required flag
- Default values
- Options (for select type)
- Descriptions

**UI**: Needs investigation - likely not implemented

## 📊 Summary Statistics

| Category | Schema Features | UI Implemented | Coverage |
|----------|----------------|----------------|----------|
| Core Entities | 7 | 7 | 100% |
| Advanced Config | 15 | 5 | 33% |
| Monitoring/Audit | 3 | 0 | 0% |
| **Total** | **25** | **12** | **48%** |

## 🎯 Priority Recommendations

### High Priority (Core functionality gaps)
1. **Parameter Schema UI** - Migration 012 feature not exposed
2. **Notification Policy Filtering** - Job type/server/tag filters
3. **Notification Policy Multi-Channel** - Can assign policy to multiple channels
4. **Server Capabilities View** - See what's detected on each server
5. **Message Templates** - Customize notification messages

### Medium Priority (Enhanced features)
6. **Composite Job Steps UI** - Manage multi-step workflows
7. **Step Results View** - See detailed step execution
8. **Notification Audit Log** - View notification history
9. **Retry/Timeout Overrides** - Per-schedule overrides
10. **Command Template Advanced** - OS filters, env vars, output parsing

### Low Priority (Nice to have)
11. **Settings Management UI** - Expose app settings
12. **Variable Substitution UI** - Template variable editor
13. **Throttling/Severity** - Rate limiting notifications

## 🔧 Next Steps

1. **Audit existing UI forms** - Check which fields are actually present
2. **Review JobExecutor** - Verify if step results are being captured
3. **Check notification_service.rs** - Verify filtering logic exists
4. **Test composite jobs** - See if steps UI works
5. **Plan phased implementation** - High priority items first
