/**
 * SvrCtlRS Terminal Manager
 *
 * Manages xterm.js terminal instances and WebSocket connections for
 * interactive command execution on remote servers.
 */

class TerminalManager {
    constructor() {
        this.terminal = null;
        this.socket = null;
        this.fitAddon = null;
        this.searchAddon = null;
        this.connected = false;
        this.connecting = false;
        this.outputHistory = [];
        this.commandHistory = [];
        this.historyIndex = -1;
        this.maxHistory = 50;
        this.initialized = false;

        // Load command history from localStorage
        this.loadHistory();
    }

    /**
     * Initialize xterm.js terminal
     */
    init(containerId) {
        if (this.initialized && this.terminal) {
            // Already initialized, just fit
            this.fit();
            return this;
        }

        // Check if xterm.js is loaded
        if (typeof Terminal === 'undefined') {
            console.error('xterm.js not loaded yet. Retrying in 100ms...');
            setTimeout(() => this.init(containerId), 100);
            return this;
        }

        // Terminal configuration with Tokyo Night / Nord compatible theme
        const isDark = document.documentElement.getAttribute('data-theme') !== 'light';

        this.terminal = new Terminal({
            cursorBlink: true,
            cursorStyle: 'block',
            fontSize: 14,
            fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", "SF Mono", Menlo, Monaco, "Courier New", monospace',
            fontWeight: 400,
            fontWeightBold: 700,
            lineHeight: 1.4,
            letterSpacing: 0,
            scrollback: 5000,
            tabStopWidth: 4,
            allowProposedApi: true,
            theme: isDark ? this.getDarkTheme() : this.getLightTheme()
        });

        // Load addons
        if (typeof FitAddon !== 'undefined') {
            this.fitAddon = new FitAddon.FitAddon();
            this.terminal.loadAddon(this.fitAddon);
        }

        if (typeof SearchAddon !== 'undefined') {
            this.searchAddon = new SearchAddon.SearchAddon();
            this.terminal.loadAddon(this.searchAddon);
        }

        // Open terminal in container
        const container = document.getElementById(containerId);
        if (container) {
            this.terminal.open(container);
            this.fit();
        }

        // Handle window resize
        window.addEventListener('resize', () => this.fit());

        // Watch for theme changes
        const observer = new MutationObserver(() => {
            const isDark = document.documentElement.getAttribute('data-theme') !== 'light';
            this.terminal.options.theme = isDark ? this.getDarkTheme() : this.getLightTheme();
        });
        observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });

        this.initialized = true;
        return this;
    }

    /**
     * Get dark theme (Tokyo Night / Nord compatible)
     */
    getDarkTheme() {
        const theme = document.documentElement.getAttribute('data-theme');

        if (theme === 'tokyo') {
            // Tokyo Night theme
            return {
                background: '#1a1b26',
                foreground: '#a9b1d6',
                cursor: '#c0caf5',
                cursorAccent: '#1a1b26',
                selectionBackground: '#33467c',
                selectionForeground: '#c0caf5',
                black: '#15161e',
                red: '#f7768e',
                green: '#9ece6a',
                yellow: '#e0af68',
                blue: '#7aa2f7',
                magenta: '#bb9af7',
                cyan: '#7dcfff',
                white: '#a9b1d6',
                brightBlack: '#414868',
                brightRed: '#f7768e',
                brightGreen: '#9ece6a',
                brightYellow: '#e0af68',
                brightBlue: '#7aa2f7',
                brightMagenta: '#bb9af7',
                brightCyan: '#7dcfff',
                brightWhite: '#c0caf5',
            };
        }

        // Nord theme (default dark)
        return {
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
        };
    }

    /**
     * Get light theme
     */
    getLightTheme() {
        return {
            background: '#ffffff',
            foreground: '#2e3440',
            cursor: '#2e3440',
            cursorAccent: '#ffffff',
            selectionBackground: '#d8dee9',
            selectionForeground: '#2e3440',
            black: '#2e3440',
            red: '#bf616a',
            green: '#a3be8c',
            yellow: '#d08770',
            blue: '#5e81ac',
            magenta: '#b48ead',
            cyan: '#88c0d0',
            white: '#e5e9f0',
            brightBlack: '#4c566a',
            brightRed: '#bf616a',
            brightGreen: '#a3be8c',
            brightYellow: '#d08770',
            brightBlue: '#5e81ac',
            brightMagenta: '#b48ead',
            brightCyan: '#8fbcbb',
            brightWhite: '#eceff4',
        };
    }

    /**
     * Fit terminal to container
     */
    fit() {
        if (this.fitAddon && this.terminal) {
            try {
                this.fitAddon.fit();
            } catch (e) {
                console.warn('Failed to fit terminal:', e);
            }
        }
    }

    /**
     * Connect to WebSocket and execute command
     */
    connect(serverId, command) {
        // Save command to history
        this.saveCommand(command);

        this.connecting = true;
        this.connected = false;

        // Initialize terminal if not already
        if (!this.initialized) {
            this.init('terminal-output');
        }

        // Clear terminal for new command
        this.terminal.clear();
        this.outputHistory = [];

        // Disconnect existing connection
        this.disconnect();

        // Show connecting message
        this.terminal.writeln('\x1b[33mConnecting to server...\x1b[0m');

        // Determine WebSocket protocol
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws/terminal`;

        try {
            this.socket = new WebSocket(wsUrl);
        } catch (e) {
            this.terminal.writeln(`\r\n\x1b[31mFailed to create WebSocket: ${e.message}\x1b[0m`);
            this.connecting = false;
            return;
        }

        this.socket.onopen = () => {
            console.log('Terminal WebSocket connected');
            this.connected = true;
            this.connecting = false;

            // Send execute command
            const { cols, rows } = this.terminal;
            this.socket.send(JSON.stringify({
                type: 'execute',
                server_id: parseInt(serverId),
                command: command,
                cols: cols,
                rows: rows
            }));
        };

        this.socket.onmessage = (event) => {
            try {
                const response = JSON.parse(event.data);

                switch (response.type) {
                    case 'output':
                        this.terminal.write(response.data);
                        this.outputHistory.push(response.data);
                        break;

                    case 'exit':
                        this.terminal.write(response.data);
                        this.outputHistory.push(response.data);
                        break;

                    case 'error':
                        this.terminal.write(response.data);
                        this.outputHistory.push(response.data);
                        break;

                    case 'pong':
                        // Keep-alive response, ignore
                        break;

                    default:
                        console.warn('Unknown terminal response type:', response.type);
                }
            } catch (e) {
                console.error('Failed to parse WebSocket message:', e);
                this.terminal.writeln(`\r\n\x1b[31mError parsing server response\x1b[0m`);
            }
        };

        this.socket.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.terminal.writeln('\r\n\x1b[31m✗ Connection error\x1b[0m');
            this.connected = false;
            this.connecting = false;
        };

        this.socket.onclose = (event) => {
            console.log('WebSocket closed:', event.code, event.reason);
            if (this.connected) {
                this.terminal.writeln('\r\n\x1b[33m[Connection closed]\x1b[0m');
            }
            this.connected = false;
            this.connecting = false;
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
        this.connecting = false;
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
        // Strip ANSI codes for plain text
        const stripAnsi = (str) => str.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '');
        return this.outputHistory.map(stripAnsi).join('');
    }

    /**
     * Copy output to clipboard
     */
    async copyOutput() {
        const output = this.getOutput();
        try {
            await navigator.clipboard.writeText(output);
            this.terminal.writeln('\r\n\x1b[32m✓ Output copied to clipboard\x1b[0m');
        } catch (err) {
            console.error('Failed to copy:', err);
            this.terminal.writeln('\r\n\x1b[31m✗ Failed to copy output\x1b[0m');
        }
    }

    /**
     * Download output as file
     */
    downloadOutput(filename) {
        const output = this.getOutput();
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
        const finalFilename = filename || `svrctlrs-terminal-${timestamp}.txt`;

        const blob = new Blob([output], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = finalFilename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);

        this.terminal.writeln(`\r\n\x1b[32m✓ Downloaded: ${finalFilename}\x1b[0m`);
    }

    /**
     * Save command to history
     */
    saveCommand(command) {
        if (!command || !command.trim()) return;

        // Don't add duplicates at the end
        if (this.commandHistory.length > 0 &&
            this.commandHistory[this.commandHistory.length - 1] === command) {
            return;
        }

        this.commandHistory.push(command);

        // Limit history size
        if (this.commandHistory.length > this.maxHistory) {
            this.commandHistory.shift();
        }

        this.historyIndex = this.commandHistory.length;

        // Save to localStorage
        try {
            localStorage.setItem('svrctlrs-terminal-history', JSON.stringify(this.commandHistory));
        } catch (e) {
            console.warn('Failed to save terminal history:', e);
        }
    }

    /**
     * Load command history from localStorage
     */
    loadHistory() {
        try {
            const saved = localStorage.getItem('svrctlrs-terminal-history');
            if (saved) {
                this.commandHistory = JSON.parse(saved);
                this.historyIndex = this.commandHistory.length;
            }
        } catch (e) {
            console.warn('Failed to load terminal history:', e);
        }
    }

    /**
     * Get previous command from history
     */
    getPreviousCommand() {
        if (this.historyIndex > 0) {
            this.historyIndex--;
            return this.commandHistory[this.historyIndex];
        }
        return null;
    }

    /**
     * Get next command from history
     */
    getNextCommand() {
        if (this.historyIndex < this.commandHistory.length - 1) {
            this.historyIndex++;
            return this.commandHistory[this.historyIndex];
        }
        this.historyIndex = this.commandHistory.length;
        return '';
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
        this.initialized = false;
    }
}

// Global terminal manager instance
window.terminalManager = new TerminalManager();

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
        connecting: false,
        servers: [],

        async init() {
            // Load servers list
            await this.loadServers();

            // Watch terminal manager state
            this.$watch('executing', () => {
                if (window.terminalManager) {
                    this.connected = window.terminalManager.connected;
                    this.connecting = window.terminalManager.connecting;
                }
            });
        },

        async loadServers() {
            try {
                const response = await fetch('/api/v1/servers');
                if (response.ok) {
                    const data = await response.json();
                    this.servers = data.servers || data || [];
                }
            } catch (e) {
                console.error('Failed to load servers:', e);
                this.servers = [];
            }
        },

        openTerminal(detail) {
            this.open = true;
            this.command = detail?.command || '';
            this.selectedServer = detail?.serverId ? String(detail.serverId) : '';

            // Initialize terminal after modal is visible
            this.$nextTick(() => {
                if (!window.terminalManager.initialized) {
                    window.terminalManager.init('terminal-output');
                }

                // Fit terminal after modal animation
                setTimeout(() => {
                    window.terminalManager.fit();
                    // Re-initialize lucide icons
                    if (typeof lucide !== 'undefined') {
                        lucide.createIcons();
                    }
                }, 100);
            });
        },

        closeTerminal() {
            this.open = false;
            window.terminalManager.disconnect();
            this.executing = false;
            this.connected = false;
            this.connecting = false;
        },

        serverChanged() {
            // Could pre-validate server connection here
        },

        executeCommand() {
            if (!this.selectedServer || !this.command.trim()) {
                return;
            }

            this.executing = true;
            this.connecting = true;

            window.terminalManager.connect(
                parseInt(this.selectedServer),
                this.command.trim()
            );

            // Update state from terminal manager
            const checkState = () => {
                this.connected = window.terminalManager.connected;
                this.connecting = window.terminalManager.connecting;

                if (!this.connecting) {
                    this.executing = false;
                } else {
                    setTimeout(checkState, 100);
                }
            };
            setTimeout(checkState, 100);
        },

        clearTerminal() {
            window.terminalManager.clear();
        },

        copyOutput() {
            window.terminalManager.copyOutput();
        },

        downloadOutput() {
            window.terminalManager.downloadOutput();
        },

        reconnect() {
            if (this.selectedServer && this.command) {
                this.executeCommand();
            }
        },

        historyUp() {
            const cmd = window.terminalManager.getPreviousCommand();
            if (cmd !== null) {
                this.command = cmd;
            }
        },

        historyDown() {
            const cmd = window.terminalManager.getNextCommand();
            this.command = cmd;
        }
    };
}

// Make terminalModal available globally for Alpine.js
window.terminalModal = terminalModal;
