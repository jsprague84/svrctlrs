# The `svrctl` command line

`svrctl` administers svrctlrs from a shell, for agents (Hermes, Claude Code) and for people,
with an API token: servers, credentials, host keys, diagnose, exec, onboard, rotate-key,
approvals, audit, settings and backups. It is a thin client over the REST API (`docs/API.md`,
`docs/openapi.json`) and reads the server's own `svrctlrs-api` types. Why a CLI and a skill
rather than an MCP server, and the rule that every feature ships with its agent path:
[`agents.md`](agents.md).

It is a second binary of the server package (`server/src/bin/svrctl/`), built with the server
and shipped in the image (`/app/svrctl`), where `svrctl health` is the container's
healthcheck. `server/tests/svrctl.rs` runs the real binary against the real server on a
temporary database.

For agents there is a skill, `skills/svrctlrs/SKILL.md` (Hermes, Claude Code, Codex): setup,
the exit codes, the diagnose-and-fix loop, and the rules (never a secret on a command line,
never trust a host key not compared out of band, approvals are the owner's).
`scripts/eval/hermes/` checks agents with it.

## Setup

Each release on Forgejo has `svrctl-linux-amd64` and `svrctl-linux-arm64` with
`svrctl-SHA256SUMS`; the image has `/app/svrctl`. Use the one of the server's version:
`svrctl status` shows the server's, `svrctl --version` its own (a mismatch shows as
`BAD_RESPONSE`).

```sh
export SVRCTLRS_URL=https://svrctlrs.example     # default http://localhost:8080
export SVRCTLRS_TOKEN=svr_…                      # Settings → Access → Agent tokens
svrctl status
```

`--url` and `--token` override the environment. Every command but `health` and the local ones
needs a token; a token is never accepted in a URL. Through the reference deployment's edge,
a request carrying `Bearer svr_…` reaches the API without the browser sign-in
(`docs/DEPLOYMENT.md`, "Reverse proxy and edge auth").

On hermes, the owner's always-on agent host: the token in `~/svrctl_token.md` (mode 600), the
binary at `~/svrctlrs-eval/bin/svrctl`, and each installed skill copy (both Hermes profiles,
`~/.claude/skills/svrctlrs/` for AgentOS) with a "This deployment" paragraph naming the URL and
both paths. The agent exports `SVRCTLRS_URL` and `SVRCTLRS_TOKEN` in its own subprocess.

## Tokens and presets

A token carries one preset (`api/src/presets.rs`; USER-GUIDE.md, "Agent tokens"), each
covering the one before it:

- `read_only`: every read of the inventory and configuration (servers, host keys,
  credentials without their secrets, settings without secret values, audit, sessions,
  recording details, backups) and the token's own approval requests.
- `servers_keys`: adds managing servers, credentials and host keys, and the operations that
  log in with stored credentials without a shell: test, diagnose, onboard, rotate-key,
  authorize a key.
- `full_admin`: adds `exec`, terminals, files, recordings' content, settings, the web
  terminal's profiles and snippets, and `backups --now`.

No preset reaches sign-in, accounts, signed-in devices, tokens, or deciding an approval: those
are the owner's, in the web app. The Needs column below is the least preset that runs the
command.

## Commands

| Command | Does | Needs |
|---|---|---|
| `health` | is svrctlrs up: service, version, database (`/health`); exit 1 when degraded. The container healthcheck | no token |
| `status` | version, number of servers, and the last database backup (left out when the token may not read it) | read_only |
| `servers [--tag T] [--failing]` | the servers, filtered by tag or to those whose last check failed (`last_error`); alias `ls`. A list is for finding: timestamps are in `server show` | read_only |
| `server show <server>` | one server, with timestamps | read_only |
| `server add --name N --host H [--port P] [--user U] [--credential C] [--via JUMP] [--tag T …] [--description D] [--persistent-shell] [--startup-command CMD] [--record]` | add a server (`onboard` also confirms its host key and moves it onto a key) | servers_keys |
| `server edit <server> [--name --host --port --user --credential C\|none --via JUMP\|none --tags a,b --description --persistent-shell true\|false --startup-command --record true\|false --enable\|--disable]` | change a server; only what is passed changes (`--tags` replaces all; empty clears) | servers_keys |
| `server rm <server> [--wait]` | delete a server; may be held for the owner's approval (exit 3) | servers_keys |
| `server test <server>` | the quick connection test; exit 1 when it fails, 3 when a host key waits | servers_keys |
| `diagnose <server> [--credential C] \| --all [--tag T]` | why svrctlrs can or cannot reach it: DNS → TCP or the jump host's tunnel → handshake → host key → login, per hop, a verdict and a fix per failed step; `--credential` tries a stored credential without changing the server; `--all` checks every enabled server (four at a time) and reports counts and each failure. Exit 1 when unreachable | servers_keys |
| `exec <server> [--timeout S] -- <command…>` | one command through the server's stored credential (no terminal, 1–600 s, 1 MiB per stream, audited); exits with the command's status, 124 on a timeout | full_admin |
| `onboard --name N --host H --credential C [--port --user --via --tag … --fingerprint SHA256:… --keep-credential --dry-run]` | create (or resume) a server with a stored credential as the way in, confirm its host key only with `--fingerprint` you compared, check the login, then switch to a key generated on svrctlrs. Stops with exit 3 for the fingerprint | servers_keys |
| `rotate-key <server> [--keep-old] [--dry-run]` | replace the server's key: generate, install with the current key, prove, switch, remove the old public key | servers_keys |
| `host-keys <server>` | the recorded host keys, trusted or unconfirmed, with `SHA256:` fingerprints | read_only |
| `trust <server> --fingerprint SHA256:…` | trust the host key recorded on first contact, by the fingerprint you compared out of band | servers_keys |
| `forget-keys <server> [--wait]` | forget the server's host keys (after you know why its key changed); may be held for approval | servers_keys |
| `import [FILE\|-] [--credential C] [--tag T …]` | create servers from an `ssh_config`'s `Host` blocks (`ProxyJump` becomes the jump host) | servers_keys |
| `tag <tag> <server…> [--remove]` | add a tag to servers, or take it off | servers_keys |
| `credentials` | the credentials, never their secrets, with fingerprints and the servers using each; alias `creds` | read_only |
| `credential show <credential>` | one credential; an SSH key's `public_key` (its `authorized_keys` line) | read_only |
| `credential edit <credential> [--name N] [--description D] [--user U]` | rename it, or change its description or login user; never the secret | servers_keys |
| `credential rm <credential> [--wait]` | delete it (refused while a server uses it); may be held for approval | servers_keys |
| `key generate --name N [--comment C]` | generate an ed25519 key on svrctlrs; prints its public half | servers_keys |
| `key authorize <server> --key K --with C` | install key credential K on the server, logging in once with C, and switch the server to K | servers_keys |
| `approval <id> [--wait]` | one approval request (from a held call); `--wait` waits for the owner's answer | read_only |
| `audit [--limit N] [--action server.,token.] [-q TEXT]` | the audit log, newest first; rows a token caused name it | read_only |
| `settings [KEY]` | every setting, or one (`backup.keep`, `notification.url`, …); a secret one shows only whether it is set | read_only |
| `settings set <KEY> <VALUE\|->` | change one setting; `-` reads the value from stdin, so a secret never goes on a command line | full_admin |
| `backups [--now]` | where the nightly copies go, the last attempt, the copies kept; `--now` has the server write one now (before a bulk change) | read_only; `--now` full_admin |
| `openapi [--find TEXT]` | the API's OpenAPI document; `--find` lists the operations whose path, id or summary contain TEXT, each with its `svrctl` command, or why it has none | read_only |
| `api <METHOD> <PATH> [--body JSON\|-] [--timeout S] [--wait]` | any API call with the token and svrctl's error handling, for what no command covers (`svrctl api GET /sessions`); the path is under `/api/v1`. A held call exits 3, or waits with `--wait` | the call's preset (`openapi --find`) |
| `backup [--out DIR] [--keep N] [--with-key]` | **local**: a consistent copy of the database into the backup folder (the nightly copy: no sign-ins, 0600), then retention; the key only with `--with-key` | the host (no token) |
| `restore <FILE> [--check]` | **local**: check a copy (integrity, schema, every credential readable with this key) and stage it for the next start; `--check` only looks | the host (no token) |
| `reset-password <USER> [--password-stdin \| --generate] [--keep-sessions] [--clear-totp]` | **local**: a new password when nobody can sign in; signs the account out everywhere; `--clear-totp` also turns two-factor sign-in off | the host (no token) |

Every API operation has its way for agents in `server/src/bin/svrctl/surface.rs`: one of these
commands, or `svrctl api` with the reason there is no command (the web terminal's profiles and
snippets, the file browser, recordings, detached shells), or none (the owner's: sign-in,
accounts, devices, tokens, deciding approvals; and a credential's secret, which the owner
enters in the web app). `svrctl openapi --find <word>` shows it per operation. `cargo test`
fails when an operation has no row, a row names a command that is gone, a command is missing
here or in the skill, or the preset named here could not make the call or is more than it
needs (`docs/agents.md`, "The parity check").

A server or credential is named by its id or its name (ignoring case). A name that matches
nothing is `NOT_FOUND` with the names there are in `error.details.known`; two with the same
name are `AMBIGUOUS` (use the id).

**Local commands** run next to the database, without the server or a token:
`docker exec svrctlrs /app/svrctl backup`, `… restore <file>`, `… reset-password <user>`
(`docs/DEPLOYMENT.md`, "Backups" and "Locked out"). `--database-url` defaults to
`DATABASE_URL`. An agent with a token never runs them.

## Approvals

A token that asks first (the default for new tokens) has three calls held for the owner:
`server rm`, `credential rm` and `forget-keys` (`docs/API.md`, "Approvals"). svrctl then
prints the held request (`{"approval": {id, status, summary, url, …}, …}`) and exits **3**: a
human must act. With `--wait` it waits for the answer instead (up to `--wait-timeout`, default
an hour) and prints the decided request: exit 0 when it was approved and done, 1 when it was
denied, expired or the call failed. `svrctl approval <id> --wait` waits on one later. An agent
tells the owner what it asked for and where to approve it; it never looks for another way.

## Output

On a terminal: short text. On a pipe (an agent, a script), or with `--json`: one compact JSON
line, the server's own answer with every `null` left out (an absent key means `null`).
`--text` forces text on a pipe. Lists are for finding: `servers` and `settings` leave out
timestamps, `diagnose` the steps a failure skipped. Filter with the flags (`--tag`,
`--failing`, `--action`), not with jq.

Errors are one JSON document on stdout (on a pipe) or lines on stderr (on a terminal):

```json
{"error":{"code":"TOKEN_SCOPE","message":"…","hint":"This token's access does not cover that; …","status":403}}
```

`code` is the API's (`docs/API.md`, "Envelope") or svrctl's own: `UNREACHABLE`, `NO_TOKEN`,
`AMBIGUOUS`, `BAD_RESPONSE` (the server answered something this svrctl does not read: two
different versions), `USAGE`, `FAILED`. `hint` says what to do next.

## Exit codes

| Code | Meaning | Error codes |
|---|---|---|
| 0 | done | |
| 1 | it ran, and the answer is a failure: unreachable, onboarding or rotation stopped, an approval denied | `FAILED` |
| 2 | bad arguments | `USAGE` (and clap's own) |
| 3 | a human must act: a host key to confirm, an approval to give | `HOST_KEY_UNCONFIRMED`, `APPROVAL_PENDING` |
| 4 | not found | `NOT_FOUND` |
| 5 | not allowed | `UNAUTHORIZED`, `FORBIDDEN`, `NO_TOKEN`, `TOKEN_*` |
| 6 | invalid, or conflicts with the current state | `BAD_REQUEST`, `CONFLICT`, `AMBIGUOUS`, … |
| 7 | svrctlrs unreachable, or failed itself | `UNREACHABLE`, `INTERNAL_ERROR`, `BAD_RESPONSE` |
| 124 | `exec`: the command was still running at its timeout | |

`exec` otherwise exits with the remote command's own status: read its JSON (`exit_code`, or
`error`) to tell the two apart.
