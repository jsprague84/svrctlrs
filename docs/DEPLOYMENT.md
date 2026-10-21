# Deployment

SvrCtlRS ships as one container image, `git.js-node.cc/jsprague/svrctlrs`, built by CI from
`master` (tags `latest`, `<sha7>`, `<sha>`) and from release tags (`vX.Y.Z`), for `linux/amd64`
and `linux/arm64` (a Raspberry Pi 4 or 5 on a 64-bit OS, other ARM servers); Docker pulls the
right one. The container
runs as uid 1000, listens on 8080, stores everything under `/app/data`, and reports health
through `/app/svrctl health` (which calls `GET /api/v1/health`).

## Compose

`docker-compose.yml` in the repository is the reference. It expects a Traefik edge with an
external `proxy` network (the dev-space edge standard). For a host of its own use
`docker-compose.standalone.yml` instead: no labels, no external network, port 8080 published
on `127.0.0.1` for a reverse proxy on the same host (`SVRCTLRS_BIND=8080` in `.env` exposes it
on every interface; on plain HTTP set `SESSION_SECURE=false` as well, or the browser drops the
session cookie). Put TLS in front before it faces the internet.

```bash
mkdir -p ~/docker-compose/svrctlrs && cd ~/docker-compose/svrctlrs
curl -fsSLO https://git.js-node.cc/jsprague/svrctlrs/raw/branch/master/docker-compose.yml
curl -fsSL  https://git.js-node.cc/jsprague/svrctlrs/raw/branch/master/.env.example -o .env
chmod 600 .env && $EDITOR .env
docker compose up -d
docker compose logs -f svrctlrs
```

Standalone: fetch `docker-compose.standalone.yml` as `docker-compose.yml` in the second line;
the rest is the same.

## Environment

| Variable | Required | Default | Meaning |
|---|---|---|---|
| `ADMIN_PASSWORD` | first start | — | password of the admin user created when the `users` table is empty; at least 12 characters or the first start fails. Can be removed from `.env` afterwards. |
| `ADMIN_USERNAME` | no | `admin` | admin username for that first start |
| `ENCRYPTION_KEY` | strongly recommended | generated | 64 hex chars (`openssl rand -hex 32`). If the variable is absent the server writes `data/encryption.key` (mode 0600) and uses that; if it is present but invalid — including an empty string — the server refuses to start. Losing the key makes every stored credential unrecoverable. |
| `DATABASE_URL` | no | `sqlite:/app/data/svrctlrs.db` | SQLite location (WAL mode; `-wal`/`-shm` files sit beside it) |
| `PUBLIC_URL` | with OIDC | — | the address people open, e.g. `https://svrctlrs.example.com`; the provider's callback is `PUBLIC_URL/auth/oidc/callback` |
| `OIDC_ISSUER` | no | unset | an OpenID Connect provider's issuer URL (any provider with discovery: Pocket ID, Authelia, Kanidm, Keycloak, …). Unset: password sign-in only |
| `OIDC_CLIENT_ID` | with OIDC | — | the client registered at the provider |
| `OIDC_CLIENT_SECRET` | no | unset | its secret (confidential client); unset for a public client with PKCE only |
| `OIDC_NAME` | no | `single sign-on` | what the button says: "Sign in with …" |
| `OIDC_SCOPES` | no | `openid profile email` (+ `groups` when groups are set) | scopes asked for; must include `openid` |
| `OIDC_ALLOWED_GROUPS` | no | unset | comma-separated; the ID token's `groups` must name one. A second check beside the provider's own restriction |
| `PASSWORD_LOGIN` | no | `on` | `off` refuses every password sign-in (form and JSON), leaving the provider as the only way in; needs `OIDC_ISSUER`. API tokens are not affected |
| `SESSION_SECURE` | no | `true` | set `false` only when serving plain HTTP on a trusted LAN |
| `ALLOWED_ORIGINS` | for native clients | unset | comma-separated origins of the Tauri apps, e.g. `tauri://localhost,https://tauri.localhost,http://tauri.localhost`. Enables CORS for them and switches the session cookie to `SameSite=None`. Leave empty for browser-only use. |
| `SESSION_DETACH_TTL_SECS` | no | `900` | how long an interactive shell is kept after its last client disconnects (reload, lost link, phone backgrounded) before it is closed |
| `SESSION_MAX_PER_USER` | no | `16` | live interactive sessions per user; the 17th `shell` request fails with `quota` |
| `SSH_KEEPALIVE_SECS` | no | `15` | seconds of silence before the SSH client sends a keepalive to the target; three unanswered close the connection. `0` disables keepalives |
| `RECORDINGS_DIR` | no | `data/recordings` (in the data volume) | where session recordings are written, one directory per user. Keep it on the data volume: the files are encrypted with `ENCRYPTION_KEY` and need the database rows to be found |
| `RECORDING_MAX_MB` | no | `100` | asciicast size at which a recording stops (the shell goes on, unrecorded) |
| `SPA_DIR` | no | `/app/ui/build` | where the built SPA lives |
| `RUST_LOG` | no | `info` | tracing filter |
| `LOG_FORMAT` | no | text | `json` for one JSON object per line (log shippers); text otherwise |
| `TAG` | compose only | `latest` | image tag; the deploy script writes the deployed commit's short SHA here |
| `SSH_KEY_PATH` | compose only | `./ssh` | host directory mounted read-only at `/home/svrctlrs/.ssh` for key-path credentials. Use a dedicated directory, not your own `~/.ssh`. |
| `SVRCTLRS_HOST` | compose only | `svrctlrs.js-node.app` | Traefik host rule |
| `SVRCTLRS_BIND` | standalone compose only | `127.0.0.1:8080` | host address and port published for the container's 8080 |

The server also accepts `--addr` (default `0.0.0.0:8080`). Servers and credentials live in
the database and are managed from the UI or API; there is no configuration file.

## Reverse proxy and edge auth

The reference compose uses three Traefik routers on the same host:

- `svrctlrs-secure` — every path (UI, `/api/`, `/ws/`), behind `secure-auth-chain@file`
  (Pocket ID passkey via tinyauth, then the app's own login). The web app's API calls and its
  terminal socket carry the passkey cookie like any page load. When that sign-in expires, an
  API call comes back as a redirect to tinyauth; the app reloads the page so tinyauth can ask
  for the passkey again.
- `svrctlrs-token` — `/api/` and `/ws/` requests that carry `Authorization: Bearer svr_…`,
  except the login, behind `secure-chain@file` (headers only). Agents and scripts (`svrctl`,
  Hermes) cannot do a passkey; the app checks their token and gives the request exactly what
  the token allows (SECURITY.md, "API tokens").
- `svrctlrs-health` — `/api/v1/health` (version and revision only), headers only, for
  monitors and the deploy check.

Do not route the rest of `/api/` around the passkey: that would let a password alone sign in
from the internet (SECURITY.md, known weakness 13). If you do not run tinyauth or another
login proxy, use the headers-only chain or drop the middlewares; the app's own login is then
the only gate, so keep it on a private network or behind a VPN. The server sets its own CSP
and security headers either way. WebSockets need no special proxy
configuration with Traefik; with nginx add the usual `Upgrade`/`Connection` headers on `/ws/`.

## Signing in with an identity provider (OIDC)

The server can sign people in with any OpenID Connect provider itself: no proxy and no other
service. It is optional; without `OIDC_ISSUER` nothing changes.

1. Register a client at the provider: callback URL `https://<host>/auth/oidc/callback`,
   confidential (with a secret) or public with PKCE, scopes `openid profile email` (plus
   `groups` if you use `OIDC_ALLOWED_GROUPS`). Restrict it to the people who should get a
   shell on your servers — at Pocket ID, a group-restricted client.
2. Set `PUBLIC_URL`, `OIDC_ISSUER`, `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET` and `OIDC_NAME`
   in `.env`, then `docker compose up -d`. The log says `OpenID Connect sign-in configured`.
   A half-made configuration stops the server with the reason.
3. Sign in with your password, open Settings → Access → Sign-in and choose **Link**. You go to
   the provider and back; your account now lists the identity. Accounts are never created or
   matched by e-mail address: an identity the server has not been told about is refused
   (audited as `login.oidc_unlinked`).
4. Sign out and use **Sign in with …** on the login page.
5. Optionally set `PASSWORD_LOGIN=off`. The login page then goes straight to the provider
   (except right after signing out), and both password routes answer 403. The start-up log
   warns if no account has a linked identity.

**Break-glass** (the provider is down, or the identity was unlinked by mistake): on the host,
set `PASSWORD_LOGIN=on` in `.env`, `docker compose up -d`, sign in with the password (and the
TOTP code, if the account has one), fix things, set it back. If the password is forgotten too,
`svrctl reset-password` (below). With the edge described next, password sign-in is reachable
from the internet while it is on, so do this from the LAN or put the passkey chain back on the
router for that time.

API tokens never go through any of this: agents keep working whatever the sign-in settings.

## Native clients against this server

Behind a passkey edge like the reference one the desktop and Android apps cannot sign in:
they cannot complete the passkey redirect. That returns with sign-in through the system
browser (ROADMAP items 21–22). Without such an edge:

1. Add the app origins to `ALLOWED_ORIGINS` and restart.
2. In the desktop or Android app, enter `https://<host>` on first launch. The app checks
   `/api/v1/health`, then shows the login form. Release Android builds refuse plain `http://`.
3. The apps authenticate with a Bearer token (the session id) rather than cookies; the token
   is stored on the device.

## Backups

A backup is the database **and the encryption key**; one without the other is useless.

```bash
# consistent snapshot while the server runs; keeps the newest 14 copies in the volume
docker exec svrctlrs /app/svrctl backup --out /app/data/backups --keep 14
# copy the newest one (and the key) off the host
docker cp svrctlrs:/app/data/backups ./svrctlrs-backups
```

`svrctl backup` uses SQLite's `VACUUM INTO`, so the copy is consistent without stopping
anything. It copies `data/encryption.key` next to the copies when the key comes from that
file; when the key is in `ENCRYPTION_KEY` (the compose setup) it says so — back up `.env`
alongside, it holds the only copy. A cron line on the host, nightly:

```
15 3 * * * docker exec svrctlrs /app/svrctl backup --out /app/data/backups --keep 14 >/dev/null && docker cp svrctlrs:/app/data/backups /srv/backups/svrctlrs
```

Restore: stop the container, put the copy back as `svrctlrs.db` in the volume (owned by uid
1000; remove stale `-wal`/`-shm` files), make sure `.env` has the matching `ENCRYPTION_KEY`,
start again — migrations run forward if the backup predates the image.

## Upgrades

`docker compose pull && docker compose up -d`. Migrations run at start-up; they are embedded
in the binary and never need manual steps. On docker-vm the CI deploy job does this for every
green `master` commit (below), so manual pulls are only needed when CI is down.

## CI deploy to docker-vm (owner setup, done once)

The `deploy` job in `.forgejo/workflows/docker.yaml` runs `ssh ubuntu@docker-vm <sha>` with the
`DOCKER_VM_DEPLOY_KEY` repository secret. On docker-vm that key is restricted to a forced
command, `~/docker-compose/svrctlrs/deploy.sh` (= `scripts/deploy/docker-vm-deploy.sh`),
which accepts only a 40-hex commit SHA, waits for `git.js-node.cc/jsprague/svrctlrs:<sha7>`,
writes `TAG=<sha7>` to `.env`, runs `docker compose up -d`, and fails unless the running
container is healthy and carries that revision label.

```
# ~/.ssh/authorized_keys on docker-vm
command="/home/ubuntu/docker-compose/svrctlrs/deploy.sh",no-port-forwarding,no-agent-forwarding,no-pty,no-X11-forwarding ssh-ed25519 AAAA… svrctlrs-forgejo-deploy@epyc-ai
```

The job's `known_hosts` pins docker-vm's ed25519 host key; update it in the workflow if the
host is reinstalled. When the secret is missing the job logs a notice and exits 0, so an image
still publishes.

## Locked out

If nobody can sign in (a forgotten password, the only account's password changed by someone
else), set a new one from the host, next to the database:

```bash
docker exec -it svrctlrs /app/svrctl reset-password admin           # asks twice, no echo
docker exec svrctlrs /app/svrctl reset-password admin --generate    # prints a new one
echo "$NEW" | docker exec -i svrctlrs /app/svrctl reset-password admin --password-stdin
```

It needs no running server and no credentials: whoever can `docker exec` into the container
already owns the data. The account is signed out everywhere (its browser sessions and app
tokens end; other accounts are untouched; `--keep-sessions` skips that), and the audit log
records `user.password_reset`. The 12-character minimum still applies. A lost authenticator
app with no recovery codes left: add `--clear-totp`, which also turns the account's two-factor
sign-in off (turn it on again under Settings → Access → Sign-in). Outside a container,
pass `--database-url sqlite:/path/to/svrctlrs.db`.

## Health and troubleshooting

- `docker exec svrctlrs /app/svrctl health` — JSON with `version` and `revision`.
- `docker logs svrctlrs` — tracing output; `RUST_LOG=debug` for more. Login failures are
  logged at `warn` with the attempted username. Every response has an `x-request-id` header
  (browser dev tools → Network); grep the logs for that value to see the request's span.
- A changed SSH host key on a target refuses the connection with an error in the terminal;
  Servers → Keys → **Forget all keys** clears the stored keys.
- Interactive sessions survive a client disconnect but not a server restart: `docker compose
  up -d` after a pull ends every open shell (the terminal shows "Server shutting down").
- `Session cookie SameSite=None` in the start-up log means `ALLOWED_ORIGINS` is set.
