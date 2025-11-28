use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Notification channel type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Gotify,
    Ntfy,
    Email,
    Slack,
    Discord,
    Webhook,
}

impl ChannelType {
    /// Parse channel type from string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "gotify" => Some(Self::Gotify),
            "ntfy" => Some(Self::Ntfy),
            "email" => Some(Self::Email),
            "slack" => Some(Self::Slack),
            "discord" => Some(Self::Discord),
            "webhook" => Some(Self::Webhook),
            _ => None,
        }
    }

    /// Convert channel type to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gotify => "gotify",
            Self::Ntfy => "ntfy",
            Self::Email => "email",
            Self::Slack => "slack",
            Self::Discord => "discord",
            Self::Webhook => "webhook",
        }
    }
}

/// NotificationChannel model - replaces notification_backends with more flexibility
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationChannel {
    pub id: i64,
    pub name: String,
    #[sqlx(rename = "channel_type")]
    pub channel_type_str: String,
    pub description: Option<String>,

    /// Configuration as JSON string (type-specific)
    /// Examples:
    /// - Gotify: {"url": "...", "token": "..."}
    /// - ntfy: {"url": "...", "topic": "...", "token": "..."}
    /// - email: {"smtp_host": "...", "from": "...", "to": "..."}
    pub config: String,

    // Settings
    pub enabled: bool,
    pub default_priority: i32,

    // Testing/validation
    pub last_test_at: Option<DateTime<Utc>>,
    pub last_test_success: Option<bool>,
    pub last_test_error: Option<String>,

    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NotificationChannel {
    /// Get channel type as enum
    pub fn channel_type(&self) -> Option<ChannelType> {
        ChannelType::from_str(&self.channel_type_str)
    }

    /// Get config as JSON value
    pub fn get_config(&self) -> JsonValue {
        serde_json::from_str(&self.config).unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Check if the channel has been tested successfully
    pub fn is_tested(&self) -> bool {
        self.last_test_at.is_some() && self.last_test_success == Some(true)
    }

    /// Check if the channel is ready to use
    pub fn is_ready(&self) -> bool {
        self.enabled && (self.is_tested() || self.last_test_at.is_none())
    }
}

/// NotificationPolicy model - defines when/how to send notifications
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationPolicy {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,

    // Trigger conditions
    pub on_success: bool,
    pub on_failure: bool,
    pub on_timeout: bool,

    // Filtering (JSON arrays)
    /// JSON array of job type names (e.g., ["docker", "os"])
    pub job_type_filter: Option<String>,
    /// JSON array of server IDs (e.g., [1, 2, 3])
    pub server_filter: Option<String>,
    /// JSON array of tag names (e.g., ["prod", "critical"])
    pub tag_filter: Option<String>,

    // Throttling
    /// Only notify if severity >= this (1-5)
    pub min_severity: i32,
    /// Rate limiting (max notifications per hour)
    pub max_per_hour: Option<i32>,

    // Message customization
    pub title_template: Option<String>,
    pub body_template: Option<String>,

    pub enabled: bool,
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NotificationPolicy {
    /// Get job type filter as a vector
    pub fn get_job_type_filter(&self) -> Vec<String> {
        self.job_type_filter
            .as_ref()
            .and_then(|f| serde_json::from_str(f).ok())
            .unwrap_or_default()
    }

    /// Get server filter as a vector of IDs
    pub fn get_server_filter(&self) -> Vec<i64> {
        self.server_filter
            .as_ref()
            .and_then(|f| serde_json::from_str(f).ok())
            .unwrap_or_default()
    }

    /// Get tag filter as a vector
    pub fn get_tag_filter(&self) -> Vec<String> {
        self.tag_filter
            .as_ref()
            .and_then(|f| serde_json::from_str(f).ok())
            .unwrap_or_default()
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Check if policy should trigger for a job type
    pub fn matches_job_type(&self, job_type: &str) -> bool {
        let filter = self.get_job_type_filter();
        filter.is_empty() || filter.contains(&job_type.to_string())
    }

    /// Check if policy should trigger for a server
    pub fn matches_server(&self, server_id: i64) -> bool {
        let filter = self.get_server_filter();
        filter.is_empty() || filter.contains(&server_id)
    }

    /// Check if policy should trigger for tags
    pub fn matches_tags(&self, tags: &[String]) -> bool {
        let filter = self.get_tag_filter();
        if filter.is_empty() {
            return true;
        }
        tags.iter().any(|tag| filter.contains(tag))
    }
}

/// NotificationPolicyChannel model - links policies to channels (many-to-many)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationPolicyChannel {
    pub policy_id: i64,
    pub channel_id: i64,
    /// Override channel default priority
    pub priority_override: Option<i32>,
}

/// NotificationLog model - audit trail of sent notifications
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationLog {
    pub id: i64,
    pub channel_id: i64,
    pub policy_id: Option<i64>,
    /// Which job triggered this notification
    pub job_run_id: Option<i64>,

    // Message details
    pub title: String,
    pub body: Option<String>,
    pub priority: i32,

    // Delivery status
    pub success: bool,
    pub error_message: Option<String>,
    pub retry_count: i32,

    pub sent_at: DateTime<Utc>,
}

impl NotificationLog {
    /// Check if notification was sent successfully
    pub fn is_successful(&self) -> bool {
        self.success
    }

    /// Check if notification failed
    pub fn is_failed(&self) -> bool {
        !self.success
    }

    /// Check if notification was retried
    pub fn was_retried(&self) -> bool {
        self.retry_count > 0
    }
}

/// Input for creating a notification channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNotificationChannel {
    pub name: String,
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    pub description: Option<String>,
    pub config: JsonValue,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_priority")]
    pub default_priority: i32,
    pub metadata: Option<JsonValue>,
}

impl CreateNotificationChannel {
    /// Convert config to JSON string
    pub fn config_string(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_else(|_| "{}".to_string())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }
}

/// Input for updating a notification channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNotificationChannel {
    pub name: Option<String>,
    pub description: Option<String>,
    pub config: Option<JsonValue>,
    pub enabled: Option<bool>,
    pub default_priority: Option<i32>,
    pub metadata: Option<JsonValue>,
}

impl UpdateNotificationChannel {
    /// Convert config to JSON string
    pub fn config_string(&self) -> Option<String> {
        self.config
            .as_ref()
            .and_then(|c| serde_json::to_string(c).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some()
            || self.description.is_some()
            || self.config.is_some()
            || self.enabled.is_some()
            || self.default_priority.is_some()
            || self.metadata.is_some()
    }
}

/// Input for creating a notification policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNotificationPolicy {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub on_success: bool,
    #[serde(default = "default_on_failure")]
    pub on_failure: bool,
    #[serde(default = "default_on_timeout")]
    pub on_timeout: bool,
    pub job_type_filter: Option<Vec<String>>,
    pub server_filter: Option<Vec<i64>>,
    pub tag_filter: Option<Vec<String>>,
    #[serde(default = "default_min_severity")]
    pub min_severity: i32,
    pub max_per_hour: Option<i32>,
    pub title_template: Option<String>,
    pub body_template: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub metadata: Option<JsonValue>,
}

impl CreateNotificationPolicy {
    /// Convert job_type_filter to JSON string
    pub fn job_type_filter_string(&self) -> Option<String> {
        self.job_type_filter
            .as_ref()
            .and_then(|f| serde_json::to_string(f).ok())
    }

    /// Convert server_filter to JSON string
    pub fn server_filter_string(&self) -> Option<String> {
        self.server_filter
            .as_ref()
            .and_then(|f| serde_json::to_string(f).ok())
    }

    /// Convert tag_filter to JSON string
    pub fn tag_filter_string(&self) -> Option<String> {
        self.tag_filter
            .as_ref()
            .and_then(|f| serde_json::to_string(f).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }
}

/// Input for updating a notification policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNotificationPolicy {
    pub name: Option<String>,
    pub description: Option<String>,
    pub on_success: Option<bool>,
    pub on_failure: Option<bool>,
    pub on_timeout: Option<bool>,
    pub job_type_filter: Option<Vec<String>>,
    pub server_filter: Option<Vec<i64>>,
    pub tag_filter: Option<Vec<String>>,
    pub min_severity: Option<i32>,
    pub max_per_hour: Option<i32>,
    pub title_template: Option<String>,
    pub body_template: Option<String>,
    pub enabled: Option<bool>,
    pub metadata: Option<JsonValue>,
}

impl UpdateNotificationPolicy {
    /// Convert job_type_filter to JSON string
    pub fn job_type_filter_string(&self) -> Option<String> {
        self.job_type_filter
            .as_ref()
            .and_then(|f| serde_json::to_string(f).ok())
    }

    /// Convert server_filter to JSON string
    pub fn server_filter_string(&self) -> Option<String> {
        self.server_filter
            .as_ref()
            .and_then(|f| serde_json::to_string(f).ok())
    }

    /// Convert tag_filter to JSON string
    pub fn tag_filter_string(&self) -> Option<String> {
        self.tag_filter
            .as_ref()
            .and_then(|f| serde_json::to_string(f).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some()
            || self.description.is_some()
            || self.on_success.is_some()
            || self.on_failure.is_some()
            || self.on_timeout.is_some()
            || self.job_type_filter.is_some()
            || self.server_filter.is_some()
            || self.tag_filter.is_some()
            || self.min_severity.is_some()
            || self.max_per_hour.is_some()
            || self.title_template.is_some()
            || self.body_template.is_some()
            || self.enabled.is_some()
            || self.metadata.is_some()
    }
}

fn default_priority() -> i32 {
    3
}

fn default_on_failure() -> bool {
    true
}

fn default_on_timeout() -> bool {
    true
}

fn default_min_severity() -> i32 {
    1
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_type_conversion() {
        assert_eq!(ChannelType::from_str("gotify"), Some(ChannelType::Gotify));
        assert_eq!(ChannelType::from_str("ntfy"), Some(ChannelType::Ntfy));
        assert_eq!(ChannelType::from_str("email"), Some(ChannelType::Email));
        assert_eq!(ChannelType::from_str("slack"), Some(ChannelType::Slack));
        assert_eq!(ChannelType::from_str("discord"), Some(ChannelType::Discord));
        assert_eq!(ChannelType::from_str("webhook"), Some(ChannelType::Webhook));
        assert_eq!(ChannelType::from_str("invalid"), None);

        assert_eq!(ChannelType::Gotify.as_str(), "gotify");
        assert_eq!(ChannelType::Ntfy.as_str(), "ntfy");
    }

    #[test]
    fn test_notification_channel_ready() {
        let mut channel = NotificationChannel {
            id: 1,
            name: "test".to_string(),
            channel_type_str: "gotify".to_string(),
            description: None,
            config: r#"{"url": "http://localhost", "token": "test"}"#.to_string(),
            enabled: true,
            default_priority: 3,
            last_test_at: None,
            last_test_success: None,
            last_test_error: None,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Ready because enabled and not tested yet (no failures)
        assert!(channel.is_ready());

        // Not ready if disabled
        channel.enabled = false;
        assert!(!channel.is_ready());

        // Ready again if enabled and tested successfully
        channel.enabled = true;
        channel.last_test_at = Some(Utc::now());
        channel.last_test_success = Some(true);
        assert!(channel.is_ready());
        assert!(channel.is_tested());
    }

    #[test]
    fn test_notification_policy_matching() {
        let policy = NotificationPolicy {
            id: 1,
            name: "test".to_string(),
            description: None,
            on_success: false,
            on_failure: true,
            on_timeout: true,
            job_type_filter: Some(r#"["docker", "os"]"#.to_string()),
            server_filter: Some(r#"[1, 2, 3]"#.to_string()),
            tag_filter: Some(r#"["prod", "critical"]"#.to_string()),
            min_severity: 1,
            max_per_hour: None,
            title_template: None,
            body_template: None,
            enabled: true,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(policy.matches_job_type("docker"));
        assert!(policy.matches_job_type("os"));
        assert!(!policy.matches_job_type("backup"));

        assert!(policy.matches_server(1));
        assert!(policy.matches_server(2));
        assert!(!policy.matches_server(99));

        assert!(policy.matches_tags(&["prod".to_string()]));
        assert!(policy.matches_tags(&["critical".to_string()]));
        assert!(policy.matches_tags(&["prod".to_string(), "staging".to_string()]));
        assert!(!policy.matches_tags(&["staging".to_string()]));
    }
}
