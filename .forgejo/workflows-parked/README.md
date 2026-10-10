# Parked workflows

Forgejo only runs files under `.forgejo/workflows/`. Workflows here are kept for reference and
can be moved back when wanted.

- `android.yaml` — parked 2026-10-10. The Tauri Android app had too many terminal-input problems
  to be used; the owner uses the web app on the phone. The identifier changed to
  `cc.jsnode.svrctlrs` since, so `ui/src-tauri/gen/android` must be regenerated
  (`npm run tauri android init`) before this workflow can run again.
