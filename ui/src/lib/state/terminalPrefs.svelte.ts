import type { TerminalPreferences } from '$lib/types/index.js';

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

const DEFAULTS: TerminalPreferences = {
	fontSize: 14,
	fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", monospace',
	fontWeight: 400,
	fontWeightBold: 700,
	lineHeight: 1.4,
	letterSpacing: 0,
	cursorStyle: 'block',
	cursorBlink: true,
	scrollback: 5000,
	scrollSensitivity: 1,
	fastScrollSensitivity: 5,
	smoothScrollDuration: 0,
	scrollOnUserInput: true,
	tabStopWidth: 4,
	drawBoldTextInBrightColors: true,
	minimumContrastRatio: 1
};

function loadFromStorage(): TerminalPreferences {
	if (typeof window === 'undefined') return { ...DEFAULTS };
	try {
		const saved = localStorage.getItem(STORAGE_KEY);
		if (saved) {
			const parsed = JSON.parse(saved);
			return { ...DEFAULTS, ...parsed };
		}
	} catch {
		// ignore
	}
	return { ...DEFAULTS };
}

function saveToStorage(p: TerminalPreferences) {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(p));
	} catch {
		// ignore
	}
}

let prefs = $state<TerminalPreferences>(loadFromStorage());

export function getPrefs(): TerminalPreferences {
	return prefs;
}

export function getDefaults(): TerminalPreferences {
	return { ...DEFAULTS };
}

export function updatePref<K extends keyof TerminalPreferences>(key: K, value: TerminalPreferences[K]) {
	prefs = { ...prefs, [key]: value };
	saveToStorage(prefs);
}

export function resetPrefs() {
	prefs = { ...DEFAULTS };
	saveToStorage(prefs);
}
