/**
 * Terminal preferences: the account's (synced through `#lib/terminal/prefsSync.ts`), with a
 * copy in localStorage so the terminal starts with them before the server answers.
 */
import type { TerminalPreferences } from '#lib/types/index.js';
import { DEFAULT_PREFS, PrefsSync, mergePrefs } from '#lib/terminal/prefsSync.js';
import * as usersApi from '#lib/api/users.js';

const STORAGE_KEY = 'svrctlrs-terminal-prefs';

export const FONT_FAMILIES: Record<string, string> = {
	'JetBrains Mono': '"JetBrains Mono", "Fira Code", "Cascadia Code", monospace',
	'Fira Code': '"Fira Code", "JetBrains Mono", monospace',
	'Cascadia Code': '"Cascadia Code", "Fira Code", monospace',
	'Source Code Pro': '"Source Code Pro", monospace',
	'IBM Plex Mono': '"IBM Plex Mono", monospace',
	'Ubuntu Mono': '"Ubuntu Mono", monospace',
	'System Monospace': 'monospace'
};

function readStorage(): string | null {
	if (typeof window === 'undefined') return null;
	try {
		return localStorage.getItem(STORAGE_KEY);
	} catch {
		return null; // storage blocked
	}
}

function loadFromStorage(saved: string | null): TerminalPreferences {
	if (!saved) return { ...DEFAULT_PREFS };
	try {
		return mergePrefs(JSON.parse(saved));
	} catch {
		return { ...DEFAULT_PREFS };
	}
}

function saveToStorage(p: TerminalPreferences) {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(p));
	} catch {
		// ignore
	}
}

const stored = readStorage();
let prefs = $state<TerminalPreferences>(loadFromStorage(stored));

const sync = new PrefsSync({
	remote: {
		load: () => usersApi.getPreference('terminal'),
		save: (p) => usersApi.putPreference('terminal', p)
	},
	current: () => prefs,
	apply: (p) => {
		prefs = p;
		saveToStorage(p);
	},
	hadLocal: stored !== null
});

export function getPrefs(): TerminalPreferences {
	return prefs;
}

export function updatePref<K extends keyof TerminalPreferences>(
	key: K,
	value: TerminalPreferences[K]
) {
	prefs = { ...prefs, [key]: value };
	saveToStorage(prefs);
	sync.changed();
}

export function resetPrefs() {
	prefs = { ...DEFAULT_PREFS };
	saveToStorage(prefs);
	sync.changed();
}

/** Load the account's preferences (app start, the page shown again). Never throws. */
export function syncPrefs(): Promise<void> {
	return sync.pull();
}

/** Save a change still waiting (the page is being hidden). Never throws. */
export function flushPrefs(): Promise<void> {
	return sync.flush();
}
