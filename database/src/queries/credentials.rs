//! Credential database queries

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::encryption;
use svrctlrs_core::{Error, Result};
use tracing::{instrument, warn};

use crate::models::{CreateCredential, Credential, UpdateCredential};

/// Decrypt a credential's value if it's marked as encrypted
fn decrypt_credential(mut cred: Credential) -> Result<Credential> {
    if cred.encrypted && encryption::is_initialized() {
        cred.value = encryption::decrypt(&cred.value)?;
    }
    Ok(cred)
}

/// List all credentials (values are decrypted transparently)
#[instrument(skip(pool))]
pub async fn list_credentials(pool: &Pool<Sqlite>) -> Result<Vec<Credential>> {
    let creds = sqlx::query_as::<_, Credential>(
        r#"
        SELECT id, name, credential_type, description, value, username, metadata,
               encrypted, created_at, updated_at
        FROM credentials
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list credentials")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    creds.into_iter().map(decrypt_credential).collect()
}

/// Get credential by ID (value is decrypted transparently)
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

/// Create a new credential (value is encrypted if encryption is initialized)
#[instrument(skip(pool, input))]
pub async fn create_credential(pool: &Pool<Sqlite>, input: &CreateCredential) -> Result<i64> {
    let (stored_value, encrypted) = if encryption::is_initialized() {
        (encryption::encrypt(&input.value)?, true)
    } else {
        warn!("Encryption not initialized — storing credential value in plaintext");
        (input.value.clone(), false)
    };

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

/// Update an existing credential (value is encrypted if changed and encryption is initialized)
#[instrument(skip(pool, input))]
pub async fn update_credential(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateCredential,
) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE credentials SET updated_at = CURRENT_TIMESTAMP");
    let mut params: Vec<String> = Vec::new();

    if let Some(name) = &input.name {
        query.push_str(", name = ?");
        params.push(name.clone());
    }
    if let Some(description) = &input.description {
        query.push_str(", description = ?");
        params.push(description.clone());
    }
    if let Some(value) = &input.value {
        if encryption::is_initialized() {
            let encrypted_value = encryption::encrypt(value)?;
            query.push_str(", value = ?, encrypted = 1");
            params.push(encrypted_value);
        } else {
            query.push_str(", value = ?, encrypted = 0");
            params.push(value.clone());
        }
    }
    if let Some(username) = &input.username {
        query.push_str(", username = ?");
        params.push(username.clone());
    }
    if let Some(metadata) = input.metadata_string() {
        query.push_str(", metadata = ?");
        params.push(metadata);
    }

    query.push_str(" WHERE id = ?");

    let mut q = sqlx::query(&query);
    for param in params {
        q = q.bind(param);
    }
    q = q.bind(id);

    q.execute(pool)
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
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_credential_lifecycle() {
        let pool = setup_test_db().await;

        // Create credential (without encryption initialized, stores plaintext)
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

        // Get credential
        let cred = get_credential(&pool, id).await.unwrap();
        assert_eq!(cred.name, "test-key");
        assert_eq!(cred.value, "/path/to/key");
        assert!(!cred.encrypted);

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
    async fn test_credential_encrypted_flag_default() {
        let pool = setup_test_db().await;

        let input = CreateCredential {
            name: "unencrypted-cred".to_string(),
            credential_type: CredentialType::Password,
            description: None,
            value: "my-password".to_string(),
            username: Some("admin".to_string()),
            metadata: None,
        };

        let id = create_credential(&pool, &input).await.unwrap();
        let cred = get_credential(&pool, id).await.unwrap();

        // Without encryption initialized, should not be encrypted
        assert!(!cred.encrypted);
        assert_eq!(cred.value, "my-password");
    }
}
