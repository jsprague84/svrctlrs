//! The SvrCtlRS REST API's request and response types.
//!
//! Each type serializes to exactly the JSON the API has always sent (the web UI and agents
//! depend on it; `server/testdata/api-shapes.json` guards every key and type) and carries an
//! OpenAPI schema (`utoipa::ToSchema`), so `GET /api/v1/openapi.json` and `docs/openapi.json`
//! describe what the server really answers. The server's handlers return these types and
//! `svrctl` reads them.

pub mod audit;
pub mod common;
pub mod notifications;
pub mod sessions;
pub mod system;

pub use common::*;
