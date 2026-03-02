export type Theme = 'dark' | 'light';

const STORAGE_KEY = 'svrctlrs-theme';

let theme = $state<Theme>('dark');

// Initialize from localStorage
if (typeof window !== 'undefined') {
	const saved = localStorage.getItem(STORAGE_KEY);
	if (saved === 'light' || saved === 'dark') {
		theme = saved;
		document.documentElement.dataset.theme = saved;
	}
}

export function getTheme(): Theme {
	return theme;
}

export function isDark(): boolean {
	return theme === 'dark';
}

export function toggleTheme() {
	theme = theme === 'dark' ? 'light' : 'dark';
	localStorage.setItem(STORAGE_KEY, theme);
	document.documentElement.dataset.theme = theme;
}
