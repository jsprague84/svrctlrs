# Changelog

All notable changes to SvrCtlRS. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
the release workflow uses the matching section as the release notes.

## [Unreleased]

### Added
- **Terminal preferences follow your account.** Font, cursor, scrollback, scrolling, the
  leader key and predictive echo are saved on the server, so the phone and the laptop agree;
  another device picks a change up when you switch back to it. The browser keeps a copy, so
  the terminal starts with them at once and works offline. The first sign-in after the update
  uploads the preferences that browser already had. API: `GET/PUT
  /api/v1/users/me/preferences/terminal` (your own only; a JSON object up to 16 KiB).
- **Predictive echo.** On a slow or lossy link, typed characters appear at once, dimmed and
  underlined, and become normal text when the server's echo arrives; a wrong guess disappears.
  Predictions are checked against what the terminal shows, so coloured prompts and zsh redraws
  work. Nothing is drawn until the shell has echoed in the current context, so a password
  prompt (or any prompt that does not echo) shows nothing; never in full-screen programs.
  Preferences → Behavior → Predictive Echo: on slow links (default), always, or off.
- **Session recording.** Switch on **Record sessions** for a server and every shell opened on
  it is recorded for the user who opened it: what the terminal shows (never the keystrokes, so
  a password typed at a silent prompt is not in it) and its resizes, as asciicast v2. The new
  shell says it is recorded. Recordings are encrypted at rest with the credential key, belong
  to their user alone (another account gets 404), stop at `RECORDING_MAX_MB` (default 100) and
  are deleted after `recording.retention_days` (default 30; Settings → General). Recorded
  panes show a red **REC** mark. **Recordings** in the sidebar lists yours, with a player
  (seek, speeds from ½× to 8×, long pauses shortened, Space and arrow keys) and a download as
  a standard `.cast` file. API: `GET /api/v1/recordings`, `GET /api/v1/recordings/{id}/cast`
  (play or `?download=true`), `DELETE /api/v1/recordings/{id}`.
- **Home-screen shortcuts.** Long-press the installed app's icon for **New terminal** and the
  three servers you opened shells on most recently. The server now serves
  `/manifest.webmanifest` itself and adds those shortcuts for the signed-in user (from their
  own audit rows; signed out, only the generic one). `/?connect=<server id>` opens a shell on a
  server, or shows the tab already on it; `/?open=servers` opens the server picker.
- An empty terminal (no tab, or a tab without a server) lists your servers and any shells
  waiting to be resumed, instead of a blank pane.
- Settings → About shows the running revision (the commit the image was built from).

### Changed
- **UI pass for phones and keyboards.** Every control is at least 44 px to the finger on a
  phone (buttons, inputs, chips through an invisible margin, close icons, sidebar rows, file
  breadcrumbs); form inputs use 16 px text, so iOS no longer zooms into them. On the Servers page a
  phone row is **Connect** plus a **⋯** menu instead of six icons, and shows its tags. The
  Settings tabs are a 3×2 grid on a phone, so Users, Audit and About are no longer off screen.
  Toasts sit at the bottom, above the extra-keys row, on an opaque background. Every dialog,
  sheet and side panel closes with Escape and gives focus back where it was; modal ones keep
  Tab inside; closed side panels no longer take focus. Loading, empty and error states look
  and behave the same everywhere, with Retry where something failed.
- The SSH client, host-key handling and the PTY driver are tested in CI against an
  in-process SSH server (first contact, trust, a changed key, password and key logins, a jump
  host, the shell's input, resize, exit and close); before, only a manual run against a real
  host covered them. The PTY start is split into `open_shell` and `spawn_pty_driver`, and the
  driver now holds the whole connection, jump hosts included, for the session's life.
- **Contrast.** Muted text in both themes and the light theme's accent, success, warning, error
  and info colours were below WCAG AA (the light warning was 2.4:1); all text colours now
  reach 4.5:1 on every surface, and a test checks the stylesheet so they stay there.
- The terminal pane's WebSocket session (connect, attach and replay, the ping/pong watchdog,
  reconnect backoff, the host-key challenge, the session record and saved screen) moved out of
  `TerminalPane.svelte` into `ui/src/lib/terminal/ptySession.ts`, with Vitest tests for the
  protocol. No behaviour change; the pane keeps xterm, gestures and the overlays (1,232 → 800
  lines).

### Fixed
- **A terminal that falls behind no longer goes blank.** When a client stops reading during a
  large burst (a stalled phone link, a `cat` of a big file) and the server's replay ring moves
  past what it saw, the server said so but sent nothing more, so the screen was cleared and
  only later output appeared. It now redraws from what the ring still holds, as an attach
  already did. Measured on the way, with `scripts/dev/run-e2e.sh backpressure`: a client that
  stops reading costs the server a bounded amount of memory (about 15 MB with a 200 MiB burst
  and a 15 s stall), a fast one gets every byte, and Ctrl-C under `yes` brings the prompt back
  in a quarter of a second, so no flow control is needed.
- **Hidden terminals no longer shrink their shell to 10×3.** A pane behind another tab, or
  every pane while the Servers or Settings page was open, fitted itself to its computed `100%`
  size read as 100 px and resized the remote PTY to about 10 columns by 3 rows. Programs in
  background tabs redrew for that size, and the shell you came back to echoed each keystroke
  with a full prompt redraw until the next command. Hidden panes now keep their size and refit
  when shown.
- The new shell started after "previous session has ended" asks for binary output frames like
  every other shell, so a multibyte character split across two SSH reads stays intact.
- The desktop toolbar's server picker follows the active tab instead of keeping whatever was
  picked last ("Select server…" next to a connected tab).
- **Authorize key only ever installed the first key.** Picking another key in the dialog
  snapped back to the first one: the effect that resets the form when the dialog opens also
  read the picked key, so every pick re-ran the reset. It now runs only when the dialog opens
  for a server.
- **The Credentials page described every SSH key as a file path**, with no fingerprint and no
  passphrase marker: the list query redacts values in SQL, and the key's kind was then read
  from the redacted text. The list now reads each key's decrypted row on the server (the
  values still never leave it), and `GET /credentials/{id}` reports `key_source` and
  `fingerprint` too.

## [3.1.0] - 2026-10-18

The "complete SSH client" release. Every peer that is a terminal manager had these and we did
not: a first-contact host-key prompt, keepalives, jump hosts, tags, `ssh_config` import. They
are in, along with a per-server startup command, a file browser, typed errors, a migration
baseline for fresh installs and a long list of hardening fixes since 3.0.0. Upgrading needs
nothing from you: an existing database finishes its old migration chain and is marked as being
at the baseline, and host keys you already trusted stay trusted.

### Security
- **First contact asks before trusting.** The first connection to a server used to store its
  host key silently. It now records the key unconfirmed and refuses the shell, the connection
  test, SFTP and *Authorize key* until the user has compared the key type and `SHA256:`
  fingerprint and chosen **Trust and connect** in the terminal (or **Trust key** on a failed
  *Test*, or **Trust** under Servers → Keys). API: `GET /servers/{id}/host-keys` reports
  `trusted`/`trusted_at`, `POST /servers/{id}/host-keys/trust` takes the fingerprint that was
  compared, refusals are `428 HOST_KEY_UNCONFIRMED` with the fingerprint in `error.details`
  (`host_key_unconfirmed` on the PTY socket). Confirming and forgetting are audited
  (`server.host_key_trusted`, `server.host_keys_forgotten`). Keys stored before this upgrade
  stay trusted; nothing to confirm for existing servers.

### Added
- **Terminal polish.** The shell's window title renames its tab (server name on hover); a
  bell flashes the pane, vibrates on a phone and marks an inactive tab until it is opened;
  grapheme clusters (emoji sequences, combining marks) take their real width (the Unicode 11
  addon was loaded but never selected, so width tables were at Unicode 6); the SSH client asks
  for `COLORTERM=truecolor` (best effort, subject to the server's `AcceptEnv`) and offers
  `zlib@openssh.com` compression.
- **Per-server startup command.** One line (`startup_command`; *Startup command* in the form)
  typed into every new shell on that server right after it opens, such as
  `tmux new -A -s main`, which gives sessions that outlive the app's detach grace period.
- **Tags.** Servers take free-form tags (comma separated in the form; `tags` in the API).
  The Servers page, the sidebar and the phone's server picker show the tags in use as filter
  chips, the palette and the page search match them, and a tag on a row is clickable.
- **Import from `~/.ssh/config`.** *Import* on the Servers page (API `POST /servers/import`)
  turns the `Host` blocks of a pasted `ssh_config` into servers: `HostName` (with `%h`), `Port`,
  `User`, and `ProxyJump` resolved to a jump host when its first hop is a known server; a
  credential and tags can be applied to all of them. Wildcards, `Match` blocks and existing
  names are reported as skipped; nothing is connected to during the import.
- **Jump hosts.** *Connect through* on a server (API `via_server_id`) reaches it through
  another server's SSH connection, like `ProxyJump`: the target's hostname is resolved from the
  jump host, so hosts on a private VLAN work. Both host keys are confirmed and checked, chains
  of up to four hops work, loops are refused (400), deleting a jump host makes its servers
  direct again, and the health check leaves such servers to connections and *Test*. The shell,
  *Test*, SFTP and *Authorize key* all go through it.
- **SSH keepalives.** Every SSH connection sends a keepalive after 15 s of silence and drops
  after three unanswered ones (`SSH_KEEPALIVE_SECS`, `0` disables), so idle shells survive NAT
  timeouts and phone networks and a dead link is noticed within a minute. Nagle's algorithm is
  off on the SSH socket for snappier keystrokes.
- rustls 0.23.45 → 0.23.46 for RUSTSEC-2026-0346 through -0349 (TLS secrets in `Debug` output,
  ECH and certificate-decompression issues); the CI audit gate caught them.

### Fixed
- A database failure while loading a server, credential or setting no longer masquerades as
  `404 NOT_FOUND`; it is a 500 with the cause, and a missing row is a plain 404 (previously the
  lookup queries reported a missing row as a database error). Deleting a credential that a
  server still uses is 409 at the database layer too, not only in the handler's pre-check.
- The command palette now honours a quick command's **Available on** server: a snippet saved for
  one server is listed only while a tab on that server is active. The setting was stored but
  never applied.
- Creating or renaming a credential or server to a name that is already taken answers
  `409 CONFLICT` with "A credential/server with that name already exists" instead of a generic
  500. Other database write failures now carry the driver's reason in the message.
- **Multibyte characters no longer garble in the terminal.** PTY output used to be converted to
  text per SSH read, so a box-drawing character, emoji or accented letter that straddled two
  reads came out as `�`. The server now keeps the shell's raw bytes (replay ring included) and,
  for clients that send `binary: true` on `shell`/`attach`, delivers them as binary WebSocket
  frames (8-byte `seq` + bytes) that xterm decodes as one stream. The web UI always asks for
  this; the JSON `output` frames remain for older clients.

### Removed
- **Command mode.** The one-shot "run a command, show its exit code" tab mode, its
  `/ws/terminal` socket, the mode selector and the command bar are gone; the owner never used
  it once interactive shells worked. With it goes local execution (`is_local` servers: creating
  one now answers 400, legacy rows cannot be connected to) and the `async-ssh2-tokio`
  dependency. Saved profiles that still say `mode: "cmd"` open as shells.
- The `--config <toml>` flag and `config/example.toml`. The file was parsed and then never
  read: servers and credentials have lived in the database since 3.0. The unused monitoring-era
  `Server`/`ServerStatus`/`MetricValue` types in `svrctlrs-core` went with it.
- The never-populated `quick_commands` JSON column on terminal profiles is no longer read or
  written (quick commands have had their own table since migration 019; the column stays in
  the schema until the migrations are squashed). Profile JSON no longer includes it.

### Changed
- **Migration baseline.** A fresh database now runs one `100_baseline.sql` instead of
  replaying the 24 migrations of the plugin, job and command-mode eras. Existing databases
  finish the old chain (now frozen in `database/migrations-legacy/`, with a final `024` that
  disables retired local servers, gives them `hostname = 'localhost'`, and drops the columns
  nothing reads) and are marked as being at the baseline; nothing to do on upgrade. The
  `servers` model lost `is_local`, the detected-OS facts and `metadata`; the API's server
  object no longer carries them, `hostname` is always present, and `POST /servers` requires
  it.
- `svrctlrs_core::Error` is typed by kind (`NotFound`, `Conflict`, `InvalidInput`, `Database`
  with its cause chain, `Encryption`, `Config`, `Notification`, `Other`) and the API's
  `ApiError` implements `IntoResponse` with `From` conversions, so handlers propagate query
  errors with `?`. The JSON error envelope is unchanged; 404 messages now name the row
  ("Server 3 not found") and 500 messages carry the full cause chain.
- **One SSH client.** `server/src/ssh.rs` (russh) now makes every connection: the shell, the
  connection test and **Authorize key**. Host keys are verified during the handshake on all of
  them (trust on first use, fail closed, mismatch message shown to the user), and the
  authentication order is the same everywhere: a password credential's password, else the
  key credential's key and then the container's default keys. **Password credentials now work
  for interactive shells**, which they did not before. The audit entry for an opened shell
  records the login and the key or `password` used.

### Added
- **File browser (SFTP).** A folder button in the terminal toolbar (phone: *Files on this
  server* in the tab sheet; palette: *Browse Files*) opens the active tab's server over SFTP:
  browse from the home directory with a breadcrumb, download a file, upload files into the
  current folder, make a folder, rename, delete (an empty folder only). The browser follows the
  active tab and remembers the last directory per server. API under
  `/api/v1/servers/{id}/files` (list, `content` GET/PUT streamed, `mkdir`, `rename`, DELETE),
  one SSH connection per call with the shell's host-key and credential rules, outside the 30 s
  request timeout; every write and download is audited (`files.*`).
- **Request ids.** Every HTTP response carries an `x-request-id` header with a UUID the server
  minted (a client-supplied value is dropped, so the log cannot be written to from outside), and
  the request's log span has the same `request_id` field. WebSocket upgrade URIs now also redact
  the one-time `ticket` value in logs.
- **Resume lost sessions.** A shell whose tab was closed, whose browser tab was lost, or that
  was opened on another device is still running on the server for the grace period; it can now
  be picked up again. The terminal toolbar's **Sessions** button (phone: *Sessions on the
  server* in the tab sheet; palette: *Resume Session*) lists the account's live shells from
  `GET /api/v1/sessions`, split into *Not open here* (**Resume** opens a tab attached to it with
  a full replay, **End** terminates it via `DELETE /api/v1/sessions/{id}`) and *Open in this
  window* (jump to the tab, or end it). The button shows how many shells are waiting. A refused
  end (unknown id, another user's session) answers 404; ending a session a pane here is attached
  to drops that pane to its *Session ended* overlay.
- **Persistent terminal sessions.** An interactive shell now lives in the server
  (`server/src/sessions.rs`), not in the WebSocket. When the connection drops — page reload,
  lost Wi-Fi, a phone that slept, leaving the terminal page — the shell keeps running for
  `SESSION_DETACH_TTL_SECS` (default 15 min) and the pane reattaches to it, replaying the output
  it missed from a per-session ring (4096 frames / 2 MiB). Open tabs are remembered for the life
  of the browser tab, so a reload brings them back attached. **Disconnect** and closing a tab
  still end the shell. Protocol: `attach {session_id, last_seq}`, numbered `output` frames,
  `replay_start`/`replay_end`/`replay_window_lost`, `closed`, `error {code: session_gone}`;
  `GET /api/v1/sessions`, `DELETE /api/v1/sessions/{id}`; `SESSION_MAX_PER_USER` (default 16).
  The "leave page?" prompt is gone for PTY tabs.
- **Leader key.** `Ctrl+Space` (Preferences → Behavior: `Ctrl+Shift+Space`, `Alt+Space` or
  off) then one key: `c`/`x` new/close tab, `n`/`p` next/previous, `1`–`9` tab by number,
  `` ` `` last tab, `h j k l` focus pane, `[`/`]` cycle panes, `s` layout, `/` search, `:`
  palette, `?` keeps the cheat sheet open. Browsers reserve `Ctrl+Tab` and `Ctrl+Shift+T/W`,
  which made keyboard navigation unreliable in the web app.
- **Installable web app.** Web manifest, icons and a service worker that caches only the hashed
  app shell (never `/api/`, `/ws/` or pages). "Add to Home screen" on a phone gives a standalone
  window; together with persistent sessions this replaces the parked Android app.
- Key setup without leaving the app: **Generate SSH key** (ed25519, private half stored
  encrypted, public half shown), **Public key** on any key credential, and **Authorize key** on
  a server, which installs the public key in `~/.ssh/authorized_keys` over a one-time password
  or an existing credential, verifies the key logs in, and switches the server to it
  (`POST /credentials/generate-key`, `POST /servers/{id}/authorize-key`).
- Host-key management: `GET/DELETE /api/v1/servers/{id}/host-keys` and a **Keys** modal on
  the Servers page showing `SHA256:` fingerprints, with **Forget all keys** for a server whose
  key legitimately changed. The mismatch error in the terminal points there.
- Accounts: `/api/v1/users` (list, create, delete — never yourself or the last account) and
  `PUT /api/v1/users/me/password`; a **Users** tab in Settings with a change-password form.
- `svrctl backup --out DIR [--keep N]`: a consistent `VACUUM INTO` copy of the database plus
  the key file (when the key is not in the environment), with retention; restore steps are in
  the deployment guide. `svrctl --token` is now sent as a Bearer header; the dead
  `status metrics` command is gone.
- Audit log: logins (successful, failed, rate-limited), logouts, interactive sessions,
  command-mode commands with exit codes, and changes to servers, credentials and accounts are
  recorded with the client address (`GET /api/v1/audit`, Settings → Audit); entries are kept
  for 180 days and never contain secrets.

### Security
- The HTML login and logout routes refuse cross-site requests (403), closing the last open
  item on the known-weaknesses list.

### Changed
- App identifier is `cc.jsnode.svrctlrs` on every platform. Flatpak packaging removed; the
  Android build is parked (the web app is the phone client for now).
- Health monitor: servers are probed concurrently instead of one 5-second timeout after
  another, and the result is recorded on the server (`last_seen_at`, `last_error`), so the
  Servers page reflects what the monitor knows; a server found down at the very first check
  is logged rather than silently ignored.
- Terminal tabs follow the WAI-ARIA pattern: arrow keys, Home and End move between tabs.
- Reconnect delays carry ±15 % jitter; the command palette's "Manage servers" navigates
  in-app instead of reloading the page.
- `LOG_FORMAT=json` switches the server to JSON log lines; the image builds both binaries in
  one cargo invocation.
- Icons from `@lucide/svelte` 1.x (`lucide-svelte` is deprecated upstream).
- SvelteKit 3, adapter-static 4 and TypeScript 6; `svelte.config.js` folded into
  `vite.config.ts`. Tauri JS packages stay at 2.10 to match the Rust crate.
- In-range dependency updates (Svelte 5.57, SvelteKit 2.70, Vite 8.3, Tailwind 4.3, Tauri JS
  2.12); unused Tauri JS plugin packages removed.
- `GET /api/v1/health` pings the database and answers 503 (`status: degraded`) when it is
  unreachable, so the container healthcheck reflects a broken volume.
- Accessibility: icon buttons carry labels, layout buttons report `aria-pressed`, toasts are an
  `aria-live` status region, and the command palette is a proper combobox/listbox.
- UI tooling: ESLint, Prettier and Vitest with the first unit tests; `engines.node >= 22.12`.
  The whole UI tree is Prettier-formatted (a formatting-only diff).

### Changed
- **Settings reworked.** Tabs live in the URL (`/settings?tab=audit`), are real `tablist`
  tabs with arrow-key navigation, and keep their labels on phones. Notifications: one *Save
  changes* request with dirty tracking, *Send test* tests the form as it is on screen, the
  token is write-only (never returned by the API; "stored" shown instead), URL validated
  inline, and a *Monitoring* section that says plainly that reachability is always probed.
  Audit: filter chips (logins / sessions / changes), a user filter, free-text search, relative
  times (absolute on hover), rows expand on phones to show address and details, and a real
  error state. Users: inline password errors, last login per account, in-app delete
  confirmation. Quick commands: a proper form (Enter submits), category suggestions, server
  scope as a select, labelled icon buttons. General: no more fake "Connected" line, server
  change asks first, default profile under *Terminal*.
- **Servers and Credentials pages reworked.** Server rows show the status dot, `user@host`,
  the credential in use (or a *no key* badge), last seen / last error, and the inline result
  of *Test*; a **Connect** button opens a terminal tab; icon buttons have labels and
  phone-sized targets; a filter box appears past six servers. Creating a remote server without
  a credential goes straight to *Authorize key*, which is now a shared dialog (also reachable
  from a key's public-key view, with a server picker) that can generate a key on the spot,
  names the `user@host` it will log in as, and only offers usable bootstrap credentials.
  Credential rows show the key's `SHA256:` fingerprint, whether it is pasted material or a file
  path, and which servers use it (Delete is disabled while in use). Forms submit with Enter,
  validate inline (the public-key/`.pub`/relative-path mistakes are flagged as you type),
  show a saving state, and the key form defaults to pasting. *Generate SSH key* is the primary
  action and the empty-state call to action. Deletes use an in-app confirmation that names
  what goes away instead of `window.confirm`.
- The credential **Username** is now meaningful: it is the default login name when the server
  has none (server → credential → `root`). *API Token* and *Certificate* are no longer offered
  in the form (SSH cannot use them; existing rows are untouched).

### Changed
- **Mobile pass.** The viewport uses `interactive-widget=resizes-content` and the shell is
  `100dvh`, so the terminal, the extra keys and every bottom sheet sit above the soft keyboard
  instead of behind it, and rotation or pinch-zoom no longer counts as "keyboard open".
  Tapping the terminal focuses it inside the gesture (no 380 ms wait) and re-opens the keyboard
  after Android Back closed it; switching tabs by swipe and closing the palette return focus to
  the terminal. The extra-keys row is two rows (CTL, ALT, Esc, Tab, arrows, palette; `^C ^D
  ^Z ^R`, Home/End, PgUp/PgDn, `| ~ / -`), with 36 px keys that fire on touch, repeat while
  held, vibrate, and never steal focus; CTL/ALT work with Esc, Tab and arrows (proper CSI
  modifiers), can be locked with a second tap, and arrows honour application cursor mode.
  Keys injected this way go through xterm, so the view snaps to the bottom like typing. Touch
  scrolling keeps sub-cell movement and scrolls full-screen programs (`less`, `man`) with
  arrow keys. Toasts sit under the status bar on phones instead of over the keys. The PWA's
  theme colour matches the surface and follows the theme; pull-to-refresh and pinch-zoom are
  off on the app shell; a padded maskable icon is provided. Pages below the terminal no longer
  overflow the screen by the header height. WebGL stops retrying after three context losses
  and, on phones, only the visible pane keeps a WebGL context. The status bar has 40 px
  targets: menu, server/tabs (opens a **tab sheet** to switch, close or start a terminal),
  terminal settings, and **+** which opens a **server picker** (a new tab is never left
  serverless). The palette offers *Terminal Settings*, uses the server picker for *New Tab*,
  closes tabs with confirmation, and hides desktop-only items on phones. The phone's back
  gesture closes the drawer, sheets, palette and settings instead of navigating away.
  Font size has −/+ buttons. JetBrains Mono is bundled (woff2), so the default font no longer
  falls back to Android's monospace.

### Fixed
- Reaching the open-terminal limit was a dead end on a phone: the message pointed nowhere and
  the only way to close a terminal is the tab list. Hitting the cap now opens that list, from
  the server picker or the sidebar, so a terminal can be closed to free a slot.
- Terminal output was garbled with no clean prompt on a phone after the bundled terminal font
  shipped. The WebGL renderer bakes a glyph atlas from whatever font is loaded when it starts,
  and the web font arrives later, so the atlas kept the fallback font's metrics while glyphs
  were drawn from the real one. The atlas is now rebuilt once the font settles and whenever the
  font preferences change.
- The login form submitted twice (form submit plus the button's click handler).
- Terminal tab dots showed green as soon as the WebSocket opened, before the SSH shell was up;
  a PTY tab now turns green on the shell's `connected` and red when the shell fails to start.
- In split layouts, clicking or tapping into a pane only moved keyboard focus; the toolbar,
  shortcuts and palette still acted on pane 1. Focusing a pane now makes its tab active in
  place, and selecting an already visible tab no longer rearranges the panes.
- Mobile: the *Reconnect* / *New Session* buttons after a shell ended ignored taps — the
  pane's touch interception swallowed them. Controls drawn over the terminal now receive
  touches. Tapping the prompt line moves the cursor there; long-pressing a word selects and
  copies it.
- Clearing a server's hostname, username, description or credential in *Edit* had no effect
  (the update only applied non-empty values); the *Local server* box showed when editing but
  was ignored. Clearing works now and the box only shows when creating.
- A credential's description/username could not be cleared, and a stored passphrase could not
  be removed; *Remove the stored passphrase* does that, and `has_passphrase` no longer reports
  a passphrase for keys whose metadata is empty.
- A terminal tab pointed at another server could try to resume the previous server's
  session; a saved session now belongs to the server it was opened on, and only idle tabs
  without a server are reused by *Connect*.
- Resuming a session no longer writes "Resuming session…"/"Reconnecting…" lines into the
  shell's buffer (which left the restored prompt stranded above them until a key was pressed);
  progress shows in a small banner over the pane, and after the replay the pane sends two
  window-size changes so prompts, editors and `top` repaint themselves.
- Overlapping connects (a tab shown again while a reconnect was already in flight) could leave
  a pane reporting "Connecting…" forever or bind input to a dead socket; connects are now
  serialized and events from a superseded socket are ignored. A connection refused before it
  opened now reports an error instead of staying "connecting".
- Mobile: the CTL/ALT keys in the extra-keys row now apply to the next character typed on the
  soft keyboard (so `CTL` then `c` sends Ctrl+C), and there is a `^C` key. Previously the
  modifiers only affected the row's own keys, so there was no way to interrupt `top`.
- Sidebar and Servers-page status dots were stale (orange/grey) because the reachability probe
  only ran when health *notifications* were enabled. The probe (one TCP connect per server,
  every `notification.health_check_interval_seconds`, 30 s–1 h, default 5 min) now always runs;
  the setting only gates the up/down notifications. **Test connection** now records its
  result too, so a green test shows as online immediately.
- Sidebar and Servers page showed every enabled server as green; the dot now reflects what the
  health monitor and connections recorded (online, unreachable with the reason, unknown).
- A terminal whose connection died while the tab was in the background looked connected but
  ignored input until a reload; pings are now watched for their pong, and returning to the tab
  checks the socket immediately.
- Saving an SSH key credential with the *public* key (`id_ed25519.pub`), a `.pub` path or a
  path from your own machine used to be accepted and then fail at connect time with
  "No such file or directory". The form and the API now refuse these and say which file to
  use; a connection that still hits a bad value explains it instead of echoing an OS error.
- The native apps' server-URL screen normalises what you type (defaults to https, strips
  trailing slashes), refuses plain `http://` on Android — release builds cannot use it — and
  warns about it on desktop.
- Form labels are associated with their controls throughout the Settings and login forms;
  `svelte-check` is at zero warnings.
- Desktop: the tray icon had no image and was invisible on most desktops, leaving its
  Show/Quit menu unreachable after the window was closed; it now uses the application icon,
  and when no tray can be created closing the window quits instead of hiding it.

### Added
- Server-crate tests for the authentication middleware: a session cookie, a Bearer token and a
  query token each identify the user from the stored session (never from client-supplied
  headers), expired and unknown tokens are refused, one-time WebSocket tickets redeem exactly
  once and only on upgrades, every exempt route passes through while its lookalikes stay
  protected, and an OPTIONS preflight bypasses auth. With the existing TOFU host-key and
  WebSocket-origin unit tests, the security-critical request paths are now covered.

### Removed
- The `ralph/` autonomous-loop tooling and the archived documentation trees (`docs-archived/`
  and `docs/history/`), none of which were part of the product; they remain in git history.
  The dead `SchedulerError` left over from the retired job scheduler is gone too.

## [3.0.0] - 2026-10-05

First release of SvrCtlRS as a terminal/SSH management tool. The 1.x/2.x line (a plugin-based
monitoring platform with an HTMX UI) was retired in March 2026; this version shares nothing
with it except the repository and the SQLite migration history.

### Security
- The server refuses to start when the credential encryption key cannot be initialised
  (invalid or empty `ENCRYPTION_KEY`, unreadable key file) instead of falling back to storing
  credentials in plaintext. The database layer likewise refuses to store or return a secret
  without a key.
- On start-up, any credential rows still stored in plaintext by an earlier run are encrypted
  in one transaction; the count is logged.
- WebSocket upgrades refuse a cross-site `Origin` (403): the origin must be the request's own
  host or one of `ALLOWED_ORIGINS`. Closes cross-site WebSocket hijacking of a logged-in
  browser session.
- Token authentication (`Authorization: Bearer`, `?token=`) now resolves the session through
  the store, so expired sessions are refused, and takes the user id from the session record.
  The client-supplied `X-User-Id` header and `user_id` query parameter are ignored.
- Request tracing logs a redacted URI; the WebSocket token no longer appears in logs.
- Host-key verification compares the presented key against every key stored for the server,
  so a different key *algorithm* is a mismatch rather than a first contact; both connectors
  refuse the connection on a database error instead of accepting the key.
- `POST /api/v1/servers/{id}/test` verifies the host key before authenticating and supports
  password credentials; the legacy unverified SSH module is gone.
- Terminal profiles and quick commands: `get`, `update` and `delete` are scoped to the calling
  user and answer 404 for another user's id; setting a default profile no longer clears other
  users' defaults.
- First tests in the `server` crate: auth middleware, origin rules, URI redaction, host-key
  decisions; database tests for per-user scoping.
- The web UI no longer stores the session id in `localStorage`; the HttpOnly cookie is its
  only credential. Only the desktop and Android apps keep a token, and no client sends a
  user id any more.
- Login attempts are rate limited: 10 failures per 15 minutes per client address and per
  username, answered with 429 and `Retry-After`. New passwords must be at least 12
  characters (this applies to `ADMIN_PASSWORD` on a first start). The session id is cycled
  on login.
- Credential metadata — the SSH key passphrase — is encrypted at rest like the value
  (migration 022 adds `metadata_encrypted`); legacy plaintext metadata is encrypted at
  start-up.
- SSH stack upgraded: russh 0.51 → 0.64, async-ssh2-tokio 0.8 → 0.13, clearing
  RUSTSEC-2026-0154 and RUSTSEC-2026-0153 (unbounded allocation reachable by a malicious SSH
  server). A server presenting a host *certificate* is now pinned by the certificate's key.

### Added
- Interactive PTY terminals over WebSocket (russh + xterm.js 6 with WebGL), tabs, four
  layouts, output search, session restore across navigation, reconnect with buffer restore.
- Command mode: run a single command on a server and capture output and exit code.
- Server profiles, credentials encrypted at rest (AES-256-GCM), SSH host-key trust on first use.
- Terminal profiles (saved layouts with per-pane servers) and a default profile loaded at start.
- Quick commands with categories, launched from the command palette (`Ctrl+K`).
- Settings UI with General, Notifications, Quick Commands and About tabs.
- Push notifications (rstify/Gotify/ntfy) for health checks and command completion.
- Mobile layout: drawer sidebar, status bar with tab dots, swipe navigation, extra-keys row,
  safe-area and on-screen-keyboard handling.
- Tauri 2 desktop app for Linux (`.deb`, `.rpm`, AppImage, Flatpak) with tray icon, and a Tauri
  Android app.
- `svrctl` CLI used as the container healthcheck; `/api/v1/health` reports version and the
  build revision.
- Forgejo Actions CI/CD on the dev-space runners: lint, audit, test and frontend checks on
  every branch; image publishing and auto-deploy from `master`; releases with APK and desktop
  packages from `v*` tags.

- SSH key passphrases: a passphrase field on SSH key credentials in the UI and API
  (`passphrase`), used by both the interactive and the command connector.
- `POST /api/v1/auth/ws-ticket`: one-time, 30-second tickets that native clients use to
  authenticate WebSocket upgrades instead of putting their session token in the URL. The
  `?token=` form is deprecated and will be removed after the next app release.

### Fixed
- "Paste key" credentials never worked: both connectors treated the stored key material as a
  file path. Key material is now used directly; paths still work.
- The Notifications and General settings forms opened blank because the settings store was
  never loaded; they now load it first.
- A terminal session that dropped with an error never auto-reconnected (the error status
  masked the "was connected" check); it now reconnects with the full backoff schedule.
### Changed
- Versions unified at 3.0.0 across the Cargo workspace, the UI and the Tauri apps.
- Container image pinned to Rust 1.98, built without OpenSSL, stamped with
  `org.opencontainers.image.revision`.
- Reference compose uses the Forgejo registry and the edge-auth standard middlewares.
- An empty `ALLOWED_ORIGINS` is treated as unset (it used to switch the session cookie to
  `SameSite=None`).

### Removed
- GitHub Actions workflows and the ghcr.io image; GitHub is no longer used.
- Legacy migration repair scripts and the empty root `package.json`.
- Job-era documentation moved to `docs-archived/job-era-2025-12/`.

### Known issues
See `docs/SECURITY.md`, "Known weaknesses" (remaining: no CSRF token on the HTML login
and logout forms; the deprecated `?token=` WebSocket parameter) and `docs/ROADMAP.md`.

## Earlier history

- `v2.1.0`–`v2.2.4` (2025-11): the retired monitoring platform (Dioxus, then HTMX + Askama).
- `v1.0.0` (2025-11-25): first stable release of that platform.
- `v0.1.0`/`v0.1.1` (2026-03): pre-release tags of the terminal rewrite; superseded by 3.0.0.
