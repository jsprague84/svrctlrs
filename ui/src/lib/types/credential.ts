export type CredentialType = 'ssh_key' | 'password' | 'api_token' | 'certificate';

export interface Credential {
	id: number;
	name: string;
	credential_type: CredentialType;
	description: string | null;
	username: string | null;
	has_value: boolean;
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
}

export interface UpdateCredential {
	name?: string;
	value?: string;
	description?: string;
	username?: string;
}

export const CREDENTIAL_TYPE_LABELS: Record<CredentialType, string> = {
	ssh_key: 'SSH Key',
	password: 'Password',
	api_token: 'API Token',
	certificate: 'Certificate'
};
