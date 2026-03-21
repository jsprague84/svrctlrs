import type { QuickCommand, CreateQuickCommand, UpdateQuickCommand } from '$lib/types/index.js';
import * as api from '$lib/api/quickCommands.js';
import { extractErrorMessage } from '$lib/utils/error.js';

let commands = $state<QuickCommand[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);

export function getCommands() {
	return commands;
}

export function isLoading() {
	return loading;
}

export function getError() {
	return error;
}

export function getCommandsByCategory(): Record<string, QuickCommand[]> {
	const grouped: Record<string, QuickCommand[]> = {};
	for (const cmd of commands) {
		const cat = cmd.category || 'general';
		if (!grouped[cat]) grouped[cat] = [];
		grouped[cat].push(cmd);
	}
	return grouped;
}

export async function loadCommands() {
	loading = true;
	error = null;
	try {
		commands = await api.listQuickCommands();
	} catch (e) {
		error = extractErrorMessage(e, 'Failed to load quick commands');
	} finally {
		loading = false;
	}
}

export async function loadCommandsForServer(serverId: number) {
	loading = true;
	error = null;
	try {
		commands = await api.listQuickCommandsForServer(serverId);
	} catch (e) {
		error = extractErrorMessage(e, 'Failed to load quick commands');
	} finally {
		loading = false;
	}
}

export async function createCommand(input: CreateQuickCommand): Promise<number> {
	const id = await api.createQuickCommand(input);
	await loadCommands();
	return id;
}

export async function updateCommand(id: number, input: UpdateQuickCommand): Promise<void> {
	await api.updateQuickCommand(id, input);
	await loadCommands();
}

export async function deleteCommand(id: number): Promise<void> {
	await api.deleteQuickCommand(id);
	await loadCommands();
}
