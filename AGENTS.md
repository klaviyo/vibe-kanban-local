# Repository Guidelines

This is a local-only fork of `BloopAI/vibe-kanban` taken at the
pre-sunset cutover. The cutover removed the cloud, relay, analytics,
and team-collab features; what remains is a single-user kanban that
runs against a local SQLite database. See `README.md` for the
user-facing summary.

## Project layout

- `crates/`: Rust workspace.
  - `server` — HTTP API + main binary
  - `mcp` — `vibe-kanban-mcp` MCP server binary
  - `db` — SQLx models, migrations, identity seeder
  - `executors` — coding-agent integrations (claude, codex, gemini, copilot, qwen, opencode, cursor, droid, ccr, amp)
  - `services` — diff streaming, container service, pr monitor, config
  - `git`, `git-host`, `worktree-manager`, `workspace-manager` — git + worktree plumbing
  - `deployment`, `local-deployment` — process supervision
  - `api-types` — wire types shared between server and frontend (and generator for `shared/types.ts`)
  - `utils`, `client-info`, `server-info`, `preview-proxy`, `ws-bridge`, `trusted-key-auth` — supporting crates
  - `remote`, `remote-info` — vestigial cloud-side code retained as a thin shell after the cutover. Some routes are still live (`/api/remote/*` is the cutover-era kanban API), but the cloud-deployment binary itself is no longer maintained.
  - `tauri-app` — desktop wrapper
  - `review` — PR review tool
- `packages/`:
  - `local-web` — the React + Vite + Tailwind app served by `crates/server` (single-user local UI, the canonical frontend)
  - `web-core` — shared React/TS frontend library imported by `local-web`
  - `ui` — design-system component library
  - `public` — shared static assets
  - `remote-web` — vestigial cloud-deployment frontend; not built or served as part of the local product
- `shared/`: generated TypeScript types (`types.ts`, `remote-types.ts`) and agent tool schemas. Do not hand-edit; regenerate from Rust (see below).
- `assets/`, `dev_assets_seed/`, `dev_assets/`: packaged + dev assets, including the seed SQLite copied on first dev run.
- `npx-cli/`: vestigial — used by upstream to publish to npm. This fork does not publish; ignore unless touching upstream-compatible build paths.
- `scripts/`: dev helpers (port allocation, DB prep, etc.).
- `docs/`: docs that still apply post-cutover. Cloud-only sections have been removed.

### Per-crate / per-package guides

- `packages/local-web/AGENTS.md` — design-system styling guidelines for the web app.
- `crates/remote/AGENTS.md` — historical context on the remote server architecture. Most of what it describes has been deleted; treat as a guide to the *shape* of the remaining `remote` crate, not a prescriptive doc.
- `docs/AGENTS.md` — Mintlify documentation writing guidelines (only relevant if rebuilding the public docs site, which the fork does not do).

## Shared types between Rust and TypeScript

`ts-rs` derives TypeScript types from Rust structs/enums. Annotate
Rust types with `#[derive(TS)]` and regenerate:

```bash
pnpm run generate-types          # writes shared/types.ts
pnpm run generate-types:check    # CI variant — fails if regen produces a diff
```

Do not hand-edit `shared/types.ts`; edit
`crates/server/src/bin/generate_types.rs`.

`shared/remote-types.ts` exists as a holdover from upstream's cloud
crate and is regenerated via `pnpm run remote:generate-types`. It is
still consumed by `web-core` for type compatibility but is not
required for the local build to function.

## Build, test, dev commands

| Action | Command |
|---|---|
| Install | `pnpm i` |
| Run dev (web app + backend, ports auto-assigned) | `pnpm run dev` |
| Backend in watch mode | `pnpm run backend:dev:watch` |
| Web app dev only | `pnpm run local-web:dev` |
| Frontend type-check + Rust check across all crates | `pnpm run check` |
| Rust workspace tests | `cargo test --workspace` |
| Regenerate TS from Rust | `pnpm run generate-types` |
| Prepare SQLx offline metadata | `pnpm run prepare-db` |
| Format (cargo fmt + Prettier) | `pnpm run format` |
| Lint (clippy + ESLint) | `pnpm run lint` |
| Build release binaries | `cargo build --release --bin server --bin vibe-kanban-mcp` |

## Distribution

Releases are GitHub Releases on this repo: prebuilt `server` and
`vibe-kanban-mcp` binaries are attached as assets named
`{server,vibe-kanban-mcp}-<platform>` (e.g. `server-macos-arm64`).
On download, callers can write the release tag (with an optional
leading `v` stripped) to `.installed_tag` next to the binary; the
server's `display_version()` reads that file and surfaces it in the
UI's bottom-left version label. Without `.installed_tag`, version
falls back to `CARGO_PKG_VERSION`.

This fork does not publish to npm. The `npx-cli/` directory is left
in place for upstream-compat reasons but isn't part of the release
flow.

## Coding style

- **Rust**: `rustfmt` enforced via `rustfmt.toml`. Group imports by crate. `snake_case` modules, `PascalCase` types.
- **TypeScript/React**: ESLint + Prettier (2 spaces, single quotes, 80 cols). `PascalCase` components, `camelCase` vars/functions, `kebab-case` files where practical.
- Keep functions small. Derive `Debug`, `Serialize`, `Deserialize` where useful.

## Before completing a task

- `pnpm run format`
- `pnpm run check` (or at least `pnpm run lint`)

## Testing

- **Rust**: prefer unit tests alongside code (`#[cfg(test)]`). `cargo test --workspace` from the repo root. Add tests for new logic and edge cases.
- **Web**: ensure `pnpm run check` and `pnpm run lint` pass. For runtime logic, include lightweight tests (Vitest) colocated with the source.

## Security & config

- Use `.env` for local overrides; never commit secrets.
- Key runtime envs: `PORT`, `FRONTEND_PORT`, `BACKEND_PORT`, `HOST`, `VK_ALLOWED_ORIGINS`, `VK_NO_BROWSER` — see `README.md` for full table.
- Dev ports and assets are managed by `scripts/setup-dev-environment.js`.
