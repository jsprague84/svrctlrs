---
name: svrctlrs
description: Administer a svrctlrs server (a self-hosted SSH terminal manager) with the svrctl CLI — list and fix servers, credentials and host keys, find out why a server cannot be reached and fix it, onboard a host, rotate a key, import an ssh_config, tag servers. Use when asked about servers in svrctlrs, "why can't I reach X", or SSH keys/host keys managed there.
---

# svrctlrs via svrctl

svrctlrs holds SSH servers, encrypted credentials and trusted host keys; people open shells
through it. You administer it with `svrctl`, which talks to its API with a token.

## Setup

```bash
export SVRCTLRS_URL=https://svrctlrs.example     # the server
export SVRCTLRS_TOKEN=svr_...                     # an API token from its owner
svrctl status                                     # check: version and server count
```

Output is one JSON document when piped (always, for you); errors are
`{"error": {"code", "message", "hint"}}` — follow the `hint`. Exit codes: 0 done, 1 ran but
failed (unreachable, not done, denied), 2 bad arguments, **3 a human must act** (host key to
confirm, approval to give), 4 not found, 5 token not allowed, 6 invalid/conflict, 7 svrctlrs
unreachable. Servers and credentials are named by id or name. `svrctl <cmd> --help` has examples.

## "Why can't I reach X?"

1. `svrctl diagnose X` — walks DNS → TCP (or the jump host's tunnel) → SSH handshake → host
   key → login, from where svrctlrs runs, for each jump host then X. Read `verdict`, then the
   first step with `"status": "failed"` and its `fix`.
2. Act on that step:
   - `dns`/`tcp`: wrong hostname/port, sshd down, firewall. SSH comes from the svrctlrs
     machine (Docker NAT), not from you: "works from my laptop" proves nothing. Fix the
     server entry (`svrctl server edit X --host … --port …`) or the host.
   - `host_key` first contact: get the real fingerprint **over a path you trust** (your own
     access to the host: the SHA256 fingerprint of its /etc/ssh/ssh_host_ed25519_key.pub),
     compare with the one in `fix`, and only if equal: `svrctl trust X --fingerprint SHA256:…`.
   - `host_key` mismatch: stop. Find out why the key changed (reinstall?) and tell the owner;
     only then `svrctl forget-keys X` and trust the new key.
   - `login`: the credential is wrong or the key is not installed. Install a stored key with
     a way in that works: `svrctl key authorize X --key "<key cred>" --with "<working cred>"`.
3. `svrctl diagnose X` again until `"ok": true`.

## Common tasks

```bash
svrctl servers                                # [--tag prod]
svrctl server show X
svrctl onboard --name lab-02 --host 10.0.0.12 --credential "lab password" [--user ops] [--tag lab]
#   stops (exit 3) with the host key fingerprint: compare it yourself, then repeat with
#   --fingerprint SHA256:… ; it then switches the server to a key generated on svrctlrs
svrctl rotate-key X --dry-run                 # then without --dry-run
svrctl import hosts.ssh_config --tag imported # a file the owner gave you, or - for stdin
svrctl tag prod web-01 web-02                 # add; --remove to take off
svrctl server add --name N --host H [--user U] [--credential C] [--via JUMP]
svrctl server edit X --credential "<cred>"    # --via none, --tags a,b, --disable …
svrctl host-keys X ; svrctl credentials ; svrctl audit --action server.
svrctl exec X -- uptime                       # one command via svrctlrs (full-admin token)
```

## Rules

- Never put a password or private key on a command line or in a message. Credentials live
  in svrctlrs; you refer to them by name (`--credential`, `--key`, `--with`). New keys are
  generated there (`svrctl key generate`, `onboard`, `rotate-key`).
- Never trust a host key you have not compared out of band.
- Exit 3 on a delete means the owner must approve: tell them the `approval.url`, then rerun
  with `--wait` (or `svrctl approval <id> --wait`). You cannot approve it yourself.
- Use `--dry-run` before `onboard`/`rotate-key` when unsure. `exec` is for checks from
  svrctlrs's side; general server work belongs to your own SSH.
