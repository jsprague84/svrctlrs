export type CredentialType = 'ssh_key' | 'password' | 'api_token' | 'certificate';

export interface Credential {
	id: number;
	name: string;
	credential_type: CredentialType;
	description: string | null;
	username: string | null;
	has_value: boolean;
	/** An SSH key credential has a passphrase stored (encrypted) with it. */
	has_passphrase?: boolean;
	/** Public half of an SSH key credential (on single reads), for authorized_keys. */
	public_key?: string | null;
	in_use?: boolean;
	created_at: string;
	updated_at: string;
}

export interface CreateCredential {
	name: string;
	credential_type: CredentialType;
	value: string;
	description?: string;
	username?: string;
	/** Passphrase of an encrypted SSH key (ssh_key only). */
	passphrase?: string;
}

export interface UpdateCredential {
	name?: string;
	value?: string;
	description?: string;
	username?: string;
	/** New passphrase; omit to keep, empty string to remove. */
	passphrase?: string;
}

export const CREDENTIAL_TYPE_LABELS: Record<CredentialType, string> = {
	ssh_key: 'SSH Key',
	password: 'Password',
	api_token: 'API Token',
	certificate: 'Certificate'
};
