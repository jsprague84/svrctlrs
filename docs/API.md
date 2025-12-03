# SvrCtlRS REST API v1 Documentation

This document provides comprehensive documentation for the SvrCtlRS REST API v1.

## Base URL

```
/api/v1/
```

## Authentication

Currently, the API does not require authentication. This will be added in a future release.

## Response Format

All API responses use JSON format with a consistent structure.

### Success Response

```json
{
  "data": { ... },
  "message": "Optional success message"
}
```

### Error Response

```json
{
  "success": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error message"
  }
}
```

### Common Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `NOT_FOUND` | 404 | Requested resource does not exist |
| `BAD_REQUEST` | 400 | Invalid request parameters |
| `INTERNAL_ERROR` | 500 | Server-side error |
| `CONFLICT` | 409 | Resource conflict (e.g., cannot delete due to dependencies) |

---

## System Endpoints

### Health Check

Check if the API is running.

```
GET /api/v1/health
```

**Response:**
```json
{
  "status": "ok",
  "service": "svrctlrs",
  "version": "1.0.0"
}
```

### Server Status

Get detailed server status information.

```
GET /api/v1/status
```

**Response:**
```json
{
  "status": "running",
  "version": "1.0.0",
  "scheduler": {
    "running": true,
    "scheduled_tasks": 5
  },
  "resources": {
    "servers": 3,
    "job_types": 4,
    "job_templates": 10,
    "job_schedules": 5
  }
}
```

### System Metrics

Get system metrics and statistics.

```
GET /api/v1/metrics
```

**Response:**
```json
{
  "metrics": {
    "servers": {
      "total": 3
    },
    "job_runs": {
      "total": 150,
      "recent_success": 45,
      "recent_failed": 2,
      "currently_running": 1
    },
    "schedules": {
      "total": 5,
      "enabled": 4
    }
  }
}
```

### Reload Configuration

Reload configuration and reschedule jobs.

```
POST /api/v1/config/reload
```

**Response:**
```json
{
  "success": true,
  "message": "Configuration reloaded successfully",
  "scheduled_tasks": 5
}
```

---

## Servers

Manage SSH server connections.

### List Servers

```
GET /api/v1/servers
```

**Response:**
```json
{
  "servers": [
    {
      "id": 1,
      "name": "web-server-01",
      "hostname": "192.168.1.10",
      "port": 22,
      "username": "admin",
      "enabled": true,
      "is_local": false,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

### Get Server

```
GET /api/v1/servers/{id}
```

**Response:**
```json
{
  "server": {
    "id": 1,
    "name": "web-server-01",
    "hostname": "192.168.1.10",
    "port": 22,
    "username": "admin",
    "credential_id": 1,
    "description": "Production web server",
    "enabled": true,
    "is_local": false,
    "os_type": "linux",
    "os_distro": "ubuntu",
    "package_manager": "apt",
    "docker_available": true,
    "systemd_available": true,
    "created_at": "2024-01-15T10:30:00Z"
  },
  "tags": [
    { "id": 1, "name": "production" },
    { "id": 2, "name": "web" }
  ],
  "capabilities": ["docker", "systemd", "apt"]
}
```

### Create Server

```
POST /api/v1/servers
```

**Request Body:**
```json
{
  "name": "web-server-01",
  "hostname": "192.168.1.10",
  "port": 22,
  "username": "admin",
  "credential_id": 1,
  "description": "Production web server",
  "is_local": false,
  "enabled": true,
  "tag_ids": [1, 2]
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Server created successfully"
}
```

### Update Server

```
PUT /api/v1/servers/{id}
```

**Request Body:** (all fields optional)
```json
{
  "name": "web-server-01-updated",
  "hostname": "192.168.1.11",
  "port": 22,
  "username": "newadmin",
  "credential_id": 2,
  "description": "Updated description",
  "enabled": false,
  "tag_ids": [1, 3]
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Server updated successfully"
}
```

### Delete Server

```
DELETE /api/v1/servers/{id}
```

**Response:**
```json
{
  "id": 1,
  "message": "Server deleted successfully"
}
```

### Test Server Connection

```
POST /api/v1/servers/{id}/test
```

**Response:**
```json
{
  "success": true,
  "message": "Connection test successful",
  "server_id": 1,
  "hostname": "192.168.1.10",
  "port": 22
}
```

### Get Server Tags

```
GET /api/v1/servers/{id}/tags
```

**Response:**
```json
{
  "tags": [
    { "id": 1, "name": "production" },
    { "id": 2, "name": "web" }
  ]
}
```

### Set Server Tags

```
PUT /api/v1/servers/{id}/tags
```

**Request Body:**
```json
{
  "tag_ids": [1, 2, 3]
}
```

**Response:**
```json
{
  "server_id": 1,
  "message": "Server tags updated successfully"
}
```

### Get Server Capabilities

```
GET /api/v1/servers/{id}/capabilities
```

**Response:**
```json
{
  "capabilities": ["docker", "systemd", "apt", "python3"]
}
```

---

## Credentials

Manage SSH credentials (keys, passwords, API tokens).

### List Credentials

```
GET /api/v1/credentials
```

**Response:**
```json
{
  "credentials": [
    {
      "id": 1,
      "name": "production-key",
      "credential_type": "ssh_key",
      "description": "Production SSH key",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

### Get Credential

```
GET /api/v1/credentials/{id}
```

**Note:** Sensitive data (private keys, passwords) is redacted in responses.

**Response:**
```json
{
  "credential": {
    "id": 1,
    "name": "production-key",
    "credential_type": "ssh_key",
    "username": "admin",
    "description": "Production SSH key",
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

### Create Credential

```
POST /api/v1/credentials
```

**Request Body:**
```json
{
  "name": "production-key",
  "credential_type": "ssh_key",
  "username": "admin",
  "private_key": "-----BEGIN OPENSSH PRIVATE KEY-----\n...",
  "passphrase": "optional-passphrase",
  "description": "Production SSH key"
}
```

Supported credential types:
- `ssh_key` - SSH private key authentication
- `password` - Password authentication
- `api_token` - API token

**Response:**
```json
{
  "id": 1,
  "message": "Credential created successfully"
}
```

### Update Credential

```
PUT /api/v1/credentials/{id}
```

**Request Body:** (all fields optional)
```json
{
  "name": "updated-name",
  "username": "newadmin",
  "private_key": "-----BEGIN OPENSSH PRIVATE KEY-----\n...",
  "passphrase": "new-passphrase",
  "description": "Updated description"
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Credential updated successfully"
}
```

### Delete Credential

```
DELETE /api/v1/credentials/{id}
```

**Response:**
```json
{
  "id": 1,
  "message": "Credential deleted successfully"
}
```

---

## Tags

Manage server organization tags.

### List Tags

```
GET /api/v1/tags
```

**Response:**
```json
{
  "tags": [
    {
      "id": 1,
      "name": "production",
      "color": "#4CAF50",
      "description": "Production servers",
      "server_count": 5
    }
  ]
}
```

### Get Tag

```
GET /api/v1/tags/{id}
```

**Response:**
```json
{
  "tag": {
    "id": 1,
    "name": "production",
    "color": "#4CAF50",
    "description": "Production servers",
    "created_at": "2024-01-15T10:30:00Z"
  },
  "server_count": 5
}
```

### Create Tag

```
POST /api/v1/tags
```

**Request Body:**
```json
{
  "name": "production",
  "color": "#4CAF50",
  "description": "Production servers"
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Tag created successfully"
}
```

### Update Tag

```
PUT /api/v1/tags/{id}
```

**Request Body:** (all fields optional)
```json
{
  "name": "staging",
  "color": "#FF9800",
  "description": "Updated description"
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Tag updated successfully"
}
```

### Delete Tag

```
DELETE /api/v1/tags/{id}
```

**Response:**
```json
{
  "id": 1,
  "message": "Tag deleted successfully"
}
```

---

## Job Types

Manage job categories and their command templates.

### List Job Types

```
GET /api/v1/job-types
```

**Response:**
```json
{
  "job_types": [
    {
      "id": 1,
      "name": "docker_operations",
      "display_name": "Docker Operations",
      "description": "Manage Docker containers and images",
      "icon": "docker",
      "color": "#2496ED",
      "requires_capabilities": ["docker"],
      "enabled": true,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

### Get Job Type

```
GET /api/v1/job-types/{id}
```

**Response:**
```json
{
  "job_type": {
    "id": 1,
    "name": "docker_operations",
    "display_name": "Docker Operations",
    "description": "Manage Docker containers and images",
    "icon": "docker",
    "color": "#2496ED",
    "requires_capabilities": ["docker"],
    "enabled": true,
    "created_at": "2024-01-15T10:30:00Z"
  },
  "command_templates": [
    {
      "id": 1,
      "name": "list_containers",
      "display_name": "List Containers",
      "command": "docker ps --format '{{.Names}} - {{.Status}}'",
      "timeout_seconds": 30
    }
  ]
}
```

### Create Job Type

```
POST /api/v1/job-types
```

**Request Body:**
```json
{
  "name": "docker_operations",
  "display_name": "Docker Operations",
  "description": "Manage Docker containers and images",
  "icon": "docker",
  "color": "#2496ED",
  "requires_capabilities": ["docker"],
  "enabled": true
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Job type created successfully"
}
```

### Update Job Type

```
PUT /api/v1/job-types/{id}
```

**Request Body:** (all fields optional)
```json
{
  "display_name": "Updated Display Name",
  "description": "Updated description",
  "icon": "new-icon",
  "color": "#FF0000",
  "requires_capabilities": ["docker", "compose"],
  "enabled": false
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Job type updated successfully"
}
```

### Delete Job Type

```
DELETE /api/v1/job-types/{id}
```

**Note:** Cannot delete job types that are in use by job templates.

**Response:**
```json
{
  "id": 1,
  "message": "Job type deleted successfully"
}
```

---

## Command Templates

Manage reusable command templates with variable substitution.

### List Command Templates

```
GET /api/v1/job-types/{job_type_id}/command-templates
```

**Response:**
```json
{
  "command_templates": [
    {
      "id": 1,
      "name": "restart_container",
      "display_name": "Restart Container",
      "command": "docker restart {{container_name}}",
      "description": "Restart a Docker container",
      "timeout_seconds": 60,
      "required_capabilities": ["docker"]
    }
  ]
}
```

### Get Command Template

```
GET /api/v1/job-types/command-templates/{template_id}
```

**Response:**
```json
{
  "command_template": {
    "id": 1,
    "job_type_id": 1,
    "name": "restart_container",
    "display_name": "Restart Container",
    "command": "docker restart {{container_name}}",
    "description": "Restart a Docker container",
    "required_capabilities": ["docker"],
    "timeout_seconds": 60,
    "working_directory": null,
    "notify_on_success": false,
    "notify_on_failure": true,
    "parameter_schema": [
      {
        "name": "container_name",
        "type": "string",
        "required": true,
        "description": "Name of the container to restart"
      }
    ],
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

### Create Command Template

```
POST /api/v1/job-types/{job_type_id}/command-templates
```

**Request Body:**
```json
{
  "name": "restart_container",
  "display_name": "Restart Container",
  "command": "docker restart {{container_name}}",
  "description": "Restart a Docker container",
  "required_capabilities": ["docker"],
  "timeout_seconds": 60,
  "working_directory": "/home/admin",
  "notify_on_success": false,
  "notify_on_failure": true,
  "parameter_schema": [
    {
      "name": "container_name",
      "type": "string",
      "required": true,
      "description": "Name of the container to restart"
    }
  ]
}
```

**Variable Syntax:**
- `{{variable_name}}` - Basic variable substitution
- Variables are defined in `parameter_schema`

**Parameter Types:**
- `string` - Text input
- `number` - Numeric input
- `boolean` - True/false checkbox
- `select` - Dropdown with predefined options

**Response:**
```json
{
  "id": 1,
  "message": "Command template created successfully"
}
```

### Update Command Template

```
PUT /api/v1/job-types/command-templates/{template_id}
```

**Request Body:** (all fields optional)
```json
{
  "display_name": "Updated Display Name",
  "description": "Updated description",
  "command": "docker restart {{container_name}} --timeout {{timeout}}",
  "required_capabilities": ["docker"],
  "timeout_seconds": 120,
  "working_directory": "/opt/app",
  "notify_on_success": true,
  "notify_on_failure": true,
  "parameter_schema": [...]
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Command template updated successfully"
}
```

### Delete Command Template

```
DELETE /api/v1/job-types/command-templates/{template_id}
```

**Note:** Cannot delete command templates that are in use by job templates.

**Response:**
```json
{
  "id": 1,
  "message": "Command template deleted successfully"
}
```

---

## Job Templates

Manage user-defined job configurations.

### List Job Templates

```
GET /api/v1/job-templates
```

**Response:**
```json
{
  "job_templates": [
    {
      "id": 1,
      "name": "restart_nginx",
      "display_name": "Restart Nginx Container",
      "job_type_id": 1,
      "is_composite": false,
      "command_template_id": 1,
      "enabled": true,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

### Get Job Template

```
GET /api/v1/job-templates/{id}
```

**Response:**
```json
{
  "job_template": {
    "id": 1,
    "name": "restart_nginx",
    "display_name": "Restart Nginx Container",
    "job_type_id": 1,
    "job_type_name": "docker_operations",
    "is_composite": false,
    "command_template_id": 1,
    "variables": {
      "container_name": "nginx"
    },
    "description": "Restarts the Nginx container",
    "timeout_seconds": 60,
    "notify_on_success": false,
    "notify_on_failure": true,
    "enabled": true,
    "created_at": "2024-01-15T10:30:00Z"
  },
  "steps": []
}
```

### Create Job Template

```
POST /api/v1/job-templates
```

**Request Body:**
```json
{
  "name": "restart_nginx",
  "display_name": "Restart Nginx Container",
  "job_type_id": 1,
  "is_composite": false,
  "command_template_id": 1,
  "variables": {
    "container_name": "nginx"
  },
  "description": "Restarts the Nginx container",
  "timeout_seconds": 60,
  "notify_on_success": false,
  "notify_on_failure": true,
  "enabled": true
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Job template created successfully"
}
```

### Update Job Template

```
PUT /api/v1/job-templates/{id}
```

**Request Body:** (all fields optional)
```json
{
  "display_name": "Updated Display Name",
  "description": "Updated description",
  "command_template_id": 2,
  "variables": {
    "container_name": "nginx-new"
  },
  "timeout_seconds": 120,
  "notify_on_success": true,
  "notify_on_failure": true,
  "enabled": false
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Job template updated successfully"
}
```

### Delete Job Template

```
DELETE /api/v1/job-templates/{id}
```

**Note:** Cannot delete job templates that have active schedules.

**Response:**
```json
{
  "id": 1,
  "message": "Job template deleted successfully"
}
```

### Execute Job Template

Execute a job template immediately on a specific server.

```
POST /api/v1/job-templates/{id}/execute
```

**Request Body:**
```json
{
  "server_id": 1,
  "variables": {
    "container_name": "nginx-override"
  }
}
```

**Response:**
```json
{
  "job_run_id": 123,
  "message": "Job execution started"
}
```

---

## Job Template Steps

Manage steps for composite job templates (multi-step workflows).

### List Steps

```
GET /api/v1/job-templates/{id}/steps
```

**Response:**
```json
{
  "steps": [
    {
      "id": 1,
      "job_template_id": 1,
      "step_order": 0,
      "name": "Stop Service",
      "command_template_id": 1,
      "variables": {},
      "continue_on_failure": false,
      "timeout_seconds": 30
    },
    {
      "id": 2,
      "job_template_id": 1,
      "step_order": 1,
      "name": "Update Image",
      "command_template_id": 2,
      "variables": {},
      "continue_on_failure": false,
      "timeout_seconds": 120
    }
  ]
}
```

### Add Step

```
POST /api/v1/job-templates/{id}/steps
```

**Request Body:**
```json
{
  "name": "Start Service",
  "command_template_id": 3,
  "step_order": 2,
  "variables": {},
  "continue_on_failure": false,
  "timeout_seconds": 30
}
```

**Response:**
```json
{
  "id": 3,
  "message": "Step added successfully"
}
```

### Update Step

```
PUT /api/v1/job-templates/steps/{step_id}
```

**Request Body:** (all fields optional)
```json
{
  "name": "Updated Step Name",
  "command_template_id": 4,
  "step_order": 1,
  "variables": { "new_var": "value" },
  "continue_on_failure": true,
  "timeout_seconds": 60
}
```

**Response:**
```json
{
  "id": 3,
  "message": "Step updated successfully"
}
```

### Delete Step

```
DELETE /api/v1/job-templates/steps/{step_id}
```

**Response:**
```json
{
  "id": 3,
  "message": "Step deleted successfully"
}
```

---

## Job Schedules

Manage cron-scheduled job instances.

### List Job Schedules

```
GET /api/v1/job-schedules
```

**Response:**
```json
{
  "job_schedules": [
    {
      "id": 1,
      "name": "hourly_nginx_health",
      "job_template_id": 1,
      "job_template_name": "Nginx Health Check",
      "server_id": 1,
      "server_name": "web-server-01",
      "schedule": "0 * * * *",
      "description": "Hourly Nginx health check",
      "enabled": true,
      "next_run_at": "2024-01-15T11:00:00Z",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

### Get Job Schedule

```
GET /api/v1/job-schedules/{id}
```

**Response:**
```json
{
  "job_schedule": {
    "id": 1,
    "name": "hourly_nginx_health",
    "job_template_id": 1,
    "server_id": 1,
    "schedule": "0 * * * *",
    "description": "Hourly Nginx health check",
    "timeout_seconds": 60,
    "retry_count": 3,
    "notify_on_success": false,
    "notify_on_failure": true,
    "notification_policy_id": 1,
    "enabled": true,
    "next_run_at": "2024-01-15T11:00:00Z",
    "last_run_at": "2024-01-15T10:00:00Z",
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

### Create Job Schedule

```
POST /api/v1/job-schedules
```

**Request Body:**
```json
{
  "name": "hourly_nginx_health",
  "job_template_id": 1,
  "server_id": 1,
  "schedule": "0 * * * *",
  "description": "Hourly Nginx health check",
  "timeout_seconds": 60,
  "retry_count": 3,
  "notify_on_success": false,
  "notify_on_failure": true,
  "notification_policy_id": 1,
  "enabled": true
}
```

**Cron Expression Format:**
```
* * * * *
│ │ │ │ │
│ │ │ │ └─ Day of week (0-7, 0 and 7 are Sunday)
│ │ │ └─── Month (1-12)
│ │ └───── Day of month (1-31)
│ └─────── Hour (0-23)
└───────── Minute (0-59)
```

**Examples:**
- `0 * * * *` - Every hour at minute 0
- `*/15 * * * *` - Every 15 minutes
- `0 0 * * *` - Daily at midnight
- `0 0 * * 0` - Weekly on Sunday at midnight

**Response:**
```json
{
  "id": 1,
  "message": "Job schedule created successfully"
}
```

### Update Job Schedule

```
PUT /api/v1/job-schedules/{id}
```

**Request Body:** (all fields optional)
```json
{
  "name": "updated_schedule_name",
  "schedule": "*/30 * * * *",
  "description": "Updated description",
  "timeout_seconds": 120,
  "retry_count": 5,
  "notify_on_success": true,
  "notify_on_failure": true,
  "notification_policy_id": 2,
  "enabled": false
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Job schedule updated successfully"
}
```

### Delete Job Schedule

```
DELETE /api/v1/job-schedules/{id}
```

**Response:**
```json
{
  "id": 1,
  "message": "Job schedule deleted successfully"
}
```

### Toggle Job Schedule

Enable or disable a job schedule.

```
POST /api/v1/job-schedules/{id}/toggle
```

**Response:**
```json
{
  "id": 1,
  "enabled": false,
  "message": "Job schedule disabled"
}
```

---

## Job Runs

View job execution history and manage running jobs.

### List Job Runs

```
GET /api/v1/job-runs
```

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `page` | integer | Page number (default: 1) |
| `per_page` | integer | Items per page (10-100, default: 50) |
| `status` | string | Filter by status: `running`, `success`, `failure`, `timeout`, `cancelled` |
| `server_id` | integer | Filter by server ID |
| `job_template_id` | integer | Filter by job template ID |

**Response:**
```json
{
  "job_runs": [
    {
      "id": 123,
      "job_schedule_id": 1,
      "job_template_id": 1,
      "job_template_name": "Nginx Health Check",
      "server_id": 1,
      "server_name": "web-server-01",
      "status": "success",
      "started_at": "2024-01-15T10:00:00Z",
      "finished_at": "2024-01-15T10:00:05Z",
      "duration_ms": 5000,
      "exit_code": 0
    }
  ],
  "pagination": {
    "page": 1,
    "per_page": 50,
    "total": 150,
    "total_pages": 3
  }
}
```

### Get Job Run

```
GET /api/v1/job-runs/{id}
```

**Response:**
```json
{
  "job_run": {
    "id": 123,
    "job_schedule_id": 1,
    "job_template_id": 1,
    "server_id": 1,
    "status": "success",
    "started_at": "2024-01-15T10:00:00Z",
    "finished_at": "2024-01-15T10:00:05Z",
    "duration_ms": 5000,
    "exit_code": 0,
    "output": "Container nginx is healthy",
    "error": null,
    "rendered_command": "docker exec nginx curl -s http://localhost/health",
    "retry_attempt": 0,
    "is_retry": false,
    "notification_sent": false
  },
  "step_results": []
}
```

### Cancel Job Run

Cancel a running or pending job.

```
POST /api/v1/job-runs/{id}/cancel
```

**Response:**
```json
{
  "id": 123,
  "message": "Job run cancelled successfully"
}
```

---

## Notification Channels

Manage notification backends (Gotify, ntfy.sh, etc.).

### List Notification Channels

```
GET /api/v1/notifications/channels
```

**Response:**
```json
{
  "channels": [
    {
      "id": 1,
      "name": "gotify-alerts",
      "channel_type": "gotify",
      "description": "Main Gotify server",
      "enabled": true,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

### Get Notification Channel

```
GET /api/v1/notifications/channels/{id}
```

**Response:**
```json
{
  "channel": {
    "id": 1,
    "name": "gotify-alerts",
    "channel_type": "gotify",
    "config": {
      "url": "https://gotify.example.com",
      "token": "***REDACTED***"
    },
    "description": "Main Gotify server",
    "enabled": true,
    "default_priority": 5,
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

### Create Notification Channel

```
POST /api/v1/notifications/channels
```

**Request Body:**
```json
{
  "name": "gotify-alerts",
  "channel_type": "gotify",
  "config": {
    "url": "https://gotify.example.com",
    "token": "your-gotify-token"
  },
  "description": "Main Gotify server",
  "enabled": true,
  "default_priority": 5
}
```

**Supported Channel Types:**
- `gotify` - Gotify push notifications
- `ntfy` - ntfy.sh notifications
- `email` - Email notifications
- `slack` - Slack webhooks
- `discord` - Discord webhooks
- `webhook` - Generic webhook

**Channel-Specific Config:**

**Gotify:**
```json
{
  "url": "https://gotify.example.com",
  "token": "your-token"
}
```

**ntfy:**
```json
{
  "url": "https://ntfy.sh",
  "topic": "your-topic",
  "auth_token": "optional-token"
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Notification channel created successfully"
}
```

### Update Notification Channel

```
PUT /api/v1/notifications/channels/{id}
```

**Request Body:** (all fields optional)
```json
{
  "name": "updated-name",
  "config": {
    "url": "https://new-url.example.com",
    "token": "new-token"
  },
  "description": "Updated description",
  "enabled": false,
  "default_priority": 10
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Notification channel updated successfully"
}
```

### Delete Notification Channel

```
DELETE /api/v1/notifications/channels/{id}
```

**Response:**
```json
{
  "id": 1,
  "message": "Notification channel deleted successfully"
}
```

### Test Notification Channel

```
POST /api/v1/notifications/channels/{id}/test
```

**Response:**
```json
{
  "success": true,
  "message": "Test notification sent successfully"
}
```

---

## Notification Policies

Manage notification routing rules.

### List Notification Policies

```
GET /api/v1/notifications/policies
```

**Response:**
```json
{
  "policies": [
    {
      "id": 1,
      "name": "alert-on-failures",
      "on_success": false,
      "on_failure": true,
      "on_timeout": true,
      "enabled": true,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

### Get Notification Policy

```
GET /api/v1/notifications/policies/{id}
```

**Response:**
```json
{
  "policy": {
    "id": 1,
    "name": "alert-on-failures",
    "on_success": false,
    "on_failure": true,
    "on_timeout": true,
    "job_type_filter": null,
    "server_filter": null,
    "tag_filter": null,
    "min_severity": 1,
    "max_per_hour": 10,
    "title_template": "Job {{job_name}} Failed",
    "body_template": "Job failed on {{server_name}} with exit code {{exit_code}}",
    "enabled": true,
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

### Create Notification Policy

```
POST /api/v1/notifications/policies
```

**Request Body:**
```json
{
  "name": "alert-on-failures",
  "on_success": false,
  "on_failure": true,
  "on_timeout": true,
  "job_type_filter": null,
  "server_filter": null,
  "tag_filter": ["production"],
  "min_severity": 1,
  "max_per_hour": 10,
  "title_template": "Job {{job_name}} Failed",
  "body_template": "Job failed on {{server_name}} with exit code {{exit_code}}",
  "enabled": true
}
```

**Template Variables:**
- `{{job_name}}` - Job template name
- `{{server_name}}` - Server name
- `{{exit_code}}` - Exit code
- `{{status}}` - Job status
- `{{output}}` - Command output (truncated)
- `{{error}}` - Error message
- `{{duration}}` - Execution duration

**Response:**
```json
{
  "id": 1,
  "message": "Notification policy created successfully"
}
```

### Update Notification Policy

```
PUT /api/v1/notifications/policies/{id}
```

**Request Body:** (all fields optional)
```json
{
  "name": "updated-name",
  "on_success": true,
  "on_failure": true,
  "on_timeout": true,
  "job_type_filter": ["docker_operations"],
  "server_filter": null,
  "tag_filter": ["production", "critical"],
  "min_severity": 2,
  "max_per_hour": 5,
  "title_template": "Updated Title",
  "body_template": "Updated Body",
  "enabled": false
}
```

**Response:**
```json
{
  "id": 1,
  "message": "Notification policy updated successfully"
}
```

### Delete Notification Policy

```
DELETE /api/v1/notifications/policies/{id}
```

**Response:**
```json
{
  "id": 1,
  "message": "Notification policy deleted successfully"
}
```

---

## Settings

Manage application settings.

### List Settings

```
GET /api/v1/settings
```

**Response:**
```json
{
  "settings": {
    "general": [
      {
        "key": "general.site_name",
        "value": "SvrCtlRS",
        "value_type": "string",
        "description": "Application name",
        "updated_at": "2024-01-15T10:30:00Z"
      }
    ],
    "notifications": [
      {
        "key": "notifications.enabled",
        "value": "true",
        "value_type": "boolean",
        "description": "Enable notifications",
        "updated_at": "2024-01-15T10:30:00Z"
      }
    ]
  }
}
```

### Get Setting

```
GET /api/v1/settings/{key}
```

**Note:** Use URL encoding for keys with dots, e.g., `/api/v1/settings/general.site_name`

**Response:**
```json
{
  "key": "general.site_name",
  "value": "SvrCtlRS",
  "value_type": "string",
  "description": "Application name",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

### Update Setting

```
PUT /api/v1/settings/{key}
```

**Request Body:**
```json
{
  "value": "New Value"
}
```

**Value Type Validation:**
- `string` - Any text value
- `number` - Must be valid integer or float
- `boolean` - Must be `"true"` or `"false"`
- `json` - Must be valid JSON

**Response:**
```json
{
  "key": "general.site_name",
  "message": "Setting updated successfully"
}
```

---

## Error Handling

### Common HTTP Status Codes

| Status | Meaning |
|--------|---------|
| 200 | Success |
| 201 | Created (POST success) |
| 400 | Bad Request - Invalid input |
| 404 | Not Found - Resource doesn't exist |
| 409 | Conflict - Resource cannot be modified/deleted |
| 500 | Internal Server Error |

### Error Response Format

All errors return a consistent JSON structure:

```json
{
  "success": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "Detailed error message"
  }
}
```

---

## Rate Limiting

Currently, no rate limiting is implemented. This will be added in a future release.

---

## Changelog

### v1.0.0 (Current)

- Initial API release
- Full CRUD for servers, credentials, tags
- Job-based system: types, templates, schedules, runs
- Notification channels and policies
- Settings management

---

## Support

For issues and feature requests, please visit:
https://github.com/jsprague84/svrctlrs/issues
