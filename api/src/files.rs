//! Remote files over SFTP, through the server's stored credentials.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// A path on the server: absolute; empty or missing means the login's home directory.
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PathQuery {
    #[serde(default)]
    pub path: String,
}

/// One directory entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Entry {
    pub name: String,
    /// `dir`, `file`, `symlink` or `other`
    pub kind: String,
    pub size: u64,
    /// RFC 3339, when the server reported a modification time
    pub modified: Option<String>,
    /// `rwxr-xr-x`
    pub mode: String,
    pub owner: Option<String>,
    pub group: Option<String>,
}

/// `GET /servers/{id}/files`: directories first, then names ignoring case.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Listing {
    pub server_id: i64,
    /// The directory listed, resolved (home is shown as its real path)
    pub path: String,
    /// Null at `/`
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Uploaded {
    pub path: String,
    pub bytes: u64,
    pub message: String,
}

/// `POST /servers/{id}/files/mkdir`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewFolder {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FolderCreated {
    pub path: String,
    pub message: String,
}

/// `POST /servers/{id}/files/rename`; never overwrites
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Renamed {
    pub from: String,
    pub to: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Deleted {
    pub path: String,
    /// `file` or `dir`
    pub kind: String,
    pub message: String,
}
