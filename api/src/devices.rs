//! Signed-in devices: the caller's browser and desktop-app sign-ins (signed-in users only: no
//! token can list or sign out devices). API tokens are listed under `/tokens`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A browser or desktop-app sign-in
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Device {
    pub id: i64,
    /// A short name from its user agent: "Firefox on Android", "Desktop app on Linux"
    pub label: String,
    /// `password`, `password+totp`, `password+recovery_code`, `oidc`, `app` or `unknown`
    pub method: String,
    /// The device this request came from
    pub current: bool,
    pub created_at: DateTime<Utc>,
    /// Written at most once a minute
    pub last_seen_at: DateTime<Utc>,
    pub last_ip: Option<String>,
}

/// `GET /users/me/devices`: this device first, then the most recently used
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeviceList {
    pub devices: Vec<Device>,
}

/// `POST /users/me/devices/sign-out-others`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DevicesSignedOut {
    pub signed_out: usize,
}
