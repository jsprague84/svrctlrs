export type Theme = 'dark' | 'light';
/** What the user picked: a theme, or whatever the OS uses */
export type ThemeChoice = Theme | 'system';

const STORAGE_KEY = 'svrctlrs-theme';

/** The OS theme; the starting point until the user picks one with the toggle */
function systemTheme(): Theme {
	return typeof matchMedia !== 'undefined' && matchMedia('(prefers-color-scheme: light)').matches
		? 'light'
		: 'dark';
}

function savedTheme(): Theme | null {
	try {
		const saved = localStorage.getItem(STORAGE_KEY);
		return saved === 'light' || saved === 'dark' ? saved : null;
	} catch {
		return null;
	}
}

let theme = $state<Theme>('dark');
let choice = $state<ThemeChoice>('system');

// app.html already set data-theme before the first paint; this mirrors it into state
if (typeof window !== 'undefined') {
	choice = savedTheme() ?? 'system';
	theme = savedTheme() ?? systemTheme();
	apply(theme);
	// Without a choice of its own, the app follows the OS when it switches (sunset, a toggle)
	matchMedia('(prefers-color-scheme: light)').addEventListener('change', () => {
		if (choice === 'system') {
			theme = systemTheme();
			apply(theme);
		}
	});
}

export function getTheme(): Theme {
	return theme;
}

export function isDark(): boolean {
	return theme === 'dark';
}

export function getChoice(): ThemeChoice {
	return choice;
}

export function toggleTheme() {
	setChoice(theme === 'dark' ? 'light' : 'dark');
}

export function setChoice(next: ThemeChoice) {
	choice = next;
	try {
		if (next === 'system') localStorage.removeItem(STORAGE_KEY);
		else localStorage.setItem(STORAGE_KEY, next);
	} catch {
		// Private mode: the choice lasts for this page only
	}
	theme = next === 'system' ? systemTheme() : next;
	apply(theme);
}

function apply(t: Theme) {
	document.documentElement.dataset.theme = t;
	syncThemeColor();
}

/** Keep the browser/PWA status bar the same colour as the app's surface. */
function syncThemeColor() {
	if (typeof document === 'undefined') return;
	const meta = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
	const surface = getComputedStyle(document.documentElement)
		.getPropertyValue('--color-surface')
		.trim();
	if (meta && surface) meta.content = surface;
}
