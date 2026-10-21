use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::FromRow;

/// Longest tag kept, in characters.
pub const MAX_TAG_LEN: usize = 32;
/// Most tags kept per server.
pub const MAX_TAGS: usize = 20;

/// Tags as stored: trimmed, non-empty, clipped to [`MAX_TAG_LEN`], unique ignoring case (the
/// first spelling wins), sorted ignoring case, at most [`MAX_TAGS`].
pub fn normalize_tags<I, S>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag: String = tag.as_ref().trim().chars().take(MAX_TAG_LEN).collect();
        if tag.is_empty() {
            continue;
        }
        if out.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            continue;
        }
        out.push(tag);
        if out.len() == MAX_TAGS {
            break;
        }
    }
    out.sort_by_key(|t| t.to_lowercase());
    out
}

/// An SSH target.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Server {
    pub id: i64,
    pub name: String,
    /// Hostname or IP address
    pub hostname: String,
    /// SSH port
    pub port: i32,
    /// SSH user; `None` means the credential's user, else `root` (see `ssh::target`)
    pub username: Option<String>,
    /// Foreign key to credentials
    pub credential_id: Option<i64>,
    /// Reached through this server's SSH connection (a jump host), when set
    pub via_server_id: Option<i64>,
    /// Free-form labels for filtering; see [`normalize_tags`]
    pub tags: Json<Vec<String>>,
    /// Typed into every new shell right after it starts (one line, without the newline)
    pub startup_command: Option<String>,
    /// Shells opened on this server are recorded for the user who opens them
    pub record_sessions: bool,
    /// New shells run in tmux on the server, so they outlive a restart of SvrCtlRS
    pub persistent_shell: bool,
    pub description: Option<String>,
    pub enabled: bool,

    // Status tracking (health monitor and connection attempts)
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Server {
    /// `user@host[:port]` for logs and messages; the port only when it is not 22.
    pub fn display_address(&self) -> String {
        let user = self.username.as_deref().unwrap_or("root");
        if self.port != 22 {
            format!("{}@{}:{}", user, self.hostname, self.port)
        } else {
            format!("{}@{}", user, self.hostname)
        }
    }
}

/// Input for creating a new server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServer {
    pub name: String,
    pub hostname: String,
    #[serde(default = "default_port")]
    pub port: i32,
    pub username: Option<String>,
    pub credential_id: Option<i64>,
    #[serde(default)]
    pub via_server_id: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub startup_command: Option<String>,
    #[serde(default)]
    pub record_sessions: bool,
    #[serde(default)]
    pub persistent_shell: bool,
    pub description: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// One line of at most 500 characters, trimmed; empty means none.
pub fn validate_startup_command(command: &str) -> Result<Option<String>, String> {
    let command = command.trim();
    if command.is_empty() {
        return Ok(None);
    }
    if command.chars().any(|c| c == '\n' || c == '\r' || c == '\0') {
        return Err("The startup command must be a single line".to_string());
    }
    if command.chars().count() > 500 {
        return Err("The startup command is longer than 500 characters".to_string());
    }
    Ok(Some(command.to_string()))
}

impl CreateServer {
    /// Validate server input
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Server name cannot be empty".to_string());
        }
        if self.hostname.trim().is_empty() {
            return Err("A server needs a hostname".to_string());
        }
        if self.port < 1 || self.port > 65535 {
            return Err(format!("Invalid port number: {}", self.port));
        }
        if let Some(cmd) = &self.startup_command {
            validate_startup_command(cmd)?;
        }
        Ok(())
    }
}

/// Input for updating an existing server
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateServer {
    pub name: Option<String>,
    pub hostname: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    /// `None` keeps the current credential, `Some(None)` unassigns it, `Some(Some(id))` sets it.
    pub credential_id: Option<Option<i64>>,
    /// Same shape: `Some(None)` makes the server direct again.
    pub via_server_id: Option<Option<i64>>,
    /// Replaces the whole list
    pub tags: Option<Vec<String>>,
    /// An empty string clears it
    pub startup_command: Option<String>,
    pub record_sessions: Option<bool>,
    pub persistent_shell: Option<bool>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub last_error: Option<String>,
}

impl UpdateServer {
    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some()
            || self.hostname.is_some()
            || self.port.is_some()
            || self.username.is_some()
            || self.credential_id.is_some()
            || self.via_server_id.is_some()
            || self.tags.is_some()
            || self.startup_command.is_some()
            || self.record_sessions.is_some()
            || self.persistent_shell.is_some()
            || self.description.is_some()
            || self.enabled.is_some()
            || self.last_error.is_some()
    }
}

fn default_port() -> i32 {
    22
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(hostname: &str, username: Option<&str>, port: i32) -> Server {
        Server {
            id: 1,
            name: "web".to_string(),
            hostname: hostname.to_string(),
            port,
            username: username.map(String::from),
            credential_id: None,
            via_server_id: None,
            tags: Json(vec![]),
            startup_command: None,
            record_sessions: false,
            persistent_shell: false,
            description: None,
            enabled: true,
            last_seen_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn display_address_shows_user_host_and_a_non_default_port() {
        assert_eq!(
            server("192.168.1.100", Some("admin"), 22).display_address(),
            "admin@192.168.1.100"
        );
        assert_eq!(
            server("example.com", Some("user"), 2222).display_address(),
            "user@example.com:2222"
        );
        assert_eq!(server("h", None, 22).display_address(), "root@h");
    }

    #[test]
    fn tags_are_trimmed_deduplicated_and_sorted() {
        assert_eq!(
            normalize_tags(["  Prod ", "home", "", "prod", "db", "Home"]),
            ["db", "home", "Prod"]
        );
        let long = "x".repeat(50);
        assert_eq!(normalize_tags([long.as_str()])[0].len(), MAX_TAG_LEN);
        let many: Vec<String> = (0..30).map(|i| format!("t{i:02}")).collect();
        assert_eq!(normalize_tags(&many).len(), MAX_TAGS);
        assert!(normalize_tags(Vec::<String>::new()).is_empty());
    }

    #[test]
    fn startup_command_is_one_trimmed_line_or_nothing() {
        assert_eq!(validate_startup_command("  ").unwrap(), None);
        assert_eq!(
            validate_startup_command(" tmux new -A -s main ").unwrap(),
            Some("tmux new -A -s main".into())
        );
        assert!(validate_startup_command("a\nb").is_err());
        assert!(validate_startup_command(&"x".repeat(501)).is_err());
    }

    #[test]
    fn create_server_validation() {
        let valid = CreateServer {
            name: "remote".to_string(),
            hostname: "192.168.1.100".to_string(),
            port: 22,
            username: Some("admin".to_string()),
            credential_id: Some(1),
            via_server_id: None,
            tags: vec![],
            startup_command: None,
            record_sessions: false,
            persistent_shell: false,
            description: None,
            enabled: true,
        };
        assert!(valid.validate().is_ok());
        assert!(CreateServer {
            startup_command: Some("tmux new -A -s main\nrm -rf /".into()),
            ..valid.clone()
        }
        .validate()
        .is_err());
        assert!(CreateServer {
            hostname: "  ".to_string(),
            ..valid.clone()
        }
        .validate()
        .is_err());
        assert!(CreateServer {
            port: 99999,
            ..valid.clone()
        }
        .validate()
        .is_err());
        assert!(CreateServer {
            name: String::new(),
            ..valid
        }
        .validate()
        .is_err());
    }
}
