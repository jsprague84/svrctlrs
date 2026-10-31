# User guide

## Signing in

Open the server URL and sign in with your SvrCtlRS username and password. Behind the dev-space
edge you first pass the Pocket ID passkey prompt; when that sign-in runs out (after 15 days
there) the app reloads and the prompt comes back. In the desktop app the first
screen asks for the server URL (`https://…`); the app checks it can reach the server, then
**Sign in with your browser** opens your browser, where you sign in as usual and approve the
app. The app stays signed in for 30 days (its session is in the system keyring) and signs
out on its own with Logout. In the app, Ctrl+W goes to the terminal (vim's window commands),
which a browser tab keeps for itself.

## Setting up a new server (the short way)

1. **Credentials → Generate SSH key** once. The private key is created and stored encrypted
   on the SvrCtlRS server; you never handle it. The public key is shown with a copy button and
   a *Authorize on a server* picker.
2. **Servers → Add Server** with hostname, port and user. A new remote server without a
   credential goes straight to step 3.
3. **Authorize key** (on the row of a server that has no key yet, in its **⋯** menu otherwise,
   or from the public-key dialog): pick the key — or
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
for example `cd /srv/app && git status`. It echoes in the terminal like anything you type.

**Keep shells in tmux.** With this on, every new shell on that server runs inside tmux there,
so it survives SvrCtlRS restarting (an update or a deploy ends every other shell) and a dropped
connection to the server: the pane reconnects and you are back in the same shell, programs
still running. Scrolling back then happens in tmux (its copy mode, or the mouse and a drag on
a phone if your `.tmux.conf` has `set -g mouse on`), not in the pane. A server without tmux
gets a plain shell and a note. Do not also start tmux from the startup command: that would put tmux inside tmux.

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
**Trust and connect** (or **Trust key** on the row, or **Trust** under **Host keys**). The file
browser refuses with the same fingerprint until then. If the server later presents a key that
matches none trusted for it, the connection is refused and the terminal shows a mismatch
error. **Host keys** in a server's **⋯** menu lists the recorded keys, trusted or unconfirmed, with
their fingerprints and, once you have confirmed out of band that the key really changed,
**Forget all keys** makes the next connection ask for confirmation again.

### Can't reach a server?

**Diagnose** on the server's row (in the **⋯** menu on a phone) makes the connection a shell
would, one step at a time, from where SvrCtlRS runs: the name lookup, the TCP connection (or,
behind a jump host, the jump host's tunnel), the SSH handshake, the host key and the login,
for each jump host and then the server. It says in one sentence what is wrong and gives a fix
for the step that failed, for example *nothing listens on port 22: is sshd running*, *a
firewall drops the connection* (SSH comes from the machine SvrCtlRS runs on, not from your
phone), *the key is not in authorized_keys: Authorize key*, or the fingerprint to compare on
first contact. It runs from the server's side, so it also answers "it works from my laptop
but not from here".

**Rotate key** on a server that logs in with a stored key replaces that key: SvrCtlRS
generates a new one, installs it with the current key, checks that it logs in, switches the
server to it and removes the old public key from the server's `authorized_keys`. The old
credential stays under Credentials; delete it once no server uses it.

Agents get the same operations through the API, plus **exec** (one command, output and exit
status back, audited; full-admin tokens only) and **onboard** (add a server, confirm its key with the fingerprint you
compared, switch it to a generated key); see API.md, "Operations".

## Audit log

Settings → Audit (`/settings?tab=audit`) shows who did what, filtered by kind (logins,
sessions, changes), user and free text; on a phone, tap a row for the address and details.
It shows: logins (including failed and rate-limited attempts,
with the client address), logouts, shells opened (with the login, whether a password or
which key was used, and whether the shell was recorded), recording downloads and deletions,
file transfers, and every change to servers, credentials, accounts and tokens. Anything done
with an API token shows "via <token name>" under the user. Entries are kept for 180 days.

## Sign-in

Settings → Access → **Sign-in** shows how you can sign in:

- **Pocket ID** (or whatever the server's provider is called), when the server has one:
  **Link** sends you to the provider and back, and from then on **Sign in with …** on the
  login page signs you in without a password. Several identities can be linked; unlinking
  one stops it working. When the server has password sign-in turned off, the login page goes
  straight to the provider, and your last linked identity cannot be unlinked.
- **Password**: whether this server takes one at all.
- **Two-factor (authenticator app)**: **Turn on** asks for your password, shows a QR code
  (on a phone, a link opens the authenticator app directly) and wants one code from the app.
  You then get ten recovery codes, once: download or copy them somewhere safe. From then on a
  password sign-in asks for a code on a second page; a recovery code works there too, once.
  **New recovery codes** replaces them; **Turn off** asks for the password. Lost the phone and
  the codes? Whoever runs the server: `svrctl reset-password <you> --clear-totp`.

None of this changes what agent tokens can do: they never sign in.

**Devices**, under Sign-in, lists where your account is signed in: each browser and the
desktop app, named after it ("Firefox on Android", "Desktop app on Linux"), with when it was
last used, its address, and when and how it signed in. This device is first. **Sign out**
(the arrow on a row) ends that sign-in at once — the way to deal with a lost phone, without
changing your password. Its open terminals close; your shells keep running and wait for you
on your other devices. **Sign out other devices** ends all of them but this one.

## Agent tokens

Settings → Access (`/settings?tab=access`) → **Agent tokens** is where agents and scripts get
their own way in: `svrctl`, your always-on agent, Claude Code, a cron job. Each token is
named, acts as you within its access, and can be revoked on its own, so give each agent its
own.

- **New token**: a name (which agent holds it), an access preset and a lifetime.
  - *Full admin*: everything you can do, terminals, files, **exec** and "Back up now"
    included.
  - *Servers & keys*: read-only, plus add, change and delete servers, credentials and host
    keys, and the operations that log in with the stored credentials without a shell: test,
    diagnose, onboard, rotate key, install a key. No terminals, files or exec. The right one
    for an agent that fixes your server list.
  - *Read-only*: look at servers, host keys, credentials (never their secrets), settings, the
    audit log, sessions, recording details and backups; never touch, and no file or recording
    contents.

  No token can create or revoke tokens or manage accounts, whatever its access. Lifetime is
  30 days, 90 days, a year or never. **Ask me before it deletes** (on by default) holds the
  three calls that cannot be undone — deleting a server, deleting a credential, forgetting a
  server's host keys — until you approve them (below); everything else runs at once.
- The token (`svr_…`) is shown **once**, with a Copy button; SvrCtlRS keeps only a hash. Use
  it as `Authorization: Bearer svr_…`, or `export SVRCTLRS_TOKEN=svr_…` for `svrctl`.
- The list shows each token's access, its first characters, when it was last used (hover for
  the address on a desktop) and when it expires. **Revoke** takes effect at once (a terminal an
  agent opened with it closes); a revoked or expired token stays listed until you **Remove** it.

A full-admin token that never expires is as good as your password on every server for as long
as it exists; the dialog says so. See SECURITY.md, "API tokens".

Agents use the token with **svrctl** (`export SVRCTLRS_URL=… SVRCTLRS_TOKEN=svr_…`;
`svrctl --help` lists everything, with examples, and `docs/cli.md` says which access each
command needs; *servers & keys* covers all of it except `svrctl exec`, `svrctl settings set`
and `svrctl backups --now`, which need *full admin*) and the skill file `skills/svrctlrs/SKILL.md`,
which teaches them the diagnose-and-fix loop and the rules (never a password on a command
line, never trust a host key without comparing it, approvals are yours).

### Approvals

When a token that asks first wants to delete something, SvrCtlRS sends you a link (Settings →
Notifications → **Approval requests**: as a push notification, to a Matrix room, to a webhook
such as an n8n workflow, or nowhere — the agent gets the link in its answer either way and can
pass it on). The link opens **Approvals** (`/approvals`), signed in, on the phone or the
desktop: what is asked ("Delete server web-01"), by which token, from where, and **Approve** or
**Deny**. Approving makes the call there and then, and shows what came back; the agent, which
can wait for your answer, sees the same. A request expires after an hour. Settings → Access
shows how many are waiting. Nobody but you can approve: a token cannot, whatever its access.

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
closed after 15 minutes by default; the **Sessions** panel lets you keep them for 1 or 8 hours
instead (your account's choice, applied to shells already waiting too; the server may cap it).
Open tabs are remembered for the life of the browser tab, so a reload brings them back with
their shells. A restart of SvrCtlRS ends every shell, except on servers set to **Keep shells
in tmux**, whose panes reconnect to the same shell.

- **New tab**: `Ctrl+Shift+T` or the `+` in the tab bar, then pick a server. Up to 10 tabs. A
  tab without a server lists your servers (and any shells waiting to be resumed) in the pane.
- **Close tabs**: the `×` on a tab, a middle click, or `Ctrl+Shift+W`. The `⋯` next to the
  layout buttons (shown with two or more tabs) closes the other tabs, those to the right of
  the active one, or all of them; the palette has *Close Other Tabs* and *Close All Tabs*, and
  the phone's tab list *Close all*. Closing several asks once, naming how many live shells end;
  tabs without a live shell close without asking.
- **Layouts**: `Ctrl+\` cycles single → split horizontal → split vertical → quad. Tabs fill the
  visible slots; `Alt+1`–`Alt+4` focuses a slot, `Alt+[`/`Alt+]` cycles panes, `Alt+Arrow`
  moves spatially.
- **Search output**: `Ctrl+Shift+F`.
- **Reconnect**: if the connection drops, the pane retries with backoff and resumes the same
  shell, replaying the output you missed. If the shell has ended meanwhile (exit, keep time
  run out, server restart) the pane says so and offers **Reconnect** (new shell) or
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
directory. Tap a folder to enter it, the breadcrumb or the up arrow to go back. The toolbar
makes a folder or uploads files into the current one (on the desktop you can also drop files
from your file manager onto the panel); each row can be downloaded, renamed or deleted (a
folder only when empty — use the shell for anything recursive). The browser follows the
active tab, and remembers where you were on each server.

**Tap a file to open it**, not to download it. The viewer shows text files up to 1 MB and
images (PNG, JPEG, GIF, WebP, AVIF, BMP, ICO, SVG) up to 10 MB; anything else, or anything
larger, offers its buttons only:

- **Edit** (text files up to 256 KB, UTF-8, one kind of line ending): a plain editor that
  suits a phone. Save writes the file back with its own line endings (CRLF stays CRLF) and
  byte-order mark, in place, so its owner and permissions do not change. If the file changed
  on the server since you opened it, nothing is saved: your text stays in the editor so you
  can copy it, and **Reload** shows the file as it is now. A file you may not write is
  refused when you save.
- **Edit in terminal** opens a new tab on that server with the file in your own editor:
  `$VISUAL`, else `$EDITOR`, else `vi`, from your shell's settings, so vim keeps your config.
  Quitting the editor leaves you at the prompt. Use this for large files, other encodings,
  or anything that needs `sudoedit`.
- **Download** saves the file.

Downloads, views, edits, uploads, renames and deletions are recorded in the audit log.

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

## Backups

The server copies its database every night, at 03:00 UTC unless you change it, and keeps the
newest 14 copies on its data volume (Settings → General → *Backups*; 0 copies turns nightly
backups off). Settings → **About** shows the last copy (when, how large), the last error if
the latest attempt failed, the schedule, and **Back up now**. If the server was down at that
hour, it makes the copy a few minutes after it starts; with notifications on, a failed night
sends one notification.

A copy holds everything except the sign-ins and the encryption key. Keep the key yourself:
it is `ENCRYPTION_KEY` in the server's `.env` (About says so), and without it the
credentials in a copy cannot be read. Restoring happens on the server's host, not in the app
(`svrctl restore`; DEPLOYMENT.md, "Backups"); afterwards everyone signs in again.

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

A new browser or device starts in the system's theme (Tokyo Night dark, or a light theme)
and follows it when it changes. The sidebar's toggle picks one for good on that device. The
terminal colours follow, and so does the sign-in page (always the system's theme).

## Mobile (Android app or a phone browser)

- The sidebar becomes a drawer (menu button); the status bar shows the connection dot, the
  server name and one dot per tab — tap it for the **tab list** (switch, close one or all, new terminal),
  the **+** for a server picker, the gear for terminal settings (font size has −/+ buttons).
  The phone's back gesture closes drawers, sheets and the palette instead of leaving the app.
- **Swipe left/right** on the terminal to move between tabs; a one-time hint explains this
  (the keyboard comes along only if it was up). **Tap** to type (the keyboard comes up) and,
  on the prompt line, to move the cursor there (arrow keys are sent); **long-press** a word to
  select and copy it, **double-tap** for the command palette.
- **Drag up/down** to read: a shell scrolls its scrollback; `less` and `man` get arrow keys. A
  drag closes the keyboard, so the whole screen is there to read.
- **In a program that reads the mouse** (herdr, tmux with `mouse on`, vim with `mouse=a`, htop,
  mc) your finger is the mouse: a drag sends wheel notches and scrolls the pane under it, and a
  tap is a click there (herdr's tabs, workspaces, machines and panes; tmux's panes and windows)
  and brings up no keyboard. Use the **keyboard key** in the extra-keys row to type. If a
  program crashed and left the mouse on, taps print `^[[<0;…` at the prompt: run `reset`.
- The **extra-keys row** above the keyboard has two rows: CTL, ALT, Esc, Tab, arrows, Paste,
  the keyboard key (shows or hides the soft keyboard) and the palette key, then `^C ^D ^Z ^R`,
  Home/End, PgUp/PgDn and `| ~ / -` (the chevron hides the second row). Keys fire on touch and repeat while held. **CTL** and **ALT** are sticky for one
  key — tap CTL then type `d` to send Ctrl+D, or CTL then an arrow — and tapping a modifier
  twice locks it until tapped again.
- The **Paste** key (clipboard icon, first row) types the clipboard into the shell the way a
  paste on a desktop does: line breaks become Enter, and when the shell asks for bracketed
  paste (bash, zsh and fish do) a multi-line paste waits for your Enter instead of running
  line by line. The browser decides whether the page may read the clipboard: it works only
  over HTTPS (or on localhost), Chrome asks once per site, and Safari and Firefox show a small
  **Paste** button to tap. If the browser refuses (permission denied, the prompt dismissed, or
  the clipboard holds no text) a message says so and nothing is typed; allow clipboard access
  in the site's settings, or use the keyboard's own paste.
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
