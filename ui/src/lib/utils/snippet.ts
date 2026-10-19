/**
 * Quick-command variables, so one snippet serves every server.
 *
 * `{{host}}`, `{{user}}`, `{{port}}` and `{{name}}` are filled from the server of the tab the
 * command is typed into. Any other `{{word}}` is asked for before the command is typed, and
 * `{{word:default}}` pre-fills the answer. A name is a plain identifier (letters, digits, `_`
 * and `-`, not starting with a digit), so Go templates such as
 * `docker ps --format '{{.Names}}'` or `{{json .}}` are left exactly as written; so are the
 * template keywords that stand alone (`{{end}}`, `{{else}}`, …).
 *
 * Values are inserted as given, not shell-quoted: an answer may be a glob or a pipeline.
 */

/** `{{ name }}` or `{{ name:default }}`; the default may hold anything but braces */
const VARIABLE = /\{\{\s*([A-Za-z_][A-Za-z0-9_-]*)\s*(?::([^{}]*))?\}\}/g;

/** Go template keywords that appear without arguments; never variables */
const RESERVED = new Set(['end', 'else', 'break', 'continue', 'nil', 'true', 'false']);

/** Every variable occurrence: its name and the default it gives, if any */
function* occurrences(command: string): Generator<{ name: string; fallback?: string }> {
	for (const m of command.matchAll(VARIABLE)) {
		if (!RESERVED.has(m[1])) yield { name: m[1], fallback: m[2] };
	}
}

export const BUILTIN_VARIABLES = ['host', 'user', 'port', 'name'] as const;
export type BuiltinVariable = (typeof BUILTIN_VARIABLES)[number];

/** A variable the user is asked for */
export interface SnippetVariable {
	name: string;
	/** Pre-filled answer ('' when the snippet gives none) */
	default: string;
}

/** A quick command with variables to fill in before it is typed into the shell */
export interface SnippetRequest {
	command: string;
	/** What the tab's server provides (`{{host}}`, `{{user}}`, …) */
	known: Partial<Record<string, string>>;
	/** What to ask for */
	variables: SnippetVariable[];
}

/** What the tab's server provides, from the server record */
export function serverVariables(server: {
	hostname: string;
	port: number;
	username: string | null;
	name: string;
}): Partial<Record<BuiltinVariable, string>> {
	const vars: Partial<Record<BuiltinVariable, string>> = {
		host: server.hostname,
		port: String(server.port),
		name: server.name
	};
	if (server.username) vars.user = server.username;
	return vars;
}

/**
 * The variables to ask for, in order of first appearance, each once. A built-in the server
 * cannot provide (a server without a username has no `{{user}}`) is asked for too.
 */
export function promptedVariables(
	command: string,
	known: Partial<Record<string, string>> = {}
): SnippetVariable[] {
	const found = new Map<string, SnippetVariable>();
	for (const { name, fallback } of occurrences(command)) {
		if (known[name] !== undefined) continue;
		const seen = found.get(name);
		if (!seen) found.set(name, { name, default: fallback?.trim() ?? '' });
		else if (!seen.default && fallback) seen.default = fallback.trim();
	}
	return [...found.values()];
}

/** The command with every variable replaced; one without a value keeps its default. */
export function fillSnippet(command: string, values: Partial<Record<string, string>>): string {
	return command.replace(VARIABLE, (match, name: string, fallback: string | undefined) => {
		if (RESERVED.has(name)) return match;
		const value = values[name];
		return value !== undefined ? value : (fallback?.trim() ?? '');
	});
}

/** True when the command has any variable at all */
export function hasVariables(command: string): boolean {
	return promptedVariables(command).length > 0;
}
