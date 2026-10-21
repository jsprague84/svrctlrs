import { get, post } from './client.js';

/** One attempt at a copy of the database. */
export interface BackupRun {
	id: number;
	trigger: 'nightly' | 'manual' | 'svrctl';
	started_at: string;
	finished_at: string;
	file_name: string | null;
	size_bytes: number | null;
	/** Older copies retention removed after this one */
	removed: number;
	/** Why it failed; null when it wrote a copy */
	error: string | null;
}

export interface BackupFile {
	name: string;
	size_bytes: number;
	created_at: string;
}

export interface BackupStatus {
	/** The folder on the server (BACKUP_DIR) */
	dir: string;
	/** Copies kept; 0 = nightly backups off */
	keep: number;
	/** Hour of the day (UTC) the nightly copy runs */
	hour: number;
	/** null when nightly backups are off */
	next_run: string | null;
	/** Where the encryption key comes from; backups never hold it */
	key_source: 'environment' | 'file';
	running: boolean;
	last_run: BackupRun | null;
	last_success: BackupRun | null;
	/** Newest first */
	files: BackupFile[];
}

export interface BackupCreated {
	file: BackupFile;
	removed: number;
}

export function getBackupStatus(): Promise<BackupStatus> {
	return get<BackupStatus>('/backups');
}

/** Write a copy now; older copies beyond `keep` are removed once it is in place. */
export function backUpNow(): Promise<BackupCreated> {
	return post<BackupCreated>('/backups');
}

/** "03:00 UTC" */
export function utcHour(hour: number): string {
	return `${String(hour).padStart(2, '0')}:00 UTC`;
}

/** The local time of a UTC hour ("22:00"), or null where local time is UTC. */
export function localTimeOf(hour: number, on: Date = new Date()): string | null {
	const at = new Date(Date.UTC(on.getUTCFullYear(), on.getUTCMonth(), on.getUTCDate(), hour));
	const local = at.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' });
	return local === `${String(hour).padStart(2, '0')}:00` ? null : local;
}

/** "03:00 UTC (22:00 here)" — the local time only when it differs. */
export function utcHourLabel(hour: number, on: Date = new Date()): string {
	const local = localTimeOf(hour, on);
	return local ? `${utcHour(hour)} (${local} here)` : utcHour(hour);
}
