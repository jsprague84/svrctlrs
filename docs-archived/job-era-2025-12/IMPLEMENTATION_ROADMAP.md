# Implementation Roadmap: Complete UI Feature Coverage

**Goal**: Implement all backend features in UI for complete flexibility, configurability, and ease of use

**Current Coverage**: 48% → Target: 100%

## 🎯 Vision

Create a **powerful, flexible, user-friendly** interface for:
- **Remote Task Execution**: Any command on any SSH-accessible server
- **Flexible Scheduling**: Cron-based with full override capabilities
- **Extensible Integration**: Docker, Kubernetes (future), etc.
- **Smart Notifications**: Multi-platform (Gotify, ntfy, Slack, Discord future)
- **Complete Customization**: Variables, templates, filters, conditions

---

## Phase 1: Notification System Excellence (Week 1)

**Priority**: HIGH - Core messaging capability

### 1.1 Message Templates ⭐
**Backend**: `notification_policies.title_template`, `body_template`
**UI**: Add template editor with variable hints

**Variables Available**:
- `{{job_name}}` - Job template name
- `{{server_name}}` - Target server
- `{{status}}` - success/failure/timeout
- `{{started_at}}` - Start timestamp
- `{{duration}}` - Execution time
- `{{output}}` - Command output (truncated)
- `{{error}}` - Error message if failed

**Implementation**:
- [ ] Add title_template textarea to policy form
- [ ] Add body_template textarea with variable hints
- [ ] Add "Insert Variable" dropdown helper
- [ ] Preview functionality (optional)
- [ ] Default template if blank

### 1.2 Advanced Filtering ⭐
**Backend**: `job_type_filter`, `server_filter`, `tag_filter` (JSON arrays)
**UI**: Multi-select dropdowns

**Implementation**:
- [ ] Add job type multi-select (from job_types table)
- [ ] Add server multi-select (from servers table)
- [ ] Add tag multi-select (from tags table)
- [ ] Build JSON arrays on form submit
- [ ] Parse JSON on form load for editing

### 1.3 Multi-Channel Assignment ⭐
**Backend**: `notification_policy_channels` table (many-to-many)
**UI**: Channel checklist with priority overrides

**Implementation**:
- [ ] Replace single channel dropdown with checkbox list
- [ ] Add priority override input per channel
- [ ] Update backend to handle array of channel assignments
- [ ] Create/update policy_channels associations

### 1.4 Throttling & Severity
**Backend**: `min_severity`, `max_per_hour`
**UI**: Number inputs with explanations

**Implementation**:
- [ ] Add severity level selector (1-5)
- [ ] Add max notifications per hour input
- [ ] Add helpful descriptions
- [ ] Backend enforcement in NotificationService

---

## Phase 2: Command Template Power (Week 2)

**Priority**: HIGH - Foundation for extensibility

### 2.1 Parameter Schema UI ⭐ (Migration 012)
**Backend**: `command_templates.parameter_schema` (JSON)
**UI**: Dynamic parameter editor

**Schema Structure**:
```json
[
  {
    "name": "container_name",
    "type": "string|number|boolean|select",
    "required": true,
    "description": "Container to operate on",
    "default": "app",
    "options": ["app", "db", "cache"]  // for select type
  }
]
```

**Implementation**:
- [ ] Parameter list UI (add/remove params)
- [ ] Type selector dropdown
- [ ] Required checkbox
- [ ] Default value input
- [ ] Options editor (for select type)
- [ ] Save as JSON to parameter_schema column
- [ ] **Job execution**: Render param inputs based on schema
- [ ] **Variable substitution**: Replace `{{param_name}}` in commands

### 2.2 OS Filtering
**Backend**: `command_templates.os_filter` (JSON)
**UI**: OS compatibility selector

**Implementation**:
- [ ] Distro multi-select (ubuntu, debian, fedora, arch, etc.)
- [ ] OS type selector (linux, windows, macos)
- [ ] Save as JSON to os_filter column
- [ ] Filter commands by server OS on execution

### 2.3 Environment Variables
**Backend**: `command_templates.environment` (JSON)
**UI**: Key-value editor

**Implementation**:
- [ ] Dynamic env var editor (add/remove rows)
- [ ] Key input + Value input pairs
- [ ] Save as JSON
- [ ] Pass to executor on job run

### 2.4 Output Parser Configuration
**Backend**: `command_templates.parse_output`, `output_parser` (JSON)
**UI**: Parser config editor

**Implementation**:
- [ ] Enable parsing checkbox
- [ ] Parser type selector (regex, json, table)
- [ ] Parser config editor (regex patterns, JSON paths)
- [ ] Display parsed output in job run results

---

## Phase 3: Job Template Flexibility (Week 3)

**Priority**: MEDIUM - Enhanced workflow capabilities

### 3.1 Composite Job Steps UI ⭐
**Backend**: `job_template_steps` table
**UI**: Step builder/editor

**Implementation**:
- [ ] "Add Step" button on job template form
- [ ] Step list with drag-to-reorder
- [ ] Per-step settings:
  - [ ] Command template selector
  - [ ] Variables override
  - [ ] Continue on failure checkbox
  - [ ] Timeout override
- [ ] Step execution order indicator
- [ ] Save/update job_template_steps records

### 3.2 Variables Editor
**Backend**: `job_templates.variables` (JSON)
**UI**: Key-value editor for template-level variables

**Implementation**:
- [ ] Dynamic variable editor
- [ ] Variable hints from command template
- [ ] Default values from parameter schema
- [ ] Validation against required parameters

### 3.3 Retry Configuration UI
**Backend**: `job_templates.retry_count`, `retry_delay_seconds`
**UI**: Retry settings inputs

**Implementation**:
- [ ] Retry count number input
- [ ] Retry delay seconds input
- [ ] Explanation of retry behavior

---

## Phase 4: Monitoring & Visibility (Week 4)

**Priority**: MEDIUM - Operational visibility

### 4.1 Server Capabilities View ⭐
**Backend**: `server_capabilities` table
**UI**: Capabilities tab on server detail

**Implementation**:
- [ ] Capabilities list view per server
- [ ] Detection status (available/unavailable)
- [ ] Version info display
- [ ] "Re-detect" button to trigger capability scan
- [ ] Capability-based command filtering hints

### 4.2 Notification Audit Log
**Backend**: `notification_log` table
**UI**: New page at `/settings/notifications/log`

**Implementation**:
- [ ] Notification log list page
- [ ] Filters: channel, success/failure, date range
- [ ] View sent message content
- [ ] Retry count display
- [ ] Error message display for failures
- [ ] Link to originating job run

### 4.3 Step Execution Results
**Backend**: `step_execution_results` table
**UI**: Step-by-step view on job run detail

**Implementation**:
- [ ] Expandable step list on job run page
- [ ] Per-step status, timing, output
- [ ] Step failure highlights
- [ ] Continue-on-failure indicator

---

## Phase 5: Schedule Overrides (Week 5)

**Priority**: LOW - Nice to have

### 5.1 Per-Schedule Overrides
**Backend**: `job_schedules.timeout_seconds`, `retry_count`, `notify_on_*`
**UI**: Override section in schedule form

**Implementation**:
- [ ] "Override Template Defaults" checkbox
- [ ] Timeout override input
- [ ] Retry override inputs
- [ ] Notification override checkboxes
- [ ] Show template defaults for comparison

---

## Phase 6: Settings Management (Week 6)

**Priority**: LOW - Admin features

### 6.1 Settings UI
**Backend**: `settings` table
**UI**: Settings management page

**Implementation**:
- [ ] Settings list with categories
- [ ] Edit settings values
- [ ] Type-aware inputs (string, number, boolean)
- [ ] Save to settings table
- [ ] Apply settings reload

---

## 🔧 Technical Approach

### Backend Pattern
```rust
// Example: Multi-channel policy
#[derive(Deserialize)]
struct CreatePolicyInput {
    name: String,
    on_success: bool,
    on_failure: bool,
    on_timeout: bool,
    channel_ids: Vec<i64>,  // NEW
    priority_overrides: HashMap<i64, i32>,  // NEW
    job_type_filter: Option<Vec<String>>,  // NEW
    server_filter: Option<Vec<i64>>,  // NEW
    tag_filter: Option<Vec<String>>,  // NEW
    title_template: Option<String>,  // NEW
    body_template: Option<String>,  // NEW
    min_severity: Option<i32>,  // NEW
    max_per_hour: Option<i32>,  // NEW
}

async fn create_policy(input: CreatePolicyInput) -> Result<()> {
    // 1. Create notification_policies record
    let policy_id = insert_policy(...).await?;

    // 2. Create notification_policy_channels records
    for channel_id in input.channel_ids {
        let priority = input.priority_overrides.get(&channel_id);
        insert_policy_channel(policy_id, channel_id, priority).await?;
    }

    Ok(())
}
```

### Frontend Pattern
```html
<!-- Message Templates -->
<div class="form-group">
    <label>Title Template</label>
    <input type="text" name="title_template"
           placeholder="{{job_name}} {{status}} on {{server_name}}">
    <small>Variables: {{job_name}}, {{server_name}}, {{status}}, {{duration}}</small>
</div>

<!-- Multi-Channel Selection -->
<div class="form-group">
    <label>Notification Channels</label>
    {% for channel in channels %}
    <label class="checkbox-inline">
        <input type="checkbox" name="channel_ids" value="{{ channel.id }}">
        {{ channel.name }}
        <input type="number" name="priority_{{ channel.id }}"
               placeholder="Priority" min="1" max="10">
    </label>
    {% endfor %}
</div>
```

---

## 📊 Success Metrics

- [ ] **100% Feature Coverage** - All schema features in UI
- [ ] **Zero Manual SQL** - Everything via UI
- [ ] **Template Variables Work** - Messages fully customizable
- [ ] **Filtering Works** - Policies apply to correct jobs
- [ ] **Multi-Channel Works** - One policy → many channels
- [ ] **Parameter Schema Works** - Dynamic command inputs
- [ ] **Composite Jobs Work** - Multi-step execution visible
- [ ] **Audit Trail Visible** - Can see notification history

---

## 🚀 Getting Started

**This Session**: Phase 1.1 - Message Templates
**Next Session**: Phase 1.2 - Advanced Filtering
**By End of Week**: Complete notification system enhancement

Let's build the most flexible, powerful remote task management system! 🎉
