//! Session recording queries. Everything a user can reach takes their `user_id` and answers
//! `NotFound` for another user's recording, exactly as for a missing one. The unscoped
//! functions are the server's own housekeeping: finishing a recording, retention, orphans.

use anyhow::Context;
use sqlx::{Pool, QueryBuilder, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{NewRecording, Recording, RecordingStatus};

const COLUMNS: &str = "id, user_id, server_id, server_name, session_id, file_name, cols, rows, \
                       started_at, ended_at, duration_ms, size_bytes, status, end_reason";

/// Register a recording that is about to start. Returns its id.
#[instrument(skip(pool, input), fields(user_id = input.user_id, server = %input.server_name))]
pub async fn create(pool: &Pool<Sqlite>, input: &NewRecording) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO session_recordings (user_id, server_id, server_name, session_id, file_name,
                                        cols, rows)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(input.user_id)
    .bind(input.server_id)
    .bind(&input.server_name)
    .bind(&input.session_id)
    .bind(&input.file_name)
    .bind(input.cols)
    .bind(input.rows)
    .execute(pool)
    .await
    .context("Failed to create recording")
    .map_err(Error::Database)?;
    Ok(result.last_insert_rowid())
}

/// How far a running recording has got, for the list while the shell is still open.
#[instrument(skip(pool))]
pub async fn update_progress(
    pool: &Pool<Sqlite>,
    id: i64,
    duration_ms: i64,
    size_bytes: i64,
) -> Result<()> {
    sqlx::query(
        "UPDATE session_recordings SET duration_ms = ?, size_bytes = ? \
         WHERE id = ? AND status = 'recording'",
    )
    .bind(duration_ms)
    .bind(size_bytes)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to update recording progress")
    .map_err(Error::Database)?;
    Ok(())
}

/// The recording stopped, for good.
#[instrument(skip(pool))]
pub async fn finish(
    pool: &Pool<Sqlite>,
    id: i64,
    status: RecordingStatus,
    duration_ms: i64,
    size_bytes: i64,
    end_reason: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE session_recordings SET status = ?, ended_at = CURRENT_TIMESTAMP, \
         duration_ms = ?, size_bytes = ?, end_reason = ? WHERE id = ?",
    )
    .bind(status.as_str())
    .bind(duration_ms)
    .bind(size_bytes)
    .bind(end_reason)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to finish recording")
    .map_err(Error::Database)?;
    Ok(())
}

/// A user's recordings, newest first, `limit` at a time; pass the smallest `id` of the
/// previous page as `before` to continue. `server_id` narrows it to one server.
#[instrument(skip(pool))]
pub async fn list_for_user(
    pool: &Pool<Sqlite>,
    user_id: i64,
    server_id: Option<i64>,
    limit: i64,
    before: Option<i64>,
) -> Result<Vec<Recording>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(format!(
        "SELECT {COLUMNS} FROM session_recordings WHERE user_id = "
    ));
    qb.push_bind(user_id);
    if let Some(server_id) = server_id {
        qb.push(" AND server_id = ").push_bind(server_id);
    }
    if let Some(before) = before {
        qb.push(" AND id < ").push_bind(before);
    }
    qb.push(" ORDER BY id DESC LIMIT ")
        .push_bind(limit.clamp(1, 500));
    qb.build_query_as::<Recording>()
        .fetch_all(pool)
        .await
        .context("Failed to list recordings")
        .map_err(Error::Database)
}

/// One of the user's recordings. Another user's is `NotFound`, like a missing one.
#[instrument(skip(pool))]
pub async fn get_for_user(pool: &Pool<Sqlite>, id: i64, user_id: i64) -> Result<Recording> {
    sqlx::query_as::<_, Recording>(&format!(
        "SELECT {COLUMNS} FROM session_recordings WHERE id = ? AND user_id = ?"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get recording")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("recording {id}")))
}

/// Remove one of the user's recordings and return the row, so the caller can delete its
/// file. Another user's is `NotFound` and stays.
#[instrument(skip(pool))]
pub async fn delete_for_user(pool: &Pool<Sqlite>, id: i64, user_id: i64) -> Result<Recording> {
    sqlx::query_as::<_, Recording>(&format!(
        "DELETE FROM session_recordings WHERE id = ? AND user_id = ? RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to delete recording")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("recording {id}")))
}

/// Finished recordings that started more than `days` days ago (retention). Running ones are
/// left alone however old.
#[instrument(skip(pool))]
pub async fn older_than_days(pool: &Pool<Sqlite>, days: i64) -> Result<Vec<Recording>> {
    sqlx::query_as::<_, Recording>(&format!(
        "SELECT {COLUMNS} FROM session_recordings \
         WHERE status <> 'recording' AND started_at < datetime('now', ?)"
    ))
    .bind(format!("-{} days", days.max(0)))
    .fetch_all(pool)
    .await
    .context("Failed to list expired recordings")
    .map_err(Error::Database)
}

/// Remove a recording's row (housekeeping; the caller deletes the file).
#[instrument(skip(pool))]
pub async fn delete(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM session_recordings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete recording")
        .map_err(Error::Database)?;
    Ok(())
}

/// At start-up: whatever was still recording when the server stopped will never finish.
#[instrument(skip(pool))]
pub async fn mark_interrupted(pool: &Pool<Sqlite>) -> Result<u64> {
    let done = sqlx::query(
        "UPDATE session_recordings SET status = 'interrupted', ended_at = CURRENT_TIMESTAMP, \
         end_reason = 'The server stopped while this shell was being recorded' \
         WHERE status = 'recording'",
    )
    .execute(pool)
    .await
    .context("Failed to mark interrupted recordings")
    .map_err(Error::Database)?;
    Ok(done.rows_affected())
}

/// Every recording's owner and file, to find files on disk that no row knows (orphans).
#[instrument(skip(pool))]
pub async fn all_files(pool: &Pool<Sqlite>) -> Result<Vec<(i64, String)>> {
    sqlx::query_as::<_, (i64, String)>("SELECT user_id, file_name FROM session_recordings")
        .fetch_all(pool)
        .await
        .context("Failed to list recording files")
        .map_err(Error::Database)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn db() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        for name in ["ann", "bob"] {
            sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, 'x')")
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO servers (name, hostname) VALUES ('web', '10.0.0.1')")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn rec(user_id: i64, file: &str) -> NewRecording {
        NewRecording {
            user_id,
            server_id: 1,
            server_name: "web".into(),
            session_id: "s".into(),
            file_name: file.into(),
            cols: 80,
            rows: 24,
        }
    }

    #[tokio::test]
    async fn a_recording_is_visible_to_its_owner_only() {
        let pool = db().await;
        let ann = create(&pool, &rec(1, "a.cast.enc")).await.unwrap();
        let bob = create(&pool, &rec(2, "b.cast.enc")).await.unwrap();

        let mine = list_for_user(&pool, 1, None, 50, None).await.unwrap();
        assert_eq!(mine.iter().map(|r| r.id).collect::<Vec<_>>(), [ann]);
        assert_eq!(
            get_for_user(&pool, ann, 1).await.unwrap().status,
            "recording"
        );

        // Bob's recording does not exist for Ann: not listed, not readable, not deletable
        assert!(matches!(
            get_for_user(&pool, bob, 1).await.unwrap_err(),
            Error::NotFound(_)
        ));
        assert!(matches!(
            delete_for_user(&pool, bob, 1).await.unwrap_err(),
            Error::NotFound(_)
        ));
        assert!(get_for_user(&pool, bob, 2).await.is_ok(), "still there");

        let gone = delete_for_user(&pool, ann, 1).await.unwrap();
        assert_eq!(gone.file_name, "a.cast.enc");
        assert!(list_for_user(&pool, 1, None, 50, None)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn the_list_pages_newest_first_and_filters_by_server() {
        let pool = db().await;
        let ids: Vec<i64> = create_n(&pool, 5).await;
        let page = list_for_user(&pool, 1, None, 2, None).await.unwrap();
        assert_eq!(
            page.iter().map(|r| r.id).collect::<Vec<_>>(),
            [ids[4], ids[3]]
        );
        let next = list_for_user(&pool, 1, None, 2, Some(ids[3]))
            .await
            .unwrap();
        assert_eq!(
            next.iter().map(|r| r.id).collect::<Vec<_>>(),
            [ids[2], ids[1]]
        );
        assert_eq!(
            list_for_user(&pool, 1, Some(1), 50, None)
                .await
                .unwrap()
                .len(),
            5
        );
        assert!(list_for_user(&pool, 1, Some(99), 50, None)
            .await
            .unwrap()
            .is_empty());
    }

    async fn create_n(pool: &Pool<Sqlite>, n: usize) -> Vec<i64> {
        let mut ids = Vec::new();
        for i in 0..n {
            ids.push(
                create(pool, &rec(1, &format!("{i}.cast.enc")))
                    .await
                    .unwrap(),
            );
        }
        ids
    }

    #[tokio::test]
    async fn finishing_retention_and_interrupted_recordings() {
        let pool = db().await;
        let done = create(&pool, &rec(1, "done")).await.unwrap();
        let running = create(&pool, &rec(1, "running")).await.unwrap();
        let old_running = create(&pool, &rec(1, "old-running")).await.unwrap();
        update_progress(&pool, done, 1500, 4096).await.unwrap();
        finish(
            &pool,
            done,
            RecordingStatus::Complete,
            2000,
            5000,
            Some("Shell exited with status 0"),
        )
        .await
        .unwrap();
        let r = get_for_user(&pool, done, 1).await.unwrap();
        assert_eq!(
            (r.status.as_str(), r.duration_ms, r.size_bytes),
            ("complete", 2000, 5000)
        );
        assert!(r.ended_at.is_some());
        // Progress no longer moves a finished recording
        update_progress(&pool, done, 1, 1).await.unwrap();
        assert_eq!(get_for_user(&pool, done, 1).await.unwrap().size_bytes, 5000);

        // Age everything by 40 days: only the finished one is due
        sqlx::query("UPDATE session_recordings SET started_at = datetime('now', '-40 days')")
            .execute(&pool)
            .await
            .unwrap();
        let due = older_than_days(&pool, 30).await.unwrap();
        assert_eq!(due.iter().map(|r| r.id).collect::<Vec<_>>(), [done]);
        assert!(older_than_days(&pool, 60).await.unwrap().is_empty());

        // A restart: the running ones can never finish now
        assert_eq!(mark_interrupted(&pool).await.unwrap(), 2);
        for id in [running, old_running] {
            assert_eq!(
                get_for_user(&pool, id, 1).await.unwrap().status,
                "interrupted"
            );
        }
        assert_eq!(all_files(&pool).await.unwrap().len(), 3);
        delete(&pool, done).await.unwrap();
        assert_eq!(all_files(&pool).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn deleting_the_user_or_the_server() {
        let pool = db().await;
        let id = create(&pool, &rec(2, "bob")).await.unwrap();
        sqlx::query("DELETE FROM servers WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let r = get_for_user(&pool, id, 2).await.unwrap();
        assert_eq!((r.server_id, r.server_name.as_str()), (None, "web"));
        sqlx::query("DELETE FROM users WHERE id = 2")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            all_files(&pool).await.unwrap().is_empty(),
            "rows go with the user"
        );
    }
}
