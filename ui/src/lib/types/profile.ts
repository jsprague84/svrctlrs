export interface TerminalProfile {
	id: number;
	name: string;
	description: string | null;
	layout: 'single' | 'split-h' | 'split-v' | 'quad';
	pane_configs: PaneConfig[] | null;
	is_default: boolean;
	user_id: number | null;
	created_at: string;
	updated_at: string;
}

export interface PaneConfig {
	server_id: number | null;
	mode: 'pty' | 'cmd' | null;
}

export interface CreateProfile {
	name: string;
	description?: string;
	layout: string;
	pane_configs?: PaneConfig[];
	is_default?: boolean;
}

export interface UpdateProfile {
	name?: string;
	description?: string;
	layout?: string;
	pane_configs?: PaneConfig[];
	is_default?: boolean;
}

export interface QuickCommand {
	id: number;
	name: string;
	command: string;
	server_id: number | null;
	category: string;
	sort_order: number;
	created_at: string;
	updated_at: string;
}

export interface CreateQuickCommand {
	name: string;
	command: string;
	server_id?: number | null;
	category?: string;
	sort_order?: number;
}

export interface UpdateQuickCommand {
	name?: string;
	command?: string;
	server_id?: number | null;
	category?: string;
	sort_order?: number;
}
