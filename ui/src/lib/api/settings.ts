import { get, post, put } from './client.js';
import type { Setting, SettingValueType, GroupedSettings } from '$lib/types/index.js';

interface SettingsListResponse {
	settings: GroupedSettings;
}

interface SettingResponse {
	key: string;
	value: string;
	value_type: string;
	description: string | null;
	updated_at: string;
}

export interface CreateSettingInput {
	key: string;
	value: string;
	value_type?: SettingValueType;
	description?: string;
}

export async function listSettings(): Promise<GroupedSettings> {
	const res = await get<SettingsListResponse>('/settings');
	return res.settings;
}

export async function getSetting(key: string): Promise<Setting> {
	return get<SettingResponse>(`/settings/${encodeURIComponent(key)}`) as Promise<Setting>;
}

export async function createSetting(input: CreateSettingInput): Promise<void> {
	await post('/settings', input);
}

export async function updateSetting(key: string, value: string): Promise<void> {
	await put(`/settings/${encodeURIComponent(key)}`, { value });
}
