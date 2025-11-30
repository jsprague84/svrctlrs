//! Job scheduler for SvrCtlRS
//!
//! This module provides a database-driven scheduler that:
//! - Reads job schedules from the `job_schedules` table
//! - Calculates next run times from cron expressions
//! - Triggers job executions via the JobExecutor
//! - Tracks running jobs to prevent duplicates
//! - Supports graceful shutdown

use anyhow::Context;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use sqlx::{Pool, Sqlite};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error, info, instrument, warn};

use svrctlrs_core::{executor::JobExecutor, Error, Result};
use svrctlrs_database::models::{JobSchedule, JobTemplate};
use svrctlrs_database::NotificationService;

/// Default poll interval for checking schedules
const DEFAULT_POLL_INTERVAL_SECS: u64 = 60;

/// Job scheduler - database-driven task execution
pub struct Scheduler {
    /// Database connection pool
    db_pool: Pool<Sqlite>,

    /// Job executor for running jobs
    executor: Arc<JobExecutor>,

    /// Set of job_run_ids currently executing (prevents duplicates)
    running_jobs: Arc<Mutex<HashSet<i64>>>,

    /// Shutdown signal sender
    shutdown_tx: Option<broadcast::Sender<()>>,

    /// Polling interval
    poll_interval: Duration,

    /// Notification service for job notifications
    notification_service: Option<Arc<NotificationService>>,
}

impl Scheduler {
    /// Create a new scheduler
    ///
    /// # Arguments
    ///
    /// * `db_pool` - Database connection pool
    /// * `executor` - Job executor instance
    /// * `notification_service` - Optional notification service for job notifications
    pub fn new(
        db_pool: Pool<Sqlite>,
        executor: Arc<JobExecutor>,
        notification_service: Option<Arc<NotificationService>>,
    ) -> Self {
        Self {
            db_pool,
            executor,
            running_jobs: Arc::new(Mutex::new(HashSet::new())),
            shutdown_tx: None,
            poll_interval: Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS),
            notification_service,
        }
    }

    /// Set the poll interval
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Get the number of currently running tasks
    ///
    /// Returns the count of jobs currently being executed by the scheduler.
    pub async fn task_count(&self) -> usize {
        let jobs = self.running_jobs.lock().await;
        jobs.len()
    }

    /// Start the scheduler
    ///
    /// This spawns a background task that polls for due schedules and triggers job executions.
    #[instrument(skip(self))]
    pub async fn start(&mut self) -> Result<()> {
        info!(
            "Starting scheduler with poll interval {:?}",
            self.poll_interval
        );

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let db_pool = self.db_pool.clone();
        let executor = self.executor.clone();
        let running_jobs = self.running_jobs.clone();
        let poll_interval = self.poll_interval;
        let notification_service = self.notification_service.clone();

        tokio::spawn(async move {
            loop {
                // Check for shutdown signal with timeout
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Scheduler received shutdown signal");
                        break;
                    }
                    _ = tokio::time::sleep(poll_interval) => {
                        // Poll for due schedules
                        if let Err(e) = Self::poll_schedules_impl(
                            &db_pool,
                            &executor,
                            &running_jobs,
                            &notification_service,
                        ).await {
                            error!(error = %e, "Error polling schedules");
                        }
                    }
                }
            }

            info!("Scheduler stopped");
        });

        Ok(())
    }

    /// Stop the scheduler
    ///
    /// Sends shutdown signal and waits for running jobs to complete.
    #[instrument(skip(self))]
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping scheduler");

        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            // Send shutdown signal
            let _ = shutdown_tx.send(());

            // Wait for running jobs to complete (with timeout)
            let timeout = Duration::from_secs(30);
            let start = std::time::Instant::now();

            loop {
                let running_count = {
                    let jobs = self.running_jobs.lock().await;
                    jobs.len()
                };

                if running_count == 0 {
                    info!("All running jobs completed");
                    break;
                }

                if start.elapsed() > timeout {
                    warn!(
                        running_count,
                        "Shutdown timeout reached with {} jobs still running", running_count
                    );
                    break;
                }

                debug!(running_count, "Waiting for running jobs to complete");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        info!("Scheduler stopped");
        Ok(())
    }

    /// Poll for due schedules (internal implementation)
    #[instrument(skip(db_pool, executor, running_jobs, notification_service))]
    async fn poll_schedules_impl(
        db_pool: &Pool<Sqlite>,
        executor: &Arc<JobExecutor>,
        running_jobs: &Arc<Mutex<HashSet<i64>>>,
        notification_service: &Option<Arc<NotificationService>>,
    ) -> Result<()> {
        debug!("Polling for due schedules");

        // Get all enabled schedules that are due (next_run_at <= now)
        let now = Utc::now();
        let due_schedules = Self::get_due_schedules(db_pool, now).await?;

        if !due_schedules.is_empty() {
            info!(count = due_schedules.len(), "Found due schedules");
        }

        for schedule in due_schedules {
            // Load job template
            let template = match Self::get_job_template(db_pool, schedule.job_template_id).await {
                Ok(t) => t,
                Err(e) => {
                    error!(
                        schedule_id = schedule.id,
                        template_id = schedule.job_template_id,
                        error = %e,
                        "Failed to load job template for schedule"
                    );
                    continue;
                }
            };

            // Trigger job execution
            if let Err(e) = Self::trigger_job_execution(
                db_pool,
                executor,
                running_jobs,
                &schedule,
                &template,
                notification_service,
            )
            .await
            {
                error!(
                    schedule_id = schedule.id,
                    error = %e,
                    "Failed to trigger job execution"
                );
            }
        }

        Ok(())
    }

    /// Get schedules that are due to run
    async fn get_due_schedules(
        pool: &Pool<Sqlite>,
        now: DateTime<Utc>,
    ) -> Result<Vec<JobSchedule>> {
        sqlx::query_as::<_, JobSchedule>(
            r#"
            SELECT id, name, description, job_template_id, server_id, schedule, enabled,
                   timeout_seconds, retry_count, notify_on_success, notify_on_failure,
                   notification_policy_id, last_run_at, last_run_status, next_run_at,
                   success_count, failure_count, metadata, created_at, updated_at
            FROM job_schedules
            WHERE enabled = 1
              AND (next_run_at IS NULL OR next_run_at <= ?)
            ORDER BY next_run_at ASC
            "#,
        )
        .bind(now)
        .fetch_all(pool)
        .await
        .context("Failed to get due schedules")
        .map_err(|e| Error::DatabaseError(e.to_string()))
    }

    /// Get job template by ID
    async fn get_job_template(pool: &Pool<Sqlite>, id: i64) -> Result<JobTemplate> {
        sqlx::query_as::<_, JobTemplate>(
            r#"
            SELECT id, name, display_name, description, job_type_id, is_composite,
                   command_template_id, variables, timeout_seconds, retry_count,
                   retry_delay_seconds, notify_on_success, notify_on_failure,
                   notification_policy_id, metadata, created_at, updated_at
            FROM job_templates
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to get job template")
        .map_err(|e| Error::DatabaseError(e.to_string()))
    }

    /// Trigger a job execution
    #[instrument(skip(
        db_pool,
        executor,
        running_jobs,
        schedule,
        template,
        notification_service
    ))]
    async fn trigger_job_execution(
        db_pool: &Pool<Sqlite>,
        executor: &Arc<JobExecutor>,
        running_jobs: &Arc<Mutex<HashSet<i64>>>,
        schedule: &JobSchedule,
        template: &JobTemplate,
        notification_service: &Option<Arc<NotificationService>>,
    ) -> Result<()> {
        info!(
            schedule_id = schedule.id,
            schedule_name = %schedule.name,
            template_name = %template.name,
            server_id = schedule.server_id,
            "Triggering job execution"
        );

        // Create job_run record
        let job_run_id = Self::create_job_run(
            db_pool,
            schedule.id,
            schedule.job_template_id,
            schedule.server_id,
        )
        .await?;

        info!(schedule_id = schedule.id, job_run_id, "Created job run");

        // Add to running jobs set
        {
            let mut jobs = running_jobs.lock().await;
            jobs.insert(job_run_id);
        }

        // Calculate next run time
        let next_run = Self::calculate_next_run(&schedule.schedule, "UTC", Utc::now())?;

        // Update schedule with next_run_at and last_run_at
        Self::update_schedule_after_trigger(db_pool, schedule.id, Utc::now(), next_run).await?;

        // Spawn job execution in background
        let executor_clone = executor.clone();
        let running_jobs_clone = running_jobs.clone();
        let db_pool_clone = db_pool.clone();
        let schedule_id = schedule.id;
        let notification_service_clone = notification_service.clone();

        tokio::spawn(async move {
            // Execute the job
            let result = executor_clone.execute_job_run(job_run_id).await;

            // Remove from running jobs set
            {
                let mut jobs = running_jobs_clone.lock().await;
                jobs.remove(&job_run_id);
            }

            // Update schedule statistics based on result
            let success = result.is_ok();
            if let Err(e) =
                Self::update_schedule_statistics(&db_pool_clone, schedule_id, success).await
            {
                error!(
                    schedule_id,
                    error = %e,
                    "Failed to update schedule statistics"
                );
            }

            // Send notification if service is available
            if let Some(ref notif_service) = notification_service_clone {
                debug!(job_run_id, "Sending job completion notification");
                if let Err(e) = notif_service.notify_job_run(job_run_id).await {
                    warn!(
                        job_run_id,
                        error = %e,
                        "Failed to send job completion notification"
                    );
                }
            }

            match result {
                Ok(_) => {
                    info!(
                        schedule_id,
                        job_run_id, "Job execution completed successfully"
                    );
                }
                Err(e) => {
                    error!(schedule_id, job_run_id, error = %e, "Job execution failed");
                }
            }
        });

        Ok(())
    }

    /// Create a job run record
    async fn create_job_run(
        pool: &Pool<Sqlite>,
        job_schedule_id: i64,
        job_template_id: i64,
        server_id: i64,
    ) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO job_runs (
                job_schedule_id, job_template_id, server_id, status,
                started_at, retry_attempt, is_retry
            )
            VALUES (?, ?, ?, 'running', CURRENT_TIMESTAMP, 0, 0)
            "#,
        )
        .bind(job_schedule_id)
        .bind(job_template_id)
        .bind(server_id)
        .execute(pool)
        .await
        .context("Failed to create job run")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(result.last_insert_rowid())
    }

    /// Update schedule after triggering execution
    async fn update_schedule_after_trigger(
        pool: &Pool<Sqlite>,
        schedule_id: i64,
        last_run_at: DateTime<Utc>,
        next_run_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE job_schedules
            SET last_run_at = ?,
                next_run_at = ?,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(last_run_at)
        .bind(next_run_at)
        .bind(schedule_id)
        .execute(pool)
        .await
        .context("Failed to update schedule")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Update schedule statistics after job completion
    async fn update_schedule_statistics(
        pool: &Pool<Sqlite>,
        schedule_id: i64,
        success: bool,
    ) -> Result<()> {
        // Get the latest job run status
        let status_str: Option<String> = sqlx::query_scalar(
            r#"
            SELECT status
            FROM job_runs
            WHERE job_schedule_id = ?
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(schedule_id)
        .fetch_optional(pool)
        .await
        .context("Failed to get latest job run status")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

        // Update success/failure counts and last_run_status
        if success {
            sqlx::query(
                r#"
                UPDATE job_schedules
                SET success_count = success_count + 1,
                    last_run_status = ?,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?
                "#,
            )
            .bind(status_str)
            .bind(schedule_id)
            .execute(pool)
            .await
            .context("Failed to update schedule statistics (success)")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
        } else {
            sqlx::query(
                r#"
                UPDATE job_schedules
                SET failure_count = failure_count + 1,
                    last_run_status = ?,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?
                "#,
            )
            .bind(status_str)
            .bind(schedule_id)
            .execute(pool)
            .await
            .context("Failed to update schedule statistics (failure)")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }

    /// Calculate next run time from cron expression
    ///
    /// # Arguments
    ///
    /// * `cron_expr` - Cron expression (6 fields: sec min hour day month day_of_week)
    /// * `timezone` - Timezone string (e.g., "UTC", "America/New_York")
    /// * `after` - Calculate next run after this time
    fn calculate_next_run(
        cron_expr: &str,
        timezone: &str,
        after: DateTime<Utc>,
    ) -> Result<DateTime<Utc>> {
        // Parse cron expression
        let schedule = Schedule::from_str(cron_expr)
            .map_err(|e| Error::SchedulerError(format!("Invalid cron expression: {}", e)))?;

        // Parse timezone
        let tz: Tz = timezone
            .parse()
            .map_err(|e| Error::SchedulerError(format!("Invalid timezone: {}", e)))?;

        // Convert to timezone-aware datetime
        let after_tz = after.with_timezone(&tz);

        // Get next occurrence
        let next = schedule
            .upcoming(tz)
            .next()
            .ok_or_else(|| Error::SchedulerError("No upcoming scheduled time".to_string()))?;

        // Ensure it's actually after the given time
        if next <= after_tz {
            // Get the next one after that
            schedule
                .after(&after_tz)
                .next()
                .map(|dt| dt.with_timezone(&Utc))
                .ok_or_else(|| Error::SchedulerError("No next scheduled time".to_string()))
        } else {
            Ok(next.with_timezone(&Utc))
        }
    }

    /// Get count of currently running jobs
    pub async fn running_job_count(&self) -> usize {
        let jobs = self.running_jobs.lock().await;
        jobs.len()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        // This is primarily for testing - in production, use new()
        panic!("Scheduler requires a database pool and executor - use new() instead of default()");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_next_run() {
        // Test daily at midnight UTC
        let cron_expr = "0 0 0 * * *"; // 6-field cron (seconds, minutes, hours, day, month, day_of_week)
        let now = Utc::now();
        let next = Scheduler::calculate_next_run(cron_expr, "UTC", now).unwrap();

        assert!(next > now);
        println!("Now: {}", now);
        println!("Next run: {}", next);
    }

    #[test]
    fn test_calculate_next_run_with_seconds() {
        // Test every 5 minutes with seconds
        let cron_expr = "0 */5 * * * *"; // 6-field cron
        let now = Utc::now();
        let next = Scheduler::calculate_next_run(cron_expr, "UTC", now).unwrap();

        assert!(next > now);
        println!("Now: {}", now);
        println!("Next run: {}", next);
    }

    #[test]
    fn test_calculate_next_run_with_timezone() {
        // Test daily at midnight Eastern time
        let cron_expr = "0 0 0 * * *"; // 6-field cron
        let now = Utc::now();
        let next = Scheduler::calculate_next_run(cron_expr, "America/New_York", now).unwrap();

        assert!(next > now);
        println!("Now: {}", now);
        println!("Next run (Eastern): {}", next);
    }

    #[test]
    fn test_invalid_cron() {
        let cron_expr = "invalid cron";
        let now = Utc::now();
        let result = Scheduler::calculate_next_run(cron_expr, "UTC", now);

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_timezone() {
        let cron_expr = "0 0 * * *";
        let now = Utc::now();
        let result = Scheduler::calculate_next_run(cron_expr, "Invalid/Timezone", now);

        assert!(result.is_err());
    }
}
