# Command Templates User Guide

This guide explains how to create and use command templates in SvrCtlRS for executing shell commands on your servers.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Template Fields](#template-fields)
- [Command Syntax](#command-syntax)
- [Output Handling](#output-handling)
- [Template Examples Library](#template-examples-library)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Overview

Command Templates allow you to:
- Execute shell commands on local and remote servers
- Create reusable command configurations
- Parse and analyze command output
- Set custom timeouts and working directories
- Filter by operating system type
- Integrate with notification policies

### Key Concepts

**Command Template**: A reusable configuration that defines:
- The shell command to execute
- Where it runs (which OS types)
- How to handle output
- Timeout and environment settings

**Job Type**: The category the command belongs to (OS, Docker, Custom, etc.)

**OS Filter**: Limits which operating systems can run this command

**Parameter Schema**: Defines customizable inputs (future feature)

## Quick Start

### 1. Create Your First Command Template

Navigate to **Job Types** → Select a job type → Click the **+** button next to "Command Templates"

**Example: Check Disk Space**
```
Name: check-disk-space
Display Name: Check Disk Space
Description: Report disk usage for all mounted filesystems
Command: df -h
OS Filter: linux
Timeout: 30 seconds
```

### 2. Test the Command

1. Create a job template using this command template
2. Click "Run Now"
3. Check job run details for output
4. Verify the command executed successfully

### 3. Use in Production

Once tested, schedule the command to run automatically or integrate with notification policies for alerts.

## Template Fields

### Basic Information

#### Name (Required)
Unique identifier for the command template.
```
✓ Good: "check-disk-space", "restart-nginx", "backup-database"
✗ Avoid: "cmd1", "test", "TODO"
```

**Format**: Use kebab-case (lowercase with hyphens)

#### Display Name (Required)
Human-readable name shown in the UI.
```
✓ Good: "Check Disk Space", "Restart Nginx", "Backup Database"
✗ Avoid: "Command 1", "Test"
```

#### Description (Optional but Recommended)
Explain what the command does and when to use it.
```
✓ Good: "Checks disk usage across all mounted filesystems and reports partitions over 80% full"
✗ Avoid: "Checks disk"
```

### Command Configuration

#### Command (Required)
The shell command to execute. Can be:
- Single command: `df -h`
- Multiple commands: `cd /var/log && tail -100 syslog`
- With pipes: `ps aux | grep nginx`
- With redirects: `echo "test" > /tmp/test.txt`

#### Working Directory (Optional)
Directory to execute the command from.
```
Example: /var/www/app
Default: User's home directory
```

#### Environment Variables (Optional)
Set environment variables for the command.
```json
{
  "PATH": "/usr/local/bin:/usr/bin:/bin",
  "NODE_ENV": "production"
}
```

#### Timeout (Optional)
Maximum execution time in seconds.
```
Default: 300 (5 minutes)
Range: 1-3600 seconds
```

### Filtering and Requirements

#### Required Capabilities (Optional)
Specify what the server needs to run this command.
```json
["docker", "systemd", "postgresql"]
```

Use for commands that require specific software installed.

#### OS Filter (Optional)
Limit which operating systems can run this command.
```
Options: linux, windows, macos, debian, ubuntu, fedora, rhel, centos
Example: linux (all Linux distributions)
Example: debian,ubuntu (Debian-based only)
```

### Output Configuration

#### Output Format (Optional)
Expected output format for parsing.
```
Options: text, json, xml, csv
Default: text
```

#### Parse Output (Optional)
Enable output parsing and analysis.
```
Enabled: ✓ - Parse and extract data
Disabled: ✗ - Store raw output only
```

#### Output Parser (Optional)
Specify how to parse the output (JSON configuration).
```json
{
  "type": "regex",
  "pattern": "Usage: (\\d+)%"
}
```

### Notifications

#### Notify on Success (Optional)
Send notification when command succeeds.
```
Enabled: ✓ - Always notify on success
Disabled: ✗ - Only notify on failure
```

#### Notify on Failure (Optional)
Send notification when command fails.
```
Enabled: ✓ - Notify on failures (recommended)
Disabled: ✗ - Silent failures
```

## Command Syntax

### Basic Commands

**Single Command**:
```bash
uptime
```

**Command with Arguments**:
```bash
df -h /var
```

**Multiple Commands** (sequential execution):
```bash
cd /var/log && tail -100 syslog && ls -lh
```

### Pipes and Redirects

**Pipe Output**:
```bash
ps aux | grep nginx | wc -l
```

**Redirect Output**:
```bash
dmesg > /tmp/dmesg.log 2>&1
```

**Here Documents**:
```bash
cat << EOF > /tmp/config.txt
Setting1=value1
Setting2=value2
EOF
```

### Conditional Execution

**Run Second Command Only If First Succeeds** (&&):
```bash
mkdir -p /backup && cp -r /var/www /backup/
```

**Run Second Command Only If First Fails** (||):
```bash
systemctl start nginx || echo "Failed to start nginx" >&2
```

### Variables and Substitution

**Use Shell Variables**:
```bash
BACKUP_DIR=/backup/$(date +%Y%m%d) && mkdir -p $BACKUP_DIR && cp -r /var/www $BACKUP_DIR/
```

**Command Substitution**:
```bash
echo "Disk usage: $(df -h / | tail -1 | awk '{print $5}')"
```

## Output Handling

### Text Output

Most commands produce text output:
```bash
df -h
# Output:
# Filesystem      Size  Used Avail Use% Mounted on
# /dev/sda1        50G   30G   18G  63% /
```

### JSON Output

Some commands support JSON output:
```bash
docker inspect nginx --format='{{json .}}'
```

Enable **Parse Output** and set **Output Format: json** to extract specific fields.

### Parsing with Regular Expressions

Extract specific data from output:

**Disk Usage Percentage**:
```json
{
  "type": "regex",
  "pattern": "Use% (\\d+)",
  "capture": 1
}
```

**Extract IP Address**:
```json
{
  "type": "regex",
  "pattern": "inet (\\d+\\.\\d+\\.\\d+\\.\\d+)",
  "capture": 1
}
```

### Exit Codes

Commands return exit codes:
- `0` = Success
- `1-255` = Various failure codes

Check exit code in scripts:
```bash
if systemctl is-active nginx; then
  echo "Nginx is running"
else
  echo "Nginx is not running"
  exit 1
fi
```

## Template Examples Library

### System Monitoring

#### Disk Space Check
```yaml
Name: check-disk-space
Display Name: Check Disk Space
Description: Report disk usage for all mounted filesystems
Command: df -h
OS Filter: linux
Timeout: 30
```

#### Disk Usage with Alert
```yaml
Name: disk-usage-alert
Display Name: Disk Usage Alert
Description: Alert if any partition exceeds 80% usage
Command: df -h | awk '$5 > 80 {print "WARNING: "$0}'
OS Filter: linux
Notify on Success: false
Notify on Failure: true
```

#### Memory Usage
```yaml
Name: check-memory
Display Name: Check Memory Usage
Description: Display memory usage statistics
Command: free -h
OS Filter: linux
Timeout: 30
```

#### CPU Information
```yaml
Name: cpu-info
Display Name: CPU Information
Description: Display CPU model and core count
Command: lscpu | grep -E "Model name|CPU\(s\):"
OS Filter: linux
```

#### System Load Average
```yaml
Name: system-load
Display Name: System Load Average
Description: Display current system load
Command: uptime
OS Filter: linux
Timeout: 10
```

#### Top Processes by CPU
```yaml
Name: top-cpu-processes
Display Name: Top CPU Processes
Description: Show top 10 processes by CPU usage
Command: ps aux --sort=-%cpu | head -11
OS Filter: linux
```

#### Top Processes by Memory
```yaml
Name: top-memory-processes
Display Name: Top Memory Processes
Description: Show top 10 processes by memory usage
Command: ps aux --sort=-%mem | head -11
OS Filter: linux
```

#### Check Inode Usage
```yaml
Name: check-inodes
Display Name: Check Inode Usage
Description: Report inode usage (can fill up even with disk space available)
Command: df -i
OS Filter: linux
```

### Network Diagnostics

#### Ping Test
```yaml
Name: ping-test
Display Name: Ping Connectivity Test
Description: Test connectivity to a specific host
Command: ping -c 4 8.8.8.8
OS Filter: linux
Timeout: 30
```

#### DNS Resolution Test
```yaml
Name: dns-test
Display Name: DNS Resolution Test
Description: Test DNS resolution
Command: nslookup google.com
OS Filter: linux
Timeout: 15
```

#### Network Interface Status
```yaml
Name: network-interfaces
Display Name: Network Interface Status
Description: Show all network interfaces and their status
Command: ip addr show
OS Filter: linux
```

#### Port Connectivity Test
```yaml
Name: port-test
Display Name: Port Connectivity Test
Description: Test if a specific port is open (requires nc/netcat)
Command: nc -zv 192.168.1.1 80
OS Filter: linux
Timeout: 10
```

#### Active Network Connections
```yaml
Name: network-connections
Display Name: Active Network Connections
Description: Show all active network connections
Command: netstat -tuln
OS Filter: linux
Required Capabilities: ["netstat"]
```

#### Traceroute
```yaml
Name: traceroute
Display Name: Network Traceroute
Description: Trace network path to destination
Command: traceroute -m 15 8.8.8.8
OS Filter: linux
Timeout: 60
Required Capabilities: ["traceroute"]
```

#### Bandwidth Test (Local)
```yaml
Name: bandwidth-test
Display Name: Local Bandwidth Test
Description: Test disk I/O bandwidth
Command: dd if=/dev/zero of=/tmp/test bs=1M count=100 conv=fdatasync && rm /tmp/test
OS Filter: linux
Timeout: 60
```

#### Check Open Ports
```yaml
Name: check-ports
Display Name: Check Open Ports
Description: List all listening ports
Command: ss -tuln | grep LISTEN
OS Filter: linux
```

### Service Management

#### Systemd Service Status
```yaml
Name: service-status
Display Name: Check Service Status
Description: Check if a systemd service is running
Command: systemctl status nginx
OS Filter: linux
Required Capabilities: ["systemd"]
```

#### List All Services
```yaml
Name: list-services
Display Name: List All Services
Description: List all systemd services and their states
Command: systemctl list-units --type=service --all
OS Filter: linux
Required Capabilities: ["systemd"]
```

#### Restart Service
```yaml
Name: restart-service
Display Name: Restart Service
Description: Restart a systemd service
Command: systemctl restart nginx
OS Filter: linux
Required Capabilities: ["systemd"]
Notify on Failure: true
```

#### Service Logs
```yaml
Name: service-logs
Display Name: Service Logs
Description: Show recent logs for a service
Command: journalctl -u nginx -n 50 --no-pager
OS Filter: linux
Required Capabilities: ["systemd"]
```

### Docker Operations

#### List Running Containers
```yaml
Name: docker-ps
Display Name: List Running Containers
Description: Show all running Docker containers
Command: docker ps
OS Filter: linux
Required Capabilities: ["docker"]
```

#### Docker System Info
```yaml
Name: docker-info
Display Name: Docker System Info
Description: Display Docker system information
Command: docker info
OS Filter: linux
Required Capabilities: ["docker"]
```

#### Container Resource Usage
```yaml
Name: docker-stats
Display Name: Container Resource Usage
Description: Show resource usage for all containers
Command: docker stats --no-stream --no-trunc
OS Filter: linux
Required Capabilities: ["docker"]
Timeout: 30
```

#### Docker Disk Usage
```yaml
Name: docker-disk-usage
Display Name: Docker Disk Usage
Description: Show disk usage by Docker images, containers, and volumes
Command: docker system df -v
OS Filter: linux
Required Capabilities: ["docker"]
```

#### Restart Container
```yaml
Name: restart-container
Display Name: Restart Docker Container
Description: Restart a specific Docker container
Command: docker restart nginx
OS Filter: linux
Required Capabilities: ["docker"]
Notify on Failure: true
```

#### Pull Image
```yaml
Name: pull-image
Display Name: Pull Docker Image
Description: Pull latest version of a Docker image
Command: docker pull nginx:latest
OS Filter: linux
Required Capabilities: ["docker"]
Timeout: 300
```

#### Prune Unused Images
```yaml
Name: prune-images
Display Name: Prune Unused Images
Description: Remove unused Docker images to free space
Command: docker image prune -af
OS Filter: linux
Required Capabilities: ["docker"]
```

#### Container Health Check
```yaml
Name: container-health
Display Name: Container Health Check
Description: Check health status of all containers
Command: docker ps --format "table {{.Names}}\t{{.Status}}" | grep -E "(unhealthy|Restarting)"
OS Filter: linux
Required Capabilities: ["docker"]
Notify on Success: false
Notify on Failure: true
```

### Security and System Checks

#### Check Failed Login Attempts
```yaml
Name: failed-logins
Display Name: Failed Login Attempts
Description: Show recent failed SSH login attempts
Command: grep "Failed password" /var/log/auth.log | tail -20
OS Filter: debian,ubuntu
```

#### List Sudo Users
```yaml
Name: list-sudo-users
Display Name: List Sudo Users
Description: Show all users with sudo privileges
Command: getent group sudo | cut -d: -f4
OS Filter: linux
```

#### Check Firewall Status
```yaml
Name: firewall-status
Display Name: Firewall Status
Description: Show firewall status and rules
Command: ufw status verbose
OS Filter: debian,ubuntu
Required Capabilities: ["ufw"]
```

#### Security Updates Available
```yaml
Name: security-updates
Display Name: Security Updates Available
Description: Check for available security updates
Command: apt list --upgradable 2>/dev/null | grep -i security
OS Filter: debian,ubuntu
```

#### List Listening Services
```yaml
Name: listening-services
Display Name: Listening Services
Description: Show all services listening on network ports
Command: ss -tulnp
OS Filter: linux
```

#### Check Certificate Expiry
```yaml
Name: check-cert-expiry
Display Name: Check SSL Certificate Expiry
Description: Check when SSL certificate expires
Command: echo | openssl s_client -connect localhost:443 2>/dev/null | openssl x509 -noout -dates
OS Filter: linux
Timeout: 15
```

### File System Operations

#### Find Large Files
```yaml
Name: find-large-files
Display Name: Find Large Files
Description: Find files larger than 100MB
Command: find / -type f -size +100M -exec ls -lh {} \; 2>/dev/null | head -20
OS Filter: linux
Timeout: 120
```

#### Directory Size
```yaml
Name: directory-size
Display Name: Directory Size Report
Description: Show size of directories in /var
Command: du -sh /var/* | sort -h
OS Filter: linux
Timeout: 60
```

#### Find Old Log Files
```yaml
Name: old-log-files
Display Name: Find Old Log Files
Description: Find log files older than 90 days
Command: find /var/log -name "*.log" -type f -mtime +90
OS Filter: linux
```

#### Check Broken Symlinks
```yaml
Name: broken-symlinks
Display Name: Check Broken Symlinks
Description: Find broken symbolic links
Command: find /etc /var -xtype l
OS Filter: linux
Timeout: 60
```

### Log Analysis

#### Recent System Errors
```yaml
Name: system-errors
Display Name: Recent System Errors
Description: Show recent system errors from syslog
Command: grep -i "error\|critical" /var/log/syslog | tail -50
OS Filter: debian,ubuntu
```

#### Nginx Error Log
```yaml
Name: nginx-errors
Display Name: Nginx Error Log
Description: Show recent Nginx errors
Command: tail -50 /var/log/nginx/error.log
OS Filter: linux
Required Capabilities: ["nginx"]
```

#### Count Log Entries by Level
```yaml
Name: log-level-count
Display Name: Log Level Count
Description: Count log entries by severity level
Command: grep -E "ERROR|WARN|INFO" /var/log/syslog | cut -d' ' -f6 | sort | uniq -c
OS Filter: debian,ubuntu
```

#### Application Logs
```yaml
Name: app-logs
Display Name: Application Logs
Description: Show recent application logs
Command: tail -100 /var/log/app/application.log
OS Filter: linux
Working Directory: /var/log/app
```

### Backup Operations

#### Backup Directory
```yaml
Name: backup-directory
Display Name: Backup Directory
Description: Create compressed backup of a directory
Command: tar czf /backup/www-$(date +%Y%m%d).tar.gz -C / var/www
OS Filter: linux
Timeout: 600
Notify on Failure: true
```

#### Database Backup (PostgreSQL)
```yaml
Name: backup-postgres
Display Name: Backup PostgreSQL Database
Description: Create PostgreSQL database backup
Command: pg_dump -U postgres mydb > /backup/mydb-$(date +%Y%m%d).sql
OS Filter: linux
Required Capabilities: ["postgresql"]
Timeout: 600
```

#### Database Backup (MySQL)
```yaml
Name: backup-mysql
Display Name: Backup MySQL Database
Description: Create MySQL database backup
Command: mysqldump -u root mydb > /backup/mydb-$(date +%Y%m%d).sql
OS Filter: linux
Required Capabilities: ["mysql"]
Timeout: 600
```

#### Rotate Backups
```yaml
Name: rotate-backups
Display Name: Rotate Old Backups
Description: Delete backups older than 30 days
Command: find /backup -name "*.tar.gz" -mtime +30 -delete
OS Filter: linux
```

### Performance Testing

#### Network Speed Test (iperf)
```yaml
Name: network-speed-test
Display Name: Network Speed Test
Description: Test network throughput with iperf
Command: iperf3 -c server.example.com -t 10
OS Filter: linux
Required Capabilities: ["iperf3"]
Timeout: 30
```

#### Disk Read Speed
```yaml
Name: disk-read-speed
Display Name: Disk Read Speed Test
Description: Test disk read performance
Command: hdparm -t /dev/sda
OS Filter: linux
Required Capabilities: ["hdparm"]
```

#### Stress Test CPU
```yaml
Name: stress-test-cpu
Display Name: CPU Stress Test
Description: Run CPU stress test for 30 seconds
Command: stress-ng --cpu 2 --timeout 30s --metrics-brief
OS Filter: linux
Required Capabilities: ["stress-ng"]
Timeout: 60
```

### Database Operations

#### PostgreSQL Connection Test
```yaml
Name: postgres-connection
Display Name: PostgreSQL Connection Test
Description: Test PostgreSQL database connectivity
Command: psql -U postgres -c "SELECT version();"
OS Filter: linux
Required Capabilities: ["postgresql"]
```

#### MySQL Connection Test
```yaml
Name: mysql-connection
Display Name: MySQL Connection Test
Description: Test MySQL database connectivity
Command: mysql -u root -e "SELECT VERSION();"
OS Filter: linux
Required Capabilities: ["mysql"]
```

#### Database Size
```yaml
Name: database-size
Display Name: Database Size Report
Description: Show size of all PostgreSQL databases
Command: psql -U postgres -c "SELECT pg_database.datname, pg_size_pretty(pg_database_size(pg_database.datname)) FROM pg_database ORDER BY pg_database_size DESC;"
OS Filter: linux
Required Capabilities: ["postgresql"]
```

#### Vacuum Database
```yaml
Name: vacuum-database
Display Name: Vacuum PostgreSQL Database
Description: Run VACUUM on PostgreSQL database
Command: psql -U postgres -d mydb -c "VACUUM ANALYZE;"
OS Filter: linux
Required Capabilities: ["postgresql"]
Timeout: 1800
```

### Web Server Operations

#### Nginx Configuration Test
```yaml
Name: nginx-config-test
Display Name: Nginx Configuration Test
Description: Test Nginx configuration syntax
Command: nginx -t
OS Filter: linux
Required Capabilities: ["nginx"]
```

#### Apache Configuration Test
```yaml
Name: apache-config-test
Display Name: Apache Configuration Test
Description: Test Apache configuration syntax
Command: apachectl configtest
OS Filter: linux
Required Capabilities: ["apache"]
```

#### Reload Nginx
```yaml
Name: reload-nginx
Display Name: Reload Nginx
Description: Reload Nginx configuration without downtime
Command: nginx -s reload
OS Filter: linux
Required Capabilities: ["nginx"]
Notify on Failure: true
```

#### Check Web Server Status
```yaml
Name: check-web-status
Display Name: Check Web Server Status
Description: Check if web server is responding
Command: curl -I -s http://localhost | head -1
OS Filter: linux
Timeout: 10
```

### System Maintenance

#### Clear Package Cache
```yaml
Name: clear-package-cache
Display Name: Clear Package Cache
Description: Clear apt package cache to free space
Command: apt-get clean
OS Filter: debian,ubuntu
```

#### Update Package Database
```yaml
Name: update-package-db
Display Name: Update Package Database
Description: Update apt package database
Command: apt-get update
OS Filter: debian,ubuntu
Timeout: 300
```

#### Clear Journal Logs
```yaml
Name: clear-journal
Display Name: Clear Old Journal Logs
Description: Keep only last 7 days of journal logs
Command: journalctl --vacuum-time=7d
OS Filter: linux
Required Capabilities: ["systemd"]
```

#### Clean Temp Files
```yaml
Name: clean-temp
Display Name: Clean Temporary Files
Description: Remove old files from /tmp
Command: find /tmp -type f -atime +7 -delete
OS Filter: linux
```

### Custom Homelab Operations

#### Plex Media Server Status
```yaml
Name: plex-status
Display Name: Plex Media Server Status
Description: Check Plex Media Server status
Command: systemctl status plexmediaserver
OS Filter: linux
Required Capabilities: ["systemd"]
```

#### Pi-hole Statistics
```yaml
Name: pihole-stats
Display Name: Pi-hole Statistics
Description: Show Pi-hole blocking statistics
Command: pihole -c -e
OS Filter: linux
Required Capabilities: ["pihole"]
```

#### Home Assistant Check
```yaml
Name: homeassistant-check
Display Name: Home Assistant Configuration Check
Description: Validate Home Assistant configuration
Command: hass --script check_config
OS Filter: linux
Required Capabilities: ["homeassistant"]
```

#### Backup Media Library
```yaml
Name: backup-media
Display Name: Backup Media Library
Description: Backup media library metadata
Command: tar czf /backup/media-$(date +%Y%m%d).tar.gz /mnt/media/metadata
OS Filter: linux
Timeout: 1800
```

## Best Practices

### 1. Test Commands First

Always test commands manually before creating templates:
```bash
# SSH to server
ssh user@server

# Run command manually
df -h

# Verify output is what you expect
# Then create template
```

### 2. Use Descriptive Names

Make templates easy to find and understand:
```
✓ Good: "check-nginx-config", "backup-postgres-db"
✗ Avoid: "cmd1", "test", "script"
```

### 3. Set Appropriate Timeouts

Consider command execution time:
```
Quick checks: 10-30 seconds
Normal operations: 60-300 seconds
Backups/updates: 600-1800 seconds
```

### 4. Filter by OS When Needed

Use OS filters for OS-specific commands:
```
apt commands: debian,ubuntu
dnf commands: fedora,rhel,centos
systemctl: linux (most distributions)
```

### 5. Require Capabilities

Specify required software:
```json
["docker", "postgresql", "nginx"]
```

This prevents errors when running on servers without the required software.

### 6. Handle Errors Gracefully

Use exit codes and error handling:
```bash
# Good: Check if command succeeded
if systemctl is-active nginx; then
  echo "Nginx is running"
else
  echo "ERROR: Nginx is not running"
  exit 1
fi
```

### 7. Use Full Paths for Reliability

Specify full paths to avoid PATH issues:
```bash
# Safer
/usr/bin/docker ps

# May fail if PATH not set
docker ps
```

### 8. Limit Output Size

For long-running or verbose commands:
```bash
# Limit output
ps aux | head -20

# Or filter
journalctl -n 100
```

### 9. Test on One Server First

Before deploying to all servers:
1. Create template
2. Test on single development server
3. Verify output and behavior
4. Deploy to more servers gradually

### 10. Document Complex Commands

Use the description field:
```
Description: "Checks disk usage and alerts if any partition exceeds 80%.
Uses 'df -h' to get human-readable sizes. Parses output with awk to
check the usage percentage. Returns non-zero exit code if threshold exceeded."
```

## Troubleshooting

### Command Fails Immediately

**Symptom**: Command exits with error code immediately

**Common Causes**:

**Command Not Found**:
```
Error: "command not found"
Solution:
  - Install required package on server
  - Use full path: /usr/bin/docker instead of docker
  - Check if command is in PATH
```

**Permission Denied**:
```
Error: "Permission denied"
Solution:
  - Ensure SSH user has permission to run command
  - Add user to required group (docker, etc.)
  - Use sudo if configured
```

**Syntax Error**:
```
Error: "syntax error near unexpected token"
Solution:
  - Check command syntax
  - Escape special characters
  - Test command manually first
```

### Timeout Issues

**Symptom**: Command times out before completing

**Solutions**:
1. Increase timeout setting
2. Optimize command to run faster
3. Split into smaller operations
4. Schedule during low-usage periods

### Output Not Captured

**Symptom**: Command succeeds but no output visible

**Causes**:
- Command produces no output
- Output too large (truncated)
- Output goes to stderr not stdout

**Solutions**:
```bash
# Capture both stdout and stderr
command 2>&1

# Add explicit output
echo "Starting command..." && command && echo "Completed"

# Use verbose flags
docker ps -a  # Instead of docker ps
```

### OS Filter Not Working

**Symptom**: Command runs on wrong OS type

**Verification**:
```bash
# Check detected OS type
uname -s  # Should show "Linux"
cat /etc/os-release  # Shows distribution
```

**Solutions**:
- Verify OS filter matches server's OS
- Use "linux" for general Linux commands
- Use specific distribution names for package managers

### Environment Variables Missing

**Symptom**: Command fails due to missing environment variables

**Solution**:
Set environment variables in template:
```json
{
  "PATH": "/usr/local/bin:/usr/bin:/bin",
  "HOME": "/home/user",
  "LANG": "en_US.UTF-8"
}
```

Or set in command:
```bash
export PATH=/usr/local/bin:$PATH && command
```

### Working Directory Issues

**Symptom**: Command fails because files not found

**Solution**:
Set working directory in template or use absolute paths:
```bash
# In command
cd /var/www && ./script.sh

# Or use absolute paths
/var/www/script.sh
```

## Advanced Topics

### Multi-Line Commands

Use semicolons or && to chain commands:
```bash
# Sequential execution
cd /tmp; ls -la; pwd

# Stop on error
cd /tmp && ls -la && pwd
```

### Command Substitution

Embed command output in other commands:
```bash
echo "Current time: $(date)"
echo "Disk usage: $(df -h / | tail -1 | awk '{print $5}')"
```

### Conditional Logic

Use shell conditionals:
```bash
if [ -f /var/run/nginx.pid ]; then
  echo "Nginx is running"
else
  echo "Nginx is not running"
  exit 1
fi
```

### Looping

Process multiple items:
```bash
for container in $(docker ps -q); do
  docker inspect $container
done
```

### Output Redirection

Control where output goes:
```bash
# Redirect stdout
command > output.txt

# Redirect stderr
command 2> errors.txt

# Redirect both
command > output.txt 2>&1

# Discard output
command > /dev/null 2>&1
```

## FAQ

### Can I run sudo commands?

Yes, but the SSH user must have passwordless sudo configured:
```bash
# In command template
sudo systemctl restart nginx
```

Configure on server:
```bash
# Add to /etc/sudoers.d/svrctlrs
svrctlrs-user ALL=(ALL) NOPASSWD: /usr/bin/systemctl
```

### How do I handle sensitive data?

Never include passwords or keys in command templates. Use:
- Environment variables
- Configuration files with restricted permissions
- SSH key-based authentication
- Credential management systems

### Can I use interactive commands?

No. Commands must run non-interactively. Use:
- Non-interactive flags (`-y`, `--no-input`)
- Here documents for input
- Configuration files instead of prompts

### What happens if a command runs forever?

The command will be terminated when it reaches the timeout setting. Default is 300 seconds (5 minutes). Set an appropriate timeout for your command.

### Can I use pipes and redirects?

Yes! Standard shell syntax works:
```bash
ps aux | grep nginx | wc -l
find /tmp -type f > /tmp/files.txt
```

### How do I debug command failures?

1. Check job run details for output
2. Run command manually on the server
3. Check system logs
4. Increase timeout if needed
5. Verify permissions and environment

---

**Need Help?**
- Report issues: https://github.com/jsprague84/svrctlrs/issues
- Test commands: SSH to server and run manually first
- Check logs: View job run details for command output and errors
