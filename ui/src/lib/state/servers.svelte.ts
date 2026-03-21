import type { Server, CreateServer, UpdateServer, TestConnectionResult } from '$lib/types/index.js';
import * as api from '$lib/api/servers.js';
import { extractErrorMessage } from '$lib/utils/error.js';

let servers = $state<Server[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);
let selectedServerId = $state<number | null>(null);

export function getServers() {
	return servers;
}

export function isLoading() {
	return loading;
}

export function getError() {
	return error;
}

export function getSelectedServerId() {
	return selectedServerId;
}

export function getSelectedServer(): Server | undefined {
	return servers.find((s) => s.id === selectedServerId);
}

export function selectServer(id: number | null) {
	selectedServerId = id;
}

export async function loadServers() {
	loading = true;
	error = null;
	try {
		servers = await api.listServers();
	} catch (e) {
		error = extractErrorMessage(e, 'Failed to load servers');
	} finally {
		loading = false;
	}
}

export async function createServer(input: CreateServer): Promise<number> {
	const id = await api.createServer(input);
	await loadServers();
	return id;
}

export async function updateServer(id: number, input: UpdateServer): Promise<void> {
	await api.updateServer(id, input);
	await loadServers();
}

export async function deleteServer(id: number): Promise<void> {
	await api.deleteServer(id);
	if (selectedServerId === id) selectedServerId = null;
	await loadServers();
}

export async function testConnection(id: number): Promise<TestConnectionResult> {
	return api.testConnection(id);
}
