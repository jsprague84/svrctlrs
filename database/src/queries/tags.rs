//! Tag database queries

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{CreateTag, Tag, TagWithCount, UpdateTag};

/// List all tags
#[instrument(skip(pool))]
pub async fn list_tags(pool: &Pool<Sqlite>) -> Result<Vec<Tag>> {
    sqlx::query_as::<_, Tag>(
        r#"
        SELECT id, name, color, description, created_at
        FROM tags
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list tags")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get tag by ID
#[instrument(skip(pool))]
pub async fn get_tag(pool: &Pool<Sqlite>, id: i64) -> Result<Tag> {
    sqlx::query_as::<_, Tag>(
        r#"
        SELECT id, name, color, description, created_at
        FROM tags
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get tag")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get all tags with server counts for UI display
#[instrument(skip(pool))]
pub async fn get_tags_with_counts(pool: &Pool<Sqlite>) -> Result<Vec<TagWithCount>> {
    let tags = list_tags(pool).await?;
    let mut result = Vec::new();

    for tag in tags {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) as count
            FROM server_tags
            WHERE tag_id = ?
            "#,
        )
        .bind(tag.id)
        .fetch_one(pool)
        .await
        .context("Failed to get server count for tag")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

        result.push(TagWithCount {
            tag,
            server_count: count.0,
        });
    }

    Ok(result)
}

/// Create a new tag
#[instrument(skip(pool, input))]
pub async fn create_tag(pool: &Pool<Sqlite>, input: &CreateTag) -> Result<i64> {
    // Validate input
    input
        .validate()
        .map_err(|e| Error::DatabaseError(format!("Validation error: {}", e)))?;

    let result = sqlx::query(
        r#"
        INSERT INTO tags (name, color, description)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(&input.color)
    .bind(&input.description)
    .execute(pool)
    .await
    .context("Failed to create tag")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing tag
#[instrument(skip(pool, input))]
pub async fn update_tag(pool: &Pool<Sqlite>, id: i64, input: &UpdateTag) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    // Validate input
    input
        .validate()
        .map_err(|e| Error::DatabaseError(format!("Validation error: {}", e)))?;

    let mut query = String::from("UPDATE tags SET");
    let mut params: Vec<String> = Vec::new();
    let mut first = true;

    if let Some(name) = &input.name {
        if !first {
            query.push(',');
        }
        query.push_str(" name = ?");
        params.push(name.clone());
        first = false;
    }
    if let Some(color) = &input.color {
        if !first {
            query.push(',');
        }
        query.push_str(" color = ?");
        params.push(color.clone());
        first = false;
    }
    if let Some(description) = &input.description {
        if !first {
            query.push(',');
        }
        query.push_str(" description = ?");
        params.push(description.clone());
    }

    query.push_str(" WHERE id = ?");

    let mut q = sqlx::query(&query);
    for param in params {
        q = q.bind(param);
    }
    q = q.bind(id);

    q.execute(pool)
        .await
        .context("Failed to update tag")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a tag (automatically removes server_tags entries due to CASCADE)
#[instrument(skip(pool))]
pub async fn delete_tag(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete tag")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Add a tag to a server
#[instrument(skip(pool))]
pub async fn add_server_tag(pool: &Pool<Sqlite>, server_id: i64, tag_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO server_tags (server_id, tag_id)
        VALUES (?, ?)
        "#,
    )
    .bind(server_id)
    .bind(tag_id)
    .execute(pool)
    .await
    .context("Failed to add tag to server")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Remove a tag from a server
#[instrument(skip(pool))]
pub async fn remove_server_tag(pool: &Pool<Sqlite>, server_id: i64, tag_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM server_tags
        WHERE server_id = ? AND tag_id = ?
        "#,
    )
    .bind(server_id)
    .bind(tag_id)
    .execute(pool)
    .await
    .context("Failed to remove tag from server")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Get all tags for a server
#[instrument(skip(pool))]
pub async fn get_server_tags(pool: &Pool<Sqlite>, server_id: i64) -> Result<Vec<Tag>> {
    sqlx::query_as::<_, Tag>(
        r#"
        SELECT t.id, t.name, t.color, t.description, t.created_at
        FROM tags t
        JOIN server_tags st ON t.id = st.tag_id
        WHERE st.server_id = ?
        ORDER BY t.name
        "#,
    )
    .bind(server_id)
    .fetch_all(pool)
    .await
    .context("Failed to get server tags")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Replace all tags for a server (removes old tags and adds new ones)
#[instrument(skip(pool, tag_ids))]
pub async fn set_server_tags(
    pool: &Pool<Sqlite>,
    server_id: i64,
    tag_ids: &[i64],
) -> Result<()> {
    // Use a transaction to ensure atomicity
    let mut tx = pool
        .begin()
        .await
        .context("Failed to begin transaction")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    // Remove all existing tags
    sqlx::query("DELETE FROM server_tags WHERE server_id = ?")
        .bind(server_id)
        .execute(&mut *tx)
        .await
        .context("Failed to remove existing tags")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    // Add new tags
    for tag_id in tag_ids {
        sqlx::query(
            r#"
            INSERT INTO server_tags (server_id, tag_id)
            VALUES (?, ?)
            "#,
        )
        .bind(server_id)
        .bind(tag_id)
        .execute(&mut *tx)
        .await
        .context("Failed to add new tag")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;
    }

    tx.commit()
        .await
        .context("Failed to commit transaction")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("../migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_tag_lifecycle() {
        let pool = setup_test_db().await;

        // Create tag
        let input = CreateTag {
            name: "prod".to_string(),
            color: Some("#BF616A".to_string()),
            description: Some("Production servers".to_string()),
        };

        let id = create_tag(&pool, &input).await.unwrap();
        assert!(id > 0);

        // Get tag
        let tag = get_tag(&pool, id).await.unwrap();
        assert_eq!(tag.name, "prod");
        assert_eq!(tag.color, Some("#BF616A".to_string()));

        // Update tag
        let update = UpdateTag {
            description: Some("Updated description".to_string()),
            ..Default::default()
        };
        update_tag(&pool, id, &update).await.unwrap();

        let tag = get_tag(&pool, id).await.unwrap();
        assert_eq!(tag.description, Some("Updated description".to_string()));

        // List tags
        let tags = list_tags(&pool).await.unwrap();
        assert!(tags.len() >= 1);

        // Delete tag
        delete_tag(&pool, id).await.unwrap();

        // Verify deleted
        assert!(get_tag(&pool, id).await.is_err());
    }

    #[tokio::test]
    async fn test_server_tags() {
        let pool = setup_test_db().await;

        // Note: Assumes localhost server with id=1 exists from migration
        let server_id = 1;

        // Create tags
        let tag1_input = CreateTag {
            name: "prod".to_string(),
            color: Some("#BF616A".to_string()),
            description: None,
        };
        let tag1_id = create_tag(&pool, &tag1_input).await.unwrap();

        let tag2_input = CreateTag {
            name: "docker".to_string(),
            color: Some("#88C0D0".to_string()),
            description: None,
        };
        let tag2_id = create_tag(&pool, &tag2_input).await.unwrap();

        // Add tags to server
        add_server_tag(&pool, server_id, tag1_id).await.unwrap();
        add_server_tag(&pool, server_id, tag2_id).await.unwrap();

        // Get server tags
        let tags = get_server_tags(&pool, server_id).await.unwrap();
        assert_eq!(tags.len(), 2);

        // Remove one tag
        remove_server_tag(&pool, server_id, tag1_id)
            .await
            .unwrap();

        let tags = get_server_tags(&pool, server_id).await.unwrap();
        assert_eq!(tags.len(), 1);

        // Replace all tags
        set_server_tags(&pool, server_id, &[tag1_id])
            .await
            .unwrap();

        let tags = get_server_tags(&pool, server_id).await.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "prod");
    }
}
