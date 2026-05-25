# CLAUDE.md — outl-cli

The `outl` binary. Thin shell over `outl-core` + `outl-md`. **No business
logic lives here** — only argument parsing, file orchestration, watcher
setup, and human-readable output.

## UX rule: no subcommand → open the TUI

`outl` with no subcommand opens `outl-tui` in the current directory. This
is the primary mode — Roam/Logseq users expect to launch the app and see
their notes, not a help screen.

The TUI library is reused via `use outl_tui;` (the crate exposes both a
library and a binary). Don't fork the TUI logic into the CLI.

## Commands (phase 1)

- `outl` — open TUI in current directory (also `outl tui [<path>]`).
- `outl init <path>` — scaffold a workspace (pages/, journals/, templates/, .outl/).
- `outl serve [<path>] [--once]` — run file watcher; `--once` reconciles
  every `.md` and exits (smoke tests, scripting).
- `outl export --to <fmt>` — placeholder for phase 4 (Hugo, etc).
- `outl doctor [<path>]` — integrity check (sidecar vs op log, orphans).
- `outl reconcile [<path>]` — list orphans pending manual resolution.

## Layout

```
src/
├── main.rs
└── cmd/
    ├── init.rs
    ├── serve.rs
    ├── export.rs
    ├── doctor.rs
    └── reconcile.rs
```

## Conventions

- `clap` derive for parsing.
- `anyhow::Result` at command boundaries (errors are user-facing).
- `tracing` for logs, structured. Default level `info`, `--verbose` bumps to `debug`.
- Exit codes: `0` success, `1` user error, `2` internal error.

## What this crate does NOT do

- ❌ Implement the CRDT (use `outl-core`)
- ❌ Parse markdown (use `outl-md`)
- ❌ Render TUI directly (use `outl-tui` as a library or sub-binary)
- ❌ Network anything (phase 2)
