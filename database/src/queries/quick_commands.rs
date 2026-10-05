//! Quick command database queries

use anyhow::Context;
use sqlx::{Pool, QueryBuilder, Sqlite};

use crate::models::{CreateQuickCommand, QuickCommand, UpdateQuickCommand};

/// List all quick commands for a user
pub async fn list_quick_commands(
    pool: &Pool<Sqlite>,
    user_id: i64,
) -> anyhow::Result<Vec<QuickCommand>> {
    sqlx::query_as::<_, QuickCommand>(
        r#"
        SELECT id, user_id, name, command, server_id, category, sort_order, created_at, updated_at
        FROM quick_commands
        WHERE user_id = ?
        ORDER BY category, sort_order, name
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list quick commands")
}

/// List quick commands filtered by server
pub async fn list_quick_commands_for_server(
    pool: &Pool<Sqlite>,
    user_id: i64,
    server_id: i64,
) -> anyhow::Result<Vec<QuickCommand>> {
    sqlx::query_as::<_, QuickCommand>(
        r#"
        SELECT id, user_id, name, command, server_id, category, sort_order, created_at, updated_at
        FROM quick_commands
        WHERE user_id = ? AND (server_id = ? OR server_id IS NULL)
        ORDER BY category, sort_order, name
        "#,
    )
    .bind(user_id)
    .bind(server_id)
    .fetch_all(pool)
    .await
    .context("Failed to list quick commands for server")
}

/// Get a single quick command by ID, scoped to its owner. Another user's command is
/// `svrctlrs_core::Error::NotFound`, never "forbidden", so ids cannot be probed.
pub async fn get_quick_command(
    pool: &Pool<Sqlite>,
    id: i64,
    user_id: i64,
) -> anyhow::Result<QuickCommand> {
    sqlx::query_as::<_, QuickCommand>(
        r#"
        SELECT id, user_id, name, command, server_id, category, sort_order, created_at, updated_at
        FROM quick_commands
        WHERE id = ? AND user_id = ?
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get quick command")?
    .ok_or_else(|| svrctlrs_core::Error::NotFound(format!("quick command {id}")).into())
}

/// Create a new quick command
pub async fn create_quick_command(
    pool: &Pool<Sqlite>,
    user_id: i64,
    input: &CreateQuickCommand,
) -> anyhow::Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO quick_commands (user_id, name, command, server_id, category, sort_order)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(&input.name)
    .bind(&input.command)
    .bind(input.server_id)
    .bind(input.category.as_deref().unwrap_or("general"))
    .bind(input.sort_order.unwrap_or(0))
    .execute(pool)
    .await
    .context("Failed to create quick command")?;

    Ok(result.last_insert_rowid())
}

/// Update a quick command owned by `user_id`. `svrctlrs_core::Error::NotFound` when no such
/// row exists for this user.
pub async fn update_quick_command(
    pool: &Pool<Sqlite>,
    id: i64,
    user_id: i64,
    input: &UpdateQuickCommand,
) -> anyhow::Result<()> {
    if !input.has_changes() {
        // Still report a foreign or missing id as not found.
        get_quick_command(pool, id, user_id).await?;
        return Ok(());
    }

    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("UPDATE quick_commands SET updated_at = CURRENT_TIMESTAMP");

    if let Some(name) = &input.name {
        qb.push(", name = ").push_bind(name.clone());
    }
    if let Some(command) = &input.command {
        qb.push(", command = ").push_bind(command.clone());
    }
    if let Some(server_id) = input.server_id {
        qb.push(", server_id = ").push_bind(server_id);
    }
    if let Some(category) = &input.category {
        qb.push(", category = ").push_bind(category.clone());
    }
    if let Some(sort_order) = input.sort_order {
        qb.push(", sort_order = ").push_bind(sort_order);
    }

    qb.push(" WHERE id = ").push_bind(id);
    qb.push(" AND user_id = ").push_bind(user_id);

    let result = qb
        .build()
        .execute(pool)
        .await
        .context("Failed to update quick command")?;
    if result.rows_affected() == 0 {
        return Err(svrctlrs_core::Error::NotFound(format!("quick command {id}")).into());
    }

    Ok(())
}

/// Delete a quick command owned by `user_id`.
pub async fn delete_quick_command(
    pool: &Pool<Sqlite>,
    id: i64,
    user_id: i64,
) -> anyhow::Result<()> {
    let result = sqlx::query("DELETE FROM quick_commands WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to delete quick command")?;
    if result.rows_affected() == 0 {
        return Err(svrctlrs_core::Error::NotFound(format!("quick command {id}")).into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        // Owners referenced by user_id (foreign key to users).
        sqlx::query("INSERT INTO users (username, password_hash) VALUES ('u1', 'x'), ('u2', 'x')")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn is_not_found(e: &anyhow::Error) -> bool {
        matches!(
            e.downcast_ref::<svrctlrs_core::Error>(),
            Some(svrctlrs_core::Error::NotFound(_))
        )
    }

    #[tokio::test]
    async fn quick_commands_are_scoped_to_their_owner() {
        let pool = setup().await;
        let input = CreateQuickCommand {
            name: "uptime".to_string(),
            command: "uptime".to_string(),
            server_id: None,
            category: None,
            sort_order: None,
        };
        let id = create_quick_command(&pool, 1, &input).await.unwrap();

        assert_eq!(
            get_quick_command(&pool, id, 1).await.unwrap().name,
            "uptime"
        );
        assert!(is_not_found(
            &get_quick_command(&pool, id, 2).await.unwrap_err()
        ));

        let rename = UpdateQuickCommand {
            name: Some("renamed".to_string()),
            ..Default::default()
        };
        assert!(is_not_found(
            &update_quick_command(&pool, id, 2, &rename)
                .await
                .unwrap_err()
        ));
        assert!(is_not_found(
            &delete_quick_command(&pool, id, 2).await.unwrap_err()
        ));
        assert_eq!(list_quick_commands(&pool, 2).await.unwrap().len(), 0);

        update_quick_command(&pool, id, 1, &rename).await.unwrap();
        assert_eq!(
            get_quick_command(&pool, id, 1).await.unwrap().name,
            "renamed"
        );
        delete_quick_command(&pool, id, 1).await.unwrap();
        assert!(is_not_found(
            &get_quick_command(&pool, id, 1).await.unwrap_err()
        ));
    }
}
