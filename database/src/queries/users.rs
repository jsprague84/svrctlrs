//! User account database queries

use anyhow::Context;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{CreateUser, User};

/// Create a new user with argon2-hashed password
#[instrument(skip(pool, input))]
pub async fn create_user(pool: &Pool<Sqlite>, input: &CreateUser) -> Result<i64> {
    let password_hash = hash_password(&input.password)?;

    let result = sqlx::query(
        r#"
        INSERT INTO users (username, password_hash)
        VALUES (?, ?)
        "#,
    )
    .bind(&input.username)
    .bind(&password_hash)
    .execute(pool)
    .await
    .context("Failed to create user")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Get a user by username
#[instrument(skip(pool))]
pub async fn get_user_by_username(pool: &Pool<Sqlite>, username: &str) -> Result<Option<User>> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, created_at, updated_at
        FROM users
        WHERE username = ?
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .context("Failed to get user by username")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Verify a plaintext password against a user's stored argon2 hash
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|e| Error::Other(format!("Invalid password hash: {}", e)))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Hash a plaintext password using argon2
fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| Error::Other(format!("Failed to hash password: {}", e)))?;

    Ok(password_hash.to_string())
}

/// Count total users in the system
#[instrument(skip(pool))]
pub async fn count_users(pool: &Pool<Sqlite>) -> Result<i64> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .context("Failed to count users")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_create_and_get_user() {
        let pool = setup_test_db().await;

        let input = CreateUser {
            username: "admin".to_string(),
            password: "test_password_123".to_string(),
        };

        let id = create_user(&pool, &input).await.unwrap();
        assert!(id > 0);

        let user = get_user_by_username(&pool, "admin").await.unwrap();
        assert!(user.is_some());
        let user = user.unwrap();
        assert_eq!(user.username, "admin");
        assert_ne!(user.password_hash, "test_password_123"); // Should be hashed
    }

    #[tokio::test]
    async fn test_verify_password() {
        let pool = setup_test_db().await;

        let input = CreateUser {
            username: "testuser".to_string(),
            password: "correct_password".to_string(),
        };

        create_user(&pool, &input).await.unwrap();

        let user = get_user_by_username(&pool, "testuser")
            .await
            .unwrap()
            .unwrap();

        assert!(verify_password("correct_password", &user.password_hash).unwrap());
        assert!(!verify_password("wrong_password", &user.password_hash).unwrap());
    }

    #[tokio::test]
    async fn test_get_nonexistent_user() {
        let pool = setup_test_db().await;

        let user = get_user_by_username(&pool, "nonexistent").await.unwrap();
        assert!(user.is_none());
    }

    #[tokio::test]
    async fn test_count_users() {
        let pool = setup_test_db().await;

        assert_eq!(count_users(&pool).await.unwrap(), 0);

        let input = CreateUser {
            username: "user1".to_string(),
            password: "password1".to_string(),
        };
        create_user(&pool, &input).await.unwrap();

        assert_eq!(count_users(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_duplicate_username_fails() {
        let pool = setup_test_db().await;

        let input = CreateUser {
            username: "admin".to_string(),
            password: "password1".to_string(),
        };
        create_user(&pool, &input).await.unwrap();

        let input2 = CreateUser {
            username: "admin".to_string(),
            password: "password2".to_string(),
        };
        assert!(create_user(&pool, &input2).await.is_err());
    }
}
