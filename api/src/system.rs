//! Health and status.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `GET /health`: unauthenticated; 503 with `status: degraded` when the database is down.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Health {
    /// `ok` or `degraded`
    pub status: String,
    /// `ok` or `error`
    pub database: String,
    /// Always `svrctlrs`
    pub service: String,
    pub version: String,
    /// The git commit the image was built from; `unknown` outside CI builds
    pub revision: String,
}

/// `GET /status`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Status {
    /// Always `running`
    pub status: String,
    pub version: String,
    pub resources: StatusResources,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatusResources {
    /// How many servers are configured
    pub servers: usize,
}
