use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Tag model - for organizing servers (prod, staging, docker-hosts, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    /// Hex color for UI (e.g., "#5E81AC", "#88C0D0")
    pub color: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Tag {
    /// Validate hex color format
    pub fn is_valid_color(color: &str) -> bool {
        color.starts_with('#')
            && color.len() == 7
            && color[1..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Get the color or a default if none is set
    pub fn color_or_default(&self) -> String {
        self.color.clone().unwrap_or_else(|| "#8FBCBB".to_string())
    }
}

/// ServerTag model - junction table for many-to-many relationship
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerTag {
    pub server_id: i64,
    pub tag_id: i64,
}

/// Input for creating a new tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTag {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
}

impl CreateTag {
    /// Validate the tag input
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Tag name cannot be empty".to_string());
        }

        if let Some(ref color) = self.color {
            if !Tag::is_valid_color(color) {
                return Err(format!(
                    "Invalid color format: {}. Expected format: #RRGGBB",
                    color
                ));
            }
        }

        Ok(())
    }
}

/// Input for updating an existing tag
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateTag {
    pub name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

impl UpdateTag {
    /// Validate the tag update
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref name) = self.name {
            if name.trim().is_empty() {
                return Err("Tag name cannot be empty".to_string());
            }
        }

        if let Some(ref color) = self.color {
            if !Tag::is_valid_color(color) {
                return Err(format!(
                    "Invalid color format: {}. Expected format: #RRGGBB",
                    color
                ));
            }
        }

        Ok(())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some() || self.color.is_some() || self.description.is_some()
    }
}

/// Tag with server count (for UI display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagWithCount {
    #[serde(flatten)]
    pub tag: Tag,
    pub server_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_validation() {
        assert!(Tag::is_valid_color("#5E81AC"));
        assert!(Tag::is_valid_color("#000000"));
        assert!(Tag::is_valid_color("#FFFFFF"));
        assert!(!Tag::is_valid_color("5E81AC")); // Missing #
        assert!(!Tag::is_valid_color("#5E81A")); // Too short
        assert!(!Tag::is_valid_color("#5E81ACC")); // Too long
        assert!(!Tag::is_valid_color("#5E81AG")); // Invalid hex
    }

    #[test]
    fn test_create_tag_validation() {
        let valid = CreateTag {
            name: "prod".to_string(),
            color: Some("#BF616A".to_string()),
            description: Some("Production servers".to_string()),
        };
        assert!(valid.validate().is_ok());

        let empty_name = CreateTag {
            name: "".to_string(),
            color: None,
            description: None,
        };
        assert!(empty_name.validate().is_err());

        let invalid_color = CreateTag {
            name: "test".to_string(),
            color: Some("invalid".to_string()),
            description: None,
        };
        assert!(invalid_color.validate().is_err());
    }

    #[test]
    fn test_update_tag_validation() {
        let valid = UpdateTag {
            name: Some("staging".to_string()),
            color: Some("#D08770".to_string()),
            description: None,
        };
        assert!(valid.validate().is_ok());
        assert!(valid.has_changes());

        let empty = UpdateTag {
            name: None,
            color: None,
            description: None,
        };
        assert!(empty.validate().is_ok());
        assert!(!empty.has_changes());
    }
}
