# Docker Monitoring Guide

SvrCtlRS supports Docker monitoring in two ways:

## 1. Local Docker Monitoring (docker-vm itself)

Monitor Docker containers running on the same host as the SvrCtlRS container.

### Setup

```bash
# Find your docker group GID
stat -c '%g' /var/run/docker.sock
# Example output: 999

# Edit docker-compose.yml
# Change: user: "1000:1000"
# To:     user: "1000:999"  (use your GID)

# Restart
docker compose restart svrctlrs
```

### Usage

The Docker plugin will automatically monitor local containers when enabled at `/plugins`.

**What it monitors:**
- Container health status
- Resource usage (CPU, memory)
- Unhealthy containers
- Container restarts
- Image updates available

## 2. Remote Docker Monitoring (other servers)

Monitor Docker on remote servers via SSH - **no Docker socket needed**.

### Setup

1. **Add Remote Server** (UI: `/servers`)
   - Name: `server1`
   - Host: `192.168.1.100`
   - Username: `ubuntu`
   - Port: `22`
   - Ensure passwordless SSH is configured

2. **Create Remote Docker Task** (UI: `/tasks`)
   - Name: `Docker Health (server1)`
   - Server: `server1` (select from dropdown)
   - Command: `docker ps --format "{{.Names}}: {{.Status}}"`
   - Schedule: `0 */5 * * * *` (every 5 minutes)
   - Enabled: ✓

### Common Remote Docker Commands

```bash
# List containers with status
docker ps --format "{{.Names}}: {{.Status}}"

# Container stats (one-time snapshot)
docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemPerc}}"

# Check for unhealthy containers
docker ps --filter health=unhealthy --format "{{.Names}}"

# List images
docker images --format "{{.Repository}}:{{.Tag}} - {{.Size}}"

# Disk usage
docker system df

# Check for updates (compare local vs remote tags)
docker images --format "{{.Repository}}:{{.Tag}}"
```

## Recommended Setup

**For most users**: Use remote monitoring via SSH for ALL servers (including docker-vm).

### Why?

✅ **Simpler**: No Docker socket permissions needed  
✅ **Consistent**: Same approach for all servers  
✅ **Secure**: SSH is already configured  
✅ **Flexible**: Can run any Docker command  

### How?

1. Enable SSH on docker-vm: `sudo systemctl enable ssh`
2. Add docker-vm as a server in UI: `localhost` or `127.0.0.1`
3. Create tasks with `server_id` set to docker-vm
4. No Docker socket mount needed!

## Comparison

| Feature | Local (Socket) | Remote (SSH) |
|---------|---------------|--------------|
| Setup | Manual GID config | Add server in UI |
| Permissions | Docker socket access | SSH access |
| Performance | Faster (direct API) | Slightly slower (SSH) |
| Features | Full Bollard API | Docker CLI commands |
| Security | Socket access risk | SSH only |
| Flexibility | docker-vm only | Any server |
| **Recommended** | ❌ Complex | ✅ Simple |

## Troubleshooting

### Local Monitoring: "Permission denied" on Docker socket

```bash
# Check socket permissions
ls -la /var/run/docker.sock

# Check container user GID
docker compose exec svrctlrs id

# Verify GID matches in docker-compose.yml
grep "user:" docker-compose.yml
```

### Remote Monitoring: SSH connection failed

```bash
# Test SSH from docker-vm
ssh ubuntu@server1 docker ps

# Check SSH keys are mounted
docker compose exec svrctlrs ls -la /home/svrctlrs/.ssh

# Verify server is enabled in UI
# Go to /servers and check the server is enabled
```

### Remote Monitoring: "docker: command not found"

The remote server doesn't have Docker installed, or Docker isn't in the PATH for non-interactive SSH sessions.

```bash
# Test on remote server
ssh ubuntu@server1 'which docker'

# If empty, add to PATH in ~/.bashrc or use full path
ssh ubuntu@server1 '/usr/bin/docker ps'
```

## Best Practices

1. **Use SSH for everything** - simpler and more consistent
2. **Create one task per server** - easier to track and debug
3. **Use descriptive task names** - "Docker Health (server1)"
4. **Start with basic commands** - `docker ps` before complex stats
5. **Test SSH manually first** - `ssh user@host docker ps`
6. **Monitor task history** - check `/tasks` for execution results

## Example: Complete Remote Setup

```bash
# 1. On docker-vm, test SSH to remote server
ssh ubuntu@server1 docker ps

# 2. In UI (/servers), add server:
#    Name: server1
#    Host: 192.168.1.100
#    Username: ubuntu
#    Port: 22

# 3. In UI (/tasks), create task:
#    Name: Docker Health (server1)
#    Server: server1
#    Command: docker ps --format "{{.Names}}: {{.Status}}"
#    Schedule: 0 */5 * * * *

# 4. Test manually:
#    Click "Run Now" on the task

# 5. Check results:
#    View task history at /tasks
```

Done! Docker monitoring is now running via SSH with zero Docker socket complexity.

