//! Task scheduler

use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use svrctlrs_core::{Error, Result};

/// Scheduled task
pub struct Task {
    pub id: String,
    pub schedule: Schedule,
    pub handler: Arc<dyn Fn() -> Result<()> + Send + Sync>,
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
    pub async fn add_task<F>(&self, id: impl Into<String>, cron_expr: &str, handler: F) -> Result<()>
    where
        F: Fn() -> Result<()> + Send + Sync + 'static,
    {
        let schedule = Schedule::from_str(cron_expr)
            .map_err(|e| Error::SchedulerError(format!("Invalid cron expression: {}", e)))?;

        let task = Task {
            id: id.into(),
            schedule,
            handler: Arc::new(handler),
        };

        let mut tasks = self.tasks.write().await;
        tasks.push(task);

        info!(id = %task.id, schedule = %cron_expr, "Scheduled task added");

        Ok(())
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
                        if time_until <= 60 && time_until >= 0 {
                            debug!(task_id = %task.id, "Executing scheduled task");

                            match (task.handler)() {
                                Ok(()) => {
                                    info!(task_id = %task.id, "Task completed successfully");
                                }
                                Err(e) => {
                                    error!(task_id = %task.id, error = %e, "Task execution failed");
                                }
                            }
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
