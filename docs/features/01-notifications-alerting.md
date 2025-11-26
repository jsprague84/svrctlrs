# Notifications & Alerting Features

**Category:** Notifications & Alerting  
**Current State:** Gotify and ntfy.sh support with basic notifications  
**Gap:** No aggregation, limited backends, no action button workflows  
**Priority:** Tier 1-2 (High value, low-medium effort)

---

## Overview

The current notification system supports Gotify and ntfy.sh with basic message delivery. This category expands notification capabilities with intelligent aggregation, multiple backend support, advanced action workflows, and customizable templates.

**Value Proposition:**
- Reduce notification fatigue through smart aggregation
- Support team communication tools (Slack, Teams, Discord)
- Enable actionable notifications with webhook buttons
- Provide flexibility through templates and multi-language support

---

## Features

### 1. Smart Alert Aggregation

**Priority:** Tier 1 | **Effort:** Low | **Value:** High

Batch multiple alerts into digest notifications to prevent notification fatigue.

**Capabilities:**
- Configurable aggregation windows (5min, 15min, 1hr, daily)
- Priority-based escalation (critical alerts bypass aggregation)
- Intelligent grouping by server, plugin, or severity
- Quiet hours configuration (suppress non-critical during off-hours)
- Digest summary format with counts and highlights

**Technical Approach:**
```rust
struct AlertAggregator {
    window: Duration,
    buffer: Vec<Alert>,
    last_sent: Instant,
}

impl AlertAggregator {
    async fn add_alert(&mut self, alert: Alert) {
        if alert.priority == Priority::Critical {
            self.send_immediately(alert).await;
        } else {
            self.buffer.push(alert);
            if self.should_flush() {
                self.send_digest().await;
            }
        }
    }
}
```

**Configuration:**
- UI toggle for aggregation per notification backend
- Aggregation window selector
- Priority threshold for bypass
- Quiet hours schedule

---

### 2. Advanced Action Buttons

**Priority:** Tier 2 | **Effort:** Medium | **Value:** High

Multi-step workflows via ntfy action buttons with conditional logic.

**Capabilities:**
- Multi-step workflows (Approve → Execute → Confirm)
- Conditional actions based on system state
- Custom webhook endpoints for actions
- Action approval workflow for destructive operations
- Action history and audit trail

**Technical Approach:**
```rust
struct NotificationAction {
    id: String,
    label: String,
    action_type: ActionType,
    url: String,
    method: HttpMethod,
    requires_approval: bool,
    conditions: Vec<Condition>,
}

enum ActionType {
    View,        // Open URL in browser
    Http,        // Execute HTTP request
    Workflow,    // Multi-step workflow
}
```

**Example Workflow:**
1. User receives "Update Available" notification
2. Clicks "Review Changes" → Opens changelog
3. Clicks "Approve Update" → Requires confirmation
4. Clicks "Execute Update" → Runs update task
5. Receives "Update Complete" notification

---

### 3. Notification Templates

**Priority:** Tier 2 | **Effort:** Medium | **Value:** Medium

Customizable message templates with variable substitution.

**Capabilities:**
- Per-plugin template customization
- Variable substitution (server name, metrics, timestamps)
- Markdown formatting support
- Multi-language support (i18n)
- Template preview in UI

**Template Variables:**
- `{{server.name}}` - Server name
- `{{server.host}}` - Server hostname
- `{{plugin.name}}` - Plugin name
- `{{metric.value}}` - Metric value
- `{{timestamp}}` - Current timestamp
- `{{task.status}}` - Task execution status

**Example Template:**
```markdown
## 🐳 Docker Health Alert

**Server:** {{server.name}}
**Status:** {{status}}

{{#if unhealthy_containers}}
Unhealthy Containers:
{{#each unhealthy_containers}}
- {{name}}: {{health_status}}
{{/each}}
{{/if}}

**Time:** {{timestamp}}
```

---

### 4. Slack Integration

**Priority:** Tier 1 | **Effort:** Low | **Value:** High

Send notifications to Slack channels with interactive buttons.

**Capabilities:**
- Channel notifications
- Direct messages to users
- Slash commands for quick actions (`/svrctlrs status`)
- Interactive message buttons
- Thread replies for related alerts

**Slash Commands:**
- `/svrctlrs status` - Show system status
- `/svrctlrs servers` - List servers
- `/svrctlrs tasks` - Show recent tasks
- `/svrctlrs run <task>` - Execute task

**Technical Approach:**
```rust
struct SlackBackend {
    client: reqwest::Client,
    webhook_url: String,
    bot_token: Option<String>,
}

impl NotificationBackend for SlackBackend {
    async fn send(&self, message: &NotificationMessage) -> Result<()> {
        let payload = json!({
            "text": message.title,
            "blocks": self.format_blocks(message),
            "attachments": self.format_actions(message.actions),
        });
        self.client.post(&self.webhook_url).json(&payload).send().await?;
        Ok(())
    }
}
```

---

### 5. Email Notifications (SMTP)

**Priority:** Tier 1 | **Effort:** Low | **Value:** High

Send email notifications with HTML templates and attachments.

**Capabilities:**
- HTML email templates
- Plain text fallback
- Attachment support for reports
- Email digest mode (daily/weekly summary)
- CC/BCC support
- Priority flags (high/normal/low)

**Configuration:**
```toml
[email]
smtp_host = "smtp.gmail.com"
smtp_port = 587
smtp_user = "alerts@example.com"
smtp_password = "app-password"
from_address = "SvrCtlRS <alerts@example.com>"
```

**Template Support:**
- HTML templates with CSS styling
- Embedded images (logo, charts)
- Responsive design for mobile
- Dark mode support

---

### 6. Discord Webhooks

**Priority:** Tier 1 | **Effort:** Low | **Value:** Medium

Send rich embed notifications to Discord channels.

**Capabilities:**
- Rich embeds with colors and fields
- Role mentions for alerts (@ServerAdmin)
- Embed thumbnails and images
- Timestamp display
- Footer with source information

**Example Embed:**
```rust
struct DiscordEmbed {
    title: String,
    description: String,
    color: u32,  // RGB color code
    fields: Vec<EmbedField>,
    thumbnail: Option<String>,
    timestamp: DateTime<Utc>,
}
```

**Color Coding:**
- 🔴 Red (0xFF0000): Critical alerts
- 🟡 Yellow (0xFFFF00): Warnings
- 🟢 Green (0x00FF00): Success
- 🔵 Blue (0x0000FF): Information

---

### 7. Microsoft Teams Integration

**Priority:** Tier 2 | **Effort:** Medium | **Value:** Medium

Send adaptive cards to Microsoft Teams channels.

**Capabilities:**
- Adaptive cards with rich formatting
- Actionable messages with buttons
- Fact sets for structured data
- Image support
- Deep linking to SvrCtlRS UI

**Adaptive Card Example:**
```json
{
  "type": "AdaptiveCard",
  "body": [
    {
      "type": "TextBlock",
      "text": "Docker Health Alert",
      "weight": "Bolder",
      "size": "Large"
    },
    {
      "type": "FactSet",
      "facts": [
        {"title": "Server", "value": "server1"},
        {"title": "Status", "value": "Unhealthy"}
      ]
    }
  ],
  "actions": [
    {
      "type": "Action.OpenUrl",
      "title": "View Details",
      "url": "https://svrctlrs.example.com/servers/1"
    }
  ]
}
```

---

### 8. PagerDuty Integration

**Priority:** Tier 2 | **Effort:** Medium | **Value:** High

Create incidents in PagerDuty for on-call escalation.

**Capabilities:**
- Incident creation with severity levels
- On-call rotation support
- Escalation policies
- Incident acknowledgment sync
- Resolution sync back to SvrCtlRS

**Integration Flow:**
1. Critical alert detected
2. Create PagerDuty incident
3. Notify on-call engineer
4. Engineer acknowledges in PagerDuty
5. Status syncs to SvrCtlRS
6. Engineer resolves issue
7. Incident auto-resolves

**Configuration:**
```toml
[pagerduty]
api_key = "your-api-key"
service_id = "PXXXXXX"
escalation_policy_id = "PXXXXXX"
```

---

## Implementation Roadmap

### Phase 1: Core Backends (v1.1.0)
**Duration:** 2 weeks

- Implement Slack integration
- Implement Email (SMTP) support
- Implement Discord webhooks
- Add backend configuration UI

### Phase 2: Intelligence (v1.2.0)
**Duration:** 2 weeks

- Implement smart alert aggregation
- Add quiet hours configuration
- Implement notification templates
- Add template editor in UI

### Phase 3: Advanced Features (v1.3.0)
**Duration:** 3 weeks

- Implement advanced action buttons
- Add Microsoft Teams support
- Add PagerDuty integration
- Implement multi-step workflows

---

## Dependencies

### Rust Crates
```toml
lettre = "0.11"  # Email sending
slack-morphism = "0.41"  # Slack API
serenity = "0.12"  # Discord (if using bot)
```

### Configuration
- SMTP server access for email
- Slack workspace and app credentials
- Discord webhook URLs
- Microsoft Teams webhook URLs
- PagerDuty API key

---

## Testing Strategy

### Unit Tests
- Template rendering
- Variable substitution
- Action button generation
- Aggregation logic

### Integration Tests
- Send test notifications to each backend
- Verify action button callbacks
- Test aggregation timing
- Test quiet hours suppression

### Manual Testing
- Send notifications to all backends
- Click action buttons and verify execution
- Test template customization
- Verify multi-language support

---

## Success Metrics

- Notification delivery rate > 99.9%
- Action button click-through rate > 20%
- Reduction in notification volume by 40% (via aggregation)
- User satisfaction score > 4.5/5

---

## References

- [Slack API Documentation](https://api.slack.com/)
- [Discord Webhooks](https://discord.com/developers/docs/resources/webhook)
- [Microsoft Teams Adaptive Cards](https://adaptivecards.io/)
- [PagerDuty API](https://developer.pagerduty.com/)
- [ntfy Action Buttons](https://docs.ntfy.sh/publish/#action-buttons)

---

**Last Updated:** 2025-11-25  
**Status:** Planning

