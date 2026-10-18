//! Remote files over SFTP: browse, download, upload, make a folder, rename, delete.
//!
//! Every request opens its own SSH connection through [`crate::ssh::connect`] (host key and
//! credential rules identical to the shell), asks for the `sftp` subsystem, does one thing and
//! disconnects. Stateless and a second or two per call — fine for a file panel beside a
//! terminal, and nothing to leak or time out between calls. Transfers stream in both
//! directions, so file size is bounded by the remote disk, not by this process's memory.
//!
//! These routes live outside the 30 s HTTP timeout (see `main.rs`): an upload reads its body
//! for as long as the file takes.

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileType, StatusCode as SftpStatus};
use serde::{Deserialize, Serialize};
use serde_json::json;
use svrctlrs_database::models::{NewAuditEntry, Server};
use svrctlrs_database::queries;
use tokio_util::io::{ReaderStream, StreamReader};
use tracing::{info, instrument, warn};

use super::ApiError;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;

type ApiResult<T> = Result<T, ApiError>;

/// Uploads may be as large as the remote disk allows; this is only a sanity cap.
const MAX_UPLOAD_BYTES: usize = 4 * 1024 * 1024 * 1024;

/// Full paths: this router is merged beside the API router, outside its request timeout.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/servers/{id}/files", get(list).delete(remove))
        .route(
            "/api/v1/servers/{id}/files/content",
            get(download).put(upload),
        )
        .route("/api/v1/servers/{id}/files/mkdir", post(mkdir))
        .route("/api/v1/servers/{id}/files/rename", post(rename))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
}

#[derive(Debug, Deserialize)]
struct PathQuery {
    /// Absolute remote path; empty or missing means the login's home directory
    #[serde(default)]
    path: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// `dir`, `file`, `symlink` or `other`
    pub kind: &'static str,
    pub size: u64,
    /// RFC 3339, when the server reported a modification time
    pub modified: Option<String>,
    /// `rwxr-xr-x`
    pub mode: String,
    pub owner: Option<String>,
    pub group: Option<String>,
}

/// Reject what the SFTP server would mishandle before any connection is made: relative
/// paths (there is no stable "current directory" across stateless requests) and NUL bytes.
/// Empty means "home" and is fine.
pub fn check_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Ok(());
    }
    if path.contains('\0') {
        return Err("Path contains a NUL byte".to_string());
    }
    if !path.starts_with('/') {
        return Err("Path must be absolute (start with /)".to_string());
    }
    Ok(())
}

/// The parent of an absolute path; `None` at the root.
pub fn parent_of(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.rfind('/') {
        Some(0) => Some("/".to_string()),
        Some(i) => Some(trimmed[..i].to_string()),
        None => None,
    }
}

/// `rwxr-xr-x` from the low nine permission bits, with setuid/setgid/sticky in the usual spots.
pub fn mode_string(permissions: Option<u32>) -> String {
    let Some(p) = permissions else {
        return "---------".to_string();
    };
    let mut s = String::with_capacity(9);
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    for (i, (bit, ch)) in bits.iter().enumerate() {
        let set = p & bit != 0;
        let special = match i {
            2 if p & 0o4000 != 0 => Some(if set { 's' } else { 'S' }),
            5 if p & 0o2000 != 0 => Some(if set { 's' } else { 'S' }),
            8 if p & 0o1000 != 0 => Some(if set { 't' } else { 'T' }),
            _ => None,
        };
        s.push(special.unwrap_or(if set { *ch } else { '-' }));
    }
    s
}

fn kind_of(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Dir => "dir",
        FileType::File => "file",
        FileType::Symlink => "symlink",
        FileType::Other => "other",
    }
}

fn rfc3339(mtime: Option<u32>) -> Option<String> {
    mtime.and_then(|t| chrono::DateTime::from_timestamp(i64::from(t), 0).map(|d| d.to_rfc3339()))
}

/// Directories first, then case-insensitive by name — what a file panel expects.
pub fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|a, b| {
        let da = a.kind == "dir";
        let db = b.kind == "dir";
        db.cmp(&da)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// An open SFTP session plus the server it belongs to. Dropping it ends the SSH connection.
struct Sftp {
    session: SftpSession,
    server: Server,
    handle: russh::client::Handle<crate::ssh::Handler>,
}

impl Sftp {
    async fn finish(self) {
        let _ = self.session.close().await;
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "English")
            .await;
    }
}

/// Connect to the server and start the SFTP subsystem.
async fn open(state: &AppState, server_id: i64) -> ApiResult<Sftp> {
    let server = queries::servers::get_server(&state.pool, server_id).await?;
    let credential = match server.credential_id {
        Some(cid) => queries::credentials::get_credential(&state.pool, cid)
            .await
            .ok(),
        None => None,
    };
    let conn = crate::ssh::connect(&state.pool, &server, credential.as_ref()).await?;
    let channel = conn
        .handle
        .channel_open_session()
        .await
        .map_err(|e| ApiError::bad_request(format!("Failed to open channel: {e}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| ApiError::bad_request(format!("The server refused SFTP: {e}")))?;
    let session = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| ApiError::bad_request(format!("SFTP did not start: {e}")))?;
    Ok(Sftp {
        session,
        server,
        handle: conn.handle,
    })
}

/// SFTP status codes map onto HTTP: a missing file is 404, a permission refusal 403, the
/// rest is a 400 that carries the server's own message.
fn sftp_error(what: &str, e: russh_sftp::client::error::Error) -> ApiError {
    use russh_sftp::client::error::Error;
    match &e {
        Error::Status(status) => match status.status_code {
            SftpStatus::NoSuchFile => ApiError::not_found(what),
            SftpStatus::PermissionDenied => {
                ApiError::forbidden(format!("Permission denied: {what}"))
            }
            _ => ApiError::bad_request(format!("{what}: {}", status.error_message)),
        },
        Error::Timeout => {
            ApiError::bad_request(format!("{what}: the server did not answer in time"))
        }
        other => ApiError::bad_request(format!("{what}: {other}")),
    }
}

/// Resolve the requested path: empty is the home directory, anything else is canonicalised
/// so the breadcrumb shows where symlinks really lead.
async fn resolve(sftp: &Sftp, path: &str) -> ApiResult<String> {
    check_path(path).map_err(ApiError::bad_request)?;
    let target = if path.is_empty() { "." } else { path };
    sftp.session.canonicalize(target).await.map_err(|e| {
        sftp_error(
            if path.is_empty() {
                "Home directory"
            } else {
                path
            },
            e,
        )
    })
}

fn audit(
    state: &AppState,
    me: i64,
    headers: &HeaderMap,
    action: &str,
    server: &Server,
    details: serde_json::Value,
) {
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new(action)
            .user(me)
            .client_ip(Some(client_ip(headers)))
            .target("server", server.id, &server.name)
            .details(details),
    );
}

#[instrument(skip(state))]
async fn list(
    State(state): State<AppState>,
    CurrentUser(_me): CurrentUser,
    Path(id): Path<i64>,
    Query(q): Query<PathQuery>,
) -> ApiResult<impl IntoResponse> {
    let sftp = open(&state, id).await?;
    let result = async {
        let path = resolve(&sftp, &q.path).await?;
        let dir = sftp
            .session
            .read_dir(&path)
            .await
            .map_err(|e| sftp_error(&path, e))?;
        let mut entries: Vec<Entry> = dir
            .filter(|e| e.file_name() != "." && e.file_name() != "..")
            .map(|e| {
                let meta = e.metadata();
                Entry {
                    name: e.file_name(),
                    kind: kind_of(e.file_type()),
                    size: meta.size.unwrap_or(0),
                    modified: rfc3339(meta.mtime),
                    mode: mode_string(meta.permissions),
                    owner: meta.user.clone(),
                    group: meta.group.clone(),
                }
            })
            .collect();
        sort_entries(&mut entries);
        Ok::<_, ApiError>(json!({
            "server_id": id,
            "path": path,
            "parent": parent_of(&path),
            "entries": entries,
        }))
    }
    .await;
    sftp.finish().await;
    result.map(Json)
}

/// The file's bytes, streamed, as a download.
#[instrument(skip(state))]
async fn download(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<PathQuery>,
) -> ApiResult<Response> {
    if q.path.is_empty() {
        return Err(ApiError::bad_request("A file path is required"));
    }
    let sftp = open(&state, id).await?;
    let path = match resolve(&sftp, &q.path).await {
        Ok(p) => p,
        Err(e) => {
            sftp.finish().await;
            return Err(e);
        }
    };
    let meta = match sftp.session.metadata(&path).await {
        Ok(m) => m,
        Err(e) => {
            sftp.finish().await;
            return Err(sftp_error(&path, e));
        }
    };
    if meta.is_dir() {
        sftp.finish().await;
        return Err(ApiError::bad_request(
            "That is a directory; download a file",
        ));
    }
    let file = match sftp.session.open(&path).await {
        Ok(f) => f,
        Err(e) => {
            sftp.finish().await;
            return Err(sftp_error(&path, e));
        }
    };
    audit(
        &state,
        me,
        &headers,
        "files.download",
        &sftp.server,
        json!({ "path": path }),
    );
    info!(server = %sftp.server.name, path = %path, "File download");

    let name = path.rsplit('/').next().unwrap_or("download").to_string();
    // The session and the SSH handle must outlive the stream; they ride along in the stream
    // and are dropped when the body ends, which disconnects.
    let keepalive = sftp;
    let stream = ReaderStream::with_capacity(file, 64 * 1024);
    let stream = futures_util::stream::StreamExt::map(stream, move |chunk| {
        let _hold = &keepalive;
        chunk
    });
    let mut response = Response::new(Body::from_stream(stream));
    let headers_out = response.headers_mut();
    headers_out.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Some(len) = meta.size {
        if let Ok(v) = HeaderValue::from_str(&len.to_string()) {
            headers_out.insert(header::CONTENT_LENGTH, v);
        }
    }
    let disposition = format!("attachment; filename*=UTF-8''{}", percent_encode(&name));
    if let Ok(v) = HeaderValue::from_str(&disposition) {
        headers_out.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(response)
}

/// RFC 5987 value encoding for the download file name.
pub fn percent_encode(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Create or overwrite a file with the request body, streamed.
#[instrument(skip(state, body))]
async fn upload(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<PathQuery>,
    body: Body,
) -> ApiResult<impl IntoResponse> {
    if q.path.is_empty() {
        return Err(ApiError::bad_request("A destination file path is required"));
    }
    check_path(&q.path).map_err(ApiError::bad_request)?;
    let sftp = open(&state, id).await?;
    let result = async {
        let mut file = sftp
            .session
            .create(&q.path)
            .await
            .map_err(|e| sftp_error(&q.path, e))?;
        let mut reader = StreamReader::new(futures_util::stream::TryStreamExt::map_err(
            body.into_data_stream(),
            std::io::Error::other,
        ));
        let written = tokio::io::copy(&mut reader, &mut file)
            .await
            .map_err(|e| ApiError::bad_request(format!("Upload to {} failed: {e}", q.path)))?;
        file.sync_all().await.ok();
        file.close()
            .await
            .map_err(|e| ApiError::bad_request(format!("Closing {} failed: {e}", q.path)))?;
        Ok::<u64, ApiError>(written)
    }
    .await;
    let outcome = match result {
        Ok(bytes) => {
            audit(
                &state,
                me,
                &headers,
                "files.upload",
                &sftp.server,
                json!({ "path": q.path, "bytes": bytes }),
            );
            info!(server = %sftp.server.name, path = %q.path, bytes, "File uploaded");
            Ok(Json(
                json!({ "path": q.path, "bytes": bytes, "message": "Uploaded" }),
            ))
        }
        Err(e) => Err(e),
    };
    sftp.finish().await;
    outcome
}

#[derive(Debug, Deserialize)]
struct MkdirInput {
    path: String,
}

#[instrument(skip(state))]
async fn mkdir(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<MkdirInput>,
) -> ApiResult<impl IntoResponse> {
    check_path(&input.path).map_err(ApiError::bad_request)?;
    if input.path.is_empty() {
        return Err(ApiError::bad_request("A folder path is required"));
    }
    let sftp = open(&state, id).await?;
    let result = sftp
        .session
        .create_dir(&input.path)
        .await
        .map_err(|e| sftp_error(&input.path, e));
    if result.is_ok() {
        audit(
            &state,
            me,
            &headers,
            "files.mkdir",
            &sftp.server,
            json!({ "path": input.path }),
        );
    }
    sftp.finish().await;
    result.map(|_| Json(json!({ "path": input.path, "message": "Folder created" })))
}

#[derive(Debug, Deserialize)]
struct RenameInput {
    from: String,
    to: String,
}

#[instrument(skip(state))]
async fn rename(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<RenameInput>,
) -> ApiResult<impl IntoResponse> {
    for p in [&input.from, &input.to] {
        check_path(p).map_err(ApiError::bad_request)?;
        if p.is_empty() {
            return Err(ApiError::bad_request("Both paths are required"));
        }
    }
    let sftp = open(&state, id).await?;
    let result = async {
        // SFTP rename refuses to overwrite; say so instead of a bare "Failure"
        if sftp.session.try_exists(&input.to).await.unwrap_or(false) {
            return Err(ApiError::conflict(format!("{} already exists", input.to)));
        }
        sftp.session
            .rename(&input.from, &input.to)
            .await
            .map_err(|e| sftp_error(&input.from, e))
    }
    .await;
    if result.is_ok() {
        audit(
            &state,
            me,
            &headers,
            "files.rename",
            &sftp.server,
            json!({ "from": input.from, "to": input.to }),
        );
    }
    sftp.finish().await;
    result.map(|_| Json(json!({ "from": input.from, "to": input.to, "message": "Renamed" })))
}

/// Delete a file, or an empty directory. Nothing recursive: that belongs in the shell.
#[instrument(skip(state))]
async fn remove(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<PathQuery>,
) -> ApiResult<impl IntoResponse> {
    if q.path.is_empty() {
        return Err(ApiError::bad_request("A path is required"));
    }
    check_path(&q.path).map_err(ApiError::bad_request)?;
    let sftp = open(&state, id).await?;
    let result = async {
        let meta = sftp
            .session
            .symlink_metadata(&q.path)
            .await
            .map_err(|e| sftp_error(&q.path, e))?;
        if meta.is_dir() {
            sftp.session.remove_dir(&q.path).await.map_err(|e| {
                if matches!(&e, russh_sftp::client::error::Error::Status(s) if s.status_code == SftpStatus::Failure) {
                    ApiError::conflict(format!("{} is not empty; empty it first (or use the shell)", q.path))
                } else {
                    sftp_error(&q.path, e)
                }
            })?;
            Ok::<&str, ApiError>("dir")
        } else {
            sftp.session
                .remove_file(&q.path)
                .await
                .map_err(|e| sftp_error(&q.path, e))?;
            Ok("file")
        }
    }
    .await;
    if let Ok(kind) = &result {
        audit(
            &state,
            me,
            &headers,
            "files.delete",
            &sftp.server,
            json!({ "path": q.path, "kind": kind }),
        );
        warn!(server = %sftp.server.name, path = %q.path, kind, "Remote path deleted");
    }
    sftp.finish().await;
    result.map(|kind| Json(json!({ "path": q.path, "kind": kind, "message": "Deleted" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_must_be_absolute_or_empty() {
        assert!(check_path("").is_ok());
        assert!(check_path("/etc").is_ok());
        assert!(check_path("etc").is_err());
        assert!(check_path("./x").is_err());
        assert!(check_path("/a\0b").is_err());
    }

    #[test]
    fn parent_of_absolute_paths() {
        assert_eq!(parent_of("/"), None);
        assert_eq!(parent_of("/home"), Some("/".into()));
        assert_eq!(parent_of("/home/ubuntu/"), Some("/home".into()));
        assert_eq!(parent_of("/home/ubuntu/src"), Some("/home/ubuntu".into()));
    }

    #[test]
    fn mode_string_renders_like_ls() {
        assert_eq!(mode_string(Some(0o755)), "rwxr-xr-x");
        assert_eq!(mode_string(Some(0o100644)), "rw-r--r--");
        assert_eq!(mode_string(Some(0o4755)), "rwsr-xr-x");
        assert_eq!(mode_string(Some(0o1777)), "rwxrwxrwt");
        assert_eq!(mode_string(Some(0o1000)), "--------T");
        assert_eq!(mode_string(None), "---------");
    }

    fn entry(name: &str, kind: &'static str) -> Entry {
        Entry {
            name: name.into(),
            kind,
            size: 0,
            modified: None,
            mode: String::new(),
            owner: None,
            group: None,
        }
    }

    #[test]
    fn directories_first_then_case_insensitive_names() {
        let mut v = vec![
            entry("zeta", "file"),
            entry("Alpha", "file"),
            entry("src", "dir"),
            entry("Docs", "dir"),
            entry("alpha", "file"),
        ];
        sort_entries(&mut v);
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Docs", "src", "Alpha", "alpha", "zeta"]);
    }

    #[test]
    fn download_names_are_percent_encoded_for_rfc5987() {
        assert_eq!(percent_encode("report.pdf"), "report.pdf");
        assert_eq!(percent_encode("über cool?.txt"), "%C3%BCber%20cool%3F.txt");
    }
}
