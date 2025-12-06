# Archived Documentation

**Status**: ⚠️ NOT MAINTAINED - For historical reference only

This directory contains outdated documentation from before the **v2.0 architecture restructure** (Migration 011).

## What Happened

On **2024-11-26**, SvrCtlRS underwent a complete architectural restructure:
- **Old System**: Plugin-based architecture with hardcoded monitoring features
- **New System**: Job-based architecture with user-defined workflows

## Directory Structure

```
docs-archived/
├── root-level/          # Old root-level .md files (assessments, plans, summaries)
└── old-docs/            # Old docs/ directory (plugin guides, feature specs)
```

## Current Documentation

For current, accurate documentation, see:
- **`/README.md`** - Project overview and quick start
- **`/CLAUDE.md`** - Comprehensive AI development guide (v2.0 architecture)

## Why Archived?

These documents describe the old plugin-based system and are no longer accurate:
- Plugin development guides (we now use job types)
- Task-based scheduling docs (now job schedules)
- Old architecture assessments and migration plans
- Feature specs for deprecated patterns

**Do not use this documentation for development.**

---

**Archived**: 2025-11-28
**Last Updated Architecture**: v2.0 (Job-Based System)
