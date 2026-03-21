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

/// Get a single quick command by ID
pub async fn get_quick_command(pool: &Pool<Sqlite>, id: i64) -> anyhow::Result<QuickCommand> {
    sqlx::query_as::<_, QuickCommand>(
        r#"
        SELECT id, user_id, name, command, server_id, category, sort_order, created_at, updated_at
        FROM quick_commands
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get quick command")
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

/// Update an existing quick command
pub async fn update_quick_command(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateQuickCommand,
) -> anyhow::Result<()> {
    if !input.has_changes() {
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

    qb.build()
        .execute(pool)
        .await
        .context("Failed to update quick command")?;

    Ok(())
}

/// Delete a quick command
pub async fn delete_quick_command(pool: &Pool<Sqlite>, id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM quick_commands WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete quick command")?;

    Ok(())
}
