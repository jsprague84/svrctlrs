# Embedded Terminal Debug Tool - Research & Design

**Date**: 2024-12-01
**Status**: Research Phase
**Technologies**: Rust, Axum, HTMX, Askama, WebSockets, xterm.js

## Executive Summary

This document outlines research findings and recommendations for implementing an embedded web-based terminal debug tool within the SvrCtlRS application. The feature would allow users to test command templates interactively, view real-time output, and debug commands before creating job templates.

## Use Cases

1. **Command Template Testing**: Test commands before creating job templates
2. **Output Preview**: See actual command output when designing notifications
3. **Server Selection**: Choose which server to run the test command on
4. **Real-time Feedback**: View command execution in real-time with full terminal output
5. **Debug Notifications**: Test notification templates with actual command output

## Technology Stack Analysis

### 1. Frontend: xterm.js

**Library**: `@xterm/xterm` (v5.x+)
**License**: MIT
**Reputation**: High - Used by VS Code, Hyper, and other major terminals

#### Key Features
- Full VT100/xterm terminal emulation
- WebSocket integration via `AttachAddon`
- Automatic resizing with `FitAddon`
- ANSI color support
- Copy/paste, scrollback, search
- Small bundle size (~150KB minified)

#### Integration Pattern
```javascript
import { Terminal } from '@xterm/xterm';
import { AttachAddon } from '@xterm/addon-attach';
import { FitAddon } from '@xterm/addon-fit';

const terminal = new Terminal({
  cursorBlink: true,
  fontSize: 14,
  theme: {
    background: '#2e3440',  // Nord theme colors
    foreground: '#d8dee9',
  }
});

const fitAddon = new FitAddon();
terminal.loadAddon(fitAddon);
terminal.open(document.getElementById('terminal'));
fitAddon.fit();

// Connect to WebSocket
const socket = new WebSocket('ws://localhost:8080/terminal');
socket.onopen = () => {
  const attachAddon = new AttachAddon(socket);
  terminal.loadAddon(attachAddon);

  // Send initial size
  const { cols, rows } = terminal;
  socket.send(JSON.stringify({ type: 'resize', cols, rows }));
};

// Handle resize
terminal.onResize(({ cols, rows }) => {
  if (socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: 'resize', cols, rows }));
  }
});
```

### 2. Backend: Axum WebSocket Handler

**Pattern**: WebSocket upgrade with state management
**Dependencies**: `axum`, `tokio`, `tokio-tungstenite`

#### WebSocket Route Handler
```rust
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{sink::SinkExt, stream::StreamExt};

async fn terminal_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_terminal_socket(socket, state))
}

async fn handle_terminal_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Parse command from frontend
                if let Ok(cmd) = serde_json::from_str::<TerminalCommand>(&text) {
                    match cmd.command_type.as_str() {
                        "execute" => {
                            // Execute command via RemoteExecutor
                            execute_command_streaming(
                                &state,
                                &cmd,
                                &mut sender
                            ).await;
                        }
                        "resize" => {
                            // Handle terminal resize
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }
}
```

### 3. Command Execution: Streaming Output

**Approach**: Real-time streaming via SSH with PTY allocation

```rust
async fn execute_command_streaming(
    state: &AppState,
    cmd: &TerminalCommand,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> Result<()> {
    let server = state.db.get_server(cmd.server_id).await?;

    // Use RemoteExecutor with PTY for interactive output
    let mut child = state.remote_executor
        .execute_with_pty(&server, &cmd.command)
        .await?;

    // Stream stdout in real-time
    let mut stdout = child.stdout.take().unwrap();
    let mut buffer = vec![0u8; 1024];

    while let Ok(n) = stdout.read(&mut buffer).await {
        if n == 0 { break; }

        // Send output to terminal
        let output = String::from_utf8_lossy(&buffer[..n]);
        sender.send(Message::Text(output.to_string()))
            .await
            .ok();
    }

    // Send completion status
    let status = child.wait().await?;
    let status_msg = format!("\r\n[Process exited with code: {}]\r\n",
        status.code().unwrap_or(-1));
    sender.send(Message::Text(status_msg)).await.ok();

    Ok(())
}
```

### 4. HTMX Integration

**Pattern**: Modal dialog with embedded terminal

#### Askama Template
```html
<!-- server/templates/components/terminal_test_modal.html -->
<div id="terminal-modal" class="modal" x-data="{ open: false }" x-show="open">
    <div class="modal-content">
        <div class="modal-header">
            <h3>Test Command</h3>
            <button @click="open = false">&times;</button>
        </div>

        <div class="modal-body">
            <!-- Server Selection -->
            <div class="form-group">
                <label>Server</label>
                <select id="terminal-server-select" class="form-control">
                    {% for server in servers %}
                    <option value="{{ server.id }}">{{ server.name }}</option>
                    {% endfor %}
                </select>
            </div>

            <!-- Command Input -->
            <div class="form-group">
                <label>Command</label>
                <input type="text"
                       id="terminal-command-input"
                       class="form-control"
                       value="{{ command }}"
                       placeholder="Enter command to test...">
            </div>

            <!-- Execute Button -->
            <button id="terminal-execute-btn" class="btn btn-primary">
                <i data-lucide="play"></i> Execute
            </button>

            <!-- Terminal Output -->
            <div id="terminal-container" style="height: 400px; margin-top: 1rem;"></div>
        </div>
    </div>
</div>
```

#### JavaScript Integration
```javascript
// server/static/js/terminal.js
class TerminalManager {
    constructor() {
        this.terminal = null;
        this.socket = null;
    }

    init(containerId) {
        // Initialize xterm.js
        this.terminal = new Terminal({
            cursorBlink: true,
            fontSize: 14,
            fontFamily: 'Menlo, Monaco, "Courier New", monospace',
            theme: {
                background: '#2e3440',
                foreground: '#d8dee9',
                cursor: '#d8dee9',
                black: '#3b4252',
                red: '#bf616a',
                green: '#a3be8c',
                yellow: '#ebcb8b',
                blue: '#81a1c1',
                magenta: '#b48ead',
                cyan: '#88c0d0',
                white: '#e5e9f0',
            }
        });

        const fitAddon = new FitAddon();
        this.terminal.loadAddon(fitAddon);

        const container = document.getElementById(containerId);
        this.terminal.open(container);
        fitAddon.fit();

        // Handle window resize
        window.addEventListener('resize', () => fitAddon.fit());

        return this;
    }

    connect(serverId, command) {
        // Close existing connection
        if (this.socket) {
            this.socket.close();
        }

        // Connect to WebSocket
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/terminal`;

        this.socket = new WebSocket(wsUrl);

        this.socket.onopen = () => {
            console.log('Terminal WebSocket connected');

            // Attach terminal to socket
            const attachAddon = new AttachAddon(this.socket);
            this.terminal.loadAddon(attachAddon);

            // Send execute command
            const { cols, rows } = this.terminal;
            this.socket.send(JSON.stringify({
                type: 'execute',
                server_id: serverId,
                command: command,
                cols: cols,
                rows: rows
            }));
        };

        this.socket.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.terminal.writeln('\r\n\x1b[31mConnection error\x1b[0m');
        };

        this.socket.onclose = () => {
            console.log('WebSocket closed');
            this.terminal.writeln('\r\n\x1b[33m[Connection closed]\x1b[0m');
        };

        // Handle terminal resize
        this.terminal.onResize(({ cols, rows }) => {
            if (this.socket && this.socket.readyState === WebSocket.OPEN) {
                this.socket.send(JSON.stringify({
                    type: 'resize',
                    cols: cols,
                    rows: rows
                }));
            }
        });
    }

    disconnect() {
        if (this.socket) {
            this.socket.close();
            this.socket = null;
        }
    }

    clear() {
        if (this.terminal) {
            this.terminal.clear();
        }
    }
}

// Global terminal manager
const terminalManager = new TerminalManager();

// Execute button handler
document.getElementById('terminal-execute-btn')?.addEventListener('click', () => {
    const serverId = document.getElementById('terminal-server-select').value;
    const command = document.getElementById('terminal-command-input').value;

    if (!serverId || !command) {
        alert('Please select a server and enter a command');
        return;
    }

    terminalManager.clear();
    terminalManager.connect(serverId, command);
});
```

## Architecture Recommendations

### Approach 1: Modal Dialog (Recommended)

**Pros**:
- Non-intrusive, appears on demand
- Works well with existing HTMX flow
- Easy to implement
- Can be triggered from command template form

**Cons**:
- Limited screen real estate
- Cannot view terminal while editing

**Implementation**:
```html
<!-- Add to command_template_form.html -->
<button hx-get="/command-templates/{{ template.id }}/test"
        hx-target="#terminal-modal"
        hx-swap="innerHTML"
        class="btn btn-sm btn-secondary"
        @click="$dispatch('open-terminal')">
    <i data-lucide="terminal"></i> Test Command
</button>
```

### Approach 2: Dedicated Debug Page

**Pros**:
- Full-screen terminal
- Can have multiple terminals side-by-side
- Better for complex debugging

**Cons**:
- Requires navigation away from form
- More complex state management

### Approach 3: Split-Screen Panel

**Pros**:
- View terminal while editing
- Immediate feedback

**Cons**:
- Complex UI layout
- May feel cluttered on smaller screens

## Recommended Implementation Plan

### Phase 1: Basic Modal Terminal (MVP)
1. Add WebSocket route to Axum server
2. Create terminal modal component with Askama
3. Integrate xterm.js with AttachAddon
4. Implement basic command execution via RemoteExecutor
5. Add "Test" button to command template form

### Phase 2: Enhanced Features
1. Command history (up/down arrows)
2. Environment variable preview
3. Working directory support
4. Timeout handling
5. Output capture for notification testing

### Phase 3: Advanced Features
1. Multiple simultaneous connections
2. Terminal session persistence
3. File upload/download
4. Shared terminal sessions for collaboration

## Security Considerations

1. **Authentication**: Verify user session before WebSocket upgrade
2. **Authorization**: Ensure user has access to selected server
3. **Command Validation**: Sanitize commands (especially dangerous ones)
4. **Rate Limiting**: Prevent abuse of terminal connections
5. **Audit Logging**: Log all terminal sessions for security review

```rust
async fn terminal_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    session: Session,  // From axum-sessions or similar
) -> Result<impl IntoResponse, StatusCode> {
    // Verify authentication
    let user_id = session.get::<i64>("user_id")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Log terminal session start
    info!("Terminal session started by user {}", user_id);

    Ok(ws.on_upgrade(move |socket| {
        handle_terminal_socket(socket, state, user_id)
    }))
}
```

## Dependencies Required

### Cargo.toml
```toml
# Existing dependencies
tokio = { version = "1", features = ["full"] }
axum = "0.8"

# WebSocket dependencies (may already be present)
tokio-tungstenite = "0.24"
futures-util = "0.3"

# PTY support for interactive terminals
portable-pty = "0.8"  # Cross-platform PTY

# SSH with PTY support
async-ssh2-tokio = "0.8"  # If not already using
```

### Frontend Dependencies (CDN or npm)
```html
<!-- xterm.js from CDN -->
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.3.0/css/xterm.css" />
<script src="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.3.0/lib/xterm.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-attach@0.9.0/lib/xterm-addon-attach.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10.0/lib/xterm-addon-fit.js"></script>
```

Or via npm:
```bash
npm install @xterm/xterm @xterm/addon-attach @xterm/addon-fit
```

## Performance Considerations

1. **WebSocket Connection Limits**:
   - Limit concurrent terminal connections per user
   - Implement connection pooling if needed

2. **Output Buffering**:
   - Buffer output to reduce WebSocket message frequency
   - Use binary messages for large outputs

3. **Memory Management**:
   - Set terminal scrollback limits
   - Clean up terminated sessions promptly

4. **Network Efficiency**:
   - Compress WebSocket messages if supported
   - Use binary frames for non-text data

## Testing Strategy

1. **Unit Tests**: Test command execution logic
2. **Integration Tests**: Test WebSocket handlers
3. **E2E Tests**: Test full terminal workflow with Playwright
4. **Load Tests**: Test concurrent terminal sessions
5. **Security Tests**: Test authentication bypass attempts

## Alternative Approaches Considered

### 1. Server-Sent Events (SSE)
**Rejected**: One-way communication, no input support

### 2. Long Polling
**Rejected**: Too much overhead, poor latency

### 3. HTTP Streaming
**Rejected**: Browser compatibility issues, no bidirectional

### 4. Pure HTMX (No WebSocket)
**Rejected**: Cannot provide real-time terminal experience

## References

- [xterm.js Documentation](https://xtermjs.org/)
- [Axum WebSocket Guide](https://docs.rs/axum/latest/axum/extract/ws/)
- [tokio-tungstenite Examples](https://github.com/snapview/tokio-tungstenite)
- [PTY in Rust](https://docs.rs/portable-pty/)
- [Modern Terminal Emulators](https://github.com/xtermjs/xterm.js)

## Next Steps

1. Review and approve this research document
2. Create detailed implementation plan with milestones
3. Set up development environment with xterm.js
4. Build MVP: Basic modal terminal with command execution
5. Iterate based on user feedback

---

**Status**: Ready for implementation planning
**Estimated Complexity**: Medium (3-5 days for MVP)
**Risk Level**: Low (well-established technologies)
