/** CMD mode — non-interactive command execution */
export interface CmdRequest {
	type: 'execute' | 'ping';
	server_id?: number;
	command?: string;
	env?: Record<string, string>;
	cols?: number;
	rows?: number;
}

export interface CmdResponse {
	type: 'output' | 'exit' | 'error' | 'pong';
	data: string;
	exit_code?: number;
}

/** PTY mode — interactive shell session */
export interface PtyRequest {
	type: 'shell' | 'input' | 'resize' | 'ping' | 'close';
	server_id?: number;
	data?: string;
	cols?: number;
	rows?: number;
}

export interface PtyResponse {
	type: 'output' | 'connected' | 'error' | 'pong';
	data: string;
}

export type TerminalMode = 'cmd' | 'pty';

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

export type LayoutMode = 'single' | 'split-h' | 'split-v' | 'quad';

export interface TerminalTab {
	id: string;
	label: string;
	serverId: number | null;
	serverName: string | null;
	mode: TerminalMode;
	status: ConnectionStatus;
	/** Slot index when visible in split view (0-3), null if hidden */
	slot: number | null;
}
