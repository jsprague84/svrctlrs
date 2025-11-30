-- Migration 013: Add rendered_command field to job_runs table
-- Created: 2025-11-30
-- Purpose: Store the final rendered command (with variables substituted) for audit trail

-- Add rendered_command column to job_runs table
ALTER TABLE job_runs ADD COLUMN rendered_command TEXT;

-- Add comment explaining the column (SQLite doesn't support column comments directly,
-- but this migration serves as documentation)
-- rendered_command: The final command that was executed, with all {{variables}} substituted
-- This provides an audit trail showing exactly what command ran on the server
