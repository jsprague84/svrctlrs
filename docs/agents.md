# Agents are first class

Owner, 2026-10-19 and 2026-10-20: agents such as Hermes (the owner's always-on agent) and
Claude Code administer svrctlrs from outside, and agent access stays first class: nothing may
make `svr_` tokens or `svrctl` less convenient. Everything a person can do or read in
svrctlrs that an agent has a use for, an agent can do through a token, quickly and cheaply,
and that stays true as the app changes. There is no assistant inside the app.

## What agents do here

Administer svrctlrs itself: add and fix servers, credentials and host keys, find out why a
server cannot be reached and fix it (`diagnose`), onboard a host, rotate a key, import an
`ssh_config`, tag servers, check backups and the audit log. svrctlrs is **not a second door**
to the servers: an agent keeps its own SSH for work on a host. A token may run one command
(`exec`, full admin) when that helps a diagnosis from svrctlrs's side.

## The interface: `svrctl` and an Agent Skill, not an MCP server

Decided 2026-10-19 and 2026-10-21 (ROADMAP "Agent access" and "Out of scope"):

- For an agent that has a shell, a CLI plus an Agent Skill (`SKILL.md`, the open standard
  Hermes, Claude Code and Codex read) is cheaper per turn and more reliable than a wide list
  of MCP tools: the skill loads only when the task needs it, and the CLI's output is shaped
  for agents (one compact JSON line without nulls, lists for finding, filters such as
  `--tag` and `--failing` in the CLI, exit codes that say what to do next, hints in errors,
  names where ids are awkward). Hermes's own docs prefer a skill plus `gh` to the GitHub MCP
  server; the eval below measured it here.
- MCP is moving to a few task-level tools or code against a typed API, and its 2026-07-28
  revision reworked the transport. No client without a shell is a use case, so the owner
  dropped it (2026-10-21). If one ever matters, an MCP server would be a thin wrapper over
  these same commands.
- The API stays the source of truth: `svrctl` is a thin client over it with the server's own
  `svrctlrs-api` types, `svrctl openapi --find` shows any operation and the command for it,
  and `svrctl api` reaches anything a command does not cover.

The pieces: [`cli.md`](cli.md) (every command, with the preset it needs), the skill
`skills/svrctlrs/SKILL.md` (setup, the exit codes, the diagnose-and-fix loop, the rules),
token presets and approvals (below), the Hermes eval (`scripts/eval/hermes/`).

## Tokens

The owner makes a token per agent in Settings → Access → Agent tokens (USER-GUIDE.md, "Agent
tokens"; SECURITY.md, "API tokens"): `svr_` and 40 characters, shown once, hashed, a lifetime,
revoked in one tap. Each carries a preset, decided by method and path before any handler
(`api/src/presets.rs`):

| Preset | Adds | For |
|---|---|---|
| `read_only` | every read of inventory and configuration, its own approval requests | reporting, checking |
| `servers_keys` | servers, credentials and host keys; test, diagnose, onboard, rotate-key, authorize a key | an agent that fixes the server list (the eval's) |
| `full_admin` | `exec`, terminals, files, recordings, settings, the web terminal's own, `backups --now` | rarely; as good as the owner's password on every server |

No preset reaches sign-in, accounts, signed-in devices, tokens or deciding an approval: those
are the owner's, signed in. A token never carries a secret in: credentials are entered by the
owner in the web app, and agents refer to them by name; new keys are generated on svrctlrs
(`key generate`, `onboard`, `rotate-key`).

## Approvals

A token that asks first (the default) has the three calls that cannot be undone held for the
owner: deleting a server, deleting a credential, forgetting a server's host keys (`202`, a link
by push, Matrix or webhook). `svrctl` exits **3** with the request, or waits with `--wait`; the
owner approves or denies on `/approvals`, signed in, and the call is replayed as the token.
An agent tells the owner what it asked for and the link; it never approves its own request and
never looks for another way (docs/API.md, "Approvals").

## Exit codes and output

`svrctl` answers one compact JSON line on a pipe and short text on a terminal; errors are
`{"error": {code, message, hint, …}}`. Exit 0 done, 1 ran but failed, 2 bad arguments, **3 a
human must act** (a host key to confirm, an approval to give), 4 not found, 5 not allowed,
6 invalid or conflicting, 7 svrctlrs unreachable; `exec` exits with the remote command's
status (124 on a timeout). Details: [`cli.md`](cli.md), "Output" and "Exit codes".

## The rule: every feature ships with its agent path

A change that adds or alters what a person can do or see is not done until an agent can do it
too, the same day, or the commit says why it has no use for one:

1. **An `svrctl` command**, or a row in the parity table saying `api` (rare, with the reason)
   or `none` (the owner's, the browser's, a secret). Commands follow [`cli.md`](cli.md): JSON
   on a pipe, the exit codes, names or ids, hints in errors.
2. **The skill** says when to use it, in a line or an example, and any rule that goes with it.
3. **`docs/cli.md`** lists the command, with the least preset it needs.
4. **The token presets** (`api/src/presets.rs`) allow it for the right presets, and approvals
   hold where they should.
5. **A Hermes eval task** (`scripts/eval/hermes/tasks/`) when it is a new workflow an owner
   would ask for in words ("why can't I reach X", "onboard this host").

## The parity check

Tests make forgetting impossible (`server/src/bin/svrctl/surface.rs`, run by `cargo test`;
copied from rustRoast's `bin/roast/surface.rs`):

- **Every API operation** in `docs/openapi.json` (which a server test keeps equal to the
  router's own document) has exactly one row: the `svrctl` command that makes it, or `api`
  with a reason, or `none` with a reason. A new route without a row fails; a row for a route
  that is gone fails.
- **Every command named** exists in svrctl's clap tree with the flags named (`backups --now`).
- **Every runnable command** in the clap tree has a row in `docs/cli.md`'s command table and a
  `svrctl <command>` in the skill (`backup`, `restore` and `reset-password`, which run on the
  host next to the database, are left out of the skill on purpose), and a flag the table
  names is in its row.
- **The preset named** is one that makes the call, no smaller preset would, and the row's
  "Needs" column says it; a call no preset reaches is `none`, never a command or `api`.

`svrctl openapi --find <word>` prints each operation's row, so an agent that finds a call also
finds the command for it, or why there is none. `server/src/api_tokens.rs`'s
`every_route_a_preset_allows_is_documented` checks the other direction: nothing a preset
allows is missing from the document.

What the table holds on 2026-10-30 (84 operations): 30 reached by a command; 30 by
`svrctl api` only (the web terminal's profiles, snippets and preferences, the file browser,
recordings, detached shells, its terminal ticket, notification tests, bulk and new settings,
the account list); 24 none (sign-in, accounts, devices, tokens, deciding approvals, the keep
time of detached shells, and creating a credential, whose secret the owner enters). Building
it found svrctl without `settings`, `api` or `openapi --find` (added with it), and its first
run found the skill naming `server show` only in a comment.

The Hermes eval stays the end-to-end proof: real agents on the real binary, graded on the end
state, with the transcripts read for how (was each host key compared out of band, did the
agent stay inside the task).

## The commit check

The parity table sees routes; a behaviour with no route could still ship without its agent
path. So `scripts/ci/agent-path.sh` (prepush and CI's Lint; DEVELOPMENT.md, "The commit
check"): a `feat` commit that changes app code must also touch the skill, `docs/cli.md`,
this file or an eval task, or carry `Agent-path: <why none>`; and touch `docs/` or
`CHANGELOG.md`, or carry `Docs: <why none>`. It proves the agent path was considered; the
table and the eval prove it is complete.

## Kept current

The installed copies on hermes (the skill in both Hermes profiles and in
`~/.claude/skills/svrctlrs/` for AgentOS, each with its own "This deployment" paragraph, and
`~/svrctlrs-eval/bin/svrctl`) are refreshed by hand after a release (last 2026-10-25, 4.0.1).
ROADMAP 31 has CI do it after every deploy.
