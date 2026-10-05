# User guide

## Signing in

Open the server URL and sign in with your SvrCtlRS username and password. Behind the dev-space
edge you first pass the Pocket ID passkey prompt. On the desktop or Android app the first
screen asks for the server URL (`https://…`); the app checks it can reach the server, then
shows the same login.

## Servers and credentials

**Credentials** (sidebar → Credentials) hold how to authenticate: a password, or an SSH key
given either as a path readable by the server (for example `/home/svrctlrs/.ssh/id_ed25519`
when a key directory is mounted) or pasted key material, optionally with a passphrase. Values
are encrypted at rest and never shown again.

**Servers** (sidebar → Servers) are the hosts you connect to: name, hostname, port, username
and the credential to use. *Test connection* opens an SSH session and records the result.
A **local** server opens a shell inside the SvrCtlRS container itself.

The first connection to a server stores its SSH host key. If the key ever changes, the
connection is refused and the terminal shows a mismatch error; confirm the change out of band
before an administrator clears the stored key.

## The terminal

The terminal page is always mounted, so switching to Servers or Settings and back keeps your
sessions running.

- **New tab**: `Ctrl+Shift+T` or the `+` in the tab bar, then pick a server. Up to 10 tabs.
- **Modes**: *PTY* (default) is an interactive shell. *Command* runs one command and shows its
  output and exit code — useful for scripted checks.
- **Layouts**: `Ctrl+\` cycles single → split horizontal → split vertical → quad. Tabs fill the
  visible slots; `Alt+1`–`Alt+4` focuses a slot, `Alt+[`/`Alt+]` cycles panes, `Alt+Arrow`
  moves spatially.
- **Search output**: `Ctrl+Shift+F`.
- **Reconnect**: if the WebSocket drops, the pane retries with backoff and restores the last
  screen contents; a reconnect opens a fresh shell.
- **Preferences** (gear icon): font, size, weight, line height, cursor style and blink,
  scrollback, scroll speed, tab width, contrast. Stored in the browser/app.
- **Download**: a pane's visible text can be saved as a file.

### Keyboard shortcuts

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
topic for ntfy), and optionally turn on periodic server health checks with an interval. *Send
test* verifies the setup.

## Themes

Tokyo Night dark is the default; switch to light in the sidebar. The terminal colours follow.

## Mobile (Android app or a phone browser)

- The sidebar becomes a drawer; the status bar at the top shows the connection state and the
  current tab as dots.
- **Swipe left/right** on the terminal to move between tabs; a one-time hint explains this.
- The **extra keys row** above the keyboard provides `Esc`, `Tab`, `Ctrl`, `Alt`, arrows and
  `|`. Hold `Ctrl`/`Alt` then tap a letter.
- Preferences open as a bottom sheet. Only one pane is shown at a time.

## Desktop app

Closing the window hides the app to the system tray; use the tray menu to show or quit. The
server URL can be changed in Settings → General.
