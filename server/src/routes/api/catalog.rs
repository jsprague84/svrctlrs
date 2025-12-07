//! Job Catalog API endpoints
//!
//! Provides REST API access to the job catalog for the Basic Mode workflow.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, instrument};

use super::{ApiError, PaginationMeta, PaginationParams};
use crate::state::AppState;
use svrctlrs_database::queries::job_catalog as catalog_queries;

/// Create router for catalog endpoints
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_catalog))
        .route("/categories", get(list_categories))
        .route("/favorites", get(list_favorites))
        .route("/{id}", get(get_catalog_item))
        .route("/{id}/compatible-servers", get(list_compatible_servers))
        .route("/{id}/favorite", post(toggle_favorite))
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct CatalogListResponse {
    pub success: bool,
    pub data: CatalogListData,
    pub pagination: PaginationMeta,
}

#[derive(Debug, Serialize)]
pub struct CatalogListData {
    pub items: Vec<CatalogItemSummary>,
    pub categories: Vec<CategorySummary>,
}

#[derive(Debug, Serialize)]
pub struct CatalogItemSummary {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub icon: String,
    pub difficulty: String,
    pub required_capabilities: Vec<String>,
    pub default_timeout: i64,
    pub is_favorite: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CategorySummary {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: String,
    pub color: Option<String>,
    pub item_count: i64,
}

#[derive(Debug, Serialize)]
pub struct CatalogItemResponse {
    pub success: bool,
    pub data: CatalogItemDetail,
}

#[derive(Debug, Serialize)]
pub struct CatalogItemDetail {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub icon: String,
    pub difficulty: String,
    pub command: String,
    pub parameters: serde_json::Value,
    pub required_capabilities: Vec<String>,
    pub os_filter: Option<serde_json::Value>,
    pub default_timeout: i64,
    pub default_retry_count: i64,
    pub working_directory: Option<String>,
    pub environment: Option<serde_json::Value>,
    pub success_title_template: Option<String>,
    pub success_body_template: Option<String>,
    pub failure_title_template: Option<String>,
    pub failure_body_template: Option<String>,
    pub ntfy_success_tags: Vec<String>,
    pub ntfy_failure_tags: Vec<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub is_system: bool,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct CompatibleServersResponse {
    pub success: bool,
    pub data: CompatibleServersData,
}

#[derive(Debug, Serialize)]
pub struct CompatibleServersData {
    pub catalog_item_id: i64,
    pub required_capabilities: Vec<String>,
    pub compatible: Vec<ServerCompatibility>,
    pub incompatible: Vec<ServerCompatibility>,
}

#[derive(Debug, Serialize)]
pub struct ServerCompatibility {
    pub id: i64,
    pub name: String,
    pub hostname: Option<String>,
    pub capabilities: Vec<String>,
    pub missing_capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FavoriteToggleResponse {
    pub success: bool,
    pub is_favorite: bool,
    pub message: String,
}

// ============================================================================
// Query Parameters
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CatalogQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub category: Option<String>,
    pub difficulty: Option<String>,
    pub search: Option<String>,
    pub capability: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// List all catalog items with optional filtering
///
/// ## Query Parameters
/// - `page`: Page number (default: 1)
/// - `per_page`: Items per page (default: 50)
/// - `category`: Filter by category
/// - `difficulty`: Filter by difficulty (basic, intermediate, advanced)
/// - `search`: Search in name and description
/// - `capability`: Filter by required capability
#[instrument(skip(state))]
async fn list_catalog(
    State(state): State<AppState>,
    Query(query): Query<CatalogQuery>,
) -> Result<Json<CatalogListResponse>, (StatusCode, Json<ApiError>)> {
    debug!(?query, "Listing catalog items");

    // Get all items
    let all_items = catalog_queries::list_catalog_items(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list catalog items");
            ApiError::internal_error(format!("Failed to list catalog items: {}", e))
        })?;

    // Get favorites
    let favorites = catalog_queries::list_catalog_favorites(&state.pool)
        .await
        .unwrap_or_default();
    let favorite_ids: std::collections::HashSet<i64> = favorites.iter().map(|f| f.id).collect();

    // Get categories with counts
    let categories = catalog_queries::list_catalog_categories_with_counts(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list categories");
            ApiError::internal_error(format!("Failed to list categories: {}", e))
        })?;

    // Apply filters
    let filtered: Vec<_> = all_items
        .into_iter()
        .filter(|item| {
            // Category filter
            if let Some(ref cat) = query.category {
                if item.category != *cat {
                    return false;
                }
            }
            // Difficulty filter
            if let Some(ref diff) = query.difficulty {
                if item.difficulty != *diff {
                    return false;
                }
            }
            // Search filter
            if let Some(ref search) = query.search {
                let search_lower = search.to_lowercase();
                if !item.display_name.to_lowercase().contains(&search_lower)
                    && !item.description.to_lowercase().contains(&search_lower)
                {
                    return false;
                }
            }
            // Capability filter
            if let Some(ref cap) = query.capability {
                let caps = item.get_required_capabilities();
                if !caps.contains(cap) {
                    return false;
                }
            }
            true
        })
        .collect();

    let total = filtered.len();

    // Apply pagination
    let page = query.pagination.page;
    let per_page = query.pagination.per_page;
    let start = (page - 1) * per_page;
    let items: Vec<CatalogItemSummary> = filtered
        .into_iter()
        .skip(start)
        .take(per_page)
        .map(|item| {
            let is_favorite = favorite_ids.contains(&item.id);
            // Extract computed values BEFORE moving fields
            let required_capabilities = item.get_required_capabilities();
            let tags = item.get_tags();
            CatalogItemSummary {
                id: item.id,
                name: item.name,
                display_name: item.display_name,
                description: item.description,
                category: item.category,
                subcategory: item.subcategory,
                icon: item.icon,
                difficulty: item.difficulty,
                required_capabilities,
                default_timeout: item.default_timeout,
                is_favorite,
                tags,
            }
        })
        .collect();

    let category_summaries: Vec<CategorySummary> = categories
        .into_iter()
        .map(|c| CategorySummary {
            id: c.id,
            name: c.name,
            display_name: c.display_name,
            description: c.description,
            icon: c.icon,
            color: c.color,
            item_count: c.item_count,
        })
        .collect();

    Ok(Json(CatalogListResponse {
        success: true,
        data: CatalogListData {
            items,
            categories: category_summaries,
        },
        pagination: PaginationMeta::new(page, per_page, total),
    }))
}

/// List all catalog categories
#[instrument(skip(state))]
async fn list_categories(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let categories = catalog_queries::list_catalog_categories_with_counts(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list categories");
            ApiError::internal_error(format!("Failed to list categories: {}", e))
        })?;

    let category_summaries: Vec<CategorySummary> = categories
        .into_iter()
        .map(|c| CategorySummary {
            id: c.id,
            name: c.name,
            display_name: c.display_name,
            description: c.description,
            icon: c.icon,
            color: c.color,
            item_count: c.item_count,
        })
        .collect();

    Ok(Json(json!({
        "success": true,
        "data": category_summaries
    })))
}

/// Get a single catalog item by ID
#[instrument(skip(state))]
async fn get_catalog_item(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<CatalogItemResponse>, (StatusCode, Json<ApiError>)> {
    debug!(id, "Getting catalog item");

    let item = catalog_queries::get_catalog_item(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id, "Failed to get catalog item");
            ApiError::not_found("Catalog item")
        })?;

    // Check if favorite
    let favorites = catalog_queries::list_catalog_favorites(&state.pool)
        .await
        .unwrap_or_default();
    let is_favorite = favorites.iter().any(|f| f.id == id);

    // Parse JSON fields
    let parameters: serde_json::Value = item
        .parameters
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(json!([]));

    let os_filter: Option<serde_json::Value> = item
        .os_filter
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok());

    let environment: Option<serde_json::Value> = item
        .environment
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok());

    // Extract computed values BEFORE moving fields
    let required_capabilities = item.get_required_capabilities();
    let ntfy_success_tags = item.get_ntfy_success_tags();
    let ntfy_failure_tags = item.get_ntfy_failure_tags();
    let tags = item.get_tags();

    Ok(Json(CatalogItemResponse {
        success: true,
        data: CatalogItemDetail {
            id: item.id,
            name: item.name,
            display_name: item.display_name,
            description: item.description,
            category: item.category,
            subcategory: item.subcategory,
            icon: item.icon,
            difficulty: item.difficulty,
            command: item.command,
            parameters,
            required_capabilities,
            os_filter,
            default_timeout: item.default_timeout,
            default_retry_count: item.default_retry_count,
            working_directory: item.working_directory,
            environment,
            success_title_template: item.success_title_template,
            success_body_template: item.success_body_template,
            failure_title_template: item.failure_title_template,
            failure_body_template: item.failure_body_template,
            ntfy_success_tags,
            ntfy_failure_tags,
            tags,
            is_favorite,
            is_system: item.is_system,
            enabled: item.enabled,
        },
    }))
}

/// List servers compatible with a catalog item
#[instrument(skip(state))]
async fn list_compatible_servers(
    State(state): State<AppState>,
    Path(catalog_id): Path<i64>,
) -> Result<Json<CompatibleServersResponse>, (StatusCode, Json<ApiError>)> {
    use svrctlrs_database::queries::servers;

    debug!(catalog_id, "Listing compatible servers");

    // Get catalog item
    let item = catalog_queries::get_catalog_item(&state.pool, catalog_id)
        .await
        .map_err(|_| ApiError::not_found("Catalog item"))?;

    let required_caps = item.get_required_capabilities();

    // Get all servers with their capabilities
    let all_servers = servers::list_servers_with_details(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list servers");
            ApiError::internal_error("Failed to list servers")
        })?;

    let mut compatible = Vec::new();
    let mut incompatible = Vec::new();

    for server in all_servers {
        let server_caps: Vec<String> = server
            .capabilities
            .iter()
            .filter(|c| c.available)
            .map(|c| c.capability.clone())
            .collect();

        let missing: Vec<String> = required_caps
            .iter()
            .filter(|cap| !server_caps.contains(cap))
            .cloned()
            .collect();

        let compat = ServerCompatibility {
            id: server.server.id,
            name: server.server.name.clone(),
            hostname: server.server.hostname.clone(),
            capabilities: server_caps,
            missing_capabilities: missing.clone(),
        };

        if missing.is_empty() {
            compatible.push(compat);
        } else {
            incompatible.push(compat);
        }
    }

    Ok(Json(CompatibleServersResponse {
        success: true,
        data: CompatibleServersData {
            catalog_item_id: catalog_id,
            required_capabilities: required_caps,
            compatible,
            incompatible,
        },
    }))
}

/// List user's favorite catalog items
#[instrument(skip(state))]
async fn list_favorites(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let favorites = catalog_queries::list_catalog_favorites(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list favorites");
            ApiError::internal_error("Failed to list favorites")
        })?;

    let items: Vec<CatalogItemSummary> = favorites
        .into_iter()
        .map(|item| {
            // Extract computed values BEFORE moving fields
            let required_capabilities = item.get_required_capabilities();
            let tags = item.get_tags();
            CatalogItemSummary {
                id: item.id,
                name: item.name,
                display_name: item.display_name,
                description: item.description,
                category: item.category,
                subcategory: item.subcategory,
                icon: item.icon,
                difficulty: item.difficulty,
                required_capabilities,
                default_timeout: item.default_timeout,
                is_favorite: true,
                tags,
            }
        })
        .collect();

    Ok(Json(json!({
        "success": true,
        "data": items
    })))
}

/// Toggle favorite status for a catalog item
#[instrument(skip(state))]
async fn toggle_favorite(
    State(state): State<AppState>,
    Path(catalog_id): Path<i64>,
) -> Result<Json<FavoriteToggleResponse>, (StatusCode, Json<ApiError>)> {
    debug!(catalog_id, "Toggling favorite status");

    // Verify item exists
    catalog_queries::get_catalog_item(&state.pool, catalog_id)
        .await
        .map_err(|_| ApiError::not_found("Catalog item"))?;

    let is_favorite = catalog_queries::toggle_catalog_favorite(&state.pool, catalog_id)
        .await
        .map_err(|e| {
            error!(error = %e, catalog_id, "Failed to toggle favorite");
            ApiError::internal_error("Failed to toggle favorite")
        })?;

    let message = if is_favorite {
        "Added to favorites"
    } else {
        "Removed from favorites"
    };

    Ok(Json(FavoriteToggleResponse {
        success: true,
        is_favorite,
        message: message.to_string(),
    }))
}
