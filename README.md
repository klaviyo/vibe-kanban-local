<p align="center">
  <picture>
    <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
    <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
    <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
  </picture>
</p>

<p align="center">A local-only fork of <a href="https://github.com/BloopAI/vibe-kanban">BloopAI/vibe-kanban</a>, kept alive past the upstream sunset.</p>

![](packages/public/vibe-kanban-screenshot-overview.png)

## What this fork is

This repository is a fork of `BloopAI/vibe-kanban` taken at the
pre-sunset cutover. The cutover removed every cloud-dependent feature
— authentication, organizations, team collaboration, real-time sync,
relay tunnel, analytics, error reporting — leaving a self-contained
single-user kanban that runs entirely against a local SQLite
database. Everything else (the kanban UI, workspaces, sessions,
coding-agent integrations, PR creation, MCP server) still works the
same way it did upstream.

If you want the original cloud-enabled product, use
[BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban). If
you want to keep using vibe-kanban locally now that the upstream
hosted service has shut down, you're in the right place.

## What's in here

- **Single-user local kanban** with synthetic identity auto-seeded
  on first launch (no signup, no login)
- **Workspaces, sessions, kanban board, settings** — fully functional
  against a local SQLite database
- **10+ coding agents** configurable from the UI: Claude Code, Codex,
  Gemini CLI, GitHub Copilot, Amp, Cursor, OpenCode, Droid, CCR,
  Qwen Code
- **Pull request creation** with AI-generated descriptions
- **MCP server** for programmatic access (`vibe-kanban-mcp --mode global`)

![](packages/public/vibe-kanban-screenshot-workspace.png)

## Install

### From a release (recommended)

Download the latest binaries from the
[releases page](https://github.com/kcarwileklaviyo/vibe-kanban-local/releases),
make them executable, and run:

```bash
chmod +x server vibe-kanban-mcp
./server                          # starts the UI
./vibe-kanban-mcp --mode global   # optional: stdio MCP for agent integrations
```

The server prints the URL it bound to (it auto-assigns a free port
unless `PORT` is set) and writes the chosen port to
`$TMPDIR/vibe-kanban/vibe-kanban.port` so other tools can find it.

Only `macos-arm64` is published today. For other platforms, build
from source.

### From source

```bash
git clone git@github.com:kcarwileklaviyo/vibe-kanban-local.git
cd vibe-kanban-local
pnpm i
pnpm run build
cargo build --release --bin server --bin vibe-kanban-mcp
./target/release/server
```

## Tools

A **Tools** section appears in the sidebar when a tools config exists. Each
tool is a button that runs one of your scripts. Configs are read on every
refresh, from (later files override earlier ones per tool `id`):

1. `vibe-kanban-tools.json` next to the server binary
2. the file named by `$VK_TOOLS_CONFIG`
3. `~/.vibe-kanban-tools.json`

```json
{
  "tools": [{
    "id": "my-tool",
    "title": "My Tool",
    "icon": "robot",
    "color": "#7c5cff",
    "command": ["./my-tool.sh", "{option}", "{issue_key}"],
    "click": { "options": [] },
    "ticket": {
      "options": [
        { "id": "inspect", "title": "Inspect" },
        { "id": "fix", "title": "Fix", "command": ["./fix.sh", "{issue_id}"] }
      ]
    }
  }]
}
```

- **Click:** with no `click.options` the button runs `command`. With options it
  opens a menu first and runs the chosen one.
- **Ticket:** drag the button onto a ticket (board card or list row); if the
  tool has `ticket.options` a menu appears and the chosen option runs against
  that ticket.
- **Scripted menus:** set `options_command` on `click` or `ticket` (same argv
  and placeholders as `command`) and the menu is built by that script each
  time it opens, so it can reflect state (Start vs Stop, a ticket's status).
  The script prints a JSON array to stdout; each item is one of:
  - `{"id": "stop", "title": "Stop"}` — a clickable option (`"type": "option"`
    is the default; add `"disabled": true` to grey it out). Choosing it runs
    the tool's `command` with the id as `{option}`.
  - `{"type": "text", "text": "Daemon running", "tone": "ok"}` — a status line.
    May contain newlines. `tone` is optional: `ok`, `warn`, `error`, `info`.
  - `{"type": "header", "text": "Daemon"}` and `{"type": "separator"}`.

  `options_timeout_secs` (default 10, max 30) bounds the script. If it fails or
  times out the menu shows the error and falls back to the static `options`.
  For a ticket drop the script receives `{issue_key}`, `{issue_id}` and
  `{project_id}`.
- **Commands** are argv arrays, run without a shell, in the config file's
  directory. An option's own `command` overrides the tool's. Placeholders:
  `{option}`, `{issue_id}`, `{issue_key}`, `{project_id}` (also exported as
  `VK_TOOL_OPTION`, `VK_ISSUE_ID`, `VK_ISSUE_KEY`, `VK_PROJECT_ID`,
  `VK_TOOL_ID`, `VK_TOOL_TARGET`). Relative program paths resolve against the
  config file's directory and `~/` is expanded.
- **Icons:** a name from the built-in set (`robot`, `terminal`, `wrench`,
  `bug`, `rocket`, `sparkle`, `search`, ... see
  `packages/web-core/src/features/tools/toolIcons.tsx`) or any emoji.
- **Result:** the server waits up to 30s and shows the exit status and output
  tail in a toast; slower commands keep running and log to
  `$TMPDIR/vibe-kanban-tools/`. Scripts that start long-lived processes should
  background them.

The server only runs commands that appear in these files; requests carry a
tool id, an option id and an issue id, never a command line.

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (>=20)
- [pnpm](https://pnpm.io/) (>=8)

Additional development tools:

```bash
cargo install cargo-watch
cargo install sqlx-cli
```

### Run the dev server

```bash
pnpm i
pnpm run dev
```

This starts the backend (with `cargo-watch`) and the web app. A blank
DB is copied from `dev_assets_seed/` on first launch.

### Build the web app standalone

```bash
cd packages/local-web
pnpm run build
```

### Environment variables

| Variable | Type | Default | Description |
|---|---|---|---|
| `PORT` | Runtime | Auto-assign | Server port (dev: frontend port; backend uses `PORT+1`) |
| `BACKEND_PORT` | Runtime | `0` (auto) | Backend port in dev mode (overrides `PORT+1`) |
| `FRONTEND_PORT` | Runtime | `3000` | Frontend dev server port (overrides `PORT`) |
| `HOST` | Runtime | `127.0.0.1` | Backend bind address |
| `MCP_HOST` | Runtime | `HOST` | MCP server connection host |
| `MCP_PORT` | Runtime | `BACKEND_PORT` | MCP server connection port |
| `VK_ALLOWED_ORIGINS` | Runtime | unset | Comma-separated list of origins permitted to call the backend (set when running behind a reverse proxy on a custom domain) |
| `VK_NO_BROWSER` | Runtime | unset | Skip auto-opening the browser on server start (handy for daemon-driven restarts) |
| `DISABLE_WORKTREE_CLEANUP` | Runtime | unset | Disable git worktree cleanup (debugging only) |

### Reverse proxy / custom domain

When running behind a reverse proxy (nginx, Caddy, Traefik, etc.)
or on a non-localhost domain, set `VK_ALLOWED_ORIGINS` to the public
origin(s):

```bash
VK_ALLOWED_ORIGINS=https://vk.example.com
# or comma-separated for multiple
VK_ALLOWED_ORIGINS=https://vk.example.com,https://vk-staging.example.com
```

Without this, the backend rejects requests with a 403 because the
browser's `Origin` header won't match the bound host.

## Documentation

The original upstream documentation site at vibekanban.com no longer
exists. The `docs/` directory in this repo carries the parts that
still apply post-cutover; sections describing cloud-only features
(authentication, organizations, team collaboration, etc.) have been
removed.

## Diverging from upstream

This fork tracks `BloopAI/vibe-kanban` at a specific commit; it does
not pull future upstream changes. Cherry-picking from upstream is
possible but generally requires conflict resolution against the cloud
removals. PRs welcome that bring in upstream improvements compatible
with the local-only model.
