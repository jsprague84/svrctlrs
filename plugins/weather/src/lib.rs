//! Weather monitoring plugin
//!
//! Optional add-on plugin that fetches weather data from OpenWeatherMap API
//! and sends notifications with current conditions and forecast.
//!
//! ## Configuration
//!
//! Required environment variables:
//! - `OWM_API_KEY` - OpenWeatherMap API key
//!
//! Optional environment variables:
//! - `DEFAULT_LOCATION` - Location (e.g., "Davenport,IA,US")
//! - `DEFAULT_ZIP` - ZIP code (e.g., "52801")
//! - `DEFAULT_UNITS` - Units: "imperial" (default) or "metric"
//! - `WEATHER_SCHEDULE` - Cron schedule (default: "0 6 * * *" - 6 AM daily)

use async_trait::async_trait;
use chrono::{FixedOffset, TimeZone};
use reqwest::Client;
use serde::Deserialize;
use svrctlrs_core::{
    Error, NotificationMessage, Plugin, PluginContext, PluginMetadata, PluginResult, Result,
    ScheduledTask,
};
use tracing::{info, warn};

/// Weather monitoring plugin
pub struct WeatherPlugin {
    client: Client,
    api_key: Option<String>,
    location: Option<String>,
    zip: Option<String>,
    units: String,
    schedule: String,
}

#[derive(Debug, Deserialize)]
struct OneCall {
    timezone: String,
    timezone_offset: i32,
    current: Current,
    daily: Vec<Daily>,
}

#[derive(Debug, Deserialize)]
struct Current {
    dt: i64,
    temp: f64,
    humidity: u8,
    weather: Vec<Weather>,
}

#[derive(Debug, Deserialize)]
struct Daily {
    dt: i64,
    temp: DailyTemp,
    weather: Vec<Weather>,
}

#[derive(Debug, Deserialize)]
struct DailyTemp {
    min: f64,
    max: f64,
}

#[derive(Debug, Deserialize)]
struct Weather {
    description: String,
}

#[derive(Debug, Deserialize)]
struct ZipGeoResult {
    name: String,
    lat: f64,
    lon: f64,
    country: String,
}

#[derive(Debug, Deserialize)]
struct GeoResult {
    name: String,
    lat: f64,
    lon: f64,
    country: String,
    state: Option<String>,
}

impl WeatherPlugin {
    /// Create a new weather plugin
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: std::env::var("OWM_API_KEY").ok(),
            location: std::env::var("DEFAULT_LOCATION").ok(),
            zip: std::env::var("DEFAULT_ZIP").ok(),
            units: std::env::var("DEFAULT_UNITS").unwrap_or_else(|_| "imperial".to_string()),
            schedule: std::env::var("WEATHER_SCHEDULE").unwrap_or_else(|_| "0 6 * * *".to_string()), // 6 AM daily
        }
    }

    /// Create a new Weather plugin from a JSON configuration
    pub fn from_config(config: serde_json::Value) -> svrctlrs_core::Result<Self> {
        let api_key = config["api_key"].as_str().map(|s| s.to_string());
        let location = config["location"].as_str().map(|s| s.to_string());
        let zip = config["zip"].as_str().map(|s| s.to_string());
        let units = config["units"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "imperial".to_string());
        let schedule = config["schedule"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "0 6 * * *".to_string());

        Ok(Self {
            client: Client::new(),
            api_key,
            location,
            zip,
            units,
            schedule,
        })
    }

    /// Fetch weather data and send notification
    async fn fetch_weather(
        &self,
        notify_mgr: &svrctlrs_core::NotificationManager,
    ) -> Result<String> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| Error::ConfigError("OWM_API_KEY not configured".to_string()))?;

        // Resolve location to lat/lon
        let (lat, lon, pretty_location) = self.resolve_location(api_key).await?;

        // Fetch One Call API data
        let onecall_url = format!(
            "https://api.openweathermap.org/data/3.0/onecall?lat={lat}&lon={lon}&exclude=minutely,hourly,alerts&units={}&appid={api_key}",
            self.units
        );

        let data: OneCall = self
            .client
            .get(&onecall_url)
            .send()
            .await
            .map_err(|e| Error::PluginError(format!("API request failed: {}", e)))?
            .error_for_status()
            .map_err(|e| Error::PluginError(format!("API error: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::PluginError(format!("JSON parse error: {}", e)))?;

        // Format weather data
        let (unit_label, degree) = if self.units == "metric" {
            ("°C", "°C")
        } else {
            ("°F", "°F")
        };

        let offset = FixedOffset::east_opt(data.timezone_offset)
            .ok_or_else(|| Error::PluginError("Invalid timezone offset".to_string()))?;
        let current_time = offset
            .timestamp_opt(data.current.dt, 0)
            .single()
            .ok_or_else(|| Error::PluginError("Invalid timestamp".to_string()))?;

        let current_desc = data
            .current
            .weather
            .first()
            .map(|w| w.description.as_str())
            .unwrap_or("no description");

        let today_high = data.daily.first().map(|d| d.temp.max);
        let today_low = data.daily.first().map(|d| d.temp.min);

        // Build detailed message
        let mut lines = Vec::new();
        lines.push(format!(
            "Location: {}\nTimezone: {}\nNow: {} | {} | Temp: {:.1} {} | Humidity: {}%",
            pretty_location,
            data.timezone,
            current_time.format("%Y-%m-%d %H:%M"),
            current_desc,
            data.current.temp,
            unit_label,
            data.current.humidity
        ));
        lines.push("\nNext 7 days (high/low):".to_string());

        for day in data.daily.iter().skip(1).take(7) {
            let dt = offset
                .timestamp_opt(day.dt, 0)
                .single()
                .ok_or_else(|| Error::PluginError("Invalid timestamp".to_string()))?;
            let label = dt.format("%a %d").to_string();
            let desc = day
                .weather
                .first()
                .map(|w| w.description.as_str())
                .unwrap_or("n/a");
            lines.push(format!(
                "  {label}: {:>5.1}{deg}/{:>5.1}{deg}  ({desc})",
                day.temp.max,
                day.temp.min,
                deg = degree
            ));
        }

        let detailed_message = lines.join("\n");

        // Build summary for title
        let summary = match (today_high, today_low) {
            (Some(h), Some(l)) => format!(
                "Now: {:.1}{} ({}) | Today H/L: {:.1}{}/ {:.1}{}",
                data.current.temp, degree, current_desc, h, degree, l, degree
            ),
            _ => format!("Now: {:.1}{} ({})", data.current.temp, degree, current_desc),
        };

        // Send notification
        let notification = NotificationMessage {
            title: format!("Weather: {}", pretty_location),
            body: detailed_message.clone(),
            priority: 3, // Normal priority
            actions: vec![],
        };

        notify_mgr
            .send_for_service("weather", &notification)
            .await?;

        Ok(summary)
    }

    async fn resolve_location(&self, api_key: &str) -> Result<(f64, f64, String)> {
        // Priority: zip -> location
        if let Some(zip) = &self.zip {
            return self.geocode_zip(api_key, zip).await;
        }

        if let Some(loc) = &self.location {
            return self.geocode_location(api_key, loc).await;
        }

        Err(Error::ConfigError(
            "No location configured (DEFAULT_ZIP or DEFAULT_LOCATION required)".to_string(),
        ))
    }

    async fn geocode_zip(&self, api_key: &str, zip_in: &str) -> Result<(f64, f64, String)> {
        let (zip, cc) = self.split_zip_and_cc(zip_in);
        let url =
            format!("https://api.openweathermap.org/geo/1.0/zip?zip={zip},{cc}&appid={api_key}");

        let z: ZipGeoResult = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::PluginError(format!("Geocoding failed: {}", e)))?
            .error_for_status()
            .map_err(|e| Error::PluginError(format!("Geocoding error: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::PluginError(format!("JSON parse error: {}", e)))?;

        Ok((z.lat, z.lon, format!("{}, {}", z.name, z.country)))
    }

    async fn geocode_location(&self, api_key: &str, input: &str) -> Result<(f64, f64, String)> {
        let q = self.normalize_city_query(input);
        let url =
            format!("https://api.openweathermap.org/geo/1.0/direct?q={q}&limit=1&appid={api_key}");

        let mut v: Vec<GeoResult> = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::PluginError(format!("Geocoding failed: {}", e)))?
            .error_for_status()
            .map_err(|e| Error::PluginError(format!("Geocoding error: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::PluginError(format!("JSON parse error: {}", e)))?;

        if v.is_empty() {
            return Err(Error::PluginError(format!(
                "Could not find coordinates for \"{}\"",
                input
            )));
        }

        let loc = v.remove(0);
        let state_part = loc
            .state
            .as_ref()
            .map(|s| format!(", {}", s))
            .unwrap_or_default();
        let pretty = format!("{}{}, {}", loc.name, state_part, loc.country);

        Ok((loc.lat, loc.lon, pretty))
    }

    fn split_zip_and_cc(&self, s: &str) -> (String, String) {
        let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
        let zip = parts.first().copied().unwrap_or("").to_string();
        let cc = parts
            .get(1)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "US".to_string());
        (zip, cc)
    }

    fn normalize_city_query(&self, input: &str) -> String {
        let parts: Vec<&str> = input.split(',').map(|p| p.trim()).collect();
        match parts.len() {
            1 => parts[0].to_string(),
            2 => {
                let second = parts[1];
                if second.len() == 2 && second.chars().all(|c| c.is_ascii_alphabetic()) {
                    format!("{},{},US", parts[0], second)
                } else {
                    format!("{},{}", parts[0], second)
                }
            }
            _ => parts.join(","),
        }
    }
}

impl Default for WeatherPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for WeatherPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "weather".to_string(),
            name: "Weather Monitoring".to_string(),
            description: "Monitors weather conditions via OpenWeatherMap API".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            author: "SvrCtlRS".to_string(),
        }
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![ScheduledTask {
            id: "weather_check".to_string(),
            description: "Fetch weather and send notifications".to_string(),
            schedule: self.schedule.clone(),
            enabled: true,
        }]
    }

    async fn init(&mut self) -> Result<()> {
        info!("Initializing weather plugin");

        if self.api_key.is_none() {
            warn!("OWM_API_KEY not configured - weather plugin will fail on execution");
        }

        if self.location.is_none() && self.zip.is_none() {
            warn!("No location configured (DEFAULT_ZIP or DEFAULT_LOCATION) - weather plugin will fail on execution");
        }

        Ok(())
    }

    async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult> {
        match task_id {
            "weather_check" => {
                info!("Executing weather check");
                match self.fetch_weather(&context.notification_manager).await {
                    Ok(summary) => Ok(PluginResult {
                        success: true,
                        message: format!("Weather updated: {}", summary),
                        data: None,
                        metrics: None,
                    }),
                    Err(e) => Ok(PluginResult {
                        success: false,
                        message: format!("Weather check failed: {}", e),
                        data: None,
                        metrics: None,
                    }),
                }
            }
            _ => Err(Error::PluginError(format!("Unknown task: {}", task_id))),
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down weather plugin");
        Ok(())
    }
}
