export interface Server {
	id: number;
	name: string;
	hostname: string;
	port: number;
	/** null means the credential's user, else root */
	username: string | null;
	credential_id: number | null;
	description: string | null;
	enabled: boolean;
	last_seen_at: string | null;
	last_error: string | null;
	created_at: string;
	updated_at: string;
}

/** A stored SSH host key, as returned by GET /servers/{id}/host-keys (no key material). */
export interface HostKey {
	id: number;
	key_type: string;
	fingerprint: string;
	first_seen_at: string;
	last_seen_at: string;
}

export interface CreateServer {
	name: string;
	hostname: string;
	port?: number;
	username?: string;
	credential_id?: number;
	description?: string;
	enabled?: boolean;
}

/** Omitted fields are kept; empty strings clear text fields; `credential_id: null` unassigns. */
export interface UpdateServer {
	name?: string;
	hostname?: string;
	port?: number;
	username?: string;
	credential_id?: number | null;
	description?: string;
	enabled?: boolean;
}

export interface TestConnectionResult {
	success: boolean;
	message: string;
	server_id: number;
	hostname: string;
	port: number;
	latency_ms?: number;
}
