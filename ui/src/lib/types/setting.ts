export type SettingValueType = 'string' | 'number' | 'boolean' | 'json';

export interface Setting {
	/** Write-only through the API: `value` is always empty, `is_set` says whether one is stored */
	secret?: boolean;
	is_set?: boolean;
	key: string;
	value: string;
	value_type: SettingValueType;
	description: string | null;
	updated_at: string;
}

export type GroupedSettings = Record<string, Setting[]>;
