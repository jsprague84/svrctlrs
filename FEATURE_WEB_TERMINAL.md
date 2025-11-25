# Feature: Web-Based SSH Terminal

## Overview

Add a web-based terminal interface for each server, allowing users to execute commands directly from the UI using the existing SSH connections.

**Status:** Planned for future release  
**Priority:** 🟢 Medium (after weatherust feature parity)  
**Complexity:** Medium-High

---

## User Story

**As a** system administrator  
**I want** to run commands on remote servers from the web UI  
**So that** I don't need to open a separate SSH client

---

## Use Cases

1. **Quick Commands**
   - Check disk space: `df -h`
   - View logs: `tail -f /var/log/syslog`
   - Check processes: `ps aux | grep nginx`

2. **Troubleshooting**
   - Debug issues without leaving the UI
   - Run diagnostic commands
   - Check service status

3. **One-Off Tasks**
   - Restart services
   - Clear caches
   - Run maintenance scripts

4. **Learning/Training**
   - Safe environment for junior admins
   - Command history for reference
   - Audit trail of commands run

---

## Technical Design

### Architecture

```
┌─────────────────────────────────────────┐
│         Browser (User)                  │
│  ┌───────────────────────────────────┐  │
│  │  Terminal UI (xterm.js)           │  │
│  │  - Input commands                 │  │
│  │  - Display output                 │  │
│  │  - Handle colors/formatting       │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
                  │
                  │ WebSocket
                  ▼
┌─────────────────────────────────────────┐
│         SvrCtlRS Server                 │
│  ┌───────────────────────────────────┐  │
│  │  WebSocket Handler                │  │
│  │  - Authenticate user              │  │
│  │  - Manage sessions                │  │
│  │  - Route commands                 │  │
│  └───────────────────────────────────┘  │
│                │                         │
│                ▼                         │
│  ┌───────────────────────────────────┐  │
│  │  SSH Connection Pool              │  │
│  │  - Reuse existing connections     │  │
│  │  - Handle multiple sessions       │  │
│  │  - Execute commands               │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
                  │
                  │ SSH
                  ▼
┌─────────────────────────────────────────┐
│         Remote Server                   │
│  - Execute commands                     │
│  - Return stdout/stderr                 │
│  - Handle interactive input             │
└─────────────────────────────────────────┘
```

### Frontend

**Technology:** xterm.js (industry standard web terminal)
- https://xtermjs.org/
- Used by VS Code, Jupyter, etc.
- Full terminal emulation
- Color support, cursor control
- Copy/paste support

**Implementation:**
```javascript
// server/static/js/terminal.js
import { Terminal } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import { WebLinksAddon } from 'xterm-addon-web-links';

const term = new Terminal({
    cursorBlink: true,
    fontSize: 14,
    fontFamily: 'Menlo, Monaco, "Courier New", monospace',
    theme: {
        background: '#2e3440',  // Nord theme
        foreground: '#d8dee9',
    }
});

const fitAddon = new FitAddon();
term.loadAddon(fitAddon);
term.loadAddon(new WebLinksAddon());

// Connect to WebSocket
const ws = new WebSocket(`wss://${location.host}/api/v1/terminal/${serverId}`);

ws.onmessage = (event) => {
    term.write(event.data);
};

term.onData((data) => {
    ws.send(data);
});

term.open(document.getElementById('terminal'));
fitAddon.fit();
```

### Backend

**WebSocket Handler:**
```rust
// server/src/routes/terminal.rs
use axum::{
    extract::{ws::WebSocket, Path, State, WebSocketUpgrade},
    response::Response,
};
use async_ssh2_tokio::{Session, Channel};

pub async fn terminal_handler(
    ws: WebSocketUpgrade,
    Path(server_id): Path<i64>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_terminal_socket(socket, server_id, state))
}

async fn handle_terminal_socket(
    socket: WebSocket,
    server_id: i64,
    state: AppState,
) {
    // 1. Load server from database
    let server = load_server(&state, server_id).await?;
    
    // 2. Establish SSH connection
    let session = connect_ssh(&server).await?;
    
    // 3. Open shell channel
    let mut channel = session.channel_session().await?;
    channel.request_pty("xterm", 80, 24, None).await?;
    channel.shell().await?;
    
    // 4. Bridge WebSocket <-> SSH channel
    tokio::select! {
        _ = forward_ws_to_ssh(&socket, &channel) => {},
        _ = forward_ssh_to_ws(&channel, &socket) => {},
    }
}

async fn forward_ws_to_ssh(
    ws: &WebSocket,
    channel: &Channel,
) -> Result<()> {
    while let Some(msg) = ws.recv().await {
        if let Ok(text) = msg?.to_text() {
            channel.write_all(text.as_bytes()).await?;
        }
    }
    Ok(())
}

async fn forward_ssh_to_ws(
    channel: &Channel,
    ws: &WebSocket,
) -> Result<()> {
    let mut buf = [0u8; 4096];
    loop {
        let n = channel.read(&mut buf).await?;
        if n == 0 { break; }
        ws.send(Message::Text(
            String::from_utf8_lossy(&buf[..n]).to_string()
        )).await?;
    }
    Ok(())
}
```

### UI Integration

**New Page: `/servers/{id}/terminal`**

```html
<!-- server/templates/pages/server_terminal.html -->
{% extends "base.html" %}

{% block content %}
<div class="terminal-page">
    <div class="terminal-header">
        <h1>Terminal: {{ server.name }}</h1>
        <div class="terminal-controls">
            <button id="clear-btn">Clear</button>
            <button id="disconnect-btn">Disconnect</button>
            <select id="font-size">
                <option value="12">12px</option>
                <option value="14" selected>14px</option>
                <option value="16">16px</option>
            </select>
        </div>
    </div>
    
    <div id="terminal-container"></div>
    
    <div class="terminal-info">
        <span>Connected to: {{ server.host }}</span>
        <span>User: {{ server.username }}</span>
        <span class="status" id="connection-status">Connected</span>
    </div>
</div>

<script src="/static/js/xterm.js"></script>
<script src="/static/js/terminal.js"></script>
{% endblock %}
```

**Add to Server List:**
```html
<!-- Add terminal button to server list -->
<a href="/servers/{{ server.id }}/terminal" 
   class="btn btn-secondary"
   title="Open Terminal">
    <i class="icon-terminal"></i> Terminal
</a>
```

---

## Security Considerations

### 1. Authentication Required
- Must be logged in (v1.1.0+ with auth)
- Check user permissions for server access
- Audit log all terminal sessions

### 2. Command Logging
```rust
// Log all commands executed
struct TerminalSession {
    id: i64,
    user_id: i64,
    server_id: i64,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    commands: Vec<String>,  // Store command history
}
```

### 3. Session Timeout
- Auto-disconnect after 30 minutes of inactivity
- Configurable per-user or per-server
- Warning before disconnect

### 4. Read-Only Mode (Optional)
- Allow view-only terminal sessions
- Useful for training/demos
- Block all input, only show output

### 5. Command Restrictions (Optional)
- Blacklist dangerous commands (`rm -rf /`, `dd`, etc.)
- Whitelist mode for restricted users
- Require confirmation for destructive commands

---

## Features

### Phase 1: Basic Terminal (v1.2.0)
- [x] WebSocket connection
- [x] SSH channel bridging
- [x] Basic terminal UI (xterm.js)
- [x] Command execution
- [x] Output display
- [x] Connection status

### Phase 2: Enhanced Features (v1.3.0)
- [ ] Multiple terminal tabs
- [ ] Terminal history
- [ ] Command autocomplete
- [ ] File upload/download
- [ ] Copy/paste support
- [ ] Fullscreen mode

### Phase 3: Advanced Features (v2.0.0)
- [ ] Command logging/audit
- [ ] Session recording/playback
- [ ] Collaborative sessions (multiple users)
- [ ] Command restrictions
- [ ] Custom key bindings
- [ ] Terminal themes

---

## Dependencies

### Frontend
```json
{
  "xterm": "^5.3.0",
  "xterm-addon-fit": "^0.8.0",
  "xterm-addon-web-links": "^0.9.0",
  "xterm-addon-search": "^0.13.0"
}
```

### Backend
```toml
[dependencies]
# Already have:
async-ssh2-tokio = "0.8"
tokio = { version = "1", features = ["full"] }
axum = { version = "0.8", features = ["ws"] }

# May need:
tokio-tungstenite = "0.21"  # WebSocket support
```

---

## Implementation Plan

### Week 1: Basic Terminal
1. **Day 1-2:** WebSocket handler
   - Create `/api/v1/terminal/{server_id}` endpoint
   - Handle WebSocket upgrade
   - Basic message passing

2. **Day 3-4:** SSH integration
   - Open SSH shell channel
   - Bridge WebSocket <-> SSH
   - Handle input/output

3. **Day 5:** Frontend
   - Integrate xterm.js
   - Create terminal page
   - Test basic commands

### Week 2: Polish & Testing
1. **Day 1-2:** UI improvements
   - Styling (Nord theme)
   - Connection status
   - Error handling

2. **Day 3-4:** Features
   - Clear terminal
   - Font size control
   - Disconnect button

3. **Day 5:** Testing
   - Test with multiple servers
   - Test long-running commands
   - Test connection drops

---

## User Documentation

### How to Use

1. **Open Terminal**
   - Go to Servers page
   - Click "Terminal" button for desired server
   - Terminal opens in new page/tab

2. **Run Commands**
   - Type commands as you would in SSH
   - Press Enter to execute
   - Output appears in real-time

3. **Copy/Paste**
   - Copy: Select text, Ctrl+C (or Cmd+C)
   - Paste: Ctrl+V (or Cmd+V)

4. **Disconnect**
   - Click "Disconnect" button
   - Or close browser tab
   - Session auto-closes after 30 minutes

### Tips

- Use `clear` to clear the terminal
- Use `exit` to close the SSH session
- Terminal supports colors and formatting
- Command history: Use Up/Down arrows

---

## Alternatives Considered

### 1. Gotty (External Tool)
**Pros:**
- Mature, battle-tested
- Standalone binary

**Cons:**
- External dependency
- Harder to integrate
- Less control

### 2. ttyd (External Tool)
**Pros:**
- Lightweight
- WebSocket-based

**Cons:**
- C-based, harder to modify
- Separate process

### 3. Built-in (Recommended)
**Pros:**
- Full control
- Integrated auth
- Consistent UI
- No external deps

**Cons:**
- More code to maintain
- Need to implement features

**Decision:** Build it in-house for better integration

---

## Testing Plan

### Manual Testing
1. Connect to server
2. Run basic commands (`ls`, `pwd`, `whoami`)
3. Run long commands (`apt update`)
4. Test colors (`ls --color=auto`)
5. Test interactive commands (`top`, `htop`)
6. Test large output (`cat /var/log/syslog`)
7. Test connection drop recovery
8. Test multiple simultaneous sessions

### Automated Testing
```rust
#[tokio::test]
async fn test_terminal_connection() {
    let app = create_test_app().await;
    let ws = connect_terminal(&app, server_id).await;
    
    // Send command
    ws.send("echo hello").await.unwrap();
    
    // Receive output
    let output = ws.recv().await.unwrap();
    assert!(output.contains("hello"));
}
```

---

## Future Enhancements

1. **File Manager Integration**
   - Upload files via terminal
   - Download files to browser
   - Drag-and-drop support

2. **Session Recording**
   - Record terminal sessions
   - Replay for training/debugging
   - Export as video or text

3. **Collaborative Mode**
   - Multiple users in same terminal
   - See each other's commands
   - Chat sidebar

4. **AI Assistant**
   - Command suggestions
   - Error explanations
   - Auto-fix common issues

5. **Mobile Support**
   - Touch-friendly keyboard
   - Swipe gestures
   - Mobile-optimized layout

---

## References

- **xterm.js:** https://xtermjs.org/
- **SSH2 Rust:** https://docs.rs/async-ssh2-tokio/
- **Axum WebSockets:** https://docs.rs/axum/latest/axum/extract/ws/
- **Similar Projects:**
  - Gotty: https://github.com/yudai/gotty
  - ttyd: https://github.com/tsl0922/ttyd
  - Wetty: https://github.com/butlerx/wetty

---

**Created:** 2025-11-25  
**Target Release:** v1.2.0 or v1.3.0 (after weatherust feature parity)  
**Estimated Effort:** 2 weeks (1 developer)

