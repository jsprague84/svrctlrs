//! Remote files over SFTP: browse, download (or view), upload (or save an edit), make a
//! folder, rename, delete.
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
    response::Response,
    Json,
};
use futures_util::StreamExt;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileType, OpenFlags, StatusCode as SftpStatus};
use serde_json::json;
use sha2::{Digest, Sha256};
pub use svrctlrs_api::files::Entry;
use svrctlrs_api::files::{
    ContentQuery, Deleted, FolderCreated, Listing, NewFolder, PathQuery, Rename, Renamed, Uploaded,
};
use svrctlrs_api::ErrorBody;
use svrctlrs_database::models::{NewAuditEntry, Server};
use svrctlrs_database::queries;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::io::{ReaderStream, StreamReader};
use tracing::{info, instrument, warn};

use super::ApiError;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

type ApiResult<T> = Result<T, ApiError>;

/// Uploads may be as large as the remote disk allows; this is only a sanity cap.
const MAX_UPLOAD_BYTES: usize = 4 * 1024 * 1024 * 1024;

/// Files up to this size are read with an `ETag`, and can be changed with `If-Match`: the
/// in-app editor's limit. Its check reads the file again, which is cheap at this size.
pub const TAGGED_MAX_BYTES: u64 = 256 * 1024;

/// Full paths: this router is merged beside the API router, outside its request timeout.
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, remove))
        .routes(routes!(download, upload))
        .routes(routes!(mkdir))
        .routes(routes!(rename))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
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

/// List a directory on the server
#[utoipa::path(get, path = "/api/v1/servers/{id}/files", tag = "files",
    params(("id" = i64, Path), PathQuery),
    responses(
        (status = 200, body = Listing),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 428, description = "Host key not confirmed yet", body = ErrorBody),
    ))]
#[instrument(skip(state))]
async fn list(
    State(state): State<AppState>,
    CurrentUser(_me): CurrentUser,
    Path(id): Path<i64>,
    Query(q): Query<PathQuery>,
) -> ApiResult<Json<Listing>> {
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
                    kind: kind_of(e.file_type()).to_string(),
                    size: meta.size.unwrap_or(0),
                    modified: rfc3339(meta.mtime),
                    mode: mode_string(meta.permissions),
                    owner: meta.user.clone(),
                    group: meta.group.clone(),
                }
            })
            .collect();
        sort_entries(&mut entries);
        Ok::<_, ApiError>(Listing {
            server_id: id,
            parent: parent_of(&path),
            path,
            entries,
        })
    }
    .await;
    sftp.finish().await;
    result.map(Json)
}

/// The file's bytes, streamed, as a download. A file of at most [`TAGGED_MAX_BYTES`] comes
/// with an `ETag` (its SHA-256), which an upload can send back as `If-Match`.
#[utoipa::path(get, path = "/api/v1/servers/{id}/files/content", tag = "files",
    params(("id" = i64, Path), ContentQuery),
    responses(
        (status = 200, description = "The file; `ETag` set when it is small enough to edit",
            content_type = "application/octet-stream", body = Vec<u8>),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ))]
#[instrument(skip(state))]
async fn download(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<ContentQuery>,
) -> ApiResult<Response> {
    if q.path.is_empty() {
        return Err(ApiError::bad_request("A file path is required"));
    }
    let sftp = open(&state, id).await?;
    let opened = async {
        let path = resolve(&sftp, &q.path).await?;
        let meta = sftp
            .session
            .metadata(&path)
            .await
            .map_err(|e| sftp_error(&path, e))?;
        if meta.is_dir() {
            return Err(ApiError::bad_request(
                "That is a directory; download a file",
            ));
        }
        let file = sftp
            .session
            .open(&path)
            .await
            .map_err(|e| sftp_error(&path, e))?;
        Ok((path, meta, file))
    }
    .await;
    let (path, meta, mut file) = match opened {
        Ok(o) => o,
        Err(e) => {
            sftp.finish().await;
            return Err(e);
        }
    };
    audit(
        &state,
        me,
        &headers,
        if q.view {
            "files.view"
        } else {
            "files.download"
        },
        &sftp.server,
        json!({ "path": path }),
    );
    info!(server = %sftp.server.name, path = %path, view = q.view, "File read");

    // A small file is read whole so its tag can go in the headers; one that grew past the
    // limit since it was listed streams on after what was read, without a tag
    let mut head = Vec::new();
    let mut etag = None;
    if meta.size.is_some_and(|s| s <= TAGGED_MAX_BYTES) {
        let read = (&mut file)
            .take(TAGGED_MAX_BYTES + 1)
            .read_to_end(&mut head)
            .await;
        if let Err(e) = read {
            sftp.finish().await;
            return Err(ApiError::bad_request(format!("Reading {path} failed: {e}")));
        }
        if head.len() as u64 <= TAGGED_MAX_BYTES {
            etag = Some(entity_tag(&head));
        }
    }
    let length = match etag {
        Some(_) => Some(head.len() as u64),
        None => meta.size,
    };

    let name = path.rsplit('/').next().unwrap_or("download").to_string();
    // The session and the SSH handle must outlive the stream; they ride along in the stream
    // and are dropped when the body ends, which disconnects.
    let keepalive = sftp;
    let rest = ReaderStream::with_capacity(file, 64 * 1024);
    let stream = futures_util::stream::once(async move { Ok(axum::body::Bytes::from(head)) })
        .chain(rest)
        .map(move |chunk| {
            let _hold = &keepalive;
            chunk
        });
    let mut response = Response::new(Body::from_stream(stream));
    let headers_out = response.headers_mut();
    headers_out.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Some(len) = length {
        if let Ok(v) = HeaderValue::from_str(&len.to_string()) {
            headers_out.insert(header::CONTENT_LENGTH, v);
        }
    }
    if let Some(tag) = etag.and_then(|t| HeaderValue::from_str(&t).ok()) {
        headers_out.insert(header::ETAG, tag);
    }
    // Always an attachment: a browser that navigates here saves the file and never renders
    // a server's HTML or SVG as this app's page
    let disposition = format!("attachment; filename*=UTF-8''{}", percent_encode(&name));
    if let Ok(v) = HeaderValue::from_str(&disposition) {
        headers_out.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(response)
}

/// The quoted entity tag of a file's bytes: their hex SHA-256.
pub fn entity_tag(bytes: &[u8]) -> String {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("\"{hex}\"")
}

/// An `If-Match` header: any existing file (`*`), or a list of entity tags.
#[derive(Debug, PartialEq, Eq)]
pub enum IfMatch {
    Any,
    Tags(Vec<String>),
}

impl IfMatch {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value == "*" {
            return Some(Self::Any);
        }
        let tags: Vec<String> = value
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
        (!tags.is_empty()).then_some(Self::Tags(tags))
    }

    /// RFC 9110's strong comparison: a weak tag (`W/"…"`) never matches.
    pub fn matches(&self, current: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Tags(tags) => tags.iter().any(|t| t == current),
        }
    }
}

/// For an upload sent with `If-Match`: the file must still be the one that was read, else
/// 412 and nothing is written. What is left is the moment between this read and the write.
async fn check_unchanged(sftp: &Sftp, path: &str, wanted: &IfMatch) -> ApiResult<()> {
    let changed = |why: &str| {
        ApiError::new(
            axum::http::StatusCode::PRECONDITION_FAILED,
            "CHANGED",
            format!("{path} {why} since you opened it; nothing was saved"),
        )
    };
    let mut file = match sftp.session.open(path).await {
        Ok(f) => f,
        Err(russh_sftp::client::error::Error::Status(s))
            if s.status_code == SftpStatus::NoSuchFile =>
        {
            return Err(changed("was deleted"));
        }
        Err(e) => return Err(sftp_error(path, e)),
    };
    if *wanted == IfMatch::Any {
        return Ok(());
    }
    let mut current = Vec::new();
    (&mut file)
        .take(TAGGED_MAX_BYTES + 1)
        .read_to_end(&mut current)
        .await
        .map_err(|e| ApiError::bad_request(format!("Reading {path} failed: {e}")))?;
    if current.len() as u64 > TAGGED_MAX_BYTES || !wanted.matches(&entity_tag(&current)) {
        return Err(changed("changed on the server"));
    }
    Ok(())
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

/// Create or overwrite a file with the request body, streamed. The file is written in place
/// (truncated, then written), so its owner, mode, links and the folder's permissions stay as
/// they were. With `If-Match` (the `ETag` a download gave) the file must exist and be
/// unchanged since then, or nothing is written and the answer is 412.
#[utoipa::path(put, path = "/api/v1/servers/{id}/files/content", tag = "files",
    params(("id" = i64, Path), PathQuery,
        ("If-Match" = Option<String>, Header, description = "The file's `ETag` when it was read")),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, body = Uploaded),
        (status = 400, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 412, description = "The file changed since that `ETag`", body = ErrorBody),
    ))]
#[instrument(skip(state, body))]
async fn upload(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<PathQuery>,
    body: Body,
) -> ApiResult<Json<Uploaded>> {
    if q.path.is_empty() {
        return Err(ApiError::bad_request("A destination file path is required"));
    }
    check_path(&q.path).map_err(ApiError::bad_request)?;
    let if_match = match headers.get(header::IF_MATCH) {
        None => None,
        Some(v) => Some(
            v.to_str()
                .ok()
                .and_then(IfMatch::parse)
                .ok_or_else(|| ApiError::bad_request("If-Match is not a list of entity tags"))?,
        ),
    };
    let sftp = open(&state, id).await?;
    let result = async {
        let mut file = match &if_match {
            Some(wanted) => {
                check_unchanged(&sftp, &q.path, wanted).await?;
                sftp.session
                    .open_with_flags(&q.path, OpenFlags::WRITE | OpenFlags::TRUNCATE)
                    .await
            }
            None => sftp.session.create(&q.path).await,
        }
        .map_err(|e| sftp_error(&q.path, e))?;
        let mut reader = StreamReader::new(futures_util::stream::TryStreamExt::map_err(
            body.into_data_stream(),
            std::io::Error::other,
        ));
        let failed =
            |e: std::io::Error| ApiError::bad_request(format!("Upload to {} failed: {e}", q.path));
        let mut hasher = Sha256::new();
        let mut written = 0u64;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).await.map_err(failed)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).await.map_err(failed)?;
            written += n as u64;
        }
        file.sync_all().await.ok();
        file.close()
            .await
            .map_err(|e| ApiError::bad_request(format!("Closing {} failed: {e}", q.path)))?;
        let hex: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok::<_, ApiError>((written, format!("\"{hex}\"")))
    }
    .await;
    let outcome = match result {
        Ok((bytes, etag)) => {
            audit(
                &state,
                me,
                &headers,
                if if_match.is_some() {
                    "files.edit"
                } else {
                    "files.upload"
                },
                &sftp.server,
                json!({ "path": q.path, "bytes": bytes }),
            );
            info!(server = %sftp.server.name, path = %q.path, bytes, "File written");
            Ok(Json(Uploaded {
                path: q.path.clone(),
                bytes,
                etag,
                message: if if_match.is_some() {
                    "Saved"
                } else {
                    "Uploaded"
                }
                .to_string(),
            }))
        }
        Err(e) => Err(e),
    };
    sftp.finish().await;
    outcome
}

/// Make a folder
#[utoipa::path(post, path = "/api/v1/servers/{id}/files/mkdir", tag = "files",
    params(("id" = i64, Path)), request_body = NewFolder,
    responses((status = 200, body = FolderCreated), (status = 400, body = ErrorBody)))]
#[instrument(skip(state))]
async fn mkdir(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<NewFolder>,
) -> ApiResult<Json<FolderCreated>> {
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
    result.map(|_| {
        Json(FolderCreated {
            path: input.path,
            message: "Folder created".to_string(),
        })
    })
}

/// Rename or move; an existing destination is a 409
#[utoipa::path(post, path = "/api/v1/servers/{id}/files/rename", tag = "files",
    params(("id" = i64, Path)), request_body = Rename,
    responses(
        (status = 200, body = Renamed),
        (status = 400, body = ErrorBody),
        (status = 409, description = "The destination exists", body = ErrorBody),
    ))]
#[instrument(skip(state))]
async fn rename(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<Rename>,
) -> ApiResult<Json<Renamed>> {
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
    result.map(|_| {
        Json(Renamed {
            from: input.from,
            to: input.to,
            message: "Renamed".to_string(),
        })
    })
}

/// Delete a file, or an empty directory. Nothing recursive: that belongs in the shell.
#[utoipa::path(delete, path = "/api/v1/servers/{id}/files", tag = "files",
    params(("id" = i64, Path), PathQuery),
    responses(
        (status = 200, body = Deleted),
        (status = 400, body = ErrorBody),
        (status = 409, description = "The directory is not empty", body = ErrorBody),
    ))]
#[instrument(skip(state))]
async fn remove(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<PathQuery>,
) -> ApiResult<Json<Deleted>> {
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
    result.map(|kind| {
        Json(Deleted {
            path: q.path,
            kind: kind.to_string(),
            message: "Deleted".to_string(),
        })
    })
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
            kind: kind.to_string(),
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
    fn entity_tags_are_the_quoted_sha256() {
        assert_eq!(
            entity_tag(b""),
            "\"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\""
        );
    }

    #[test]
    fn if_match_takes_star_or_a_list_and_compares_strongly() {
        assert_eq!(IfMatch::parse(" * "), Some(IfMatch::Any));
        assert_eq!(IfMatch::parse(""), None);
        assert_eq!(IfMatch::parse(" , "), None);
        let list = IfMatch::parse("\"a\", W/\"b\"").unwrap();
        assert!(list.matches("\"a\""));
        assert!(!list.matches("\"b\""), "a weak tag never matches");
        assert!(!list.matches("a"));
        assert!(IfMatch::Any.matches("\"anything\""));
    }

    #[test]
    fn download_names_are_percent_encoded_for_rfc5987() {
        assert_eq!(percent_encode("report.pdf"), "report.pdf");
        assert_eq!(percent_encode("über cool?.txt"), "%C3%BCber%20cool%3F.txt");
    }
}

/// The file routes through the whole app against the loopback SSH server's SFTP, which
/// serves a temporary directory as `/`.
#[cfg(test)]
mod loopback {
    use crate::test_support::app::AppHarness;
    use crate::test_support::ssh_server::{Options, TestSshServer};
    use crate::test_support::{add_credential, add_server, trust_recorded_keys};
    use axum::http::StatusCode;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use svrctlrs_database::models::CredentialType;

    pub(super) struct Remote {
        pub h: AppHarness,
        pub dir: tempfile::TempDir,
        pub sid: i64,
        _ssh: TestSshServer,
    }

    impl Remote {
        pub fn uri(&self, route: &str, path: &str) -> String {
            format!(
                "/api/v1/servers/{}/files{route}?path={}",
                self.sid,
                urlencoding(path)
            )
        }

        pub fn local(&self, path: &str) -> std::path::PathBuf {
            self.dir.path().join(path.trim_start_matches('/'))
        }
    }

    fn urlencoding(path: &str) -> String {
        super::percent_encode(path).replace("%2F", "/")
    }

    /// A server whose SFTP serves a fresh directory; its host key already trusted after the
    /// first call answered 428.
    pub(super) async fn remote() -> Remote {
        let dir = tempfile::tempdir().unwrap();
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let ssh =
            TestSshServer::start(Options::password("alice", "pw").with_sftp(dir.path())).await;
        let pw = add_credential(&pool, "pw", CredentialType::Password, "pw").await;
        let sid = add_server(&pool, "files", ssh.port, "alice", Some(pw), None).await;
        let (status, body) = h
            .owner("GET", &format!("/api/v1/servers/{sid}/files"), None)
            .await;
        assert_eq!(status, StatusCode::PRECONDITION_REQUIRED, "{body}");
        trust_recorded_keys(&pool, sid).await;
        Remote {
            h,
            dir,
            sid,
            _ssh: ssh,
        }
    }

    #[tokio::test]
    async fn list_download_upload_mkdir_rename_delete() {
        let r = remote().await;
        fs::create_dir(r.local("etc")).unwrap();
        fs::write(r.local("b.txt"), "bee\n").unwrap();
        fs::write(r.local("A.txt"), "ay\n").unwrap();

        // Home is the served root; directories first, then names ignoring case
        let (status, listing) = r.h.owner("GET", &r.uri("", ""), None).await;
        assert_eq!(status, StatusCode::OK, "{listing}");
        assert_eq!(listing["path"], "/");
        assert_eq!(listing["parent"], json!(null));
        let names: Vec<_> = listing["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["name"].as_str().unwrap(), e["kind"].as_str().unwrap()))
            .collect();
        assert_eq!(
            names,
            [("etc", "dir"), ("A.txt", "file"), ("b.txt", "file")]
        );

        // Download: the bytes, as an attachment
        let (status, headers, bytes) =
            r.h.raw("GET", &r.uri("/content", "/b.txt"), &[], vec![])
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bytes, b"bee\n");
        assert!(headers["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("attachment"));
        let (status, _, _) =
            r.h.raw("GET", &r.uri("/content", "/missing"), &[], vec![])
                .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Upload creates and overwrites
        let (status, _, body) =
            r.h.raw(
                "PUT",
                &r.uri("/content", "/etc/new.conf"),
                &[],
                b"x=1\n".to_vec(),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        assert_eq!(fs::read(r.local("etc/new.conf")).unwrap(), b"x=1\n");

        // A file the login may not write: 403
        fs::set_permissions(r.local("A.txt"), fs::Permissions::from_mode(0o444)).unwrap();
        let (status, _, _) =
            r.h.raw("PUT", &r.uri("/content", "/A.txt"), &[], b"no".to_vec())
                .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(fs::read(r.local("A.txt")).unwrap(), b"ay\n");

        // Folder, rename (never over an existing name), delete
        let sid = r.sid;
        let (status, _) =
            r.h.owner(
                "POST",
                &format!("/api/v1/servers/{sid}/files/mkdir"),
                Some(json!({ "path": "/tmp" })),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert!(r.local("tmp").is_dir());
        let rename = format!("/api/v1/servers/{sid}/files/rename");
        let (status, _) =
            r.h.owner(
                "POST",
                &rename,
                Some(json!({ "from": "/b.txt", "to": "/A.txt" })),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        let (status, _) =
            r.h.owner(
                "POST",
                &rename,
                Some(json!({ "from": "/b.txt", "to": "/tmp/b.txt" })),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert!(r.local("tmp/b.txt").is_file());
        let (status, _) = r.h.owner("DELETE", &r.uri("", "/tmp"), None).await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "a folder with a file in it stays"
        );
        let (status, _) = r.h.owner("DELETE", &r.uri("", "/tmp/b.txt"), None).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = r.h.owner("DELETE", &r.uri("", "/tmp"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!r.local("tmp").exists());

        // Relative paths are refused before any connection
        let (status, _) = r.h.owner("GET", &r.uri("", "etc"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    async fn audit_actions(r: &Remote) -> Vec<String> {
        let (_, audit) = r.h.owner("GET", "/api/v1/audit?limit=100", None).await;
        audit["entries"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| e["action"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn a_save_with_if_match_is_refused_once_the_file_changed() {
        let r = remote().await;
        let original = b"user nginx;\r\nworker_processes 2;\r\n".to_vec();
        fs::write(r.local("nginx.conf"), &original).unwrap();
        let uri = r.uri("/content", "/nginx.conf");

        // Opened in the viewer: the bytes as they are, with their tag
        let view = format!("{uri}&view=true");
        let (status, headers, bytes) = r.h.raw("GET", &view, &[], vec![]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bytes, original);
        let tag = headers["etag"].to_str().unwrap().to_string();
        assert_eq!(tag, super::entity_tag(&original));

        // Saved with that tag: written in place, the new tag comes back
        let edited = b"user www-data;\r\nworker_processes 2;\r\n".to_vec();
        let (status, _, body) =
            r.h.raw("PUT", &uri, &[("if-match", &tag)], edited.clone())
                .await;
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(saved["etag"], super::entity_tag(&edited));
        assert_eq!(saved["message"], "Saved");
        assert_eq!(fs::read(r.local("nginx.conf")).unwrap(), edited);

        // The old tag again: refused, nothing written
        let (status, _, body) =
            r.h.raw("PUT", &uri, &[("if-match", &tag)], b"stale".to_vec())
                .await;
        assert_eq!(status, StatusCode::PRECONDITION_FAILED);
        assert!(String::from_utf8_lossy(&body).contains("CHANGED"));
        assert_eq!(fs::read(r.local("nginx.conf")).unwrap(), edited);

        // Changed on the server behind the editor's back, same size: still refused
        let behind = b"user www-root;\r\nworker_processes 2;\r\n".to_vec();
        assert_eq!(behind.len(), edited.len());
        fs::write(r.local("nginx.conf"), &behind).unwrap();
        let current = saved["etag"].as_str().unwrap();
        let (status, _, _) =
            r.h.raw("PUT", &uri, &[("if-match", current)], b"mine".to_vec())
                .await;
        assert_eq!(status, StatusCode::PRECONDITION_FAILED);
        assert_eq!(fs::read(r.local("nginx.conf")).unwrap(), behind);

        // Deleted since: refused, and not created again
        let gone = r.uri("/content", "/gone.txt");
        let (status, _, _) =
            r.h.raw("PUT", &gone, &[("if-match", current)], b"x".to_vec())
                .await;
        assert_eq!(status, StatusCode::PRECONDITION_FAILED);
        assert!(!r.local("gone.txt").exists());

        // Not an entity tag at all
        let (status, _, _) = r.h.raw("PUT", &uri, &[("if-match", " ")], vec![]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // The audit tells a look, a download and an edit apart
        let (status, _, _) = r.h.raw("GET", &uri, &[], vec![]).await;
        assert_eq!(status, StatusCode::OK);
        let r = &r;
        assert!(
            crate::test_support::app::eventually(|| async move {
                let actions = audit_actions(r).await;
                ["files.view", "files.download", "files.edit"]
                    .iter()
                    .all(|a| actions.iter().any(|x| x == a))
            })
            .await,
            "{:?}",
            audit_actions(r).await
        );
    }

    #[tokio::test]
    async fn large_files_have_no_tag_and_edits_keep_links_and_permissions() {
        let r = remote().await;
        // Past the editor's limit: streamed whole, without a tag; a tagged save is refused
        let big = vec![b'x'; super::TAGGED_MAX_BYTES as usize + 10];
        fs::write(r.local("big.log"), &big).unwrap();
        let uri = r.uri("/content", "/big.log");
        let (status, headers, bytes) = r.h.raw("GET", &uri, &[], vec![]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bytes.len(), big.len());
        assert!(headers.get("etag").is_none());
        let tag = super::entity_tag(&big);
        let (status, _, _) =
            r.h.raw("PUT", &uri, &[("if-match", &tag)], b"x".to_vec())
                .await;
        assert_eq!(status, StatusCode::PRECONDITION_FAILED);

        // Through a symlink: the target changes, the link stays a link
        fs::write(r.local("real.conf"), b"a=1\n").unwrap();
        std::os::unix::fs::symlink(r.local("real.conf"), r.local("link.conf")).unwrap();
        let link = r.uri("/content", "/link.conf");
        let (_, headers, _) = r.h.raw("GET", &link, &[], vec![]).await;
        let tag = headers["etag"].to_str().unwrap().to_string();
        let (status, _, _) =
            r.h.raw("PUT", &link, &[("if-match", &tag)], b"a=2\n".to_vec())
                .await;
        assert_eq!(status, StatusCode::OK);
        assert!(fs::symlink_metadata(r.local("link.conf"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(r.local("real.conf")).unwrap(), b"a=2\n");

        // A file the login may not write: 403 after the check, and untouched
        fs::write(r.local("ro.conf"), b"keep\n").unwrap();
        fs::set_permissions(r.local("ro.conf"), fs::Permissions::from_mode(0o444)).unwrap();
        let ro = r.uri("/content", "/ro.conf");
        let (_, headers, _) = r.h.raw("GET", &ro, &[], vec![]).await;
        let tag = headers["etag"].to_str().unwrap().to_string();
        let (status, _, _) =
            r.h.raw("PUT", &ro, &[("if-match", &tag)], b"no\n".to_vec())
                .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(fs::read(r.local("ro.conf")).unwrap(), b"keep\n");
        assert_eq!(
            fs::metadata(r.local("ro.conf"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
    }
}
