# Terminal Implementation Plan

**Date**: 2024-12-01
**Status**: Implementation Ready
**Goal**: Build a modern, responsive web terminal that's more convenient than manual SSH sessions

## Vision: Better Than Manual SSH

### Why Users Will Prefer the Web Terminal

1. **No Context Switching**: Stay in the browser, no terminal app switching
2. **Server Selection UI**: Click dropdown vs. remembering SSH connection strings
3. **Command Templates**: Pre-filled commands from templates, not copy/paste
4. **Session History**: See what was run across all servers in one place
5. **Multi-Server Execution**: Run same command on multiple servers simultaneously
6. **Credential Management**: No SSH key juggling, centrally managed
7. **Output Capture**: Automatic logging and notification integration
8. **Shareable Sessions**: Team members can view/share terminal sessions

## Implementation Phases

### Phase 1: MVP - Single Terminal Modal (Week 1)

**Goal**: Test command templates from the command template form

**User Story**:
> "When editing a command template, I click 'Test', select a server, and see the command execute in a real terminal. I can modify the command and run it again to verify it works before saving."

**Features**:
- Modal dialog with embedded terminal
- Server selection dropdown
- Command pre-filled from template
- Real-time output with xterm.js
- ANSI color support
- Scrollback buffer (1000 lines)
- Copy/paste support

**UI Integration Points**:
1. Command Template Form → Test button → Terminal Modal
2. Job Template Form → Test button → Terminal Modal (future)

### Phase 2: Enhanced Terminal (Week 2)

**Goal**: Make it actually better than SSH

**Features**:
- Command history (up/down arrows)
- Tab completion (basic)
- Environment variable preview/editing
- Working directory support
- Multi-line commands (Shift+Enter)
- Output search (Ctrl+F)
- Download output as .txt
- Notification testing integration

### Phase 3: Multi-Terminal Debug Page (Week 3+)

**Goal**: Professional debugging interface for power users

**Features**:
- Dedicated `/debug` page
- Multiple terminals (2-4 simultaneous)
- Split-screen layouts (horizontal/vertical)
- Terminal tabs
- Session persistence
- Broadcast mode (send command to all terminals)
- Server groups (run on all web servers, all DB servers, etc.)
- Terminal profiles (saved configurations)

## Detailed Implementation: Phase 1 MVP

### 1. Backend: WebSocket Handler

**File**: `server/src/routes/terminal.rs`

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
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

// Terminal command message from frontend
#[derive(Deserialize)]
struct TerminalRequest {
    #[serde(rename = "type")]
    request_type: String,
    server_id: Option<i64>,
    command: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
}

// Terminal response message to frontend
#[derive(Serialize)]
struct TerminalResponse {
    #[serde(rename = "type")]
    response_type: String,
    data: String,
    exit_code: Option<i32>,
}

pub fn routes() -> Router<crate::state::AppState> {
    Router::new()
        .route("/ws/terminal", get(terminal_ws_handler))
}

async fn terminal_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<crate::state::AppState>,
) -> impl IntoResponse {
    // TODO: Add session authentication here
    ws.on_upgrade(move |socket| handle_terminal_socket(socket, state))
}

async fn handle_terminal_socket(
    socket: WebSocket,
    state: crate::state::AppState,
) {
    let (mut sender, mut receiver) = socket.split();

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<TerminalRequest>(&text) {
                    Ok(req) => match req.request_type.as_str() {
                        "execute" => {
                            if let (Some(server_id), Some(command)) = (req.server_id, req.command) {
                                execute_command(
                                    &state,
                                    server_id,
                                    &command,
                                    &mut sender,
                                ).await;
                            }
                        }
                        "resize" => {
                            // Handle terminal resize
                            // TODO: Implement PTY resize
                        }
                        _ => {
                            eprintln!("Unknown request type: {}", req.request_type);
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to parse terminal request: {}", e);
                    }
                }
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }
}

async fn execute_command(
    state: &crate::state::AppState,
    server_id: i64,
    command: &str,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) {
    // Get server from database
    let server = match state.db.get_server(server_id).await {
        Ok(s) => s,
        Err(e) => {
            send_error(sender, &format!("Failed to get server: {}", e)).await;
            return;
        }
    };

    // Execute via RemoteExecutor
    match state.remote_executor.execute(&server, command).await {
        Ok(output) => {
            // Send stdout
            if !output.stdout.is_empty() {
                send_output(sender, &output.stdout).await;
            }

            // Send stderr (in red)
            if !output.stderr.is_empty() {
                let stderr_colored = format!("\x1b[31m{}\x1b[0m", output.stderr);
                send_output(sender, &stderr_colored).await;
            }

            // Send exit code
            send_exit_code(sender, output.exit_code).await;
        }
        Err(e) => {
            send_error(sender, &format!("Command execution failed: {}", e)).await;
        }
    }
}

async fn send_output(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    data: &str,
) {
    let response = TerminalResponse {
        response_type: "output".to_string(),
        data: data.to_string(),
        exit_code: None,
    };

    if let Ok(json) = serde_json::to_string(&response) {
        sender.send(Message::Text(json)).await.ok();
    }
}

async fn send_exit_code(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    exit_code: i32,
) {
    let status_msg = if exit_code == 0 {
        format!("\r\n\x1b[32m[Process exited with code: {}]\x1b[0m\r\n", exit_code)
    } else {
        format!("\r\n\x1b[31m[Process exited with code: {}]\x1b[0m\r\n", exit_code)
    };

    let response = TerminalResponse {
        response_type: "output".to_string(),
        data: status_msg,
        exit_code: Some(exit_code),
    };

    if let Ok(json) = serde_json::to_string(&response) {
        sender.send(Message::Text(json)).await.ok();
    }
}

async fn send_error(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    error: &str,
) {
    let error_msg = format!("\r\n\x1b[31mError: {}\x1b[0m\r\n", error);
    send_output(sender, &error_msg).await;
}
```

### 2. Frontend: Terminal Modal Component

**File**: `server/templates/components/terminal_modal.html`

```html
<!-- Terminal Modal Component -->
<div id="terminal-modal"
     class="modal"
     x-data="terminalModal()"
     x-show="open"
     @open-terminal.window="openTerminal($event.detail)"
     @keydown.escape.window="closeTerminal()"
     style="display: none;">

    <div class="modal-overlay" @click="closeTerminal()"></div>

    <div class="modal-dialog modal-lg">
        <div class="modal-content">
            <!-- Modal Header -->
            <div class="modal-header">
                <h3 class="modal-title">
                    <i data-lucide="terminal" style="width: 20px; height: 20px;"></i>
                    Test Command
                </h3>
                <button type="button"
                        class="btn-close"
                        @click="closeTerminal()"
                        aria-label="Close">
                    &times;
                </button>
            </div>

            <!-- Modal Body -->
            <div class="modal-body">
                <!-- Server Selection -->
                <div class="form-group mb-3">
                    <label for="terminal-server-select" class="form-label">
                        <i data-lucide="server" style="width: 14px; height: 14px;"></i>
                        Target Server
                    </label>
                    <select id="terminal-server-select"
                            class="form-control"
                            x-model="selectedServer"
                            @change="serverChanged()">
                        <option value="">Select a server...</option>
                        {% for server in servers %}
                        <option value="{{ server.id }}">
                            {{ server.name }}
                            {% if server.description %}
                            - {{ server.description }}
                            {% endif %}
                        </option>
                        {% endfor %}
                    </select>
                </div>

                <!-- Command Input -->
                <div class="form-group mb-3">
                    <label for="terminal-command-input" class="form-label">
                        <i data-lucide="code" style="width: 14px; height: 14px;"></i>
                        Command
                    </label>
                    <div class="input-group">
                        <input type="text"
                               id="terminal-command-input"
                               class="form-control font-mono"
                               x-model="command"
                               @keydown.enter="executeCommand()"
                               placeholder="Enter command to test..."
                               :disabled="!selectedServer">
                        <button class="btn btn-primary"
                                @click="executeCommand()"
                                :disabled="!selectedServer || !command || executing">
                            <i data-lucide="play" style="width: 14px; height: 14px;"></i>
                            <span x-text="executing ? 'Executing...' : 'Execute'"></span>
                        </button>
                    </div>
                    <small class="form-text text-muted">
                        Press Enter to execute • Ctrl+C to cancel
                    </small>
                </div>

                <!-- Terminal Container -->
                <div class="terminal-container">
                    <div id="terminal-output" class="terminal-output"></div>
                </div>

                <!-- Terminal Actions -->
                <div class="terminal-actions mt-3">
                    <button class="btn btn-sm btn-secondary"
                            @click="clearTerminal()"
                            :disabled="executing">
                        <i data-lucide="trash-2" style="width: 14px; height: 14px;"></i>
                        Clear
                    </button>

                    <button class="btn btn-sm btn-secondary"
                            @click="copyOutput()"
                            :disabled="executing">
                        <i data-lucide="copy" style="width: 14px; height: 14px;"></i>
                        Copy Output
                    </button>

                    <button class="btn btn-sm btn-secondary"
                            @click="downloadOutput()">
                        <i data-lucide="download" style="width: 14px; height: 14px;"></i>
                        Download
                    </button>

                    <div class="terminal-status">
                        <span x-show="connected" class="status-badge status-connected">
                            <i data-lucide="wifi" style="width: 12px; height: 12px;"></i>
                            Connected
                        </span>
                        <span x-show="!connected" class="status-badge status-disconnected">
                            <i data-lucide="wifi-off" style="width: 12px; height: 12px;"></i>
                            Disconnected
                        </span>
                    </div>
                </div>
            </div>

            <!-- Modal Footer -->
            <div class="modal-footer">
                <button type="button"
                        class="btn btn-secondary"
                        @click="closeTerminal()">
                    Close
                </button>
            </div>
        </div>
    </div>
</div>

<style>
.terminal-container {
    background: #2e3440;
    border-radius: 4px;
    padding: 1rem;
    height: 400px;
    overflow: hidden;
}

.terminal-output {
    height: 100%;
    font-family: 'JetBrains Mono', 'Fira Code', 'Menlo', 'Monaco', monospace;
    font-size: 14px;
    line-height: 1.5;
}

.terminal-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
}

.terminal-status {
    margin-left: auto;
}

.status-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.875rem;
}

.status-connected {
    background: var(--color-success-bg, #a3be8c33);
    color: var(--color-success, #a3be8c);
}

.status-disconnected {
    background: var(--color-danger-bg, #bf616a33);
    color: var(--color-danger, #bf616a);
}

.modal-lg {
    max-width: 900px;
}

.font-mono {
    font-family: 'JetBrains Mono', 'Fira Code', 'Menlo', 'Monaco', monospace;
}
</style>
```

### 3. Frontend: Terminal JavaScript

**File**: `server/static/js/terminal.js`

```javascript
/**
 * Terminal Modal Manager
 * Manages xterm.js terminal instances and WebSocket connections
 */

class TerminalManager {
    constructor() {
        this.terminal = null;
        this.socket = null;
        this.fitAddon = null;
        this.searchAddon = null;
        this.connected = false;
        this.outputHistory = [];
    }

    /**
     * Initialize xterm.js terminal
     */
    init(containerId) {
        if (this.terminal) {
            this.terminal.dispose();
        }

        // Terminal configuration with Nord theme
        this.terminal = new Terminal({
            cursorBlink: true,
            cursorStyle: 'block',
            fontSize: 14,
            fontFamily: '"JetBrains Mono", "Fira Code", Menlo, Monaco, "Courier New", monospace',
            fontWeight: 400,
            fontWeightBold: 700,
            lineHeight: 1.5,
            letterSpacing: 0,
            scrollback: 1000,
            tabStopWidth: 4,
            allowProposedApi: true,
            theme: {
                background: '#2e3440',
                foreground: '#d8dee9',
                cursor: '#d8dee9',
                cursorAccent: '#2e3440',
                selectionBackground: '#4c566a',
                selectionForeground: '#d8dee9',
                black: '#3b4252',
                red: '#bf616a',
                green: '#a3be8c',
                yellow: '#ebcb8b',
                blue: '#81a1c1',
                magenta: '#b48ead',
                cyan: '#88c0d0',
                white: '#e5e9f0',
                brightBlack: '#4c566a',
                brightRed: '#bf616a',
                brightGreen: '#a3be8c',
                brightYellow: '#ebcb8b',
                brightBlue: '#81a1c1',
                brightMagenta: '#b48ead',
                brightCyan: '#8fbcbb',
                brightWhite: '#eceff4',
            }
        });

        // Load addons
        this.fitAddon = new FitAddon.FitAddon();
        this.terminal.loadAddon(this.fitAddon);

        this.searchAddon = new SearchAddon.SearchAddon();
        this.terminal.loadAddon(this.searchAddon);

        // Open terminal in container
        const container = document.getElementById(containerId);
        this.terminal.open(container);
        this.fitAddon.fit();

        // Handle window resize
        window.addEventListener('resize', () => {
            if (this.terminal) {
                this.fitAddon.fit();
            }
        });

        // Welcome message
        this.terminal.writeln('\x1b[1;36m╔════════════════════════════════════════════════════╗\x1b[0m');
        this.terminal.writeln('\x1b[1;36m║\x1b[0m         \x1b[1mSvrCtlRS Terminal v1.0\x1b[0m                   \x1b[1;36m║\x1b[0m');
        this.terminal.writeln('\x1b[1;36m╚════════════════════════════════════════════════════╝\x1b[0m');
        this.terminal.writeln('');
        this.terminal.writeln('\x1b[90mReady. Select a server and command to execute.\x1b[0m');
        this.terminal.writeln('');

        return this;
    }

    /**
     * Connect to WebSocket server
     */
    connect(serverId, command) {
        // Disconnect existing connection
        this.disconnect();

        // Clear terminal
        this.terminal.clear();
        this.outputHistory = [];

        // Show connecting message
        this.terminal.writeln('\x1b[33mConnecting to server...\x1b[0m');

        // Determine WebSocket protocol
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws/terminal`;

        this.socket = new WebSocket(wsUrl);

        this.socket.onopen = () => {
            console.log('Terminal WebSocket connected');
            this.connected = true;
            this.terminal.writeln('\x1b[32m✓ Connected\x1b[0m\r\n');

            // Send execute command
            const { cols, rows } = this.terminal;
            this.socket.send(JSON.stringify({
                type: 'execute',
                server_id: serverId,
                command: command,
                cols: cols,
                rows: rows
            }));

            // Show command being executed
            this.terminal.writeln(`\x1b[90m$ ${command}\x1b[0m`);
        };

        this.socket.onmessage = (event) => {
            try {
                const response = JSON.parse(event.data);

                if (response.type === 'output') {
                    this.terminal.write(response.data);
                    this.outputHistory.push(response.data);

                    // Check for exit code
                    if (response.exit_code !== undefined) {
                        this.terminal.writeln('');
                        if (response.exit_code === 0) {
                            this.terminal.writeln('\x1b[32m✓ Command completed successfully\x1b[0m');
                        } else {
                            this.terminal.writeln(`\x1b[31m✗ Command failed with exit code ${response.exit_code}\x1b[0m`);
                        }
                    }
                }
            } catch (e) {
                console.error('Failed to parse WebSocket message:', e);
                this.terminal.writeln(`\r\n\x1b[31mError parsing server response\x1b[0m\r\n`);
            }
        };

        this.socket.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.terminal.writeln('\r\n\x1b[31m✗ Connection error\x1b[0m\r\n');
            this.connected = false;
        };

        this.socket.onclose = () => {
            console.log('WebSocket closed');
            this.terminal.writeln('\r\n\x1b[33m[Connection closed]\x1b[0m\r\n');
            this.connected = false;
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

    /**
     * Disconnect WebSocket
     */
    disconnect() {
        if (this.socket) {
            this.socket.close();
            this.socket = null;
        }
        this.connected = false;
    }

    /**
     * Clear terminal output
     */
    clear() {
        if (this.terminal) {
            this.terminal.clear();
            this.outputHistory = [];
        }
    }

    /**
     * Get terminal output as plain text
     */
    getOutput() {
        return this.outputHistory.join('');
    }

    /**
     * Copy output to clipboard
     */
    async copyOutput() {
        const output = this.getOutput();
        try {
            await navigator.clipboard.writeText(output);
            this.terminal.writeln('\r\n\x1b[32m✓ Output copied to clipboard\x1b[0m\r\n');
        } catch (err) {
            console.error('Failed to copy:', err);
            this.terminal.writeln('\r\n\x1b[31m✗ Failed to copy output\x1b[0m\r\n');
        }
    }

    /**
     * Download output as file
     */
    downloadOutput(filename = 'terminal-output.txt') {
        const output = this.getOutput();
        const blob = new Blob([output], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
    }

    /**
     * Dispose terminal resources
     */
    dispose() {
        this.disconnect();
        if (this.terminal) {
            this.terminal.dispose();
            this.terminal = null;
        }
    }
}

/**
 * Alpine.js Terminal Modal Component
 */
function terminalModal() {
    return {
        open: false,
        selectedServer: '',
        command: '',
        executing: false,
        connected: false,
        terminalManager: null,

        init() {
            // Initialize terminal manager
            this.terminalManager = new TerminalManager();
            this.terminalManager.init('terminal-output');

            // Watch connected state
            this.$watch('terminalManager.connected', (value) => {
                this.connected = value;
            });
        },

        openTerminal(detail) {
            this.open = true;
            this.command = detail?.command || '';
            this.selectedServer = detail?.serverId || '';

            // Re-fit terminal when modal opens
            setTimeout(() => {
                if (this.terminalManager && this.terminalManager.fitAddon) {
                    this.terminalManager.fitAddon.fit();
                }
            }, 100);
        },

        closeTerminal() {
            this.open = false;
            this.terminalManager.disconnect();
            this.executing = false;
        },

        serverChanged() {
            // Could pre-load server info or validate credentials
        },

        executeCommand() {
            if (!this.selectedServer || !this.command) {
                return;
            }

            this.executing = true;
            this.terminalManager.connect(
                parseInt(this.selectedServer),
                this.command
            );

            // Reset executing state after connection attempt
            setTimeout(() => {
                this.executing = false;
            }, 1000);
        },

        clearTerminal() {
            this.terminalManager.clear();
        },

        copyOutput() {
            this.terminalManager.copyOutput();
        },

        downloadOutput() {
            const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
            this.terminalManager.downloadOutput(`svrctlrs-output-${timestamp}.txt`);
        }
    };
}

// Make terminalModal available globally
window.terminalModal = terminalModal;
```

### 4. Integration: Command Template Form

**File**: `server/templates/components/command_template_form.html` (add button)

```html
<!-- Add to the button group in the form header -->
<div class="form-actions">
    <button type="submit" class="btn btn-primary">
        <i data-lucide="save"></i> Save
    </button>

    <!-- NEW: Test Command Button -->
    <button type="button"
            class="btn btn-secondary"
            @click="$dispatch('open-terminal', {
                command: document.getElementById('command-input').value,
                serverId: null
            })"
            title="Test this command in a terminal">
        <i data-lucide="terminal"></i> Test Command
    </button>

    <a href="/job-types/{{ job_type_id }}" class="btn btn-secondary">
        <i data-lucide="x"></i> Cancel
    </a>
</div>
```

### 5. Base Template: Include Terminal Modal

**File**: `server/templates/base.html` (add before </body>)

```html
<!-- Terminal Modal (Global) -->
{% include "components/terminal_modal.html" %}

<!-- xterm.js Dependencies -->
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.3.0/css/xterm.css" />
<script src="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.3.0/lib/xterm.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10.0/lib/xterm-addon-fit.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-search@0.15.0/lib/xterm-addon-search.js"></script>

<!-- Terminal Manager -->
<script src="/static/js/terminal.js"></script>
```

## UX Enhancements for "Better Than SSH"

### 1. Smart Defaults
- Pre-fill command from template being edited
- Remember last selected server per user
- Auto-detect working directory from template
- Pre-populate environment variables

### 2. Visual Feedback
- Connection status indicator (connected/disconnected)
- Command execution progress spinner
- Success/failure color coding (green/red)
- Real-time output streaming (not wait-for-complete)

### 3. Keyboard Shortcuts
- `Enter` - Execute command
- `Ctrl+C` - Cancel execution
- `Ctrl+L` - Clear terminal
- `Ctrl+F` - Search output
- `Esc` - Close modal

### 4. Output Management
- Copy button (one-click copy all output)
- Download button (save output as .txt)
- Search within output
- Syntax highlighting for common formats (JSON, XML, etc.)

### 5. Error Handling
- Clear error messages (not just exit codes)
- Connection retry on failure
- Timeout warnings
- Suggestion hints for common errors

## Phase 2 Enhancements

### Command History
```javascript
// Add to TerminalManager
class TerminalManager {
    constructor() {
        // ...
        this.commandHistory = JSON.parse(
            localStorage.getItem('terminal-history') || '[]'
        );
        this.historyIndex = -1;
    }

    saveCommand(command) {
        this.commandHistory.push(command);
        localStorage.setItem(
            'terminal-history',
            JSON.stringify(this.commandHistory.slice(-50)) // Keep last 50
        );
    }

    getPreviousCommand() {
        if (this.historyIndex < this.commandHistory.length - 1) {
            this.historyIndex++;
            return this.commandHistory[
                this.commandHistory.length - 1 - this.historyIndex
            ];
        }
        return null;
    }

    getNextCommand() {
        if (this.historyIndex > 0) {
            this.historyIndex--;
            return this.commandHistory[
                this.commandHistory.length - 1 - this.historyIndex
            ];
        }
        this.historyIndex = -1;
        return '';
    }
}

// Add keyboard handlers
document.getElementById('terminal-command-input').addEventListener('keydown', (e) => {
    if (e.key === 'ArrowUp') {
        e.preventDefault();
        const cmd = terminalManager.getPreviousCommand();
        if (cmd) e.target.value = cmd;
    } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        const cmd = terminalManager.getNextCommand();
        e.target.value = cmd;
    }
});
```

### Environment Variables
```html
<!-- Add to modal -->
<div class="form-group mb-3">
    <label class="form-label">
        <i data-lucide="settings"></i>
        Environment Variables
        <button type="button"
                class="btn btn-xs btn-secondary"
                @click="showEnvEditor = !showEnvEditor">
            {{ showEnvEditor ? 'Hide' : 'Show' }}
        </button>
    </label>

    <div x-show="showEnvEditor" class="env-editor">
        <template x-for="(env, index) in envVars" :key="index">
            <div class="input-group mb-2">
                <input type="text"
                       class="form-control"
                       x-model="env.key"
                       placeholder="VAR_NAME">
                <input type="text"
                       class="form-control"
                       x-model="env.value"
                       placeholder="value">
                <button type="button"
                        class="btn btn-danger"
                        @click="envVars.splice(index, 1)">
                    <i data-lucide="x"></i>
                </button>
            </div>
        </template>
        <button type="button"
                class="btn btn-sm btn-secondary"
                @click="envVars.push({ key: '', value: '' })">
            <i data-lucide="plus"></i> Add Variable
        </button>
    </div>
</div>
```

## Phase 3: Multi-Terminal Debug Page

### Layout Design

```
┌─────────────────────────────────────────────────────────────┐
│  Debug Console                               [+] New Terminal │
├─────────────────────────────────────────────────────────────┤
│ ┌─ Terminal 1: web-01 ───────┬─ Terminal 2: db-01 ─────────┐│
│ │                             │                             ││
│ │  $ df -h                    │  $ systemctl status mysql   ││
│ │  Filesystem      Size       │  ● mysql.service - MySQL    ││
│ │  /dev/sda1       50G        │    Active: active (running) ││
│ │  ...                        │    ...                      ││
│ │                             │                             ││
│ │  [web-01]$ _                │  [db-01]$ _                 ││
│ └─────────────────────────────┴─────────────────────────────┘│
│ ┌─ Terminal 3: app-01 ───────┬─ Terminal 4: cache-01 ──────┐│
│ │                             │                             ││
│ │  $ docker ps                │  $ redis-cli INFO           ││
│ │  CONTAINER ID   IMAGE       │  # Server                   ││
│ │  abc123         nginx       │  redis_version:7.0.5        ││
│ │  ...                        │  ...                        ││
│ │                             │                             ││
│ │  [app-01]$ _                │  [cache-01]$ _              ││
│ └─────────────────────────────┴─────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│ Broadcast Mode: [ ] All Terminals  [ ] Web Servers Group    │
│ Command: ___________________________  [Broadcast Execute]   │
└─────────────────────────────────────────────────────────────┘
```

### Features:
- Drag-and-drop terminal rearrangement
- Save/load layouts
- Broadcast commands to selected terminals
- Terminal session persistence
- Export all outputs as ZIP

## Dependencies

### Cargo.toml
```toml
[dependencies]
# Existing dependencies...
tokio = { version = "1", features = ["full", "process"] }
futures-util = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Package.json (Frontend - Optional)
```json
{
  "name": "svrctlrs-terminal",
  "version": "1.0.0",
  "dependencies": {
    "@xterm/xterm": "^5.3.0",
    "@xterm/addon-fit": "^0.10.0",
    "@xterm/addon-search": "^0.15.0"
  }
}
```

Or use CDN (recommended for MVP):
- No npm/webpack needed
- Faster development
- Easier deployment

## Testing Plan

### Manual Testing
1. Open command template form
2. Click "Test Command"
3. Select server
4. Enter `echo "Hello World"`
5. Click Execute
6. Verify output appears
7. Test with failing command
8. Test with long-running command
9. Test connection loss/recovery
10. Test multiple consecutive executions

### Integration Testing
```bash
# Test WebSocket endpoint
curl -i -N \
  -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" \
  -H "Sec-WebSocket-Key: test" \
  http://localhost:8080/ws/terminal
```

## Deployment Checklist

- [x] Add terminal route to main router ✅ (2025-12-01)
- [x] Include terminal modal in base template ✅ (2025-12-01)
- [x] Add xterm.js CDN links ✅ (2025-12-01)
- [x] Test on Chrome ✅ (2025-12-01)
- [ ] Test on Firefox, Safari
- [ ] Test on mobile (responsive terminal)
- [x] Update documentation ✅ (2025-12-01)
- [x] Add to CLAUDE.md ✅ (2025-12-01)

## Success Metrics

**Week 1 (MVP)** - COMPLETE (2025-12-01):
- ✅ Terminal modal opens from command template form
- ✅ Can select server and execute command
- ✅ Real-time output streaming works
- ✅ ANSI colors render correctly
- ✅ Copy/download output works
- ✅ Command history (up/down arrows) - Bonus: implemented early!

**Week 2 (Enhanced)** - COMPLETE (2025-12-01):
- ✅ Command history (up/down arrows) - Done in Phase 1!
- ✅ Environment variable editing - DONE
- ✅ Multi-line command support (Shift+Enter) - DONE
- ✅ Output search functionality (Ctrl+F) - DONE
- ✅ Clickable URLs (WebLinksAddon) - DONE

**Week 3+ (Multi-Terminal)** - COMPLETE (2025-12-01):
- ✅ Dedicated `/debug` page
- ✅ 2-4 simultaneous terminals (configurable layouts)
- ✅ Broadcast mode (quick commands to all terminals)
- ✅ Session persistence infrastructure (SerializeAddon)

---

## Status Update (2025-12-01)

**Phase 1 MVP**: ✅ COMPLETE
- All core features implemented and tested
- Documentation updated (CLAUDE.md)
- Committed to develop branch

**Phase 2 Enhanced**: ✅ COMPLETE
- ✅ Output search (Ctrl+F, incremental search, case-sensitive toggle)
- ✅ Multi-line command support (Shift+Enter for newlines)
- ✅ Clickable URLs (WebLinksAddon)
- ✅ Environment variable editing (collapsible panel with add/remove)
- ✅ SerializeAddon for session persistence
- ✅ Unicode11Addon for better character support
- ⬜ PTY allocation - Future enhancement (for interactive commands)

**Phase 3 Multi-Terminal Debug Page**: ✅ COMPLETE
- ✅ Dedicated `/terminal` route and page (renamed from `/debug`)
- ✅ Layout options: single, 2-horizontal, 2-vertical, 4-grid
- ✅ Quick command buttons (uptime, df, free, hostname, docker ps, systemctl)
- ✅ Broadcast commands to all connected terminals
- ✅ Per-pane server selection and command input
- ✅ Status indicators for connection state
- ✅ Navigation link in sidebar under "Tools" section
- ✅ Fixed xterm.js output visibility issue (2025-12-02)
  - CSS fix using absolute positioning for xterm element
  - Ensures proper rendering in flexbox layouts

**Next Steps**: Phase 4 (Future)
- PTY allocation for interactive commands (sudo, vim)
- Terminal tabs within the debug page
- Server groups (run on all web servers, all DB servers, etc.)
- Terminal profiles (saved configurations)

**Known Limitation**: Non-interactive mode only
- Commands requiring PTY (sudo with password, vim) will fail
- PTY allocation planned for future phase

**Priority**: Low (core features complete, remaining are nice-to-have)
