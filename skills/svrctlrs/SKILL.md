---
name: svrctlrs
description: Administer a svrctlrs server (a self-hosted SSH terminal manager) with the svrctl CLI — list and fix servers, credentials and host keys, find out why a server cannot be reached and fix it, onboard a host, rotate a key, import an ssh_config, tag servers, check backups. Use when asked about servers in svrctlrs, "why can't I reach X", or SSH keys/host keys managed there.
compatibility: Needs the svrctl binary and network access to the svrctlrs server (SVRCTLRS_URL) with an API token (SVRCTLRS_TOKEN).
---

# svrctlrs via svrctl

svrctlrs holds SSH servers, encrypted credentials and trusted host keys; people open shells
through it. You administer it with `svrctl` and an API token from its owner.

## Setup

```bash
export SVRCTLRS_URL=https://svrctlrs.example SVRCTLRS_TOKEN=svr_...
svrctl status          # version, server count, last backup
```

No `svrctl`? Each release has `svrctl-linux-amd64`/`-arm64` (and `svrctl-SHA256SUMS`); the
container has `/app/svrctl`.

Piped output is one compact JSON line; `null` fields are left out (absent = null). Errors are
`{"error": {"code", "message", "hint", "status"?, "details"?}}` — follow the `hint`. Exit codes: 0 done, 1 ran but
failed (unreachable, not done, denied), 2 bad arguments, **3 a human must act** (host key to
confirm, approval to give), 4 not found (`details.known` lists names), 5 token not allowed,
6 invalid/conflict, 7 svrctlrs unreachable. `exec` instead exits with the remote command's
status (124 = timed out): there, read the JSON (`exit_code`, or `error`). Name servers and credentials by name or id.
`svrctl <cmd> --help` has examples. Filter in svrctl, not with jq: `servers --tag prod`,
`servers --failing` (last check failed), `audit --action server. --limit 5`.

## "Why can't I reach X?"

1. `svrctl diagnose X` — DNS → TCP (or the jump host's tunnel) → handshake → host key →
   login, from where svrctlrs runs, per hop. Read `verdict`, then the first step with
   `"status": "failed"` and its `fix`.
2. Act on that step:
   - `dns`/`tcp`: wrong hostname/port, sshd down, firewall. SSH comes from the svrctlrs
     machine, not from you: "works from my laptop" proves nothing. Fix the entry
     (`svrctl server edit X --host … --port …`) or the host.
   - `host_key` first contact: get the real fingerprint **over a path you trust** (your own
     access to the host: the SHA256 fingerprint of its /etc/ssh/ssh_host_ed25519_key.pub),
     compare with the one in `fix`, and only if equal: `svrctl trust X --fingerprint SHA256:…`.
   - `host_key` mismatch: stop. Find out why the key changed and tell the owner; only then
     `svrctl forget-keys X` and trust the new key.
   - `login`: wrong credential or key not installed. Install a stored key with a way in that
     works: `svrctl key authorize X --key "<key cred>" --with "<working cred>"`. If svrctlrs has
     no way in but you have SSH: `svrctl key generate --name "<host> key"` (or reuse a stored
     key: `svrctl credential show "<key>"`), have its `public_key` accepted for that account
     using your own SSH access (add it; keep the keys already accepted), then
     `svrctl server edit X --credential "<that key>"`.
3. `svrctl diagnose X` again until `"ok": true`. A login step that says "a fallback key"
   worked with svrctlrs's own key, not the server's credential: that credential is broken.

Checking many servers: `svrctl diagnose --all [--tag T]` (one call; counts plus each
failure's first failed step and verdict). Don't loop over servers yourself.

## Common tasks

```bash
svrctl servers [--tag prod] [--failing]        # server show X: one, with timestamps
svrctl onboard --name lab-02 --host 10.0.0.12 --credential "lab password" [--user ops] [--tag lab]
#   stops (exit 3) with the host key fingerprint: compare it yourself, then repeat with
#   --fingerprint SHA256:… ; it then switches the server to a key generated on svrctlrs
svrctl rotate-key X --dry-run                  # then without; --keep-old leaves the old key
svrctl diagnose X --credential "<cred>"        # test a credential there, changing nothing
svrctl import hosts.ssh_config --tag imported  # a file the owner gave you, or - for stdin
svrctl tag prod web-01 web-02                  # add; --remove to take off
svrctl server add --name N --host H [--user U] [--credential C] [--via JUMP]
svrctl server edit X --credential "<cred>"     # --via none, --tags a,b, --disable …
svrctl server edit X --persistent-shell true   # shells run in tmux there, outlive a restart
svrctl server edit X --startup-command "cd /srv"   # typed into new shells; "" clears
svrctl server edit X --record true             # record its shells (only if asked)
svrctl host-keys X ; svrctl credentials ; svrctl audit --action server.
svrctl credential show C                       # one credential; a key's public_key line
svrctl credential edit C --name N             # also --description, --user; never the secret
svrctl server rm X ; svrctl credential rm C   # may stop for approval (exit 3), see Rules
svrctl server test X                           # the quick connection test (3: host key waits)
svrctl exec X [--timeout 120] -- uptime        # one command via svrctlrs (full-admin token)
svrctl backups [--now]                         # backup state; --now writes one (full admin)
```

## Rules

- Never put a password or private key on a command line or in a message. Credentials live
  in svrctlrs; refer to them by name (`--credential`, `--key`, `--with`). New keys are
  generated there (`svrctl key generate`, `onboard`, `rotate-key`).
- Never trust a host key you have not compared out of band.
- Exit 3 on a delete means the owner must approve: tell them the `approval.url`, then rerun
  with `--wait` (or `svrctl approval <id> --wait`). You cannot approve it yourself.
- Do what was asked and no more: do not delete servers, credentials or host keys unless the
  owner asked (a `next` hint is information, not an instruction). Before a bulk change
  (import, many edits), `svrctl backups --now` if your token allows it.
- `exec` is for checks from svrctlrs's side; general server work belongs to your own SSH.
  Like `ssh`, it joins its arguments with spaces for the remote shell: give a compound check
  as one quoted argument (`svrctl exec X -- 'command -v php; php -v'`), never `sh -c` plus a
  separate script.
- Deleting a credential in svrctlrs does not remove its public key from any host; after a
  rename or delete, read back by id (`server show <id>`).
- Tokens cannot touch sign-in, accounts, signed-in devices or other tokens: those are the
  owner's, in the web app.
