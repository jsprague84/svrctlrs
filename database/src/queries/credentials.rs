//! Credential database queries

use anyhow::Context;
use sqlx::{Pool, QueryBuilder, Sqlite};
use svrctlrs_core::encryption;
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{CreateCredential, Credential, UpdateCredential};

/// Decrypt a credential's value if it's marked as encrypted.
///
/// Rows written before 3.0 may be plaintext (`encrypted = 0`); they are returned as-is and
/// are re-encrypted at server start-up by [`encrypt_plaintext_credentials`].
fn decrypt_credential(mut cred: Credential) -> Result<Credential> {
    if cred.encrypted {
        require_encryption()?;
        cred.value = encryption::decrypt(&cred.value)?;
    }
    Ok(cred)
}

/// Storing or reading a secret without an initialised key is a configuration error, never a
/// silent downgrade to plaintext.
fn require_encryption() -> Result<()> {
    if encryption::is_initialized() {
        Ok(())
    } else {
        Err(Error::ConfigError(
            "credential encryption is not initialised; refusing to handle a secret in plaintext"
                .to_string(),
        ))
    }
}

/// Encrypt every credential still stored in plaintext (`encrypted = 0`).
///
/// Runs at server start-up so a database that once ran without a key does not keep secrets
/// readable on disk. Returns the number of rows encrypted. All rows are updated in one
/// transaction; a failure leaves the table untouched.
#[instrument(skip(pool))]
pub async fn encrypt_plaintext_credentials(pool: &Pool<Sqlite>) -> Result<usize> {
    require_encryption()?;

    let mut tx = pool
        .begin()
        .await
        .context("Failed to begin transaction")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, value FROM credentials WHERE encrypted = 0")
            .fetch_all(&mut *tx)
            .await
            .context("Failed to list plaintext credentials")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

    for (id, value) in &rows {
        let ciphertext = encryption::encrypt(value)?;
        sqlx::query(
            "UPDATE credentials SET value = ?, encrypted = 1, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ? AND encrypted = 0",
        )
        .bind(ciphertext)
        .bind(id)
        .execute(&mut *tx)
        .await
        .context("Failed to encrypt credential")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;
    }

    tx.commit()
        .await
        .context("Failed to commit credential encryption")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(rows.len())
}

/// List all credentials with values redacted (skips decryption for efficiency).
///
/// The `value` field is set to `"[HAS_VALUE]"` if the credential has a value,
/// or `""` if it is empty. This allows callers to check `!c.value.is_empty()`
/// without exposing the actual secret.
///
/// Use `get_credential()` when you need the actual decrypted value.
#[instrument(skip(pool))]
pub async fn list_credentials(pool: &Pool<Sqlite>) -> Result<Vec<Credential>> {
    let creds = sqlx::query_as::<_, Credential>(
        r#"
        SELECT id, name, credential_type, description,
               CASE WHEN length(value) > 0 THEN '[REDACTED]' ELSE '' END AS value,
               username, metadata, encrypted, created_at, updated_at
        FROM credentials
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list credentials")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(creds)
}

/// Get credential by ID (value is decrypted transparently via `decrypt_credential()`).
///
/// Use this when the actual credential value is needed (e.g., SSH connections).
/// For listing/display purposes, use `list_credentials()` instead.
#[instrument(skip(pool))]
pub async fn get_credential(pool: &Pool<Sqlite>, id: i64) -> Result<Credential> {
    let cred = sqlx::query_as::<_, Credential>(
        r#"
        SELECT id, name, credential_type, description, value, username, metadata,
               encrypted, created_at, updated_at
        FROM credentials
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get credential")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    decrypt_credential(cred)
}

/// Get credential by name (value is decrypted transparently)
#[instrument(skip(pool))]
pub async fn get_credential_by_name(pool: &Pool<Sqlite>, name: &str) -> Result<Credential> {
    let cred = sqlx::query_as::<_, Credential>(
        r#"
        SELECT id, name, credential_type, description, value, username, metadata,
               encrypted, created_at, updated_at
        FROM credentials
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("Failed to get credential by name")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    decrypt_credential(cred)
}

/// Create a new credential; the value is always stored encrypted.
#[instrument(skip(pool, input))]
pub async fn create_credential(pool: &Pool<Sqlite>, input: &CreateCredential) -> Result<i64> {
    require_encryption()?;
    let stored_value = encryption::encrypt(&input.value)?;
    let encrypted = true;

    let result = sqlx::query(
        r#"
        INSERT INTO credentials (name, credential_type, description, value, username, metadata, encrypted)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(input.credential_type.as_str())
    .bind(&input.description)
    .bind(&stored_value)
    .bind(&input.username)
    .bind(input.metadata_string())
    .bind(encrypted)
    .execute(pool)
    .await
    .context("Failed to create credential")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing credential; a changed value is always stored encrypted.
#[instrument(skip(pool, input))]
pub async fn update_credential(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateCredential,
) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("UPDATE credentials SET updated_at = CURRENT_TIMESTAMP");

    if let Some(name) = &input.name {
        qb.push(", name = ").push_bind(name.clone());
    }
    if let Some(description) = &input.description {
        qb.push(", description = ").push_bind(description.clone());
    }
    if let Some(value) = &input.value {
        require_encryption()?;
        let encrypted_value = encryption::encrypt(value)?;
        qb.push(", value = ").push_bind(encrypted_value);
        qb.push(", encrypted = ").push_bind(true);
    }
    if let Some(username) = &input.username {
        qb.push(", username = ").push_bind(username.clone());
    }
    if let Some(metadata) = input.metadata_string() {
        qb.push(", metadata = ").push_bind(metadata);
    }

    qb.push(" WHERE id = ").push_bind(id);

    qb.build()
        .execute(pool)
        .await
        .context("Failed to update credential")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a credential (checks if in use first)
#[instrument(skip(pool))]
pub async fn delete_credential(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    // Check if credential is in use by any servers
    if credential_in_use(pool, id).await? {
        return Err(Error::DatabaseError(
            "Cannot delete credential: it is in use by one or more servers".to_string(),
        ));
    }

    sqlx::query("DELETE FROM credentials WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete credential")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Check if a credential is referenced by any servers
#[instrument(skip(pool))]
pub async fn credential_in_use(pool: &Pool<Sqlite>, id: i64) -> Result<bool> {
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM servers
        WHERE credential_id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to check if credential is in use")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count.0 > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CredentialType;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> Pool<Sqlite> {
        init_test_encryption();
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    /// The key is a process-wide `OnceLock`, so every test initialises it the same way and
    /// the second and later calls are expected to report "already initialized".
    fn init_test_encryption() {
        if !encryption::is_initialized() {
            std::env::set_var(
                "ENCRYPTION_KEY",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            );
            let _ = encryption::init_encryption_key();
        }
    }

    async fn raw_row(pool: &Pool<Sqlite>, id: i64) -> (String, bool) {
        sqlx::query_as("SELECT value, encrypted FROM credentials WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_credential_lifecycle() {
        let pool = setup_test_db().await;

        let input = CreateCredential {
            name: "test-key".to_string(),
            credential_type: CredentialType::SshKey,
            description: Some("Test SSH key".to_string()),
            value: "/path/to/key".to_string(),
            username: None,
            metadata: None,
        };

        let id = create_credential(&pool, &input).await.unwrap();
        assert!(id > 0);

        // Read back decrypted; on disk it is ciphertext and flagged
        let cred = get_credential(&pool, id).await.unwrap();
        assert_eq!(cred.name, "test-key");
        assert_eq!(cred.value, "/path/to/key");
        assert!(cred.encrypted);
        let (stored, flagged) = raw_row(&pool, id).await;
        assert!(flagged);
        assert_ne!(stored, "/path/to/key");

        // Update credential
        let update = UpdateCredential {
            description: Some("Updated description".to_string()),
            ..Default::default()
        };
        update_credential(&pool, id, &update).await.unwrap();

        let cred = get_credential(&pool, id).await.unwrap();
        assert_eq!(cred.description, Some("Updated description".to_string()));

        // Check not in use
        assert!(!credential_in_use(&pool, id).await.unwrap());

        // Delete credential
        delete_credential(&pool, id).await.unwrap();

        // Verify deleted
        assert!(get_credential(&pool, id).await.is_err());
    }

    #[tokio::test]
    async fn test_update_value_is_encrypted() {
        let pool = setup_test_db().await;

        let input = CreateCredential {
            name: "pw".to_string(),
            credential_type: CredentialType::Password,
            description: None,
            value: "my-password".to_string(),
            username: Some("admin".to_string()),
            metadata: None,
        };
        let id = create_credential(&pool, &input).await.unwrap();

        let update = UpdateCredential {
            value: Some("new-password".to_string()),
            ..Default::default()
        };
        update_credential(&pool, id, &update).await.unwrap();

        let cred = get_credential(&pool, id).await.unwrap();
        assert_eq!(cred.value, "new-password");
        assert!(cred.encrypted);
        let (stored, flagged) = raw_row(&pool, id).await;
        assert!(flagged);
        assert_ne!(stored, "new-password");
    }

    #[tokio::test]
    async fn test_encrypt_plaintext_credentials_migrates_legacy_rows() {
        let pool = setup_test_db().await;

        // Two rows as a pre-3.0 server without a key would have written them, one empty.
        for (name, value) in [("legacy-key", "-----BEGIN KEY-----"), ("legacy-empty", "")] {
            sqlx::query(
                "INSERT INTO credentials (name, credential_type, value, encrypted) \
                 VALUES (?, 'ssh_key', ?, 0)",
            )
            .bind(name)
            .bind(value)
            .execute(&pool)
            .await
            .unwrap();
        }
        // And one already-encrypted row that must be left alone.
        let input = CreateCredential {
            name: "modern".to_string(),
            credential_type: CredentialType::Password,
            description: None,
            value: "secret".to_string(),
            username: None,
            metadata: None,
        };
        let modern_id = create_credential(&pool, &input).await.unwrap();
        let (modern_stored_before, _) = raw_row(&pool, modern_id).await;

        assert_eq!(encrypt_plaintext_credentials(&pool).await.unwrap(), 2);

        let legacy = get_credential_by_name(&pool, "legacy-key").await.unwrap();
        assert_eq!(legacy.value, "-----BEGIN KEY-----");
        assert!(legacy.encrypted);
        let (stored, flagged) = raw_row(&pool, legacy.id).await;
        assert!(flagged);
        assert_ne!(stored, "-----BEGIN KEY-----");

        let empty = get_credential_by_name(&pool, "legacy-empty").await.unwrap();
        assert_eq!(empty.value, "");
        assert!(empty.encrypted);

        let (modern_stored_after, _) = raw_row(&pool, modern_id).await;
        assert_eq!(modern_stored_before, modern_stored_after);

        // Second run finds nothing to do.
        assert_eq!(encrypt_plaintext_credentials(&pool).await.unwrap(), 0);
    }
}
