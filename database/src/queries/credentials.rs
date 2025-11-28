//! Credential database queries

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{Credential, CreateCredential, UpdateCredential};

/// List all credentials
#[instrument(skip(pool))]
pub async fn list_credentials(pool: &Pool<Sqlite>) -> Result<Vec<Credential>> {
    sqlx::query_as::<_, Credential>(
        r#"
        SELECT id, name, credential_type, description, value, username, metadata,
               created_at, updated_at
        FROM credentials
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list credentials")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get credential by ID
#[instrument(skip(pool))]
pub async fn get_credential(pool: &Pool<Sqlite>, id: i64) -> Result<Credential> {
    sqlx::query_as::<_, Credential>(
        r#"
        SELECT id, name, credential_type, description, value, username, metadata,
               created_at, updated_at
        FROM credentials
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get credential")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get credential by name
#[instrument(skip(pool))]
pub async fn get_credential_by_name(pool: &Pool<Sqlite>, name: &str) -> Result<Credential> {
    sqlx::query_as::<_, Credential>(
        r#"
        SELECT id, name, credential_type, description, value, username, metadata,
               created_at, updated_at
        FROM credentials
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("Failed to get credential by name")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new credential
#[instrument(skip(pool, input))]
pub async fn create_credential(pool: &Pool<Sqlite>, input: &CreateCredential) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO credentials (name, credential_type, description, value, username, metadata)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(input.credential_type.as_str())
    .bind(&input.description)
    .bind(&input.value)
    .bind(&input.username)
    .bind(input.metadata_string())
    .execute(pool)
    .await
    .context("Failed to create credential")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing credential
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
        query.push_str(", value = ?");
        params.push(value.clone());
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

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("../migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_credential_lifecycle() {
        let pool = setup_test_db().await;

        // Create credential
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

        // Update credential
        let update = UpdateCredential {
            description: Some("Updated description".to_string()),
            ..Default::default()
        };
        update_credential(&pool, id, &update).await.unwrap();

        let cred = get_credential(&pool, id).await.unwrap();
        assert_eq!(
            cred.description,
            Some("Updated description".to_string())
        );

        // Check not in use
        assert!(!credential_in_use(&pool, id).await.unwrap());

        // Delete credential
        delete_credential(&pool, id).await.unwrap();

        // Verify deleted
        assert!(get_credential(&pool, id).await.is_err());
    }
}
