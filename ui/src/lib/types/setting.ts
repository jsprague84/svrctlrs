export type SettingValueType = 'string' | 'number' | 'boolean' | 'json';

export interface Setting {
	key: string;
	value: string;
	value_type: SettingValueType;
	description: string | null;
	updated_at: string;
}

export type GroupedSettings = Record<string, Setting[]>;
