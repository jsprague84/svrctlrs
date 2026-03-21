import { get, post, put, del } from './client.js';
import type { QuickCommand, CreateQuickCommand, UpdateQuickCommand } from '$lib/types/index.js';

interface CommandsListResponse {
	commands: QuickCommand[];
}

interface CommandResponse {
	command: QuickCommand;
}

interface MutationResponse {
	id: number;
	message: string;
}

export async function listQuickCommands(): Promise<QuickCommand[]> {
	const res = await get<CommandsListResponse>('/quick-commands');
	return res.commands;
}

export async function listQuickCommandsForServer(serverId: number): Promise<QuickCommand[]> {
	const res = await get<CommandsListResponse>(`/quick-commands?server_id=${serverId}`);
	return res.commands;
}

export async function getQuickCommand(id: number): Promise<QuickCommand> {
	const res = await get<CommandResponse>(`/quick-commands/${id}`);
	return res.command;
}

export async function createQuickCommand(input: CreateQuickCommand): Promise<number> {
	const res = await post<MutationResponse>('/quick-commands', input);
	return res.id;
}

export async function updateQuickCommand(id: number, input: UpdateQuickCommand): Promise<void> {
	await put<MutationResponse>(`/quick-commands/${id}`, input);
}

export async function deleteQuickCommand(id: number): Promise<void> {
	await del<MutationResponse>(`/quick-commands/${id}`);
}
