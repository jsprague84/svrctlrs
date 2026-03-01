export interface Server {
	id: number;
	name: string;
	hostname: string | null;
	port: number;
	username: string | null;
	credential_id: number | null;
	description: string | null;
	is_local: boolean;
	enabled: boolean;
	os_type: string | null;
	os_distro: string | null;
	package_manager: string | null;
	docker_available: boolean;
	systemd_available: boolean;
	metadata: string | null;
	last_seen_at: string | null;
	last_error: string | null;
	created_at: string;
	updated_at: string;
}

export interface ServerCapability {
	id: number;
	server_id: number;
	capability: string;
	available: boolean;
	version: string | null;
	detected_at: string;
}

export interface ServerWithDetails {
	server: Server;
	tags: string[];
	capabilities: ServerCapability[];
}

export interface CreateServer {
	name: string;
	hostname?: string;
	port?: number;
	username?: string;
	credential_id?: number;
	description?: string;
	is_local?: boolean;
	enabled?: boolean;
	tag_ids?: number[];
}

export interface UpdateServer {
	name?: string;
	hostname?: string;
	port?: number;
	username?: string;
	credential_id?: number;
	description?: string;
	enabled?: boolean;
	tag_ids?: number[];
}

export interface TestConnectionResult {
	success: boolean;
	message: string;
	server_id: number;
	hostname: string | null;
	port: number;
	latency_ms?: number;
}
