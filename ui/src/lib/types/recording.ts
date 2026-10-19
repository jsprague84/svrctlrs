export type RecordingStatus = 'recording' | 'complete' | 'truncated' | 'interrupted' | 'failed';

/** One recorded shell, as `GET /api/v1/recordings` lists it. Only its owner ever sees it. */
export interface Recording {
	id: number;
	user_id: number;
	/** null once the server was deleted; `server_name` keeps what it was called */
	server_id: number | null;
	server_name: string;
	session_id: string;
	/** Terminal size when the recording started; resizes are events in the cast */
	cols: number;
	rows: number;
	started_at: string;
	ended_at: string | null;
	duration_ms: number;
	/** Size of the asciicast, i.e. of a download */
	size_bytes: number;
	status: RecordingStatus;
	/** Why it ended: the shell's exit, the size limit, an error */
	end_reason: string | null;
}

export interface RecordingsListResponse {
	recordings: Recording[];
	/** Days a finished recording is kept; 0 keeps it until deleted */
	retention_days: number;
	/** Size at which a recording stops */
	max_bytes: number;
}
