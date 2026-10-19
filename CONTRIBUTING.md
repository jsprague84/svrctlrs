# Contributing

Thanks for looking. SvrCtlRS is a small, self-hosted SSH terminal manager for one person's (or
a small team's) servers, phone-first, with shells that survive the phone. It is deliberately
not a monitoring platform, a bastion or a sharing tool; `docs/ROADMAP.md` says what is planned
and what was declined, so read it before proposing something large.

## Reporting a bug

Open an issue with the **Bug** template: what you did, what you expected, what happened, the
version (Settings → About, or `svrctl health`), and the browser or app. The server log line
for a failing request is found by its `x-request-id` header (`docs/DEPLOYMENT.md`,
"Health and troubleshooting"). Leave hostnames, keys and passwords out of logs you paste.

**Security problems**: do not open a public issue. Contact the maintainer directly (see
`docs/SECURITY.md`, "Reporting"); that document also lists the known weaknesses, which are not
news.

## Changing code

1. Read `docs/ARCHITECTURE.md` (how the pieces fit) and `docs/DEVELOPMENT.md` (toolchain,
   running locally, tests).
2. Work on a branch; CI runs on every branch. Keep a change to one subject.
3. Run the quality gates before pushing — they are what CI runs:

   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cd ui && npm run check && npm run lint && npm run test && npm run build
   ```

4. Changes to the SSH client, sessions, recordings or the terminal pane also need the
   end-to-end and phone checks against a real SSH host (`docs/DEVELOPMENT.md`, "Quality
   gates"); say in the pull request which you ran.

What a change is expected to bring:

- **Tests.** Rust tests next to the code (an in-memory database and an in-process SSH server
  are in `server/src/test_support/`); Vitest for pure UI modules. Anything touching login,
  sessions, tokens, WebSocket auth, CORS, host keys or per-user data comes with server tests.
- **Docs in the same commit.** When behaviour changes, update the document that describes it
  (`docs/USER-GUIDE.md`, `API.md`, `DEPLOYMENT.md`, `SECURITY.md`, …) and add a line under
  `## [Unreleased]` in `CHANGELOG.md`.
- **The conventions** in `CLAUDE.md`: typed errors through `ApiError`, every per-user query
  scoped by `user_id`, new migrations with the next free number (never edit a shipped one),
  Svelte 5 runes, no `unwrap()` on request paths.
- **Commit messages** in the existing style: `type(scope): what changed` (`feat`, `fix`,
  `docs`, `test`, `chore`, …), with a body that says why.

## Licence

Dual-licensed under MIT or Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`). By contributing you
agree that your contribution is licensed the same way.
