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

/** An SSH host key recorded for a server, as returned by GET /servers/{id}/host-keys (no key
 *  material). `trusted` is false for the key seen on first contact until the user confirms it. */
export interface HostKey {
	id: number;
	key_type: string;
	fingerprint: string;
	first_seen_at: string;
	last_seen_at: string;
	trusted_at: string | null;
	trusted: boolean;
}

/** What the user has to compare before a first connection goes through. */
export interface HostKeyChallenge {
	key_type: string;
	fingerprint: string;
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
	/** On failure: `HOST_KEY_UNCONFIRMED` (with `details`), `HOST_KEY_MISMATCH`, `BAD_REQUEST` */
	code?: string;
	details?: HostKeyChallenge | null;
	server_id: number;
	hostname: string;
	port: number;
	latency_ms?: number;
}
