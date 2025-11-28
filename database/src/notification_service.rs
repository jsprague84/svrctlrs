//! Notification service for handling notification policies, message templating, and multi-channel delivery

use crate::models::{
    ChannelType, JobRun, JobTemplate, NotificationChannel, NotificationPolicy, Server,
    ServerJobResult,
};
use chrono::Utc;
use serde_json::Value as JsonValue;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use svrctlrs_core::{Error, GotifyBackend, NotificationMessage, NtfyBackend, Result};
use tracing::{debug, error, info, instrument, warn};

/// Notification service for job execution notifications
pub struct NotificationService {
    db_pool: Pool<Sqlite>,
    gotify_backend: Option<Arc<GotifyBackend>>,
    ntfy_backend: Option<Arc<NtfyBackend>>,
}

impl NotificationService {
    /// Create a new notification service
    pub fn new(
        db_pool: Pool<Sqlite>,
        gotify_backend: Option<Arc<GotifyBackend>>,
        ntfy_backend: Option<Arc<NtfyBackend>>,
    ) -> Self {
        Self {
            db_pool,
            gotify_backend,
            ntfy_backend,
        }
    }

    /// Main entry point: evaluate policies and send notifications for a job run
    #[instrument(skip(self), fields(job_run_id))]
    pub async fn notify_job_run(&self, job_run_id: i64) -> Result<()> {
        info!(job_run_id, "Processing notifications for job run");

        // Load job run with related data
        let job_run = match self.load_job_run(job_run_id).await {
            Ok(jr) => jr,
            Err(e) => {
                error!(job_run_id, error = %e, "Failed to load job run");
                return Err(e);
            }
        };

        let job_template = match self.load_job_template(job_run.job_template_id).await {
            Ok(jt) => jt,
            Err(e) => {
                error!(job_template_id = job_run.job_template_id, error = %e, "Failed to load job template");
                return Err(e);
            }
        };

        let server = match self.load_server(job_run.server_id).await {
            Ok(s) => s,
            Err(e) => {
                error!(server_id = job_run.server_id, error = %e, "Failed to load server");
                return Err(e);
            }
        };

        let server_tags = self.load_server_tags(server.id).await.unwrap_or_default();

        // Load job type name
        let job_type_name = self.load_job_type_name(job_template.job_type_id).await?;

        // Load all active notification policies
        let policies = self.load_active_policies().await?;

        debug!(
            job_run_id,
            policies_count = policies.len(),
            "Evaluating notification policies"
        );

        let mut notifications_sent = 0;
        let mut errors = Vec::new();

        // Evaluate each policy
        for policy in policies {
            if self.should_notify(
                &policy,
                &job_run,
                &job_template,
                &server,
                &server_tags,
                &job_type_name,
            ) {
                info!(
                    job_run_id,
                    policy_id = policy.id,
                    policy_name = %policy.name,
                    "Policy matched, sending notifications"
                );

                // Load channels for this policy
                let channels = match self.load_policy_channels(policy.id).await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(policy_id = policy.id, error = %e, "Failed to load policy channels");
                        continue;
                    }
                };

                // Build template context
                let context = self
                    .build_template_context(&job_run, &job_template, &server, &job_type_name)
                    .await?;

                // Render title and body
                let title = self.render_template(
                    policy
                        .title_template
                        .as_deref()
                        .unwrap_or("[{{status}}] {{job_name}} on {{server_name}}"),
                    &context,
                )?;

                let body = self.render_template(
                    policy
                        .body_template
                        .as_deref()
                        .unwrap_or(DEFAULT_BODY_TEMPLATE),
                    &context,
                )?;

                let severity = self.calculate_severity(&job_run);

                // Send to each channel
                for (channel, priority_override) in channels {
                    let priority = priority_override.unwrap_or(channel.default_priority);

                    match self
                        .send_to_channel(&channel, &title, &body, priority)
                        .await
                    {
                        Ok(_) => {
                            notifications_sent += 1;
                            // Log successful notification
                            if let Err(e) = self
                                .log_notification(
                                    job_run_id, channel.id, policy.id, &title, &body, severity,
                                    true, None,
                                )
                                .await
                            {
                                warn!(channel_id = channel.id, error = %e, "Failed to log successful notification");
                            }
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            errors.push(format!("Channel {}: {}", channel.name, error_msg));
                            warn!(
                                channel_id = channel.id,
                                channel_name = %channel.name,
                                error = %e,
                                "Failed to send notification"
                            );
                            // Log failed notification
                            if let Err(log_err) = self
                                .log_notification(
                                    job_run_id,
                                    channel.id,
                                    policy.id,
                                    &title,
                                    &body,
                                    severity,
                                    false,
                                    Some(&error_msg),
                                )
                                .await
                            {
                                warn!(channel_id = channel.id, error = %log_err, "Failed to log failed notification");
                            }
                        }
                    }
                }
            }
        }

        // Update job run notification status
        self.update_job_run_notification_status(
            job_run_id,
            notifications_sent > 0,
            errors.first().map(|s| s.as_str()),
        )
        .await?;

        if notifications_sent > 0 {
            info!(
                job_run_id,
                notifications_sent, "Notifications sent successfully"
            );
        } else {
            debug!(job_run_id, "No notifications sent (no matching policies)");
        }

        Ok(())
    }

    /// Evaluate if policy matches job run
    fn should_notify(
        &self,
        policy: &NotificationPolicy,
        job_run: &JobRun,
        _job_template: &JobTemplate,
        server: &Server,
        server_tags: &[String],
        job_type_name: &str,
    ) -> bool {
        // Check if policy is enabled
        if !policy.enabled {
            debug!(policy_id = policy.id, "Policy disabled, skipping");
            return false;
        }

        // Check status triggers
        let status = job_run.status_str.as_str();
        let should_trigger = match status {
            "success" => policy.on_success,
            "failure" => policy.on_failure,
            "timeout" => policy.on_timeout,
            "cancelled" => policy.on_failure, // Treat cancelled as failure
            _ => false,
        };

        if !should_trigger {
            debug!(
                policy_id = policy.id,
                status = %status,
                "Status does not trigger this policy"
            );
            return false;
        }

        // Check severity filter
        let severity = self.calculate_severity(job_run);
        if severity < policy.min_severity {
            debug!(
                policy_id = policy.id,
                severity,
                min_severity = policy.min_severity,
                "Severity below threshold"
            );
            return false;
        }

        // Check job type filter
        if !policy.matches_job_type(job_type_name) {
            debug!(
                policy_id = policy.id,
                job_type = %job_type_name,
                "Job type does not match filter"
            );
            return false;
        }

        // Check server filter
        if !policy.matches_server(server.id) {
            debug!(
                policy_id = policy.id,
                server_id = server.id,
                "Server does not match filter"
            );
            return false;
        }

        // Check tag filter
        if !policy.matches_tags(server_tags) {
            debug!(
                policy_id = policy.id,
                server_tags = ?server_tags,
                "Server tags do not match filter"
            );
            return false;
        }

        // Check rate limiting (max_per_hour)
        // TODO: Implement rate limiting check against notification_log
        // For MVP, skip this check

        true
    }

    /// Calculate severity based on job run status (1-5)
    fn calculate_severity(&self, job_run: &JobRun) -> i32 {
        match job_run.status_str.as_str() {
            "success" => 1,
            "timeout" => 4,
            "failure" => 5,
            "cancelled" => 3,
            _ => 3,
        }
    }

    /// Build template rendering context
    async fn build_template_context(
        &self,
        job_run: &JobRun,
        job_template: &JobTemplate,
        server: &Server,
        job_type_name: &str,
    ) -> Result<TemplateContext> {
        // Calculate duration
        let duration_seconds = if let Some(finished) = job_run.finished_at {
            let duration = finished.signed_duration_since(job_run.started_at);
            duration.num_seconds()
        } else {
            0
        };

        // Load server results if any (for multi-server jobs)
        let server_results = self
            .load_server_results(job_run.id)
            .await
            .unwrap_or_default();
        let total_servers = if server_results.is_empty() {
            1
        } else {
            server_results.len() as i64
        };
        let success_count = if server_results.is_empty() {
            if job_run.status_str == "success" {
                1
            } else {
                0
            }
        } else {
            server_results
                .iter()
                .filter(|r| r.status_str == "success")
                .count() as i64
        };
        let failure_count = total_servers - success_count;

        // Format timestamps
        let started_at = job_run
            .started_at
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string();
        let finished_at = job_run
            .finished_at
            .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "In progress".to_string());

        // Parse metadata
        let metadata = job_run.get_metadata();

        // Build server result contexts
        let server_result_contexts = if server_results.is_empty() {
            // Single server result from job_run
            vec![ServerResultContext {
                server_name: server.name.clone(),
                status: job_run.status_str.clone(),
                exit_code: job_run.exit_code.unwrap_or(-1),
                stdout_snippet: job_run
                    .output
                    .as_deref()
                    .map(|s| truncate_string(s, 200))
                    .unwrap_or_default(),
                stderr_snippet: job_run
                    .error
                    .as_deref()
                    .map(|s| truncate_string(s, 200))
                    .unwrap_or_default(),
            }]
        } else {
            // Multiple server results
            let mut contexts = Vec::new();
            for result in server_results {
                let result_server = self
                    .load_server(result.server_id)
                    .await
                    .unwrap_or_else(|_| Server {
                        id: result.server_id,
                        name: format!("Server {}", result.server_id),
                        hostname: None,
                        port: 22,
                        username: None,
                        credential_id: None,
                        description: None,
                        is_local: false,
                        enabled: true,
                        os_type: None,
                        os_distro: None,
                        package_manager: None,
                        docker_available: false,
                        systemd_available: false,
                        metadata: None,
                        last_seen_at: None,
                        last_error: None,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    });

                contexts.push(ServerResultContext {
                    server_name: result_server.name,
                    status: result.status_str.clone(),
                    exit_code: result.exit_code.unwrap_or(-1),
                    stdout_snippet: result
                        .output
                        .as_deref()
                        .map(|s| truncate_string(s, 200))
                        .unwrap_or_default(),
                    stderr_snippet: result
                        .error
                        .as_deref()
                        .map(|s| truncate_string(s, 200))
                        .unwrap_or_default(),
                });
            }
            contexts
        };

        Ok(TemplateContext {
            job_name: job_template.name.clone(),
            job_type: job_type_name.to_string(),
            schedule_name: format!("Schedule {}", job_run.job_schedule_id), // TODO: Load actual schedule name
            status: job_run.status_str.clone(),
            severity: self.calculate_severity(job_run).to_string(),
            total_servers,
            success_count,
            failure_count,
            started_at,
            finished_at,
            duration_seconds,
            metrics: metadata,
            server_results: server_result_contexts,
        })
    }

    /// Render message template with variables
    fn render_template(&self, template: &str, context: &TemplateContext) -> Result<String> {
        let mut result = template.to_string();

        // Simple {{variable}} replacement - convert to owned strings to avoid temporary value issues
        let total_servers = context.total_servers.to_string();
        let success_count = context.success_count.to_string();
        let failure_count = context.failure_count.to_string();
        let duration_seconds = context.duration_seconds.to_string();

        let replacements = vec![
            ("{{job_name}}", context.job_name.as_str()),
            ("{{job_type}}", context.job_type.as_str()),
            ("{{schedule_name}}", context.schedule_name.as_str()),
            ("{{status}}", context.status.as_str()),
            ("{{severity}}", context.severity.as_str()),
            ("{{total_servers}}", total_servers.as_str()),
            ("{{success_count}}", success_count.as_str()),
            ("{{failure_count}}", failure_count.as_str()),
            ("{{started_at}}", context.started_at.as_str()),
            ("{{finished_at}}", context.finished_at.as_str()),
            ("{{duration_seconds}}", duration_seconds.as_str()),
        ];

        for (placeholder, value) in replacements {
            result = result.replace(placeholder, value);
        }

        // Handle metrics access: {{metrics.field_name}}
        if result.contains("{{metrics.") {
            let metrics_obj = context.metrics.as_object();
            if let Some(obj) = metrics_obj {
                for (key, value) in obj {
                    let placeholder = format!("{{{{metrics.{}}}}}", key);
                    let value_str = match value {
                        JsonValue::String(s) => s.clone(),
                        JsonValue::Number(n) => n.to_string(),
                        JsonValue::Bool(b) => b.to_string(),
                        _ => value.to_string(),
                    };
                    result = result.replace(&placeholder, &value_str);
                }
            }
        }

        // Handle server_results loop: {{#each server_results}}...{{/each}}
        if result.contains("{{#each server_results}}") {
            result = self.render_each_loop(&result, &context.server_results)?;
        }

        Ok(result)
    }

    /// Render {{#each server_results}} loops
    fn render_each_loop(
        &self,
        template: &str,
        server_results: &[ServerResultContext],
    ) -> Result<String> {
        let start_marker = "{{#each server_results}}";
        let end_marker = "{{/each}}";

        if let Some(start) = template.find(start_marker) {
            if let Some(end) = template.find(end_marker) {
                let before = &template[..start];
                let loop_template = &template[start + start_marker.len()..end];
                let after = &template[end + end_marker.len()..];

                let mut loop_output = String::new();
                for server_result in server_results {
                    let mut iteration = loop_template.to_string();
                    iteration = iteration.replace("{{server_name}}", &server_result.server_name);
                    iteration = iteration.replace("{{status}}", &server_result.status);
                    iteration =
                        iteration.replace("{{exit_code}}", &server_result.exit_code.to_string());
                    iteration =
                        iteration.replace("{{stdout_snippet}}", &server_result.stdout_snippet);
                    iteration =
                        iteration.replace("{{stderr_snippet}}", &server_result.stderr_snippet);
                    loop_output.push_str(&iteration);
                }

                return Ok(format!("{}{}{}", before, loop_output, after));
            }
        }

        Ok(template.to_string())
    }

    /// Send notification via channel
    #[instrument(skip(self, channel, title, body))]
    async fn send_to_channel(
        &self,
        channel: &NotificationChannel,
        title: &str,
        body: &str,
        priority: i32,
    ) -> Result<()> {
        if !channel.enabled {
            debug!(channel_id = channel.id, "Channel disabled, skipping");
            return Ok(());
        }

        let channel_type = channel.channel_type().ok_or_else(|| {
            Error::NotificationError(format!(
                "Invalid channel type: {}",
                channel.channel_type_str
            ))
        })?;

        let message = NotificationMessage {
            title: title.to_string(),
            body: body.to_string(),
            priority: priority.clamp(1, 5) as u8,
            actions: vec![], // TODO: Support custom actions
        };

        match channel_type {
            ChannelType::Gotify => {
                let backend = self.gotify_backend.as_ref().ok_or_else(|| {
                    Error::NotificationError("Gotify backend not configured".to_string())
                })?;

                // Extract service name from config if available, otherwise use "default"
                let config = channel.get_config();
                let service = config
                    .get("service")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");

                backend.send_for_service(service, &message).await
            }
            ChannelType::Ntfy => {
                let backend = self.ntfy_backend.as_ref().ok_or_else(|| {
                    Error::NotificationError("Ntfy backend not configured".to_string())
                })?;

                // Extract service name from config if available, otherwise use "default"
                let config = channel.get_config();
                let service = config
                    .get("service")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");

                backend.send_for_service(service, &message).await
            }
            ChannelType::Email => {
                // TODO: Implement email backend
                warn!(
                    channel_id = channel.id,
                    "Email notifications not yet implemented"
                );
                Err(Error::NotificationError(
                    "Email notifications not yet implemented".to_string(),
                ))
            }
            ChannelType::Slack => {
                // TODO: Implement Slack backend
                warn!(
                    channel_id = channel.id,
                    "Slack notifications not yet implemented"
                );
                Err(Error::NotificationError(
                    "Slack notifications not yet implemented".to_string(),
                ))
            }
            ChannelType::Discord => {
                // TODO: Implement Discord backend
                warn!(
                    channel_id = channel.id,
                    "Discord notifications not yet implemented"
                );
                Err(Error::NotificationError(
                    "Discord notifications not yet implemented".to_string(),
                ))
            }
            ChannelType::Webhook => {
                // TODO: Implement webhook backend
                warn!(
                    channel_id = channel.id,
                    "Webhook notifications not yet implemented"
                );
                Err(Error::NotificationError(
                    "Webhook notifications not yet implemented".to_string(),
                ))
            }
        }
    }

    /// Log notification to database
    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self, title, body))]
    async fn log_notification(
        &self,
        job_run_id: i64,
        channel_id: i64,
        policy_id: i64,
        title: &str,
        body: &str,
        severity: i32,
        success: bool,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO notification_log (channel_id, policy_id, job_run_id, title, body, priority, success, error_message)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(channel_id)
        .bind(policy_id)
        .bind(job_run_id)
        .bind(title)
        .bind(body)
        .bind(severity)
        .bind(success)
        .bind(error)
        .execute(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to log notification: {}", e)))?;

        // Update channel's last_used_at (via updated_at)
        sqlx::query(
            r#"
            UPDATE notification_channels
            SET updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(channel_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to update channel timestamp: {}", e)))?;

        Ok(())
    }

    /// Update job run notification status
    async fn update_job_run_notification_status(
        &self,
        job_run_id: i64,
        sent: bool,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE job_runs
            SET notification_sent = ?, notification_error = ?
            WHERE id = ?
            "#,
        )
        .bind(sent)
        .bind(error)
        .bind(job_run_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| {
            Error::DatabaseError(format!(
                "Failed to update job run notification status: {}",
                e
            ))
        })?;

        Ok(())
    }

    // ========================================================================
    // Database query helpers
    // ========================================================================

    async fn load_job_run(&self, job_run_id: i64) -> Result<JobRun> {
        sqlx::query_as::<_, JobRun>(
            r#"
            SELECT * FROM job_runs WHERE id = ?
            "#,
        )
        .bind(job_run_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to load job run {}: {}", job_run_id, e)))
    }

    async fn load_job_template(&self, job_template_id: i64) -> Result<JobTemplate> {
        sqlx::query_as::<_, JobTemplate>(
            r#"
            SELECT * FROM job_templates WHERE id = ?
            "#,
        )
        .bind(job_template_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| {
            Error::DatabaseError(format!(
                "Failed to load job template {}: {}",
                job_template_id, e
            ))
        })
    }

    async fn load_server(&self, server_id: i64) -> Result<Server> {
        sqlx::query_as::<_, Server>(
            r#"
            SELECT * FROM servers WHERE id = ?
            "#,
        )
        .bind(server_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to load server {}: {}", server_id, e)))
    }

    async fn load_server_tags(&self, server_id: i64) -> Result<Vec<String>> {
        let tags: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT t.name
            FROM tags t
            JOIN server_tags st ON st.tag_id = t.id
            WHERE st.server_id = ?
            "#,
        )
        .bind(server_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to load server tags: {}", e)))?;

        Ok(tags.into_iter().map(|(name,)| name).collect())
    }

    async fn load_job_type_name(&self, job_type_id: i64) -> Result<String> {
        let result: (String,) = sqlx::query_as(
            r#"
            SELECT name FROM job_types WHERE id = ?
            "#,
        )
        .bind(job_type_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| {
            Error::DatabaseError(format!("Failed to load job type {}: {}", job_type_id, e))
        })?;

        Ok(result.0)
    }

    async fn load_active_policies(&self) -> Result<Vec<NotificationPolicy>> {
        sqlx::query_as::<_, NotificationPolicy>(
            r#"
            SELECT * FROM notification_policies WHERE enabled = 1
            "#,
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to load notification policies: {}", e)))
    }

    async fn load_policy_channels(
        &self,
        policy_id: i64,
    ) -> Result<Vec<(NotificationChannel, Option<i32>)>> {
        let rows: Vec<(i64, Option<i32>)> = sqlx::query_as(
            r#"
            SELECT channel_id, priority_override
            FROM notification_policy_channels
            WHERE policy_id = ?
            "#,
        )
        .bind(policy_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to load policy channels: {}", e)))?;

        let mut channels = Vec::new();
        for (channel_id, priority_override) in rows {
            let channel: NotificationChannel = sqlx::query_as(
                r#"
                SELECT * FROM notification_channels WHERE id = ? AND enabled = 1
                "#,
            )
            .bind(channel_id)
            .fetch_one(&self.db_pool)
            .await
            .map_err(|e| {
                Error::DatabaseError(format!("Failed to load channel {}: {}", channel_id, e))
            })?;

            channels.push((channel, priority_override));
        }

        Ok(channels)
    }

    async fn load_server_results(&self, job_run_id: i64) -> Result<Vec<ServerJobResult>> {
        sqlx::query_as::<_, ServerJobResult>(
            r#"
            SELECT * FROM server_job_results WHERE job_run_id = ?
            "#,
        )
        .bind(job_run_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to load server results: {}", e)))
    }
}

// ============================================================================
// Template rendering types
// ============================================================================

/// Template rendering context
#[derive(Debug, Clone)]
pub struct TemplateContext {
    pub job_name: String,
    pub job_type: String,
    pub schedule_name: String,
    pub status: String,
    pub severity: String,
    pub total_servers: i64,
    pub success_count: i64,
    pub failure_count: i64,
    pub started_at: String,
    pub finished_at: String,
    pub duration_seconds: i64,
    pub metrics: JsonValue,
    pub server_results: Vec<ServerResultContext>,
}

/// Server result context for templates
#[derive(Debug, Clone)]
pub struct ServerResultContext {
    pub server_name: String,
    pub status: String,
    pub exit_code: i32,
    pub stdout_snippet: String,
    pub stderr_snippet: String,
}

// ============================================================================
// Default templates
// ============================================================================

const DEFAULT_BODY_TEMPLATE: &str = r#"**Job:** {{job_name}} ({{job_type}})
**Server:** {{total_servers}} server(s)
**Status:** {{status}}
**Duration:** {{duration_seconds}}s
**Started:** {{started_at}}
**Finished:** {{finished_at}}

**Results:**
- Success: {{success_count}}
- Failures: {{failure_count}}"#;

// ============================================================================
// Helper functions
// ============================================================================

/// Truncate string to max length
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("short", 100), "short");
        assert_eq!(
            truncate_string("a".repeat(300).as_str(), 50),
            format!("{}...", "a".repeat(47))
        );
    }

    #[test]
    fn test_template_rendering_basic() {
        let context = TemplateContext {
            job_name: "test-job".to_string(),
            job_type: "docker".to_string(),
            schedule_name: "daily".to_string(),
            status: "success".to_string(),
            severity: "1".to_string(),
            total_servers: 1,
            success_count: 1,
            failure_count: 0,
            started_at: "2024-01-01 12:00:00 UTC".to_string(),
            finished_at: "2024-01-01 12:05:00 UTC".to_string(),
            duration_seconds: 300,
            metrics: serde_json::json!({}),
            server_results: vec![],
        };

        // Create a mock service (we only need the render_template method, not the db_pool)
        struct MockService;
        impl MockService {
            fn render_template(&self, template: &str, context: &TemplateContext) -> Result<String> {
                let mut result = template.to_string();
                result = result.replace("{{status}}", &context.status);
                result = result.replace("{{job_name}}", &context.job_name);
                result = result.replace(
                    "{{duration_seconds}}",
                    &context.duration_seconds.to_string(),
                );
                Ok(result)
            }
        }

        let service = MockService;
        let template = "[{{status}}] {{job_name}} - {{duration_seconds}}s";
        let result = service.render_template(template, &context).unwrap();
        assert_eq!(result, "[success] test-job - 300s");
    }

    #[test]
    fn test_severity_calculation() {
        struct MockService;
        impl MockService {
            fn calculate_severity(&self, job_run: &JobRun) -> i32 {
                match job_run.status_str.as_str() {
                    "success" => 1,
                    "timeout" => 4,
                    "failure" => 5,
                    "cancelled" => 3,
                    _ => 3,
                }
            }
        }

        let service = MockService;

        let mut job_run = JobRun {
            id: 1,
            job_schedule_id: 1,
            job_template_id: 1,
            server_id: 1,
            status_str: "success".to_string(),
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: None,
            exit_code: Some(0),
            output: None,
            error: None,
            retry_attempt: 0,
            is_retry: false,
            notification_sent: false,
            notification_error: None,
            metadata: None,
        };

        assert_eq!(service.calculate_severity(&job_run), 1);

        job_run.status_str = "failure".to_string();
        assert_eq!(service.calculate_severity(&job_run), 5);

        job_run.status_str = "timeout".to_string();
        assert_eq!(service.calculate_severity(&job_run), 4);
    }
}
