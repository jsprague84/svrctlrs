//! The SvrCtlRS REST API's request and response types.
//!
//! Each type serializes to exactly the JSON the API has always sent (the web UI and agents
//! depend on it; `server/testdata/api-shapes.json` guards every key and type) and carries an
//! OpenAPI schema (`utoipa::ToSchema`), so `GET /api/v1/openapi.json` and `docs/openapi.json`
//! describe what the server really answers. The server's handlers return these types and
//! `svrctl` reads them.

pub mod approvals;
pub mod audit;
pub mod auth;
pub mod backups;
pub mod common;
pub mod credentials;
pub mod devices;
pub mod files;
pub mod notifications;
pub mod ops;
pub mod profiles;
pub mod quick_commands;
pub mod recordings;
pub mod servers;
pub mod sessions;
pub mod settings;
pub mod sign_in;
pub mod system;
pub mod tokens;
pub mod users;

pub use common::*;
