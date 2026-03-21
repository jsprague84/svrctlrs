import type { Setting, GroupedSettings } from '$lib/types/index.js';
import type { CreateSettingInput } from '$lib/api/settings.js';
import * as api from '$lib/api/settings.js';
import { extractErrorMessage } from '$lib/utils/error.js';

let settings = $state<GroupedSettings>({});
let loading = $state(false);
let error = $state<string | null>(null);

export function getSettings() {
	return settings;
}

export function isLoading() {
	return loading;
}

export function getError() {
	return error;
}

export function getSetting(key: string): Setting | undefined {
	for (const group of Object.values(settings)) {
		const found = group.find((s) => s.key === key);
		if (found) return found;
	}
	return undefined;
}

export async function loadSettings() {
	loading = true;
	error = null;
	try {
		settings = await api.listSettings();
	} catch (e) {
		error = extractErrorMessage(e, 'Failed to load settings');
	} finally {
		loading = false;
	}
}

export async function createSetting(input: CreateSettingInput): Promise<void> {
	await api.createSetting(input);
	await loadSettings();
}

export async function updateSetting(key: string, value: string): Promise<void> {
	await api.updateSetting(key, value);
	await loadSettings();
}
