# User guide

## Signing in

Open the server URL and sign in with your SvrCtlRS username and password. Behind the dev-space
edge you first pass the Pocket ID passkey prompt. On the desktop or Android app the first
screen asks for the server URL (`https://…`); the app checks it can reach the server, then
shows the same login.

## Setting up a new server (the short way)

1. **Credentials → Generate SSH key** once. The private key is created and stored encrypted
   on the SvrCtlRS server; you never handle it. The public key is shown with a copy button and
   a *Authorize on a server* picker.
2. **Servers → Add Server** with hostname, port and user. A new remote server without a
   credential goes straight to step 3.
3. **Authorize key** (on the server row, or from the public-key dialog): pick the key — or
   generate one right there — and log in *once* with the account's password (used immediately,
   never stored) or a credential that already works. SvrCtlRS appends the public key to
   `~/.ssh/authorized_keys` — what `ssh-copy-id` does — verifies that the key logs in, and
   switches the server to it.
4. **Connect** on the row. On first contact the terminal shows the server's host-key
   fingerprint; compare it with the host's own and choose **Trust and connect**.

Repeat step 3 for every further server; one app key can open all of them, and deleting the
credential (then removing the line from `authorized_keys`) revokes it. If you prefer to
install keys yourself, **Public key** on any key credential shows the `authorized_keys` line.

## Servers and credentials

**Credentials** (sidebar → Credentials) hold how to authenticate: a password, or an SSH key
given either as pasted key material (recommended — it is stored encrypted and never touches
the server's filesystem) or as a path readable by the server (for example
`/home/svrctlrs/.ssh/id_ed25519` when a key directory is mounted). Paste the **private** key
— the file *without* `.pub`, which starts with `-----BEGIN OPENSSH PRIVATE KEY-----`; the
`.pub` file is what you put on the *servers*, and SvrCtlRS refuses it with an explanation. A
path means a path on the SvrCtlRS server (inside its container), never on your own machine. An encrypted key takes its
passphrase in the same form. Values and passphrases are encrypted at rest and never shown
again; the list only shows that they exist.

**Servers** (sidebar → Servers) are the hosts you connect to: name, hostname, port, username
and the credential to use. Each row shows the status dot, `user@host`, the credential in use
(or a *no key* badge), and when the host was last seen or why it was unreachable. *Test*
opens an SSH session and records the result in the row and in the status; *Connect* opens a
terminal tab. The username used for SSH is the server's, else the credential's *default
username*, else `root`. Password and SSH-key credentials both work for the shell.
Clearing a field in *Edit* (hostname, username, description, credential → *None*) really
clears it; a credential that is still assigned to a server cannot be deleted — the row says
which servers use it.

**Startup command.** One line typed into every new shell on that server right after it opens,
for example `tmux new -A -s main` or `cd /srv/app && git status`. With tmux or screen there
your work survives even the app's 15-minute grace period for detached shells: the next shell
reattaches to the same tmux session. It echoes in the terminal like anything you type.

**Tags and filtering.** Give a server tags (*prod*, *home*, *db*; comma separated in the
form). Every list — the Servers page, the sidebar, the phone's server picker — shows the tags
in use as chips and filters on one of them; the palette's *Connect* entries and the Servers
page search match tags too. A tag on a row is clickable.

**Import from `~/.ssh/config`.** *Import* on the Servers page takes a pasted `ssh_config`:
each concrete `Host` becomes a server with its `HostName`, `Port` and `User`, optionally with a
credential and tags for all of them; a `ProxyJump` whose first hop is another imported or
existing server becomes that server's jump host. Wildcard hosts, `Match` blocks and names
that already exist are listed as skipped. Nothing is connected to during the import; the host
key is confirmed on each server's first connection as usual.

**Jump hosts.** *Connect through* on a server picks another server to hop through, like
`ProxyJump`: SvrCtlRS opens its SSH connection to the jump host and tunnels the target's
connection inside it, so a private address that only the jump host can reach works as the
hostname. Both servers need a working credential, both host keys are confirmed and checked
(the fingerprint dialog says whose key it shows), and chains of jump hosts work up to four
hops. The row shows *via name*. Servers behind a jump host are not probed by the health check;
their status comes from connections and *Test*. Deleting a jump host makes its servers direct
again.

**Keepalives.** Every SSH connection sends a keepalive after 15 s of silence
(`SSH_KEEPALIVE_SECS`; three unanswered close it), so an idle shell behind NAT or on a phone
network is not dropped by the network, and a dead link is noticed within a minute.

The first connection to a server does not go through until you have confirmed its SSH host
key: the terminal (or a failed *Test*) shows the key type and `SHA256:` fingerprint, you compare
it with `ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub` run on the server itself, and choose
**Trust and connect** (or **Trust key** on the row, or **Trust** under **Keys**). The file
browser refuses with the same fingerprint until then. If the server later presents a key that
matches none trusted for it, the connection is refused and the terminal shows a mismatch
error. The **Keys** button on a server lists the recorded keys, trusted or unconfirmed, with
their fingerprints and, once you have confirmed out of band that the key really changed,
**Forget all keys** makes the next connection ask for confirmation again.

## Audit log

Settings → Audit (`/settings?tab=audit`) shows who did what, filtered by kind (logins,
sessions, changes), user and free text; on a phone, tap a row for the address and details.
It shows: logins (including failed and rate-limited attempts,
with the client address), logouts, shells opened (with the login, whether a password or
which key was used, and whether the shell was recorded), recording downloads and deletions,
file transfers, and every change to servers, credentials, accounts and tokens. Anything done
with an API token shows "via <token name>" under the user. Entries are kept for 180 days.

## Agent tokens

Settings → Access (`/settings?tab=access`) → **Agent tokens** is where agents and scripts get
their own way in: `svrctl`, your always-on agent, Claude Code, a cron job. Each token is
named, acts as you within its access, and can be revoked on its own, so give each agent its
own.

- **New token**: a name (which agent holds it), an access preset and a lifetime.
  - *Full admin*: everything you can do, terminals and files included.
  - *Servers & keys*: read everything; add, change and delete servers, credentials and host
    keys. No terminals or files. The right one for an agent that fixes your server list.
  - *Read-only*: look, never touch.

  No token can create or revoke tokens or manage accounts, whatever its access. Lifetime is
  30 days, 90 days, a year or never.
- The token (`svr_…`) is shown **once**, with a Copy button; SvrCtlRS keeps only a hash. Use
  it as `Authorization: Bearer svr_…`, or `export SVRCTLRS_TOKEN=svr_…` for `svrctl`.
- The list shows each token's access, its first characters, when it was last used (hover for
  the address on a desktop) and when it expires. **Revoke** takes effect at once (a terminal an
  agent opened with it closes); a revoked or expired token stays listed until you **Remove** it.

A full-admin token that never expires is as good as your password on every server for as long
as it exists; the dialog says so. See SECURITY.md, "API tokens".

## Accounts

Settings → Access lists the accounts under **Accounts**, lets you add one (username plus a password of at least
12 characters) or delete one (not your own, not the last), and holds **Your password** for
changing yours — the current password is required. Every account can open a shell on every
configured server and manage accounts; there are no roles yet. Locked out? Whoever runs the
server can set a new password with `svrctl reset-password` (DEPLOYMENT.md, "Locked out").

## The terminal

![A shell with the file browser open beside it](images/desktop-terminal-files.png)

Shells run in the server, not in your browser tab. Switching to Servers or Settings and back,
reloading the page, losing Wi-Fi for a minute or putting the phone to sleep does not end them:
the pane reattaches and the server resends what you missed. A shell with nobody attached is
closed after 15 minutes by default (`SESSION_DETACH_TTL_SECS`); open tabs are remembered for
the life of the browser tab, so a reload brings them back with their shells.

- **New tab**: `Ctrl+Shift+T` or the `+` in the tab bar, then pick a server. Up to 10 tabs. A
  tab without a server lists your servers (and any shells waiting to be resumed) in the pane.
- **Layouts**: `Ctrl+\` cycles single → split horizontal → split vertical → quad. Tabs fill the
  visible slots; `Alt+1`–`Alt+4` focuses a slot, `Alt+[`/`Alt+]` cycles panes, `Alt+Arrow`
  moves spatially.
- **Search output**: `Ctrl+Shift+F`.
- **Reconnect**: if the connection drops, the pane retries with backoff and resumes the same
  shell, replaying the output you missed. If the shell has ended meanwhile (exit, 15-minute
  grace period, server restart) the pane says so and offers **Reconnect** (new shell) or
  **New Session**. **Disconnect** and closing a tab end the shell for good.
- **Preferences** (gear icon): font, size, weight, line height, cursor style and blink,
  scrollback, scroll speed, tab width, contrast, predictive echo. They belong to your account,
  so the phone and the laptop agree: a change is saved within a second and another device
  picks it up the next time you switch to it. The first time you sign in after this arrived,
  the preferences this browser already had become your account's.
- **Predictive echo**: on a slow or lossy link (a phone on mobile data) what you type appears
  at once, dimmed and with a dotted underline, and turns into normal text when the server's
  echo arrives. A line's first character waits for its echo; the rest show immediately. If the
  shell draws something else, the guess disappears. It never shows anything at a password
  prompt (or any prompt that does not echo) or in full-screen programs like `vim`, `less` and
  `htop`. Preferences → Behavior → Predictive Echo: **On slow links** (default: only once the
  echo takes longer than about 30 ms), **Always**, or **Off**.
- **Title and bell**: a shell that sets its window title (most prompts do: `user@host: ~`)
  renames its tab; hovering shows the server name. A bell (`BEL`, for example from a finished
  `make` or an alert in `top`) flashes the pane's border, vibrates on a phone, and leaves an
  amber ring on an inactive tab's dot until you switch to it.
- **Colour and text**: emoji sequences and combining characters take their proper width. The
  connection asks the server for `COLORTERM=truecolor`; most `sshd` configurations only pass
  `LANG`/`LC_*` through, so if a tool still shows 256 colours put `export COLORTERM=truecolor`
  in the server's startup command. Connections negotiate compression (`zlib@openssh.com`) when
  the server offers it, which helps on slow links.
- **Download**: a pane's visible text can be saved as a file.

### Keyboard shortcuts

Browsers keep `Ctrl+Tab`, `Ctrl+Shift+T` and `Ctrl+Shift+W` for themselves, so the terminal has
a **leader key**, tmux style: press `Ctrl+Space` (change it under Preferences → Behavior), then
one key. A small panel lists the bindings while the leader is armed; `?` keeps it open.
Pressing the leader twice sends the chord to the shell.

| After the leader | Action |
|---|---|
| `c` / `x` | New / close tab |
| `n` / `p` (or `Tab` / `Shift+Tab`) | Next / previous tab |
| `1`–`9` | Tab by number |
| `` ` `` | Last tab |
| `h` `j` `k` `l` or arrows | Focus the pane in that direction |
| `[` / `]` | Previous / next visible pane |
| `s` | Cycle layouts |
| `/` | Search output |
| `:` | Command palette |
| `Esc` | Cancel |

Direct shortcuts still work where the browser or desktop app lets them through:

| Keys | Action |
|---|---|
| `Ctrl+K` | Command palette |
| `Ctrl+Shift+T` / `Ctrl+Shift+W` | New / close tab |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Alt+1`–`4` | Focus pane by slot |
| `Alt+[` / `Alt+]` | Cycle panes |
| `Alt+Arrow` | Move focus spatially |
| `Ctrl+\` | Cycle layouts |
| `Ctrl+Shift+F` | Search output |

The same table is in Settings → About.

## Files

The folder button in the toolbar (phone: *Files on this server* in the tab sheet; palette:
*Browse Files*) opens the active tab's server over SFTP, starting in the login's home
directory. Tap a folder to enter it, the breadcrumb or the up arrow to go back, a file to
download it. The toolbar makes a folder or uploads files into the current one; each row can be
renamed or deleted (a folder only when empty — use the shell for anything recursive). The
browser follows the active tab, and remembers where you were on each server. Downloads,
uploads, renames and deletions are recorded in the audit log.

## Session recordings

Recording is off until you switch on **Record sessions** for a server (Servers → Edit; the
server's row then shows a red *rec* badge). From then on every shell opened on that server is
recorded for the person who opens it. A recorded shell says so when it starts, its pane shows
a red **REC** mark, and the Sessions list marks it too. Shells already open are not affected.

What is recorded is what the terminal shows: output, including the echo of the commands you
type, and the terminal's resizes. Keystrokes themselves are not, so a password typed at a
prompt that does not echo it (`sudo`, `ssh`, `read -s`) is not in the recording; anything a
command prints (`cat .env`) is.

![The recording player](images/desktop-recording-player.png)

**Recordings** in the sidebar lists your own recordings, newest first, with a server filter
(a server row's ⋯ menu or *rec* badge opens it filtered). Nobody else can see them, whatever
their account can otherwise do. **Play** opens the player: Space plays and pauses, ←/→ jump
five seconds, the slider seeks, **Skip pauses** shortens any pause over two seconds (on by
default), and the speed menu goes from half to eight times. A recording still in progress
plays as far as it has got. **Download** saves it as an asciicast `.cast` file, which
`asciinema play` and other asciicast players open; **Delete** removes it (a recording still
in progress cannot be deleted until its shell ends).

Recordings are kept for 30 days by default (Settings → General → *Keep recordings for*; 0
keeps them until deleted) and stop at 100 MB each (`RECORDING_MAX_MB`; the shell carries on
unrecorded). They are stored encrypted with the server's encryption key. Downloads and
deletions appear in the audit log, and the log's *Sessions* filter includes them.

## Command palette and quick commands

`Ctrl+K` opens the palette: switch tabs, open servers, change layout, and run **quick
commands** — your saved snippets (Settings → Quick Commands), grouped by category. A command
saved for one server only appears while a tab on that server is active. Running one types it
into the focused pane followed by Enter, exactly as if you had typed it.

**Variables** let one snippet serve every server. `{{host}}`, `{{user}}`, `{{port}}` and
`{{name}}` are filled from the active tab's server. Any other `{{word}}` is asked for when you
run the command, in a small dialog that shows the finished command as you type; write
`{{word:default}}` to pre-fill the answer. For example `journalctl -u {{unit}} -n {{lines:200}}`
asks for the unit and offers 200 lines. Answers go in exactly as typed (not quoted), so a glob
or a pipe works. Go templates are left alone: `docker ps --format '{{.Names}}'`,
`{{json .}}`, `{{range …}}…{{end}}` and `{{else}}` are not variables.

## Terminal profiles

A profile saves a layout and which server opens in each pane. Create one from the
current layout with the profile menu in the sidebar; mark one as **default** (Settings →
General) to open it automatically when the app starts.

## Notifications

Settings → Notifications: enable push, choose rstify/Gotify or ntfy, set URL and token (and
topic for ntfy). *Send test* tries what is on screen, saved or not; *Save changes* writes
everything in one go. The token is write-only — once stored it is never shown again; leave
the field empty to keep it. Under **Monitoring**, reachability is always probed (that is what
the status dots show, every 1–10 minutes); the toggle only decides whether a server going
down or coming back sends a push notification.

## Themes

Tokyo Night dark is the default; switch to light in the sidebar. The terminal colours follow.

## Mobile (Android app or a phone browser)

- The sidebar becomes a drawer (menu button); the status bar shows the connection dot, the
  server name and one dot per tab — tap it for the **tab list** (switch, close, new terminal),
  the **+** for a server picker, the gear for terminal settings (font size has −/+ buttons).
  The phone's back gesture closes drawers, sheets and the palette instead of leaving the app.
- **Swipe left/right** on the terminal to move between tabs; a one-time hint explains this.
  **Tap** the prompt line to move the cursor there (arrow keys are sent), **long-press** a word
  to select and copy it, **double-tap** for the command palette.
- The **extra-keys row** above the keyboard has two rows: CTL, ALT, Esc, Tab, arrows and the
  palette key, then `^C ^D ^Z ^R`, Home/End, PgUp/PgDn and `| ~ / -` (the chevron hides the
  second row). Keys fire on touch and repeat while held. **CTL** and **ALT** are sticky for one
  key — tap CTL then type `d` to send Ctrl+D, or CTL then an arrow — and tapping a modifier
  twice locks it until tapped again.
- **Install it**: the web app is installable — "Add to Home screen" (Android Chrome, iOS Safari
  share sheet) gives a standalone window without browser chrome, its own icon and the full
  screen for the terminal. Shells keep running while the app is in the background and resume
  when you come back (see "The terminal").
- **Home-screen shortcuts**: long-press the installed app's icon (Android, or the app's menu on
  desktop Chrome/Edge) for **New terminal** and the three servers you opened shells on most
  recently; one tap opens a shell there, or shows the tab you already have on it. The list
  follows your use; Android refreshes it when the app's manifest is next checked (about once a
  day). A link to `/?connect=<server id>` does the same from anywhere.
- On the **Servers** page each row has **Connect** and a **⋯** menu with Test connection,
  Authorize key (marked with a dot when the server has no key yet), Host keys, Edit and Delete.
- Preferences open as a bottom sheet. Only one pane is shown at a time.

![A shell on a phone, with the status bar and the extra-keys row](images/phone-terminal.png)

## Desktop app

Closing the window hides the app to the system tray; use the tray menu to show or quit. The
server URL can be changed in Settings → General.
