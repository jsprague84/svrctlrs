# Migration 007 Fix Instructions

## Problem

Migration 007 is failing because the `server_id` and `server_name` columns already exist in the `tasks` table from a previous partial migration attempt. SQLite doesn't support `ALTER TABLE ADD COLUMN IF NOT EXISTS`, so the migration fails and causes the container to restart in a loop.

## Solution

You need to manually mark the migration as complete and ensure the data is correct. Run these commands on your `docker-vm`:

### Step 1: Stop the container

```bash
cd ~/docker-compose/svrctlrs
docker-compose down
```

### Step 2: Find the database file location

The database is likely in one of these locations:
- `./data/svrctlrs.db` (if using bind mount)
- Docker volume (check with `docker volume inspect svrctlrs_data`)

For bind mount:
```bash
DB_PATH="./data/svrctlrs.db"
```

For Docker volume:
```bash
# Find the volume mount point
VOLUME_PATH=$(docker volume inspect svrctlrs_data -f '{{.Mountpoint}}')
DB_PATH="${VOLUME_PATH}/svrctlrs.db"
```

### Step 3: Check if columns exist

```bash
sudo sqlite3 $DB_PATH "SELECT sql FROM sqlite_master WHERE type='table' AND name='tasks';"
```

If you see `server_id` and `server_name` in the output, proceed to Step 4.

### Step 4: Mark migration as complete

```bash
sudo sqlite3 $DB_PATH "INSERT OR IGNORE INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) VALUES (7, 'add server_id to tasks', datetime('now'), 1, x'', 0);"
```

### Step 5: Ensure localhost server exists

```bash
sudo sqlite3 $DB_PATH "INSERT OR IGNORE INTO servers (name, host, port, username, enabled, description, docker_installed, connection_timeout, retry_attempts, created_at, updated_at) VALUES ('localhost', NULL, 22, 'root', 1, 'Local system (SvrCtlRS host)', 1, 30, 3, datetime('now'), datetime('now'));"
```

### Step 6: Update tasks to have server_id and server_name

```bash
sudo sqlite3 $DB_PATH "UPDATE tasks SET server_id = COALESCE(server_id, (SELECT id FROM servers WHERE name = 'localhost' LIMIT 1)), server_name = COALESCE(server_name, 'localhost') WHERE server_id IS NULL OR server_name IS NULL;"
```

### Step 7: Verify the fix

```bash
# Check migration is marked complete
sudo sqlite3 $DB_PATH "SELECT * FROM _sqlx_migrations WHERE version = 7;"

# Check all tasks have server_id
sudo sqlite3 $DB_PATH "SELECT COUNT(*) FROM tasks WHERE server_id IS NULL;"
```

The second query should return `0`.

### Step 8: Start the container

```bash
docker-compose up -d
```

### Step 9: Check logs

```bash
docker-compose logs -f
```

The container should start successfully without migration errors.

## Alternative: Reset the database (DESTRUCTIVE)

If you don't have important data and want to start fresh:

```bash
cd ~/docker-compose/svrctlrs
docker-compose down
rm -rf ./data/svrctlrs.db*  # or delete the Docker volume
docker-compose up -d
```

This will create a fresh database with all migrations applied correctly.

## Prevention for Future

This issue occurred because SQLite doesn't support conditional column additions. For future migrations, we should:

1. Use a migration tool that checks schema first
2. Or switch to PostgreSQL which has better migration support
3. Or implement custom migration logic that checks for column existence

For now, SQLite is fine for single-instance deployments, but this is a known limitation.

