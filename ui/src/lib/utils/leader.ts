/**
 * Leader-key navigation for the terminal, tmux style: one chord arms the leader, the next key
 * is the command. Browsers reserve Ctrl+Tab, Ctrl+Shift+T and Ctrl+Shift+W for themselves, so
 * the leader is the reliable way to move between terminals from the keyboard in a web page.
 */

export type LeaderKeyOption = 'ctrl-space' | 'ctrl-shift-space' | 'alt-space' | 'off';

export const LEADER_KEY_LABELS: Record<LeaderKeyOption, string> = {
	'ctrl-space': 'Ctrl+Space',
	'ctrl-shift-space': 'Ctrl+Shift+Space',
	'alt-space': 'Alt+Space',
	off: 'Off'
};

/** The subset of KeyboardEvent the matcher reads; a plain object in tests. */
export interface KeyChord {
	key: string;
	code?: string;
	ctrlKey: boolean;
	shiftKey: boolean;
	altKey: boolean;
	metaKey: boolean;
}

/** Does this keydown arm the leader? `metaKey` never does, so Cmd shortcuts stay the OS's. */
export function isLeaderChord(e: KeyChord, option: LeaderKeyOption): boolean {
	if (option === 'off' || e.metaKey) return false;
	const space = e.key === ' ' || e.code === 'Space';
	if (!space) return false;
	switch (option) {
		case 'ctrl-space':
			return e.ctrlKey && !e.shiftKey && !e.altKey;
		case 'ctrl-shift-space':
			return e.ctrlKey && e.shiftKey && !e.altKey;
		case 'alt-space':
			return e.altKey && !e.ctrlKey && !e.shiftKey;
	}
	return false;
}

export type LeaderAction =
	| { type: 'new-tab' }
	| { type: 'close-tab' }
	| { type: 'next-tab' }
	| { type: 'prev-tab' }
	| { type: 'goto-tab'; index: number }
	| { type: 'last-tab' }
	| { type: 'focus'; direction: 'left' | 'right' | 'up' | 'down' }
	| { type: 'next-pane' }
	| { type: 'prev-pane' }
	| { type: 'cycle-layout' }
	| { type: 'search' }
	| { type: 'palette' }
	| { type: 'help' }
	| { type: 'cancel' };

/** Map the key pressed after the leader to an action; `null` means "not bound". */
export function leaderAction(e: KeyChord): LeaderAction | null {
	// Modifier-only presses (the user is still releasing Ctrl) are not commands
	if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return null;
	if (e.ctrlKey || e.altKey || e.metaKey) return { type: 'cancel' };
	switch (e.key) {
		case 'c':
			return { type: 'new-tab' };
		case 'x':
			return { type: 'close-tab' };
		case 'n':
		case 'Tab':
			return e.shiftKey ? { type: 'prev-tab' } : { type: 'next-tab' };
		case 'p':
			return { type: 'prev-tab' };
		case 'o':
			return { type: 'next-pane' };
		case ']':
			return { type: 'next-pane' };
		case '[':
			return { type: 'prev-pane' };
		case 'h':
		case 'ArrowLeft':
			return { type: 'focus', direction: 'left' };
		case 'l':
		case 'ArrowRight':
			return { type: 'focus', direction: 'right' };
		case 'k':
		case 'ArrowUp':
			return { type: 'focus', direction: 'up' };
		case 'j':
		case 'ArrowDown':
			return { type: 'focus', direction: 'down' };
		case 's':
		case ' ':
			return { type: 'cycle-layout' };
		case '/':
			return { type: 'search' };
		case ':':
			return { type: 'palette' };
		case '?':
			return { type: 'help' };
		case 'Escape':
		case 'q':
			return { type: 'cancel' };
		case '`':
			return { type: 'last-tab' };
	}
	if (e.key >= '1' && e.key <= '9') return { type: 'goto-tab', index: Number(e.key) - 1 };
	return null;
}

/** Shown while the leader is armed. Order is the order on screen. */
export const LEADER_BINDINGS: ReadonlyArray<{ keys: string; action: string }> = [
	{ keys: 'c', action: 'new tab' },
	{ keys: 'x', action: 'close tab' },
	{ keys: 'n / p', action: 'next / previous tab' },
	{ keys: '1–9', action: 'tab by number' },
	{ keys: '`', action: 'last tab' },
	{ keys: 'h j k l', action: 'focus pane' },
	{ keys: '[ / ]', action: 'previous / next pane' },
	{ keys: 's', action: 'cycle layout' },
	{ keys: '/', action: 'search' },
	{ keys: ':', action: 'command palette' },
	{ keys: 'Esc', action: 'cancel' }
];
