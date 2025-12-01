//! Tags management routes

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, put},
    Router,
};
use axum_extra::extract::Form;
use serde::Deserialize;
use svrctlrs_database::{
    models::tag::{CreateTag, Tag, TagWithCount, UpdateTag},
    queries,
};

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::User};

/// Create tags router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tags", get(tags_page).post(tag_create))
        .route("/tags/new", get(tag_form_new))
        .route("/tags/{id}/edit", get(tag_form_edit))
        .route("/tags/{id}", put(tag_update).delete(tag_delete))
}

// ============================================================================
// Template Structs
// ============================================================================

#[derive(Template)]
#[template(path = "pages/tags.html")]
pub struct TagsTemplate {
    pub user: Option<User>,
    pub tags: Vec<TagDisplay>,
}

#[derive(Template)]
#[template(path = "components/tag_list.html")]
pub struct TagListTemplate {
    pub tags: Vec<TagDisplay>,
}

#[derive(Template)]
#[template(path = "components/tag_form.html")]
pub struct TagFormTemplate {
    pub tag: Option<TagDisplay>,
    pub error: Option<String>,
}

/// Tag display model with server count
#[derive(Debug, Clone)]
pub struct TagDisplay {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    pub server_count: i64,
    #[allow(dead_code)]
    pub created_at: String,
}

impl From<TagWithCount> for TagDisplay {
    fn from(twc: TagWithCount) -> Self {
        let color = twc.tag.color_or_default();
        Self {
            id: twc.tag.id,
            name: twc.tag.name,
            color,
            description: twc.tag.description,
            server_count: twc.server_count,
            created_at: twc.tag.created_at.format("%Y-%m-%d %H:%M").to_string(),
        }
    }
}

impl From<Tag> for TagDisplay {
    fn from(tag: Tag) -> Self {
        Self {
            id: tag.id,
            name: tag.name.clone(),
            color: tag.color_or_default(),
            description: tag.description.clone(),
            server_count: 0,
            created_at: tag.created_at.format("%Y-%m-%d %H:%M").to_string(),
        }
    }
}

// ============================================================================
// Form Input Structs
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateTagInput {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTagInput {
    pub name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// Tags page handler
async fn tags_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Load tags with server counts from database
    let db = state.db().await;
    let db_tags = queries::tags::get_tags_with_counts(db.pool()).await?;
    let tags = db_tags.into_iter().map(TagDisplay::from).collect();

    let template = TagsTemplate { user, tags };
    Ok(Html(template.render()?))
}

/// New tag form
async fn tag_form_new() -> Result<Html<String>, AppError> {
    let template = TagFormTemplate {
        tag: None,
        error: None,
    };
    Ok(Html(template.render()?))
}

/// Edit tag form
async fn tag_form_edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    // Load tag from database
    let db = state.db().await;
    let db_tag = queries::tags::get_tag(db.pool(), id).await;

    let (tag, error) = match db_tag {
        Ok(t) => (Some(TagDisplay::from(t)), None),
        Err(e) => {
            tracing::warn!("Failed to load tag {}: {}", id, e);
            (None, Some(format!("Tag with ID {} not found", id)))
        }
    };

    let template = TagFormTemplate { tag, error };
    Ok(Html(template.render()?))
}

/// Create tag handler
async fn tag_create(
    State(state): State<AppState>,
    Form(input): Form<CreateTagInput>,
) -> Result<Html<String>, AppError> {
    // Validate
    if input.name.trim().is_empty() {
        let template = TagFormTemplate {
            tag: None,
            error: Some("Name is required".to_string()),
        };
        return Ok(Html(template.render()?));
    }

    // Validate color if provided
    if let Some(ref color) = input.color {
        if !color.is_empty() && !Tag::is_valid_color(color) {
            let template = TagFormTemplate {
                tag: None,
                error: Some(format!(
                    "Invalid color format: '{}'. Expected format: #RRGGBB (e.g., #5E81AC)",
                    color
                )),
            };
            return Ok(Html(template.render()?));
        }
    }

    // Create tag
    tracing::info!("Creating tag: {}", input.name);
    let db = state.db().await;

    let create_tag = CreateTag {
        name: input.name.clone(),
        color: input.color.clone().filter(|c| !c.trim().is_empty()),
        description: input.description.clone(),
    };

    // Try to create, handle duplicate name error
    match queries::tags::create_tag(db.pool(), &create_tag).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_tags = queries::tags::get_tags_with_counts(db.pool()).await?;
            let tags = db_tags.into_iter().map(TagDisplay::from).collect();
            let template = TagListTemplate { tags };
            let list_html = template.render()?;

            // Prepend success message
            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss">✓ Tag '{}' created successfully!</div>{}"#,
                input.name, list_html
            )))
        }
        Err(e) => {
            // Check if it's a duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("tags.name") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ A tag with the name '{}' already exists. Please use a different name.</div>"#,
                    input.name
                )))
            } else if error_msg.contains("Validation error") {
                // Validation error from database layer
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ {}</div>"#,
                    error_msg
                )))
            } else {
                // Other database error
                tracing::error!("Failed to create tag: {}", e);
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ Failed to create tag: {}</div>"#,
                    e
                )))
            }
        }
    }
}

/// Update tag handler
async fn tag_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateTagInput>,
) -> Result<Html<String>, AppError> {
    // Validate
    if let Some(ref name) = input.name {
        if name.trim().is_empty() {
            return Ok(Html(
                r#"<div class="alert alert-error alert-auto-dismiss">✗ Name cannot be empty</div>"#
                    .to_string(),
            ));
        }
    }

    // Validate color if provided
    if let Some(ref color) = input.color {
        if !color.is_empty() && !Tag::is_valid_color(color) {
            return Ok(Html(format!(
                r#"<div class="alert alert-error alert-auto-dismiss">✗ Invalid color format: '{}'. Expected format: #RRGGBB (e.g., #5E81AC)</div>"#,
                color
            )));
        }
    }

    // Update in database
    tracing::info!("Updating tag {}: {:?}", id, input);
    let db = state.db().await;

    // Get the tag name for the success message
    let tag_name = if let Some(ref name) = input.name {
        name.clone()
    } else {
        // If name wasn't changed, get it from database
        queries::tags::get_tag(db.pool(), id).await?.name
    };

    let update_tag = UpdateTag {
        name: input.name,
        color: input.color.filter(|c| !c.trim().is_empty()),
        description: input.description,
    };

    // Try to update, handle duplicate name error
    match queries::tags::update_tag(db.pool(), id, &update_tag).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_tags = queries::tags::get_tags_with_counts(db.pool()).await?;
            let tags = db_tags.into_iter().map(TagDisplay::from).collect();
            let template = TagListTemplate { tags };
            let list_html = template.render()?;

            // Prepend success message
            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss">✓ Tag '{}' updated successfully!</div>{}"#,
                tag_name, list_html
            )))
        }
        Err(e) => {
            // Check if it's a duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("tags.name") {
                Ok(Html(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ A tag with that name already exists. Please use a different name.</div>"#.to_string()
                ))
            } else if error_msg.contains("Validation error") {
                // Validation error from database layer
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ {}</div>"#,
                    error_msg
                )))
            } else {
                // Other database error
                tracing::error!("Failed to update tag: {}", e);
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ Failed to update tag: {}</div>"#,
                    e
                )))
            }
        }
    }
}

/// Delete tag handler
async fn tag_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    // Get tag name and server count before deleting
    let db = state.db().await;
    let tag_name = queries::tags::get_tag(db.pool(), id)
        .await
        .map(|t| t.name)
        .unwrap_or_else(|_| format!("Tag {}", id));

    // Delete from database (server_tags entries are automatically removed via CASCADE)
    tracing::info!("Deleting tag {}", id);
    match queries::tags::delete_tag(db.pool(), id).await {
        Ok(_) => {
            // Success
            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss">✓ Tag '{}' deleted successfully!</div>"#,
                tag_name
            )))
        }
        Err(e) => {
            // Error
            tracing::error!("Failed to delete tag: {}", e);
            Ok(Html(format!(
                r#"<div class="alert alert-error alert-auto-dismiss">✗ Failed to delete tag: {}</div>"#,
                e
            )))
        }
    }
}
