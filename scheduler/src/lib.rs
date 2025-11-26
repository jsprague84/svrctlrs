//! Task scheduler

use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use svrctlrs_core::{Error, Result};

/// Async task handler type
pub type AsyncTaskHandler = Arc<
    dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
        + Send
        + Sync,
>;

/// Scheduled task
pub struct Task {
    pub id: String,
    pub schedule: Schedule,
    pub handler: AsyncTaskHandler,
}

/// Task scheduler
pub struct Scheduler {
    tasks: Arc<RwLock<Vec<Task>>>,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a task to the scheduler
    pub async fn add_task(
        &self,
        id: impl Into<String>,
        cron_expr: &str,
        handler: AsyncTaskHandler,
    ) -> Result<()> {
        let schedule = Schedule::from_str(cron_expr)
            .map_err(|e| Error::SchedulerError(format!("Invalid cron expression: {}", e)))?;

        let task_id = id.into();
        let task = Task {
            id: task_id.clone(),
            schedule,
            handler,
        };

        let mut tasks = self.tasks.write().await;
        tasks.push(task);

        info!(id = %task_id, schedule = %cron_expr, "Scheduled task added");

        Ok(())
    }

    /// Remove a task from the scheduler
    pub async fn remove_task(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        let initial_len = tasks.len();
        tasks.retain(|t| t.id != id);
        let removed = tasks.len() < initial_len;

        if removed {
            info!(id = %id, "Scheduled task removed");
        }

        removed
    }

    /// Update a task's schedule
    pub async fn update_task(
        &self,
        id: &str,
        cron_expr: &str,
        handler: AsyncTaskHandler,
    ) -> Result<()> {
        // Remove old task
        self.remove_task(id).await;

        // Add new task with updated schedule
        self.add_task(id, cron_expr, handler).await?;

        info!(id = %id, schedule = %cron_expr, "Scheduled task updated");

        Ok(())
    }

    /// Clear all tasks
    pub async fn clear_all_tasks(&self) {
        let mut tasks = self.tasks.write().await;
        let count = tasks.len();
        tasks.clear();
        info!(count = %count, "Cleared all scheduled tasks");
    }

    /// Get count of scheduled tasks
    pub async fn task_count(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks.len()
    }

    /// Start the scheduler
    pub async fn start(&self) -> Result<()> {
        info!("Starting scheduler");

        let tasks = self.tasks.clone();

        tokio::spawn(async move {
            loop {
                let now = Utc::now();

                let tasks_read = tasks.read().await;
                for task in tasks_read.iter() {
                    // Check if task should run now
                    if let Some(next) = task.schedule.upcoming(Utc).next() {
                        let time_until = (next - now).num_seconds();

                        // Run if within 1 minute window
                        if (0..=60).contains(&time_until) {
                            debug!(task_id = %task.id, "Executing scheduled task");

                            let handler = task.handler.clone();
                            let task_id = task.id.clone();

                            // Spawn task execution in background
                            tokio::spawn(async move {
                                match handler().await {
                                    Ok(()) => {
                                        info!(task_id = %task_id, "Task completed successfully");
                                    }
                                    Err(e) => {
                                        error!(task_id = %task_id, error = %e, "Task execution failed");
                                    }
                                }
                            });
                        }
                    }
                }
                drop(tasks_read);

                // Check every minute
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        });

        Ok(())
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
