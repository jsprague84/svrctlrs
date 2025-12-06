//! Job Catalog database queries for Basic Mode workflow
//!
//! Provides CRUD operations for the job catalog system including:
//! - Categories with item counts
//! - Catalog items with capability filtering
//! - User favorites management
//! - Wizard job creation

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{
    CreateJobCatalogCategory, CreateJobCatalogItem, JobCatalogCategory,
    JobCatalogCategoryWithCount, JobCatalogFavorite, JobCatalogItem,
};

// ============================================================================
// Category Queries
// ============================================================================

/// List all catalog categories
#[instrument(skip(pool))]
pub async fn list_catalog_categories(pool: &Pool<Sqlite>) -> Result<Vec<JobCatalogCategory>> {
    sqlx::query_as::<_, JobCatalogCategory>(
        r#"
        SELECT id, name, display_name, description, icon, color, sort_order, created_at
        FROM job_catalog_categories
        ORDER BY sort_order, name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list catalog categories")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// List all categories with item counts
#[instrument(skip(pool))]
pub async fn list_catalog_categories_with_counts(
    pool: &Pool<Sqlite>,
) -> Result<Vec<JobCatalogCategoryWithCount>> {
    sqlx::query_as::<_, JobCatalogCategoryWithCount>(
        r#"
        SELECT
            c.id, c.name, c.display_name, c.description, c.icon, c.color, c.sort_order, c.created_at,
            COALESCE(COUNT(j.id), 0) as item_count
        FROM job_catalog_categories c
        LEFT JOIN job_catalog j ON j.category = c.name AND j.enabled = 1
        GROUP BY c.id
        ORDER BY c.sort_order, c.name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list catalog categories with counts")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get a catalog category by ID
#[instrument(skip(pool))]
pub async fn get_catalog_category(pool: &Pool<Sqlite>, id: i64) -> Result<JobCatalogCategory> {
    sqlx::query_as::<_, JobCatalogCategory>(
        r#"
        SELECT id, name, display_name, description, icon, color, sort_order, created_at
        FROM job_catalog_categories
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get catalog category")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get a catalog category by name
#[instrument(skip(pool))]
pub async fn get_catalog_category_by_name(
    pool: &Pool<Sqlite>,
    name: &str,
) -> Result<JobCatalogCategory> {
    sqlx::query_as::<_, JobCatalogCategory>(
        r#"
        SELECT id, name, display_name, description, icon, color, sort_order, created_at
        FROM job_catalog_categories
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("Failed to get catalog category by name")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new catalog category
#[instrument(skip(pool, input))]
pub async fn create_catalog_category(
    pool: &Pool<Sqlite>,
    input: &CreateJobCatalogCategory,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO job_catalog_categories (name, display_name, description, icon, color, sort_order)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(&input.display_name)
    .bind(&input.description)
    .bind(input.icon.as_deref().unwrap_or("folder"))
    .bind(&input.color)
    .bind(input.sort_order.unwrap_or(0))
    .execute(pool)
    .await
    .context("Failed to create catalog category")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Delete a catalog category (only if empty)
#[instrument(skip(pool))]
pub async fn delete_catalog_category(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    // Check if category has items
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_catalog j
        JOIN job_catalog_categories c ON j.category = c.name
        WHERE c.id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to check if category has items")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    if count.0 > 0 {
        return Err(Error::DatabaseError(
            "Cannot delete category: it still contains catalog items".to_string(),
        ));
    }

    sqlx::query("DELETE FROM job_catalog_categories WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete catalog category")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Catalog Item Queries
// ============================================================================

/// List all enabled catalog items
#[instrument(skip(pool))]
pub async fn list_catalog_items(pool: &Pool<Sqlite>) -> Result<Vec<JobCatalogItem>> {
    sqlx::query_as::<_, JobCatalogItem>(
        r#"
        SELECT id, name, display_name, description, category, subcategory, icon, difficulty,
               command, parameters, required_capabilities, os_filter,
               default_timeout, default_retry_count, working_directory, environment,
               success_title_template, success_body_template, failure_title_template, failure_body_template,
               ntfy_success_tags, ntfy_failure_tags, tags, sort_order, is_system, enabled,
               created_at, updated_at
        FROM job_catalog
        WHERE enabled = 1
        ORDER BY sort_order, display_name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list catalog items")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// List catalog items by category
#[instrument(skip(pool))]
pub async fn list_catalog_items_by_category(
    pool: &Pool<Sqlite>,
    category: &str,
) -> Result<Vec<JobCatalogItem>> {
    sqlx::query_as::<_, JobCatalogItem>(
        r#"
        SELECT id, name, display_name, description, category, subcategory, icon, difficulty,
               command, parameters, required_capabilities, os_filter,
               default_timeout, default_retry_count, working_directory, environment,
               success_title_template, success_body_template, failure_title_template, failure_body_template,
               ntfy_success_tags, ntfy_failure_tags, tags, sort_order, is_system, enabled,
               created_at, updated_at
        FROM job_catalog
        WHERE category = ? AND enabled = 1
        ORDER BY sort_order, display_name
        "#,
    )
    .bind(category)
    .fetch_all(pool)
    .await
    .context("Failed to list catalog items by category")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// List catalog items by difficulty level
#[instrument(skip(pool))]
pub async fn list_catalog_items_by_difficulty(
    pool: &Pool<Sqlite>,
    difficulty: &str,
) -> Result<Vec<JobCatalogItem>> {
    sqlx::query_as::<_, JobCatalogItem>(
        r#"
        SELECT id, name, display_name, description, category, subcategory, icon, difficulty,
               command, parameters, required_capabilities, os_filter,
               default_timeout, default_retry_count, working_directory, environment,
               success_title_template, success_body_template, failure_title_template, failure_body_template,
               ntfy_success_tags, ntfy_failure_tags, tags, sort_order, is_system, enabled,
               created_at, updated_at
        FROM job_catalog
        WHERE difficulty = ? AND enabled = 1
        ORDER BY category, sort_order, display_name
        "#,
    )
    .bind(difficulty)
    .fetch_all(pool)
    .await
    .context("Failed to list catalog items by difficulty")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get a catalog item by ID
#[instrument(skip(pool))]
pub async fn get_catalog_item(pool: &Pool<Sqlite>, id: i64) -> Result<JobCatalogItem> {
    sqlx::query_as::<_, JobCatalogItem>(
        r#"
        SELECT id, name, display_name, description, category, subcategory, icon, difficulty,
               command, parameters, required_capabilities, os_filter,
               default_timeout, default_retry_count, working_directory, environment,
               success_title_template, success_body_template, failure_title_template, failure_body_template,
               ntfy_success_tags, ntfy_failure_tags, tags, sort_order, is_system, enabled,
               created_at, updated_at
        FROM job_catalog
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get catalog item")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get a catalog item by name
#[instrument(skip(pool))]
pub async fn get_catalog_item_by_name(pool: &Pool<Sqlite>, name: &str) -> Result<JobCatalogItem> {
    sqlx::query_as::<_, JobCatalogItem>(
        r#"
        SELECT id, name, display_name, description, category, subcategory, icon, difficulty,
               command, parameters, required_capabilities, os_filter,
               default_timeout, default_retry_count, working_directory, environment,
               success_title_template, success_body_template, failure_title_template, failure_body_template,
               ntfy_success_tags, ntfy_failure_tags, tags, sort_order, is_system, enabled,
               created_at, updated_at
        FROM job_catalog
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("Failed to get catalog item by name")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new catalog item
#[instrument(skip(pool, input))]
pub async fn create_catalog_item(pool: &Pool<Sqlite>, input: &CreateJobCatalogItem) -> Result<i64> {
    let parameters_json = input
        .parameters
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let required_capabilities_json = input
        .required_capabilities
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let os_filter_json = input
        .os_filter
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let environment_json = input
        .environment
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let ntfy_success_tags_json = input
        .ntfy_success_tags
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let ntfy_failure_tags_json = input
        .ntfy_failure_tags
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let tags_json = input
        .tags
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());

    let result = sqlx::query(
        r#"
        INSERT INTO job_catalog (
            name, display_name, description, category, subcategory, icon, difficulty,
            command, parameters, required_capabilities, os_filter,
            default_timeout, default_retry_count, working_directory, environment,
            success_title_template, success_body_template, failure_title_template, failure_body_template,
            ntfy_success_tags, ntfy_failure_tags, tags, sort_order, is_system
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
        "#,
    )
    .bind(&input.name)
    .bind(&input.display_name)
    .bind(&input.description)
    .bind(&input.category)
    .bind(&input.subcategory)
    .bind(input.icon.as_deref().unwrap_or("terminal"))
    .bind(input.difficulty.as_deref().unwrap_or("basic"))
    .bind(&input.command)
    .bind(parameters_json)
    .bind(required_capabilities_json)
    .bind(os_filter_json)
    .bind(input.default_timeout.unwrap_or(300))
    .bind(input.default_retry_count.unwrap_or(0))
    .bind(&input.working_directory)
    .bind(environment_json)
    .bind(&input.success_title_template)
    .bind(&input.success_body_template)
    .bind(&input.failure_title_template)
    .bind(&input.failure_body_template)
    .bind(ntfy_success_tags_json)
    .bind(ntfy_failure_tags_json)
    .bind(tags_json)
    .bind(input.sort_order.unwrap_or(0))
    .execute(pool)
    .await
    .context("Failed to create catalog item")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Delete a catalog item (user-created only, not system items)
#[instrument(skip(pool))]
pub async fn delete_catalog_item(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    // Check if it's a system item
    let item: (bool,) = sqlx::query_as(
        r#"
        SELECT is_system
        FROM job_catalog
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to check if catalog item is system item")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    if item.0 {
        return Err(Error::DatabaseError(
            "Cannot delete system catalog items".to_string(),
        ));
    }

    // Delete any favorites referencing this item
    sqlx::query("DELETE FROM job_catalog_favorites WHERE catalog_item_id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete catalog item favorites")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    sqlx::query("DELETE FROM job_catalog WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete catalog item")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Toggle enabled status of a catalog item
#[instrument(skip(pool))]
pub async fn toggle_catalog_item_enabled(pool: &Pool<Sqlite>, id: i64) -> Result<bool> {
    // Get current status
    let current: (bool,) = sqlx::query_as(
        r#"
        SELECT enabled
        FROM job_catalog
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get catalog item status")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let new_status = !current.0;

    sqlx::query(
        r#"
        UPDATE job_catalog
        SET enabled = ?, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(new_status)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to toggle catalog item status")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(new_status)
}

// ============================================================================
// Compatibility Filtering
// ============================================================================

/// List catalog items compatible with a server's capabilities
#[instrument(skip(pool))]
pub async fn list_compatible_catalog_items(
    pool: &Pool<Sqlite>,
    server_capabilities: &[String],
) -> Result<Vec<JobCatalogItem>> {
    // Get all enabled items
    let all_items = list_catalog_items(pool).await?;

    // Filter by capabilities - items with no requirements are always compatible
    let compatible: Vec<JobCatalogItem> = all_items
        .into_iter()
        .filter(|item| item.is_compatible_with(server_capabilities))
        .collect();

    Ok(compatible)
}

/// List catalog items compatible with a server by server ID
/// This joins with server_capabilities table
#[instrument(skip(pool))]
pub async fn list_compatible_catalog_items_for_server(
    pool: &Pool<Sqlite>,
    server_id: i64,
) -> Result<Vec<JobCatalogItem>> {
    // First, get the server's capabilities
    let capabilities: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT capability
        FROM server_capabilities
        WHERE server_id = ? AND available = 1
        "#,
    )
    .bind(server_id)
    .fetch_all(pool)
    .await
    .context("Failed to get server capabilities")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let cap_list: Vec<String> = capabilities.into_iter().map(|(c,)| c).collect();

    // Get compatible items
    list_compatible_catalog_items(pool, &cap_list).await
}

// ============================================================================
// Favorites Queries
// ============================================================================

/// List user's favorite catalog items
#[instrument(skip(pool))]
pub async fn list_catalog_favorites(pool: &Pool<Sqlite>) -> Result<Vec<JobCatalogItem>> {
    sqlx::query_as::<_, JobCatalogItem>(
        r#"
        SELECT j.id, j.name, j.display_name, j.description, j.category, j.subcategory, j.icon, j.difficulty,
               j.command, j.parameters, j.required_capabilities, j.os_filter,
               j.default_timeout, j.default_retry_count, j.working_directory, j.environment,
               j.success_title_template, j.success_body_template, j.failure_title_template, j.failure_body_template,
               j.ntfy_success_tags, j.ntfy_failure_tags, j.tags, j.sort_order, j.is_system, j.enabled,
               j.created_at, j.updated_at
        FROM job_catalog j
        JOIN job_catalog_favorites f ON f.catalog_item_id = j.id
        WHERE j.enabled = 1
        ORDER BY f.sort_order, j.display_name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list catalog favorites")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Check if a catalog item is a favorite
#[instrument(skip(pool))]
pub async fn is_catalog_favorite(pool: &Pool<Sqlite>, catalog_item_id: i64) -> Result<bool> {
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_catalog_favorites
        WHERE catalog_item_id = ?
        "#,
    )
    .bind(catalog_item_id)
    .fetch_one(pool)
    .await
    .context("Failed to check if catalog item is favorite")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count.0 > 0)
}

/// Add a catalog item to favorites
#[instrument(skip(pool))]
pub async fn add_catalog_favorite(pool: &Pool<Sqlite>, catalog_item_id: i64) -> Result<i64> {
    // Get next sort order
    let max_order: Option<(i64,)> =
        sqlx::query_as("SELECT MAX(sort_order) FROM job_catalog_favorites")
            .fetch_optional(pool)
            .await
            .context("Failed to get max sort order")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let sort_order = max_order.map(|(o,)| o + 1).unwrap_or(0);

    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO job_catalog_favorites (catalog_item_id, sort_order)
        VALUES (?, ?)
        "#,
    )
    .bind(catalog_item_id)
    .bind(sort_order)
    .execute(pool)
    .await
    .context("Failed to add catalog favorite")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Remove a catalog item from favorites
#[instrument(skip(pool))]
pub async fn remove_catalog_favorite(pool: &Pool<Sqlite>, catalog_item_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM job_catalog_favorites WHERE catalog_item_id = ?")
        .bind(catalog_item_id)
        .execute(pool)
        .await
        .context("Failed to remove catalog favorite")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Toggle favorite status of a catalog item
#[instrument(skip(pool))]
pub async fn toggle_catalog_favorite(pool: &Pool<Sqlite>, catalog_item_id: i64) -> Result<bool> {
    let is_favorite = is_catalog_favorite(pool, catalog_item_id).await?;

    if is_favorite {
        remove_catalog_favorite(pool, catalog_item_id).await?;
        Ok(false)
    } else {
        add_catalog_favorite(pool, catalog_item_id).await?;
        Ok(true)
    }
}

/// Get a single favorite entry
#[instrument(skip(pool))]
pub async fn get_catalog_favorite(
    pool: &Pool<Sqlite>,
    catalog_item_id: i64,
) -> Result<JobCatalogFavorite> {
    sqlx::query_as::<_, JobCatalogFavorite>(
        r#"
        SELECT id, catalog_item_id, sort_order, created_at
        FROM job_catalog_favorites
        WHERE catalog_item_id = ?
        "#,
    )
    .bind(catalog_item_id)
    .fetch_one(pool)
    .await
    .context("Failed to get catalog favorite")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Reorder favorites
#[instrument(skip(pool, order))]
pub async fn reorder_catalog_favorites(pool: &Pool<Sqlite>, order: &[i64]) -> Result<()> {
    for (idx, item_id) in order.iter().enumerate() {
        sqlx::query(
            r#"
            UPDATE job_catalog_favorites
            SET sort_order = ?
            WHERE catalog_item_id = ?
            "#,
        )
        .bind(idx as i64)
        .bind(item_id)
        .execute(pool)
        .await
        .context("Failed to update favorite order")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;
    }

    Ok(())
}

// ============================================================================
// Search Queries
// ============================================================================

/// Search catalog items by name or description
#[instrument(skip(pool))]
pub async fn search_catalog_items(pool: &Pool<Sqlite>, query: &str) -> Result<Vec<JobCatalogItem>> {
    let search_term = format!("%{}%", query);

    sqlx::query_as::<_, JobCatalogItem>(
        r#"
        SELECT id, name, display_name, description, category, subcategory, icon, difficulty,
               command, parameters, required_capabilities, os_filter,
               default_timeout, default_retry_count, working_directory, environment,
               success_title_template, success_body_template, failure_title_template, failure_body_template,
               ntfy_success_tags, ntfy_failure_tags, tags, sort_order, is_system, enabled,
               created_at, updated_at
        FROM job_catalog
        WHERE enabled = 1
          AND (display_name LIKE ? OR description LIKE ? OR tags LIKE ?)
        ORDER BY
            CASE WHEN display_name LIKE ? THEN 0 ELSE 1 END,
            sort_order, display_name
        "#,
    )
    .bind(&search_term)
    .bind(&search_term)
    .bind(&search_term)
    .bind(&search_term)
    .fetch_all(pool)
    .await
    .context("Failed to search catalog items")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

// ============================================================================
// Statistics Queries
// ============================================================================

/// Get catalog statistics
#[instrument(skip(pool))]
pub async fn get_catalog_stats(pool: &Pool<Sqlite>) -> Result<CatalogStats> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM job_catalog WHERE enabled = 1")
        .fetch_one(pool)
        .await
        .context("Failed to count catalog items")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let favorites: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM job_catalog_favorites")
        .fetch_one(pool)
        .await
        .context("Failed to count favorites")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let categories: (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT category) FROM job_catalog WHERE enabled = 1")
            .fetch_one(pool)
            .await
            .context("Failed to count categories")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(CatalogStats {
        total_items: total.0,
        favorite_count: favorites.0,
        category_count: categories.0,
    })
}

/// Catalog statistics
#[derive(Debug, Clone)]
pub struct CatalogStats {
    pub total_items: i64,
    pub favorite_count: i64,
    pub category_count: i64,
}
