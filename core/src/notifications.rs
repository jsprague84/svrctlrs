//! Notification system with Gotify and ntfy.sh backends

use async_trait::async_trait;
use chrono::{DateTime, Local, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use tracing::{debug, warn};

use crate::{Error, Result};

// ============================================================================
// Notification Context (Template Variables)
// ============================================================================

/// All available template variables for notifications
///
/// This struct provides context for rendering notification templates with
/// variable substitution. Use `render_template()` to replace `{{variable}}`
/// placeholders with actual values.
#[derive(Debug, Clone)]
pub struct NotificationContext {
    // Job info
    pub job_name: String,
    pub job_display_name: String,
    pub job_type: String,

    // Server info
    pub server_name: String,
    pub server_hostname: String,

    // Execution info
    pub status: String,
    pub status_emoji: String, // For ntfy: checkmark or X
    pub exit_code: Option<i32>,
    pub duration_seconds: i64,
    pub duration_human: String, // "2m 34s"

    // Output
    pub output: Option<String>,
    pub output_snippet: Option<String>, // First N lines
    pub output_tail: Option<String>,    // Last N lines
    pub error: Option<String>,
    pub error_summary: Option<String>, // Extracted error lines

    // Multi-server (future)
    pub server_count: i64,
    pub success_count: i64,
    pub failure_count: i64,

    // Trigger info
    pub triggered_by: String, // "schedule", "manual", "webhook"
    pub schedule_name: Option<String>,
    pub run_id: i64,
    pub run_url: String, // Direct link to job run

    // Timestamps
    pub started_at: String,
    pub finished_at: String,
}

impl NotificationContext {
    /// Create a NotificationContext from job run data
    ///
    /// This is a simplified builder that accepts pre-resolved names
    /// (e.g., from `JobRunWithNames` query result).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: i64,
        job_template_name: &str,
        job_display_name: &str,
        job_type: &str,
        server_name: &str,
        server_hostname: Option<&str>,
        schedule_name: Option<&str>,
        status: &str,
        exit_code: Option<i32>,
        duration_ms: Option<i64>,
        output: Option<&str>,
        error: Option<&str>,
        started_at: DateTime<Utc>,
        finished_at: Option<DateTime<Utc>>,
        base_url: &str,
    ) -> Self {
        let duration = duration_ms.unwrap_or(0);
        let duration_human = format_duration(duration);

        let output_owned = output.map(|s| s.to_string());
        let output_snippet = output.map(|o| o.lines().take(10).collect::<Vec<_>>().join("\n"));
        let output_tail = output.map(|o| {
            let lines: Vec<_> = o.lines().collect();
            lines
                .iter()
                .rev()
                .take(10)
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        });

        let error_summary = error.map(|e| {
            // Extract lines containing "error", "fail", "exception"
            e.lines()
                .filter(|l| {
                    let lower = l.to_lowercase();
                    lower.contains("error") || lower.contains("fail") || lower.contains("exception")
                })
                .take(5)
                .collect::<Vec<_>>()
                .join("\n")
        });

        let status_emoji = match status {
            "success" => "white_check_mark",
            "failure" | "failed" => "x",
            "timeout" => "hourglass",
            "cancelled" => "stop_sign",
            "running" => "arrow_forward",
            "partial_success" => "warning",
            _ => "question",
        };

        Self {
            job_name: job_template_name.to_string(),
            job_display_name: job_display_name.to_string(),
            job_type: job_type.to_string(),
            server_name: server_name.to_string(),
            server_hostname: server_hostname.unwrap_or_default().to_string(),
            status: status.to_string(),
            status_emoji: status_emoji.to_string(),
            exit_code,
            duration_seconds: duration / 1000,
            duration_human,
            output: output_owned,
            output_snippet,
            output_tail,
            error: error.map(|s| s.to_string()),
            error_summary,
            server_count: 1,
            success_count: if status == "success" { 1 } else { 0 },
            failure_count: if status == "failure" || status == "failed" {
                1
            } else {
                0
            },
            triggered_by: if schedule_name.is_some() {
                "schedule"
            } else {
                "manual"
            }
            .to_string(),
            schedule_name: schedule_name.map(|s| s.to_string()),
            run_id,
            run_url: format!("{}/job-runs/{}", base_url.trim_end_matches('/'), run_id),
            started_at: started_at
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            finished_at: finished_at
                .map(|t| {
                    t.with_timezone(&Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_default(),
        }
    }

    /// Render a template string with this context
    ///
    /// Replaces `{{variable}}` placeholders with actual values.
    ///
    /// # Supported Variables
    ///
    /// - `{{job_name}}` - Job template name
    /// - `{{job_display_name}}` - Job display name
    /// - `{{job_type}}` - Job type name
    /// - `{{server_name}}` - Target server name
    /// - `{{server_hostname}}` - Target server hostname
    /// - `{{status}}` - Execution status (success, failure, etc.)
    /// - `{{status_emoji}}` - ntfy emoji tag for status
    /// - `{{exit_code}}` - Process exit code (if available)
    /// - `{{duration_seconds}}` - Duration in seconds
    /// - `{{duration_human}}` - Human-readable duration (e.g., "2m 34s")
    /// - `{{output_snippet}}` - First 10 lines of output
    /// - `{{output_tail}}` - Last 10 lines of output
    /// - `{{error_summary}}` - Lines containing error/fail/exception
    /// - `{{triggered_by}}` - "schedule", "manual", or "webhook"
    /// - `{{schedule_name}}` - Schedule name (if triggered by schedule)
    /// - `{{run_id}}` - Job run ID
    /// - `{{run_url}}` - Direct URL to job run details
    /// - `{{started_at}}` - Start timestamp
    /// - `{{finished_at}}` - End timestamp
    ///
    /// # Example
    ///
    /// ```ignore
    /// let template = "Job {{job_name}} on {{server_name}}: {{status}}";
    /// let rendered = context.render_template(template);
    /// // Result: "Job backup-job on server-01: success"
    /// ```
    pub fn render_template(&self, template: &str) -> String {
        let mut result = template.to_string();

        // Simple {{variable}} replacement
        let replacements = [
            ("{{job_name}}", &self.job_name),
            ("{{job_display_name}}", &self.job_display_name),
            ("{{job_type}}", &self.job_type),
            ("{{server_name}}", &self.server_name),
            ("{{server_hostname}}", &self.server_hostname),
            ("{{status}}", &self.status),
            ("{{status_emoji}}", &self.status_emoji),
            ("{{duration_human}}", &self.duration_human),
            ("{{triggered_by}}", &self.triggered_by),
            ("{{run_url}}", &self.run_url),
            ("{{started_at}}", &self.started_at),
            ("{{finished_at}}", &self.finished_at),
        ];

        for (placeholder, value) in replacements {
            result = result.replace(placeholder, value);
        }

        // Numeric fields
        result = result.replace("{{run_id}}", &self.run_id.to_string());
        result = result.replace("{{duration_seconds}}", &self.duration_seconds.to_string());
        result = result.replace("{{server_count}}", &self.server_count.to_string());
        result = result.replace("{{success_count}}", &self.success_count.to_string());
        result = result.replace("{{failure_count}}", &self.failure_count.to_string());

        // Optional fields
        if let Some(ref exit_code) = self.exit_code {
            result = result.replace("{{exit_code}}", &exit_code.to_string());
        } else {
            result = result.replace("{{exit_code}}", "-");
        }

        if let Some(ref output) = self.output_snippet {
            result = result.replace("{{output_snippet}}", output);
        } else {
            result = result.replace("{{output_snippet}}", "");
        }

        if let Some(ref output) = self.output_tail {
            result = result.replace("{{output_tail}}", output);
        } else {
            result = result.replace("{{output_tail}}", "");
        }

        if let Some(ref error) = self.error_summary {
            result = result.replace("{{error_summary}}", error);
        } else {
            result = result.replace("{{error_summary}}", "");
        }

        if let Some(ref schedule) = self.schedule_name {
            result = result.replace("{{schedule_name}}", schedule);
        } else {
            result = result.replace("{{schedule_name}}", "");
        }

        result
    }

    /// Get default success notification title template
    pub fn default_success_title() -> &'static str {
        "✅ {{job_display_name}} succeeded"
    }

    /// Get default success notification body template
    pub fn default_success_body() -> &'static str {
        "Job completed on {{server_name}} in {{duration_human}}\n\nRun: {{run_url}}"
    }

    /// Get default failure notification title template
    pub fn default_failure_title() -> &'static str {
        "❌ {{job_display_name}} failed"
    }

    /// Get default failure notification body template
    pub fn default_failure_body() -> &'static str {
        "Job failed on {{server_name}}\nExit code: {{exit_code}}\n\n{{error_summary}}\n\nRun: {{run_url}}"
    }
}

/// Format duration in human-readable form
fn format_duration(ms: i64) -> String {
    let seconds = ms / 1000;
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

/// Notification message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    /// Message title
    pub title: String,
    /// Message body
    pub body: String,
    /// Priority (1-5, default 3)
    #[serde(default = "default_priority")]
    pub priority: u8,
    /// Optional action buttons
    #[serde(default)]
    pub actions: Vec<NotificationAction>,
}

fn default_priority() -> u8 {
    3
}

/// Mask sensitive tokens/keys for safe logging
///
/// Masks all but the first and last 3 characters of tokens longer than 8 characters.
/// Tokens 8 characters or shorter are completely masked.
///
/// # Examples
///
/// ```
/// use svrctlrs_core::mask_token;
///
/// let token = "abc123def456ghi789";
/// let masked = mask_token(token);
/// assert_eq!(masked, "abc***789");
///
/// let short = "secret";
/// let masked_short = mask_token(short);
/// assert_eq!(masked_short, "***");
/// ```
pub fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}***{}", &token[..3], &token[token.len() - 3..])
    }
}

/// Notification action button
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAction {
    pub label: String,
    pub url: String,
    pub method: Option<String>,
}

impl NotificationAction {
    /// Create a view URL action button
    pub fn view(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
            method: None,
        }
    }

    /// Create an HTTP POST action button
    pub fn http_post(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
            method: Some("POST".to_string()),
        }
    }
}

/// Notification backend trait
#[async_trait]
pub trait NotificationBackend: Send + Sync {
    /// Send a notification
    async fn send(&self, message: &NotificationMessage) -> Result<()>;

    /// Backend name
    fn name(&self) -> &str;
}

// ============================================================================
// Gotify Backend
// ============================================================================

/// Gotify notification backend
#[derive(Debug, Clone)]
pub struct GotifyBackend {
    client: Client,
    base_url: String,
    /// Service-specific keys: service_name -> api_key
    keys: HashMap<String, String>,
    /// Global fallback key
    fallback_key: Option<String>,
    debug: bool,
}

impl GotifyBackend {
    /// Create a new Gotify backend from environment variables
    pub fn new(client: Client) -> Result<Self> {
        let base_url =
            env::var("GOTIFY_URL").unwrap_or_else(|_| "http://localhost:8080/message".to_string());

        let debug = env::var("GOTIFY_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        // Load fallback key
        let fallback_key = Self::load_key_from_env("GOTIFY_KEY");

        Ok(Self {
            client,
            base_url,
            keys: HashMap::new(),
            fallback_key,
            debug,
        })
    }

    /// Create a new Gotify backend with explicit URL and key
    pub fn with_url_and_key(
        client: Client,
        url: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            client,
            base_url: url.into(),
            keys: HashMap::new(),
            fallback_key: Some(key.into()),
            debug: false,
        })
    }

    /// Register a service-specific key
    pub fn register_service(&mut self, service: impl Into<String>, key: impl Into<String>) {
        self.keys.insert(service.into(), key.into());
    }

    /// Load service-specific keys from environment
    pub fn load_service_keys(&mut self, services: &[&str]) {
        for service in services {
            let var_name = format!("{}_GOTIFY_KEY", service.to_uppercase());
            if let Some(key) = Self::load_key_from_env(&var_name) {
                self.register_service(*service, key);
            }
        }
    }

    /// Load key from environment variable or file
    fn load_key_from_env(var_name: &str) -> Option<String> {
        // Try direct env var first
        if let Ok(key) = env::var(var_name) {
            let key = key.trim();
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }

        // Try file-based key (GOTIFY_KEY_FILE)
        if var_name == "GOTIFY_KEY" {
            if let Ok(path) = env::var("GOTIFY_KEY_FILE") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let key = content.trim();
                    if !key.is_empty() {
                        return Some(key.to_string());
                    }
                }
            }
        }

        None
    }

    /// Get key for a service (with fallback)
    fn get_key(&self, service: &str) -> Option<&str> {
        self.keys
            .get(service)
            .map(|s| s.as_str())
            .or(self.fallback_key.as_deref())
    }

    /// Mask token for debug output
    fn mask_token(token: &str) -> String {
        const MIN_LEN: usize = 8;
        const PREFIX_LEN: usize = 3;
        const SUFFIX_LEN: usize = 3;

        if token.len() > MIN_LEN {
            format!(
                "{}***{}",
                &token[..PREFIX_LEN],
                &token[token.len() - SUFFIX_LEN..]
            )
        } else {
            "***".to_string()
        }
    }

    /// Send notification for a specific service
    pub async fn send_for_service(
        &self,
        service: &str,
        message: &NotificationMessage,
    ) -> Result<()> {
        let key = match self.get_key(service) {
            Some(k) => k,
            None => {
                debug!(service = %service, "Gotify key not configured; skipping notification");
                return Ok(());
            }
        };

        if self.debug {
            debug!(
                service = %service,
                url = %self.base_url,
                key = %Self::mask_token(key),
                title_bytes = message.title.len(),
                body_bytes = message.body.len(),
                "Sending Gotify notification"
            );
        }

        // Gotify requires /message endpoint
        let url = format!("{}/message", self.base_url.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .header("X-Gotify-Key", key)
            .json(&serde_json::json!({
                "title": message.title,
                "message": message.body,
                "priority": message.priority
            }))
            .send()
            .await
            .map_err(|e| Error::HttpError(format!("Gotify request failed: {}", e)))?;

        response
            .error_for_status()
            .map_err(|e| Error::NotificationError(format!("Gotify error: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl NotificationBackend for GotifyBackend {
    async fn send(&self, message: &NotificationMessage) -> Result<()> {
        // Use fallback key if available
        self.send_for_service("default", message).await
    }

    fn name(&self) -> &str {
        "gotify"
    }
}

// ============================================================================
// ntfy Backend
// ============================================================================

/// ntfy.sh notification backend
#[derive(Debug, Clone)]
pub struct NtfyBackend {
    client: Client,
    base_url: String,
    /// Service-specific topics: service_name -> topic
    topics: HashMap<String, String>,
    /// Authentication method
    auth: Option<NtfyAuth>,
    debug: bool,
}

/// ntfy authentication methods
#[derive(Debug, Clone)]
pub enum NtfyAuth {
    /// Bearer token authentication
    Token(String),
    /// Basic authentication (username:password)
    Basic { username: String, password: String },
}

impl NtfyBackend {
    /// Create a new ntfy backend from environment variables
    pub fn new(client: Client) -> Result<Self> {
        let base_url = env::var("NTFY_URL").unwrap_or_else(|_| "https://ntfy.sh".to_string());

        // Try to load auth from environment
        let auth = if let Ok(token) = env::var("NTFY_TOKEN") {
            if !token.trim().is_empty() {
                Some(NtfyAuth::Token(token.trim().to_string()))
            } else {
                None
            }
        } else if let (Ok(username), Ok(password)) =
            (env::var("NTFY_USERNAME"), env::var("NTFY_PASSWORD"))
        {
            if !username.trim().is_empty() && !password.trim().is_empty() {
                Some(NtfyAuth::Basic {
                    username: username.trim().to_string(),
                    password: password.trim().to_string(),
                })
            } else {
                None
            }
        } else {
            None
        };

        let debug = env::var("NTFY_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Ok(Self {
            client,
            base_url,
            topics: HashMap::new(),
            auth,
            debug,
        })
    }

    /// Create a new ntfy backend with explicit URL and default topic
    pub fn with_url_and_topic(
        client: Client,
        url: impl Into<String>,
        topic: impl Into<String>,
    ) -> Result<Self> {
        let mut topics = HashMap::new();
        topics.insert("default".to_string(), topic.into());

        Ok(Self {
            client,
            base_url: url.into(),
            topics,
            auth: None,
            debug: false,
        })
    }

    /// Set authentication for this backend
    pub fn with_auth(mut self, auth: NtfyAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set token authentication
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.auth = Some(NtfyAuth::Token(token.into()));
        self
    }

    /// Set basic authentication
    pub fn with_basic_auth(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.auth = Some(NtfyAuth::Basic {
            username: username.into(),
            password: password.into(),
        });
        self
    }

    /// Register a service-specific topic
    pub fn register_service(&mut self, service: impl Into<String>, topic: impl Into<String>) {
        self.topics.insert(service.into(), topic.into());
    }

    /// Load service-specific topics from environment
    pub fn load_service_topics(&mut self, services: &[&str]) {
        for service in services {
            let var_name = format!("{}_NTFY_TOPIC", service.to_uppercase());
            if let Ok(topic) = env::var(&var_name) {
                let topic = topic.trim();
                if !topic.is_empty() {
                    self.register_service(*service, topic);
                }
            }
        }
    }

    /// Get topic for a service
    fn get_topic(&self, service: &str) -> Option<&str> {
        self.topics.get(service).map(|s| s.as_str())
    }

    /// Send notification for a specific service
    pub async fn send_for_service(
        &self,
        service: &str,
        message: &NotificationMessage,
    ) -> Result<()> {
        let topic = match self.get_topic(service) {
            Some(t) => t,
            None => {
                warn!(
                    service = %service,
                    available_topics = ?self.topics.keys().collect::<Vec<_>>(),
                    "ntfy topic not configured for service; skipping notification"
                );
                return Ok(());
            }
        };

        tracing::info!(
            service = %service,
            topic = %topic,
            url = %format!("{}/{}", self.base_url, topic),
            title = %message.title,
            priority = message.priority,
            actions = message.actions.len(),
            "Sending ntfy notification"
        );

        // Build JSON body with markdown support
        let mut json_body = serde_json::json!({
            "topic": topic,
            "title": message.title,
            "message": message.body,
            "priority": message.priority,
            "markdown": true,
        });

        // Add actions if provided
        if !message.actions.is_empty() {
            json_body["actions"] =
                serde_json::to_value(&message.actions).map_err(Error::SerializationError)?;
        }

        if self.debug {
            debug!(
                payload = %serde_json::to_string_pretty(&json_body)
                    .unwrap_or_else(|_| "error serializing".to_string()),
                "ntfy JSON payload"
            );
        }

        // Post to base URL (topic is in JSON body)
        let url = self.base_url.trim_end_matches('/');
        let mut request = self.client.post(url).json(&json_body);

        // Add authentication if configured
        if let Some(auth) = &self.auth {
            match auth {
                NtfyAuth::Token(token) => {
                    request = request.header("Authorization", format!("Bearer {}", token));
                    if self.debug {
                        debug!("Using Bearer token authentication");
                    }
                }
                NtfyAuth::Basic { username, password } => {
                    request = request.basic_auth(username, Some(password));
                    if self.debug {
                        debug!("Using Basic authentication with username: {}", username);
                    }
                }
            }
        }

        let response = request.send().await.map_err(|e| {
            tracing::error!(
                service = %service,
                topic = %topic,
                url = %url,
                error = %e,
                "ntfy HTTP request failed"
            );
            Error::HttpError(format!("ntfy request failed: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            tracing::error!(
                service = %service,
                topic = %topic,
                status = %status,
                error_body = %error_body,
                "ntfy returned error status"
            );
            return Err(Error::NotificationError(format!(
                "ntfy error: {} - {}",
                status, error_body
            )));
        }

        tracing::info!(
            service = %service,
            topic = %topic,
            "ntfy notification sent successfully"
        );

        Ok(())
    }
}

#[async_trait]
impl NotificationBackend for NtfyBackend {
    async fn send(&self, _message: &NotificationMessage) -> Result<()> {
        // ntfy requires a topic, so we can't send without one
        warn!("ntfy backend called without service context; skipping");
        Ok(())
    }

    fn name(&self) -> &str {
        "ntfy"
    }
}

// ============================================================================
// Notification Manager
// ============================================================================

/// Manages multiple notification backends
#[derive(Debug, Clone)]
pub struct NotificationManager {
    gotify: Option<GotifyBackend>,
    ntfy: Option<NtfyBackend>,
}

impl NotificationManager {
    /// Create a new notification manager with auto-detected backends from environment
    pub fn new(client: Client, services: &[&str]) -> Result<Self> {
        let mut gotify = None;
        let mut ntfy = None;

        // Try to initialize Gotify
        if let Ok(mut backend) = GotifyBackend::new(client.clone()) {
            backend.load_service_keys(services);
            gotify = Some(backend);
        }

        // Try to initialize ntfy
        if let Ok(mut backend) = NtfyBackend::new(client.clone()) {
            backend.load_service_topics(services);
            ntfy = Some(backend);
        }

        Ok(Self { gotify, ntfy })
    }

    /// Create a new notification manager from pre-configured backends
    pub fn from_backends(gotify: Option<GotifyBackend>, ntfy: Option<NtfyBackend>) -> Self {
        Self { gotify, ntfy }
    }

    /// Send notification via all configured backends for a service
    pub async fn send_for_service(
        &self,
        service: &str,
        message: &NotificationMessage,
    ) -> Result<()> {
        let mut errors = Vec::new();

        // Send to Gotify
        if let Some(backend) = &self.gotify {
            if let Err(e) = backend.send_for_service(service, message).await {
                warn!(service = %service, backend = "gotify", error = %e, "Notification failed");
                errors.push(format!("Gotify: {}", e));
            }
        }

        // Send to ntfy
        if let Some(backend) = &self.ntfy {
            if let Err(e) = backend.send_for_service(service, message).await {
                warn!(service = %service, backend = "ntfy", error = %e, "Notification failed");
                errors.push(format!("ntfy: {}", e));
            }
        }

        if !errors.is_empty() {
            return Err(Error::NotificationError(errors.join("; ")));
        }

        Ok(())
    }

    /// Get Gotify backend reference
    pub fn gotify(&self) -> Option<&GotifyBackend> {
        self.gotify.as_ref()
    }

    /// Get ntfy backend reference
    pub fn ntfy(&self) -> Option<&NtfyBackend> {
        self.ntfy.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_token_long() {
        let token = "abc123def456ghi789";
        let masked = mask_token(token);
        assert_eq!(masked, "abc***789");
    }

    #[test]
    fn test_mask_token_short() {
        let token = "secret";
        let masked = mask_token(token);
        assert_eq!(masked, "***");
    }

    #[test]
    fn test_mask_token_exactly_8() {
        let token = "12345678";
        let masked = mask_token(token);
        assert_eq!(masked, "***");
    }

    #[test]
    fn test_mask_token_exactly_9() {
        let token = "123456789";
        let masked = mask_token(token);
        assert_eq!(masked, "123***789");
    }

    #[test]
    fn test_mask_token_empty() {
        let token = "";
        let masked = mask_token(token);
        assert_eq!(masked, "***");
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(5000), "5s");
        assert_eq!(format_duration(45000), "45s");
        assert_eq!(format_duration(0), "0s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(60000), "1m 0s");
        assert_eq!(format_duration(90000), "1m 30s");
        assert_eq!(format_duration(154000), "2m 34s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3600000), "1h 0m");
        assert_eq!(format_duration(5400000), "1h 30m");
        assert_eq!(format_duration(7380000), "2h 3m");
    }

    #[test]
    fn test_notification_context_render_template() {
        let started_at = Utc::now();
        let finished_at = Some(started_at + chrono::Duration::seconds(154));

        let ctx = NotificationContext::new(
            42,
            "backup-job",
            "Daily Backup",
            "backup",
            "server-01",
            Some("192.168.1.10"),
            Some("nightly-backup"),
            "success",
            Some(0),
            Some(154000),
            Some("Backup completed successfully\nTotal files: 1234"),
            None,
            started_at,
            finished_at,
            "http://localhost:8080",
        );

        // Test basic substitution
        let rendered = ctx.render_template("Job {{job_name}} on {{server_name}}: {{status}}");
        assert_eq!(rendered, "Job backup-job on server-01: success");

        // Test multiple variables
        let rendered = ctx.render_template("{{job_display_name}} completed in {{duration_human}}");
        assert_eq!(rendered, "Daily Backup completed in 2m 34s");

        // Test run URL
        let rendered = ctx.render_template("Details: {{run_url}}");
        assert_eq!(rendered, "Details: http://localhost:8080/job-runs/42");
    }

    #[test]
    fn test_notification_context_status_emoji() {
        let started_at = Utc::now();

        let success_ctx = NotificationContext::new(
            1,
            "test",
            "Test",
            "test",
            "srv",
            None,
            None,
            "success",
            Some(0),
            None,
            None,
            None,
            started_at,
            None,
            "http://localhost",
        );
        assert_eq!(success_ctx.status_emoji, "white_check_mark");

        let failure_ctx = NotificationContext::new(
            1,
            "test",
            "Test",
            "test",
            "srv",
            None,
            None,
            "failure",
            Some(1),
            None,
            None,
            None,
            started_at,
            None,
            "http://localhost",
        );
        assert_eq!(failure_ctx.status_emoji, "x");
    }

    #[test]
    fn test_notification_context_error_summary() {
        let started_at = Utc::now();
        let error_output = "Starting backup...\nERROR: Connection failed\nRetrying...\nFailed to connect\nException in thread main";

        let ctx = NotificationContext::new(
            1,
            "test",
            "Test",
            "test",
            "srv",
            None,
            None,
            "failure",
            Some(1),
            None,
            None,
            Some(error_output),
            started_at,
            None,
            "http://localhost",
        );

        // Should extract lines containing error/fail/exception
        let summary = ctx.error_summary.as_ref().unwrap();
        assert!(summary.contains("ERROR: Connection failed"));
        assert!(summary.contains("Failed to connect"));
        assert!(summary.contains("Exception in thread main"));
        assert!(!summary.contains("Starting backup"));
        assert!(!summary.contains("Retrying"));
    }

    #[test]
    fn test_notification_context_default_templates() {
        assert!(NotificationContext::default_success_title().contains("{{job_display_name}}"));
        assert!(NotificationContext::default_failure_title().contains("{{job_display_name}}"));
        assert!(NotificationContext::default_success_body().contains("{{server_name}}"));
        assert!(NotificationContext::default_failure_body().contains("{{error_summary}}"));
    }
}
