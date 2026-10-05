//! Terminal profile queries

use anyhow::{Context, Result};
use sqlx::{Pool, QueryBuilder, Sqlite};

use crate::models::{CreateTerminalProfile, TerminalProfile, UpdateTerminalProfile};

/// List all terminal profiles (optionally filtered by user)
pub async fn list_terminal_profiles(pool: &Pool<Sqlite>) -> Result<Vec<TerminalProfile>> {
    let profiles = sqlx::query_as::<_, TerminalProfile>(
        r#"
        SELECT id, name, description, layout, pane_configs, quick_commands, is_default, user_id,
               created_at, updated_at
        FROM terminal_profiles
        ORDER BY is_default DESC, name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(profiles)
}

/// List terminal profiles for a specific user
pub async fn list_terminal_profiles_for_user(
    pool: &Pool<Sqlite>,
    user_id: i64,
) -> Result<Vec<TerminalProfile>> {
    let profiles = sqlx::query_as::<_, TerminalProfile>(
        r#"
        SELECT id, name, description, layout, pane_configs, quick_commands, is_default, user_id,
               created_at, updated_at
        FROM terminal_profiles
        WHERE user_id = ? OR user_id IS NULL
        ORDER BY is_default DESC, name ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(profiles)
}

/// Get a terminal profile by ID, as seen by `user_id`.
///
/// Profiles are per user; rows with a NULL `user_id` predate accounts and are visible to
/// everyone, matching [`list_terminal_profiles_for_user`]. Another user's profile is "not
/// found", never "forbidden", so ids cannot be probed.
pub async fn get_terminal_profile(
    pool: &Pool<Sqlite>,
    id: i64,
    user_id: i64,
) -> Result<TerminalProfile> {
    sqlx::query_as::<_, TerminalProfile>(
        r#"
        SELECT id, name, description, layout, pane_configs, quick_commands, is_default, user_id,
               created_at, updated_at
        FROM terminal_profiles
        WHERE id = ? AND (user_id = ? OR user_id IS NULL)
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        anyhow::Error::from(svrctlrs_core::Error::NotFound(format!(
            "terminal profile {id}"
        )))
    })
}

/// Get the default terminal profile (optionally for a specific user)
pub async fn get_default_terminal_profile(pool: &Pool<Sqlite>) -> Result<Option<TerminalProfile>> {
    let profile = sqlx::query_as::<_, TerminalProfile>(
        r#"
        SELECT id, name, description, layout, pane_configs, quick_commands, is_default, user_id,
               created_at, updated_at
        FROM terminal_profiles
        WHERE is_default = 1
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(profile)
}

/// Create a new terminal profile
pub async fn create_terminal_profile(
    pool: &Pool<Sqlite>,
    profile: &CreateTerminalProfile,
    user_id: Option<i64>,
) -> Result<i64> {
    // If this profile is set as default, unset any existing default
    if profile.is_default {
        sqlx::query("UPDATE terminal_profiles SET is_default = 0 WHERE is_default = 1")
            .execute(pool)
            .await?;
    }

    let pane_configs_json = profile
        .pane_configs
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("Failed to serialize pane configs")?;
    let quick_commands_json = profile
        .quick_commands
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("Failed to serialize quick commands")?;

    let result = sqlx::query(
        r#"
        INSERT INTO terminal_profiles (name, description, layout, pane_configs, quick_commands, is_default, user_id)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&profile.name)
    .bind(&profile.description)
    .bind(&profile.layout)
    .bind(&pane_configs_json)
    .bind(&quick_commands_json)
    .bind(profile.is_default)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Update a terminal profile owned by (or visible to) `user_id`. Returns `Error::NotFound`
/// when no such row exists for this user.
pub async fn update_terminal_profile(
    pool: &Pool<Sqlite>,
    id: i64,
    user_id: i64,
    update: &UpdateTerminalProfile,
) -> Result<()> {
    // If this profile is being set as default, unset this user's other default
    if update.is_default == Some(true) {
        sqlx::query(
            "UPDATE terminal_profiles SET is_default = 0 \
             WHERE is_default = 1 AND id != ? AND (user_id = ? OR user_id IS NULL)",
        )
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    }

    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("UPDATE terminal_profiles SET updated_at = CURRENT_TIMESTAMP");

    if let Some(name) = &update.name {
        qb.push(", name = ").push_bind(name.clone());
    }
    if let Some(description) = &update.description {
        qb.push(", description = ").push_bind(description.clone());
    }
    if let Some(layout) = &update.layout {
        qb.push(", layout = ").push_bind(layout.clone());
    }
    if let Some(pane_configs) = &update.pane_configs {
        let json =
            serde_json::to_string(pane_configs).context("Failed to serialize pane configs")?;
        qb.push(", pane_configs = ").push_bind(json);
    }
    if let Some(quick_commands) = &update.quick_commands {
        let json =
            serde_json::to_string(quick_commands).context("Failed to serialize quick commands")?;
        qb.push(", quick_commands = ").push_bind(json);
    }
    if let Some(is_default) = update.is_default {
        qb.push(", is_default = ").push_bind(is_default);
    }

    qb.push(" WHERE id = ").push_bind(id);
    qb.push(" AND (user_id = ").push_bind(user_id);
    qb.push(" OR user_id IS NULL)");

    let result = qb.build().execute(pool).await?;
    if result.rows_affected() == 0 {
        return Err(svrctlrs_core::Error::NotFound(format!("terminal profile {id}")).into());
    }

    Ok(())
}

/// Delete a terminal profile owned by (or visible to) `user_id`.
pub async fn delete_terminal_profile(pool: &Pool<Sqlite>, id: i64, user_id: i64) -> Result<()> {
    let result = sqlx::query(
        "DELETE FROM terminal_profiles WHERE id = ? AND (user_id = ? OR user_id IS NULL)",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(svrctlrs_core::Error::NotFound(format!("terminal profile {id}")).into());
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

    fn profile(name: &str) -> CreateTerminalProfile {
        CreateTerminalProfile {
            name: name.to_string(),
            description: None,
            layout: "single".to_string(),
            pane_configs: None,
            quick_commands: None,
            is_default: false,
        }
    }

    fn is_not_found(e: &anyhow::Error) -> bool {
        matches!(
            e.downcast_ref::<svrctlrs_core::Error>(),
            Some(svrctlrs_core::Error::NotFound(_))
        )
    }

    #[tokio::test]
    async fn profiles_are_scoped_to_their_owner() {
        let pool = setup().await;
        let mine = create_terminal_profile(&pool, &profile("mine"), Some(1))
            .await
            .unwrap();
        let shared = create_terminal_profile(&pool, &profile("shared"), None)
            .await
            .unwrap();

        // The owner and a legacy shared (NULL owner) row are visible to user 1 ...
        assert_eq!(
            get_terminal_profile(&pool, mine, 1).await.unwrap().name,
            "mine"
        );
        assert_eq!(
            get_terminal_profile(&pool, shared, 1).await.unwrap().name,
            "shared"
        );
        // ... user 2 sees only the shared row, and the other is "not found", not "forbidden".
        assert!(is_not_found(
            &get_terminal_profile(&pool, mine, 2).await.unwrap_err()
        ));
        assert_eq!(
            get_terminal_profile(&pool, shared, 2).await.unwrap().name,
            "shared"
        );

        let rename = UpdateTerminalProfile {
            name: Some("renamed".to_string()),
            ..Default::default()
        };
        assert!(is_not_found(
            &update_terminal_profile(&pool, mine, 2, &rename)
                .await
                .unwrap_err()
        ));
        assert!(is_not_found(
            &delete_terminal_profile(&pool, mine, 2).await.unwrap_err()
        ));
        assert_eq!(
            get_terminal_profile(&pool, mine, 1).await.unwrap().name,
            "mine"
        );

        update_terminal_profile(&pool, mine, 1, &rename)
            .await
            .unwrap();
        assert_eq!(
            get_terminal_profile(&pool, mine, 1).await.unwrap().name,
            "renamed"
        );
        delete_terminal_profile(&pool, mine, 1).await.unwrap();
        assert!(is_not_found(
            &get_terminal_profile(&pool, mine, 1).await.unwrap_err()
        ));
    }

    #[tokio::test]
    async fn setting_a_default_does_not_touch_other_users() {
        let pool = setup().await;
        let theirs = create_terminal_profile(
            &pool,
            &CreateTerminalProfile {
                is_default: true,
                ..profile("theirs")
            },
            Some(2),
        )
        .await
        .unwrap();
        let mine = create_terminal_profile(&pool, &profile("mine"), Some(1))
            .await
            .unwrap();

        let make_default = UpdateTerminalProfile {
            is_default: Some(true),
            ..Default::default()
        };
        update_terminal_profile(&pool, mine, 1, &make_default)
            .await
            .unwrap();

        assert!(
            get_terminal_profile(&pool, mine, 1)
                .await
                .unwrap()
                .is_default
        );
        assert!(
            get_terminal_profile(&pool, theirs, 2)
                .await
                .unwrap()
                .is_default
        );
    }
}
