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

/** PTY mode — interactive shell session.
 *  `shell` starts a new session; `attach` resumes one the server kept after a disconnect.
 *  `close` ends the shell, `detach` (or just dropping the socket) keeps it for a while. */
export interface PtyRequest {
	type: 'shell' | 'attach' | 'input' | 'resize' | 'ping' | 'close' | 'detach';
	server_id?: number;
	data?: string;
	cols?: number;
	rows?: number;
	/** `attach`: the session to resume */
	session_id?: string;
	/** `attach`: the last output `seq` this client has displayed; the server replays the rest */
	last_seq?: number;
}

export interface PtyResponse {
	type:
		| 'output'
		| 'connected'
		| 'error'
		| 'pong'
		| 'replay_start'
		| 'replay_end'
		| 'replay_window_lost'
		| 'closed';
	data: string;
	/** `output` frames are numbered so a reconnect can ask for the gap */
	seq?: number;
	/** `connected`: the id to `attach` to later */
	session_id?: string;
	/** `error`: `session_gone` (start a new shell), `quota` */
	code?: string;
}

/** A live session as listed by `GET /api/v1/sessions` */
export interface LiveSession {
	id: string;
	server_id: number;
	server_name: string;
	created_at: string;
	/** Sockets attached right now: 0 means the shell is detached and on its grace period */
	attachments: number;
	/** Sequence of the newest output frame the server holds */
	latest_seq: number;
}

export type TerminalMode = 'cmd' | 'pty';

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

export type LayoutMode = 'single' | 'split-h' | 'split-v' | 'quad';

export type CursorStyle = 'block' | 'underline' | 'bar';
export type { LeaderKeyOption } from '#lib/utils/leader.js';
export type FontWeightOption = 300 | 400 | 500 | 600 | 700;

export interface TerminalPreferences {
	fontSize: number;
	fontFamily: string;
	fontWeight: FontWeightOption;
	fontWeightBold: FontWeightOption;
	lineHeight: number;
	letterSpacing: number;
	cursorStyle: CursorStyle;
	cursorBlink: boolean;
	scrollback: number;
	scrollSensitivity: number;
	fastScrollSensitivity: number;
	smoothScrollDuration: number;
	scrollOnUserInput: boolean;
	tabStopWidth: number;
	drawBoldTextInBrightColors: boolean;
	minimumContrastRatio: number;
	/** Chord that arms tmux-style navigation (`#lib/utils/leader.ts`) */
	leaderKey: import('#lib/utils/leader.js').LeaderKeyOption;
}

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
