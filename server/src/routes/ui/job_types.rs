use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, put},
    Form, Router,
};
use serde::Deserialize;
use svrctlrs_database::{
    models::{CreateCommandTemplate, CreateJobType, UpdateCommandTemplate, UpdateJobType},
    queries::job_types as queries,
    sqlx,
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        CommandTemplateListTemplate, JobTypeDisplay, JobTypeFormTemplate, JobTypeListTemplate,
        JobTypeViewTemplate, JobTypesTemplate,
    },
};

/// Create job types router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/job-types", get(job_types_page).post(create_job_type))
        .route("/job-types/new", get(new_job_type_form))
        .route("/job-types/list", get(get_job_types_list))
        .route("/job-types/{id}/edit", get(edit_job_type_form))
        .route(
            "/job-types/{id}",
            get(view_job_type)
                .put(update_job_type)
                .delete(delete_job_type),
        )
        .route(
            "/job-types/{job_type_id}/command-templates",
            get(get_command_templates).post(create_command_template),
        )
        .route(
            "/job-types/{job_type_id}/command-templates/{id}",
            put(update_command_template).delete(delete_command_template),
        )
}

// ============================================================================
// Page Routes
// ============================================================================

/// Display the job types management page
#[instrument(skip(state))]
pub async fn job_types_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering job types page");

    let job_types_raw = queries::list_job_types(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to fetch job types");
        AppError::DatabaseError(e.to_string())
    })?;

    // Convert to display models and fetch counts
    let mut job_types = Vec::new();
    for jt in job_types_raw {
        let mut display: JobTypeDisplay = jt.clone().into();

        // Get command template count
        let cmd_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM command_templates WHERE job_type_id = ?",
        )
        .bind(jt.id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

        // Get job template count
        let job_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM job_templates WHERE job_type_id = ?",
        )
        .bind(jt.id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

        display.command_template_count = cmd_count;
        display.job_template_count = job_count;
        job_types.push(display);
    }

    let template = JobTypesTemplate {
        user: None, // TODO: Add authentication
        job_types,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job types template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// HTMX List Routes
// ============================================================================

/// Get the job types list (HTMX)
#[instrument(skip(state))]
pub async fn get_job_types_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Fetching job types list");

    let job_types = queries::list_job_types(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job types");
            AppError::DatabaseError(e.to_string())
        })?
        .into_iter()
        .map(Into::into)
        .collect();

    let template = JobTypeListTemplate { job_types };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job type list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Form Routes
// ============================================================================

/// Show the new job type form (HTMX)
#[instrument(skip(_state))]
pub async fn new_job_type_form(State(_state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering new job type form");

    let template = JobTypeFormTemplate {
        job_type: None,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job type form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show the edit job type form (HTMX)
#[instrument(skip(state))]
pub async fn edit_job_type_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id = id, "Rendering edit job type form");

    let job_type = queries::get_job_type(&state.pool, id).await.map_err(|e| {
        warn!(job_type_id = id, error = %e, "Job type not found");
        AppError::NotFound(format!("Job type {} not found", id))
    })?;

    let template = JobTypeFormTemplate {
        job_type: Some(job_type.into()),
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job type form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// View job type details (HTMX)
#[instrument(skip(state))]
pub async fn view_job_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id = id, "Viewing job type details");

    let job_type = queries::get_job_type(&state.pool, id).await.map_err(|e| {
        warn!(job_type_id = id, error = %e, "Job type not found");
        AppError::NotFound(format!("Job type {} not found", id))
    })?;

    // Get command templates for this job type
    let command_templates = queries::get_command_templates(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_type_id = id, error = %e, "Failed to fetch command templates");
            AppError::DatabaseError(e.to_string())
        })?
        .into_iter()
        .map(Into::into)
        .collect();

    let template = JobTypeViewTemplate {
        job_type: job_type.into(),
        command_templates,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job type view template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// CRUD Operations
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateJobTypeFormInput {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub requires_capabilities: Option<String>, // JSON array string
    pub metadata: Option<String>,              // JSON object string
    pub enabled: Option<String>,               // "on" or absent
}

impl CreateJobTypeFormInput {
    fn to_model(&self) -> Result<CreateJobType, AppError> {
        // Parse capabilities
        let requires_capabilities = if let Some(ref caps_str) = self.requires_capabilities {
            if !caps_str.trim().is_empty() {
                Some(serde_json::from_str(caps_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid capabilities JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse metadata
        let metadata = if let Some(ref meta_str) = self.metadata {
            if !meta_str.trim().is_empty() {
                Some(serde_json::from_str(meta_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid metadata JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(CreateJobType {
            name: self.name.clone(),
            display_name: self.display_name.clone(),
            description: self.description.clone().filter(|s| !s.trim().is_empty()),
            icon: self.icon.clone().filter(|s| !s.trim().is_empty()),
            color: self.color.clone().filter(|s| !s.trim().is_empty()),
            requires_capabilities,
            metadata,
            enabled: self.enabled.is_some(),
        })
    }
}

/// Create a new job type
#[instrument(skip(state))]
pub async fn create_job_type(
    State(state): State<AppState>,
    Form(input): Form<CreateJobTypeFormInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, "Creating job type");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Job type name is empty");
        let template = JobTypeFormTemplate {
            job_type: None,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    if input.display_name.trim().is_empty() {
        warn!("Job type display name is empty");
        let template = JobTypeFormTemplate {
            job_type: None,
            error: Some("Display name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Convert to model
    let create_input = input.to_model()?;

    // Create job type
    let job_type_id = queries::create_job_type(&state.pool, &create_input)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create job type");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(job_type_id, name = %create_input.name, "Job type created successfully");

    // Return updated list
    get_job_types_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateJobTypeFormInput {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub requires_capabilities: Option<String>, // JSON array string
    pub metadata: Option<String>,              // JSON object string
    pub enabled: Option<String>,               // "on" or absent
}

impl UpdateJobTypeFormInput {
    fn to_model(&self) -> Result<UpdateJobType, AppError> {
        // Parse capabilities
        let requires_capabilities = if let Some(ref caps_str) = self.requires_capabilities {
            if !caps_str.trim().is_empty() {
                Some(serde_json::from_str(caps_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid capabilities JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse metadata
        let metadata = if let Some(ref meta_str) = self.metadata {
            if !meta_str.trim().is_empty() {
                Some(serde_json::from_str(meta_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid metadata JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(UpdateJobType {
            display_name: self.display_name.clone().filter(|s| !s.trim().is_empty()),
            description: self.description.clone().filter(|s| !s.trim().is_empty()),
            icon: self.icon.clone().filter(|s| !s.trim().is_empty()),
            color: self.color.clone().filter(|s| !s.trim().is_empty()),
            requires_capabilities,
            metadata,
            enabled: Some(self.enabled.is_some()),
        })
    }
}

/// Update an existing job type
#[instrument(skip(state))]
pub async fn update_job_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateJobTypeFormInput>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id = id, "Updating job type");

    // Validate display name if provided
    if let Some(ref display_name) = input.display_name {
        if display_name.trim().is_empty() {
            warn!("Job type display name is empty");
            let job_type = queries::get_job_type(&state.pool, id)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            let template = JobTypeFormTemplate {
                job_type: Some(job_type.into()),
                error: Some("Display name cannot be empty".to_string()),
            };
            let html = template.render().map_err(|e| {
                error!(error = %e, "Failed to render error template");
                AppError::TemplateError(e.to_string())
            })?;
            return Ok(Html(html));
        }
    }

    // Convert to model
    let update_input = input.to_model()?;

    // Update job type
    queries::update_job_type(&state.pool, id, &update_input)
        .await
        .map_err(|e| {
            error!(job_type_id = id, error = %e, "Failed to update job type");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(job_type_id = id, "Job type updated successfully");

    // Return updated list
    get_job_types_list(State(state)).await
}

/// Delete a job type
#[instrument(skip(state))]
pub async fn delete_job_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id = id, "Deleting job type");

    // Check if job type exists
    let job_type = queries::get_job_type(&state.pool, id).await.map_err(|e| {
        warn!(job_type_id = id, error = %e, "Job type not found");
        AppError::NotFound(format!("Job type {} not found", id))
    })?;

    // Delete job type
    queries::delete_job_type(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_type_id = id, error = %e, "Failed to delete job type");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(job_type_id = id, name = %job_type.name, "Job type deleted successfully");

    // Return updated list
    get_job_types_list(State(state)).await
}

// ============================================================================
// Command Template Routes
// ============================================================================

/// Get command templates for a job type (HTMX)
#[instrument(skip(state))]
pub async fn get_command_templates(
    State(state): State<AppState>,
    Path(job_type_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id, "Fetching command templates");

    let templates = queries::get_command_templates(&state.pool, job_type_id)
        .await
        .map_err(|e| {
            error!(job_type_id, error = %e, "Failed to fetch command templates");
            AppError::DatabaseError(e.to_string())
        })?
        .into_iter()
        .map(Into::into)
        .collect();

    let template = CommandTemplateListTemplate {
        job_type_id,
        command_templates: templates,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render command template list");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct CreateCommandTemplateFormInput {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub command: String,
    pub required_capabilities: Option<String>, // JSON array string
    pub os_filter: Option<String>,             // JSON object string
    pub timeout_seconds: Option<i32>,
    pub working_directory: Option<String>,
    pub environment: Option<String>, // JSON object string
    pub output_format: Option<String>,
    pub parse_output: Option<String>,      // "on" or absent
    pub output_parser: Option<String>,     // JSON object string
    pub notify_on_success: Option<String>, // "on" or absent
    pub notify_on_failure: Option<String>, // "on" or absent
    pub metadata: Option<String>,          // JSON object string
}

impl CreateCommandTemplateFormInput {
    fn to_model(&self, job_type_id: i64) -> Result<CreateCommandTemplate, AppError> {
        // Parse required_capabilities
        let required_capabilities = if let Some(ref caps_str) = self.required_capabilities {
            if !caps_str.trim().is_empty() {
                Some(serde_json::from_str(caps_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid capabilities JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse os_filter
        let os_filter = if let Some(ref filter_str) = self.os_filter {
            if !filter_str.trim().is_empty() {
                Some(serde_json::from_str(filter_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid OS filter JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse environment
        let environment = if let Some(ref env_str) = self.environment {
            if !env_str.trim().is_empty() {
                Some(serde_json::from_str(env_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid environment JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse output_parser
        let output_parser = if let Some(ref parser_str) = self.output_parser {
            if !parser_str.trim().is_empty() {
                Some(serde_json::from_str(parser_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid output parser JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse metadata
        let metadata = if let Some(ref meta_str) = self.metadata {
            if !meta_str.trim().is_empty() {
                Some(serde_json::from_str(meta_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid metadata JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(CreateCommandTemplate {
            job_type_id,
            name: self.name.clone(),
            display_name: self.display_name.clone(),
            description: self.description.clone().filter(|s| !s.trim().is_empty()),
            command: self.command.clone(),
            required_capabilities,
            os_filter,
            timeout_seconds: self.timeout_seconds.unwrap_or(300),
            working_directory: self
                .working_directory
                .clone()
                .filter(|s| !s.trim().is_empty()),
            environment,
            output_format: self.output_format.clone().filter(|s| !s.trim().is_empty()),
            parse_output: self.parse_output.is_some(),
            output_parser,
            notify_on_success: self.notify_on_success.is_some(),
            notify_on_failure: self.notify_on_failure.is_some() || self.notify_on_failure.is_none(),
            parameter_schema: None, // TODO: Add parameter_schema form field
            metadata,
        })
    }
}

/// Add a command template to a job type
#[instrument(skip(state))]
pub async fn create_command_template(
    State(state): State<AppState>,
    Path(job_type_id): Path<i64>,
    Form(input): Form<CreateCommandTemplateFormInput>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id, name = %input.name, "Creating command template");

    // Validate input
    if input.name.trim().is_empty() || input.command.trim().is_empty() {
        warn!("Command template name or command is empty");
        return Err(AppError::ValidationError(
            "Name and command are required".to_string(),
        ));
    }

    if input.display_name.trim().is_empty() {
        warn!("Command template display name is empty");
        return Err(AppError::ValidationError(
            "Display name is required".to_string(),
        ));
    }

    // Convert to model
    let create_input = input.to_model(job_type_id)?;

    // Create command template
    queries::create_command_template(&state.pool, &create_input)
        .await
        .map_err(|e| {
            error!(job_type_id, error = %e, "Failed to create command template");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(job_type_id, "Command template created successfully");

    // Return updated list
    get_command_templates(State(state), Path(job_type_id)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommandTemplateFormInput {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command: Option<String>,
    pub required_capabilities: Option<String>, // JSON array string
    pub os_filter: Option<String>,             // JSON object string
    pub timeout_seconds: Option<i32>,
    pub working_directory: Option<String>,
    pub environment: Option<String>, // JSON object string
    pub output_format: Option<String>,
    pub parse_output: Option<String>,      // "on" or absent
    pub output_parser: Option<String>,     // JSON object string
    pub notify_on_success: Option<String>, // "on" or absent
    pub notify_on_failure: Option<String>, // "on" or absent
    pub metadata: Option<String>,          // JSON object string
}

impl UpdateCommandTemplateFormInput {
    fn to_model(&self) -> Result<UpdateCommandTemplate, AppError> {
        // Parse required_capabilities
        let required_capabilities = if let Some(ref caps_str) = self.required_capabilities {
            if !caps_str.trim().is_empty() {
                Some(serde_json::from_str(caps_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid capabilities JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse os_filter
        let os_filter = if let Some(ref filter_str) = self.os_filter {
            if !filter_str.trim().is_empty() {
                Some(serde_json::from_str(filter_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid OS filter JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse environment
        let environment = if let Some(ref env_str) = self.environment {
            if !env_str.trim().is_empty() {
                Some(serde_json::from_str(env_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid environment JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse output_parser
        let output_parser = if let Some(ref parser_str) = self.output_parser {
            if !parser_str.trim().is_empty() {
                Some(serde_json::from_str(parser_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid output parser JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse metadata
        let metadata = if let Some(ref meta_str) = self.metadata {
            if !meta_str.trim().is_empty() {
                Some(serde_json::from_str(meta_str).map_err(|e| {
                    AppError::ValidationError(format!("Invalid metadata JSON: {}", e))
                })?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(UpdateCommandTemplate {
            display_name: self.display_name.clone().filter(|s| !s.trim().is_empty()),
            description: self.description.clone().filter(|s| !s.trim().is_empty()),
            command: self.command.clone().filter(|s| !s.trim().is_empty()),
            required_capabilities,
            os_filter,
            timeout_seconds: self.timeout_seconds,
            working_directory: self
                .working_directory
                .clone()
                .filter(|s| !s.trim().is_empty()),
            environment,
            output_format: self.output_format.clone().filter(|s| !s.trim().is_empty()),
            parse_output: Some(self.parse_output.is_some()),
            output_parser,
            notify_on_success: Some(self.notify_on_success.is_some()),
            notify_on_failure: Some(self.notify_on_failure.is_some()),
            parameter_schema: None, // TODO: Add parameter_schema form field
            metadata,
        })
    }
}

/// Update a command template
#[instrument(skip(state))]
pub async fn update_command_template(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
    Form(input): Form<UpdateCommandTemplateFormInput>,
) -> Result<Html<String>, AppError> {
    info!(template_id, "Updating command template");

    // Get template to find job_type_id
    let template = queries::get_command_template(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to fetch command template");
            AppError::DatabaseError(e.to_string())
        })?;

    // Validate input
    if let Some(ref display_name) = input.display_name {
        if display_name.trim().is_empty() {
            warn!("Command template display name is empty");
            return Err(AppError::ValidationError(
                "Display name cannot be empty".to_string(),
            ));
        }
    }

    if let Some(ref command) = input.command {
        if command.trim().is_empty() {
            warn!("Command template command is empty");
            return Err(AppError::ValidationError(
                "Command cannot be empty".to_string(),
            ));
        }
    }

    // Convert to model
    let update_input = input.to_model()?;

    // Update command template
    queries::update_command_template(&state.pool, template_id, &update_input)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to update command template");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(template_id, "Command template updated successfully");

    // Return updated list
    get_command_templates(State(state), Path(template.job_type_id)).await
}

/// Delete a command template
#[instrument(skip(state))]
pub async fn delete_command_template(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(template_id, "Deleting command template");

    // Get template to find job_type_id
    let template = queries::get_command_template(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to fetch command template");
            AppError::DatabaseError(e.to_string())
        })?;

    let job_type_id = template.job_type_id;

    // Delete command template
    queries::delete_command_template(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to delete command template");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(template_id, "Command template deleted successfully");

    // Return updated list
    get_command_templates(State(state), Path(job_type_id)).await
}
