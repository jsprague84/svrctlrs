-- Extend task_history table with more details
-- The existing task_history table is basic, let's extend it

ALTER TABLE task_history ADD COLUMN started_at DATETIME;
ALTER TABLE task_history ADD COLUMN finished_at DATETIME;
ALTER TABLE task_history ADD COLUMN status TEXT DEFAULT 'success';
ALTER TABLE task_history ADD COLUMN exit_code INTEGER;
ALTER TABLE task_history ADD COLUMN stdout TEXT;
ALTER TABLE task_history ADD COLUMN stderr TEXT;
ALTER TABLE task_history ADD COLUMN error_message TEXT;
ALTER TABLE task_history ADD COLUMN triggered_by TEXT DEFAULT 'schedule';

-- Migrate existing data
UPDATE task_history SET 
    started_at = timestamp,
    finished_at = timestamp,
    status = CASE WHEN success = 1 THEN 'success' ELSE 'failed' END
WHERE started_at IS NULL;

-- Create additional indexes
CREATE INDEX IF NOT EXISTS idx_task_history_status ON task_history(status);
CREATE INDEX IF NOT EXISTS idx_task_history_started ON task_history(started_at DESC);

