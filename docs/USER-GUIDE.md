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
4. **Connect** on the row. The host key is trusted on first contact.

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
username*, else `root`. A **local** server opens a shell inside the SvrCtlRS container itself.
Clearing a field in *Edit* (hostname, username, description, credential → *None*) really
clears it; a credential that is still assigned to a server cannot be deleted — the row says
which servers use it.

The first connection to a server stores its SSH host key. If the server later presents a key
that matches none stored for it, the connection is refused and the terminal shows a mismatch
error. The **Keys** button on a server lists the stored keys with their `SHA256:` fingerprints
(compare with `ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub` on the server) and, once you have
confirmed out of band that the key really changed, **Forget all keys** lets the next connection
trust it afresh.

## Audit log

Settings → Audit (`/settings?tab=audit`) shows who did what, filtered by kind (logins,
sessions, changes), user and free text; on a phone, tap a row for the address and details.
It shows: logins (including failed and rate-limited attempts,
with the client address), logouts, interactive sessions opened, commands run in command mode
with their exit code, and every change to servers, credentials and accounts. Entries are kept
for 180 days. Commands are recorded as typed, so keep secrets out of command lines.

## Accounts

Settings → Users lists the accounts, lets you add one (username plus a password of at least
12 characters) or delete one (not your own, not the last), and holds **Your password** for
changing yours — the current password is required. Every account can open a shell on every
configured server and manage accounts; there are no roles yet.

## The terminal

Shells run in the server, not in your browser tab. Switching to Servers or Settings and back,
reloading the page, losing Wi-Fi for a minute or putting the phone to sleep does not end them:
the pane reattaches and the server resends what you missed. A shell with nobody attached is
closed after 15 minutes by default (`SESSION_DETACH_TTL_SECS`); open tabs are remembered for
the life of the browser tab, so a reload brings them back with their shells.

- **New tab**: `Ctrl+Shift+T` or the `+` in the tab bar, then pick a server. Up to 10 tabs.
- **Modes**: *PTY* (default) is an interactive shell. *Command* runs one command and shows its
  output and exit code — useful for scripted checks.
- **Layouts**: `Ctrl+\` cycles single → split horizontal → split vertical → quad. Tabs fill the
  visible slots; `Alt+1`–`Alt+4` focuses a slot, `Alt+[`/`Alt+]` cycles panes, `Alt+Arrow`
  moves spatially.
- **Search output**: `Ctrl+Shift+F`.
- **Reconnect**: if the connection drops, the pane retries with backoff and resumes the same
  shell, replaying the output you missed. If the shell has ended meanwhile (exit, 15-minute
  grace period, server restart) the pane says so and offers **Reconnect** (new shell) or
  **New Session**. **Disconnect** and closing a tab end the shell for good.
- **Preferences** (gear icon): font, size, weight, line height, cursor style and blink,
  scrollback, scroll speed, tab width, contrast. Stored in the browser/app.
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

## Command palette and quick commands

`Ctrl+K` opens the palette: switch tabs, open servers, change layout, and run **quick
commands** — your saved snippets (Settings → Quick Commands), grouped by category and
optionally tied to one server. Running one types it into the focused pane.

## Terminal profiles

A profile saves a layout and which server (and mode) opens in each pane. Create one from the
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
- The **extra keys row** above the keyboard provides `Esc`, `Tab`, `Ctrl`, `Alt`, arrows and
  `|`. Hold `Ctrl`/`Alt` then tap a letter.
- Preferences open as a bottom sheet. Only one pane is shown at a time.

## Desktop app

Closing the window hides the app to the system tray; use the tray menu to show or quit. The
server URL can be changed in Settings → General.
