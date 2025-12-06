//! Job Catalog models for the Basic Mode workflow system
//!
//! The job catalog contains pre-built job definitions that users can
//! quickly configure and run without needing to create command templates
//! or job templates from scratch.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

// ============================================================================
// Job Catalog Category
// ============================================================================

/// Category for organizing job catalog items
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JobCatalogCategory {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: String,
    pub color: Option<String>,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new catalog category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJobCatalogCategory {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: Option<i64>,
}

// ============================================================================
// Job Catalog Item
// ============================================================================

/// Pre-built job from the catalog (Basic Mode)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JobCatalogItem {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub icon: String,
    pub difficulty: String,
    pub command: String,
    pub parameters: Option<String>,            // JSON string
    pub required_capabilities: Option<String>, // JSON string
    pub os_filter: Option<String>,             // JSON string
    pub default_timeout: i64,
    pub default_retry_count: i64,
    pub working_directory: Option<String>,
    pub environment: Option<String>, // JSON string
    pub success_title_template: Option<String>,
    pub success_body_template: Option<String>,
    pub failure_title_template: Option<String>,
    pub failure_body_template: Option<String>,
    pub ntfy_success_tags: Option<String>, // JSON string
    pub ntfy_failure_tags: Option<String>, // JSON string
    pub tags: Option<String>,              // JSON string
    pub sort_order: i64,
    pub is_system: bool,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JobCatalogItem {
    /// Get required capabilities as a Vec<String>
    pub fn get_required_capabilities(&self) -> Vec<String> {
        self.required_capabilities
            .as_ref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
    }

    /// Get parameter schema as parsed struct
    pub fn get_parameters(&self) -> Vec<CatalogParameter> {
        self.parameters
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    /// Get OS filter as parsed struct
    pub fn get_os_filter(&self) -> Option<OsFilter> {
        self.os_filter
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
    }

    /// Get tags as Vec<String>
    pub fn get_tags(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
    }

    /// Get ntfy success tags as Vec<String>
    pub fn get_ntfy_success_tags(&self) -> Vec<String> {
        self.ntfy_success_tags
            .as_ref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
    }

    /// Get ntfy failure tags as Vec<String>
    pub fn get_ntfy_failure_tags(&self) -> Vec<String> {
        self.ntfy_failure_tags
            .as_ref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
    }

    /// Check if server has required capabilities
    pub fn is_compatible_with(&self, server_capabilities: &[String]) -> bool {
        let required = self.get_required_capabilities();
        if required.is_empty() {
            return true;
        }
        required.iter().all(|cap| server_capabilities.contains(cap))
    }

    /// Check if server matches OS filter
    pub fn matches_os(&self, os_distro: Option<&str>) -> bool {
        match (self.get_os_filter(), os_distro) {
            (None, _) => true,        // No filter means all OSes
            (Some(_), None) => false, // Has filter but no distro info
            (Some(filter), Some(distro)) => {
                filter.distro.iter().any(|d| d.eq_ignore_ascii_case(distro))
            }
        }
    }
}

/// OS filter for catalog items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsFilter {
    pub distro: Vec<String>,
}

// ============================================================================
// Parameter Schema Types
// ============================================================================

/// Parameter definition for catalog items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String, // "string", "number", "boolean", "select", "multiselect"
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<JsonValue>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub validation: Option<ParameterValidation>,
    #[serde(default)]
    pub options: Option<Vec<SelectOption>>,
}

impl CatalogParameter {
    /// Get the default value as a string
    pub fn get_default_string(&self) -> String {
        match &self.default {
            Some(JsonValue::String(s)) => s.clone(),
            Some(JsonValue::Number(n)) => n.to_string(),
            Some(JsonValue::Bool(b)) => b.to_string(),
            Some(v) => v.to_string(),
            None => String::new(),
        }
    }

    /// Check if this is a required parameter
    pub fn is_required(&self) -> bool {
        self.required
    }
}

/// Validation rules for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterValidation {
    pub pattern: Option<String>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    #[serde(rename = "minLength")]
    pub min_length: Option<i64>,
    #[serde(rename = "maxLength")]
    pub max_length: Option<i64>,
}

/// Option for select/multiselect parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

// ============================================================================
// Job Catalog Favorite
// ============================================================================

/// User's favorite catalog item
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JobCatalogFavorite {
    pub id: i64,
    pub catalog_item_id: i64,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Input Types for Creating Catalog Items
// ============================================================================

/// Input for creating a new catalog item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJobCatalogItem {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub icon: Option<String>,
    pub difficulty: Option<String>,
    pub command: String,
    pub parameters: Option<JsonValue>,
    pub required_capabilities: Option<JsonValue>,
    pub os_filter: Option<JsonValue>,
    pub default_timeout: Option<i64>,
    pub default_retry_count: Option<i64>,
    pub working_directory: Option<String>,
    pub environment: Option<JsonValue>,
    pub success_title_template: Option<String>,
    pub success_body_template: Option<String>,
    pub failure_title_template: Option<String>,
    pub failure_body_template: Option<String>,
    pub ntfy_success_tags: Option<JsonValue>,
    pub ntfy_failure_tags: Option<JsonValue>,
    pub tags: Option<JsonValue>,
    pub sort_order: Option<i64>,
}

// ============================================================================
// Query Result Types
// ============================================================================

/// Category with item count for display
#[derive(Debug, Clone, FromRow)]
pub struct JobCatalogCategoryWithCount {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: String,
    pub color: Option<String>,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
    pub item_count: i64,
}

/// Catalog item with favorite status
#[derive(Debug, Clone)]
pub struct JobCatalogItemWithFavorite {
    pub item: JobCatalogItem,
    pub is_favorite: bool,
}

// ============================================================================
// Wizard Input Types
// ============================================================================

/// Input from the job wizard for creating a job from catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardJobInput {
    pub catalog_item_id: i64,
    pub server_id: i64,
    pub parameters: std::collections::HashMap<String, JsonValue>,
    pub schedule_type: String, // "now" or "scheduled"
    pub cron_expression: Option<String>,
    pub name_override: Option<String>,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notification_channel_id: Option<i64>,
}
