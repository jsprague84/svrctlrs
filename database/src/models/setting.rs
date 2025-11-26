use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Setting model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Setting {
    pub key: String,
    pub value: String,
    #[sqlx(rename = "type")]
    pub value_type: String, // 'string', 'number', 'boolean', 'json'
    pub description: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Update setting input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSetting {
    pub value: String,
}

impl Setting {
    /// Parse value as the appropriate type
    pub fn parse_value<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        match self.value_type.as_str() {
            "json" => serde_json::from_str(&self.value),
            "boolean" => {
                let bool_val = self.value == "true" || self.value == "1";
                serde_json::from_value(serde_json::json!(bool_val))
            }
            "number" => {
                if let Ok(num) = self.value.parse::<i64>() {
                    serde_json::from_value(serde_json::json!(num))
                } else if let Ok(num) = self.value.parse::<f64>() {
                    serde_json::from_value(serde_json::json!(num))
                } else {
                    Err(serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid number",
                    )))
                }
            }
            _ => serde_json::from_value(serde_json::json!(self.value)),
        }
    }
}
