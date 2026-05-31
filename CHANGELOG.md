# Changelog

All notable changes to outl are documented here. Format inspired by
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
uses [Semantic Versioning](https://semver.org/).

## [0.3.1] — 2026-05-31

Mobile UX polish + autocomplete fixes. No protocol or storage
changes — drop-in upgrade from 0.3.0.

### Mobile (`outl-mobile`)

- **Autocomplete (`[[…]]`) now actually fires on iOS.** The native
  ref suggester chip strip was orphaned — `createEffect` was being
  registered after an `await` inside `onMount`, which lost Solid's
  reactive owner. State was published once at boot and never
  updated as the user typed.
- **TODO/DONE prefix is visible (and editable) in Insert mode.**
  Tapping a TODO block used to show only the checkbox + body
  (`ship it`) with the `TODO ` prefix hidden, so erasing the
  prefix from the editor was impossible. Now the prefix appears in
  the textarea (`TODO ship it`) and the checkbox flips to a bullet
  while editing — toggling state via the text Just Works.
- **Cursor lands inside `[[ ]]` / `(( ))` reliably.** `el.value =
  …` resets the textarea caret in iOS WKWebView; combined with
  Solid's `value={draft()}` rebinding the caret could end up
  outside the pair. Replaced with `setRangeText` + double
  `parkCaret` (sync + microtask) so every toolbar insert, paste
  completion, and suggester pick parks the caret where the user
  expects it.
- **Backspace inside empty `[[]]` / `(())` collapses the pair.**
  No more mashing backspace four times to undo an aborted ref.
  Same rule on TUI and mobile.
- **Smart Punctuation is OFF.** `--` no longer becomes `–`, `...`
  no longer becomes `…`, quotes stay straight. Code snippets and
  CLI commands in journals survive intact.
- **Toast pattern for errors** (auto-dismiss + Retry button) in
  place of the persistent red `<p>` that sat in the middle of the
  outline forever. Failed saves now offer a one-tap retry without
  losing the draft.
- **`commitInFlight` lock + 8 s timeout** serializes concurrent
  block edits (typing → TODO toggle → blur) so the older save
  never overwrites the newer, and a stuck Tauri command can't
  freeze Insert mode indefinitely.
- **Progressive loading message** ("Loading…" → "Connecting to
  iCloud…" → "Still waiting on iCloud…") + spinner + a Retry
  button on terminal failure. iCloud cold-start no longer reads as
  "the app froze".
- **Connectivity-aware SyncDot** uses `navigator.onLine` +
  `online`/`offline` listeners to actually show the offline pip
  (was dead code before). `aria-label` instead of `title` so iOS
  WKWebView users get the status verbally.
- **Tap targets meet Apple HIG** (~30 × 30 hit area on the
  bullet/checkbox; bullet is now actually tappable). `[[ref]]` and
  `#tag` taps navigate instead of opening the editor.
- **Long-press TODO** uses a distinct success haptic when creating
  a new TODO vs. cycling an existing one.
- **`SwipeRow` × `SwipeNavigator` conflict resolved** —
  swipe-right on the left edge no longer races the per-row
  swipe-delete (each one captures only its own direction).
- **`PageSwitcher`** opens the first match on `Enter`, dismisses
  on `Escape`, and supports swipe-down on the handle to dismiss
  (without stealing scroll from the list).
- **Backlinks empty state** so the bidirectional-linking concept
  is discoverable on freshly-created pages.
- **Performance** in long outlines: `draft()` is now a lazy getter
  prop only read by the block being edited (was triggering a
  reactive effect in every BlockRow per keystroke). Auto-resize
  coalesced into a single `requestAnimationFrame`.

### Shared (`outl-actions`)

- `edit_text` writes its argument **verbatim** instead of
  preserving a leading `TODO `/`DONE ` prefix automatically.
  Callers that surface state separately (mobile checkbox)
  reattach the prefix themselves — required so erasing the
  prefix in the editor actually sticks. TUI path is unaffected
  (it always passes the raw block text through reconcile).

### TUI (`outl-tui`)

- Backspace inside an empty `[[]]` / `(())` now collapses both
  brackets in one keystroke (matches the mobile behaviour).

## [0.3.0] — 2026-05-30

Cross-device sync goes live. A brand-new iOS app and the TUI share
the same workspace over iCloud Drive, both driven by a shared
`outl-actions` crate. Block refs / embeds land in the markdown
dialect.

### Mobile (`outl-mobile`) — new crate

- **Tauri 2 + SolidJS iOS client.** Bundle id `app.outl.mobile-app`,
  iCloud container `iCloud.app.outl.mobile-app`. Frontend is Solid +
  Tailwind; the Rust shell is intentionally thin (every workspace
  operation delegates to `outl-actions`).
- **iCloud Drive transport.** Workspace lives at
  `<ubiquity-container>/Documents/`. An ObjC bridge
  (`gen/apple/.../main.mm`) uses `NSMetadataQuery` to watch for peer
  changes and `NSFileCoordinator` + `startDownloadingUbiquitousItemAtURL`
  to force materialisation before reads — without those two steps
  the Rust side races the iCloud daemon and sees truncated op logs.
- **Per-device actor id** persisted under the app sandbox so each
  install writes to its own `ops-<actor>.jsonl`.
- iOS boot flash fixed; outl brand (palette + icon) applied across
  the app.

### Shared client (`outl-actions`) — new crate

- **Extracted every workspace mutation** (block edit, TODO toggle,
  indent / outdent, sibling create, delete, move, journal render,
  sync) out of `outl-tui` into a UI-agnostic crate. Functions take
  `&mut Workspace` + `&HlcGenerator` and route through
  `Workspace::apply` so the op log stays source of truth.
- TUI and mobile call the **same** functions for the same
  semantics — drift between clients is no longer possible by
  construction.
- `SyncEngine` interface owns the cross-device merge loop; iCloud is
  the v0 transport, iroh (phase 2) plugs in behind the same trait.

### Core (`outl-core`)

- **`JsonlStorage` op-log backend.** Single-file SQLite breaks under
  iCloud / Syncthing because the FS layer is last-write-wins per
  file. JSONL gives each actor its own `ops-<actor>.jsonl`, writes
  only to the local file, and merges every peer file on read.
- Folder layout is **`ops/`**, not `.ops/`. iCloud Documents skips
  dotted paths during cross-device sync — using a dot silently
  breaks multi-device workspaces. Same rule applied to the sidecar
  (`pages/<slug>.outl`, no leading dot).

### Markdown (`outl-md`)

- **`((blk-X))` inline refs and `!((blk-X))` embeds.** Stable
  `ref_handle` derived from the block's ULID tail (lazy 7+ char
  expansion on collision); sidecar bumped to v2. Embeds expand to
  the source root + children with a `↳` marker.
- Concurrent-safe writes over iCloud (atomic temp-file rename, no
  partial reads exposed to peers).
- `WorkspaceIndex` tracks block-ref backlinks alongside page-ref
  backlinks.

### TUI (`outl-tui`)

- Rebuilt as a **peer of shared workspaces** — same iCloud folder,
  same op log, same `outl-actions`. Edits on the laptop appear on
  the iPhone within seconds and vice versa.
- `((` autocomplete on block text, inline ref render, expanded
  embed view, Enter navigation to the source block, `/refer` and
  `/refer-embed` slash commands.
- `yr` chord copies the block's ref handle to the OS clipboard via
  arboard.
- outl brand (palette, icon, chrome) applied; mobile and TUI now
  look like the same product.

### CLI (`outl-cli`)

- **`outl migrate-to-shared` subcommand** rewrites a legacy SQLite
  workspace into the JSONL + sidecar layout consumed by both
  clients.
- `outl doctor` flags orphan `((blk-X))` and `!((blk-X))` handles.

### CI / release

- Release workflow rewritten as `prepare → tag → create_release
  (draft) → build matrix → publish_release`. Single `gh release
  create --draft` before the matrix and `gh release upload
  --clobber` per matrix leg, so paralleled jobs don't race each
  other on a repo with Immutable Releases turned on.
- macOS Intel binary now cross-compiles from `macos-latest` (ARM)
  instead of relying on the depleted `macos-13` runner pool.
- `outl-mobile` excluded from Linux CI jobs (Tauri iOS toolchain is
  macOS-only).

## [0.2.0] — 2026-05-26

Backlinks become a first-class part of the TUI: they live inline below
the outline (no more side panel), render the referencing block with
its children, and are fully editable in place.

### TUI (`outl-tui`)

- **Inline backlinks.** Replace the right-side panel with a section
  rendered below the outline, separated by a full-width `─` rule. Each
  source page shows up grouped under an icon + title header.
- **Full source block + children.** Backlinks render the referencing
  `OutlineNode` *with its subtree* (not a truncated snippet), so you
  see context without jumping to the source page.
- **Cursor navigation crosses the boundary.** `j`/`k` flow transparently
  between outline and backlinks. `app.focus: Focus::{Outline,
  Backlink{idx, sub_path}}` tracks where the cursor lives.
- **In-place edits land on the source `.md`.** `i`/`I`/`a`/`Esc`,
  `Ctrl+T` (TODO/DONE cycle), `o`/`O` (sibling create), `Tab`/`Shift+Tab`
  (indent/outdent), `dd` (delete), `K`/`J` (move up/down) — all work on
  a backlink the same way they work on the outline, persisting straight
  to the source page via `EditTarget::SourcePage`.
- **Optimistic index updates for snappy UX.** Edits patch the in-memory
  `WorkspaceIndex` immediately (next frame shows the new state), then
  save without scheduling a full workspace rebuild on the hot path.
- Cursor column preserved when entering Insert (`i` honors vim
  semantics; `I` still jumps home).
- Ghost cursor on the last outline block when focus had moved into the
  backlinks section is gone (`render_block` gates by `Focus::Outline`).
- `view.rs` split into `view/{inline, outline, overlays, backlinks}.rs`
  by responsibility — each file under 450 lines.

### Markdown (`outl-md`)

- `Backlink` carries the full `source_block: OutlineNode` and its
  `source_block_path` (DFS path in the source AST) instead of a flat
  index plus truncated snippet. Repeated refs to the same target inside
  one block collapse to a single backlink.
- `WorkspaceIndex::refresh_backlinks_from_source(path, &page)` —
  optimistic patch of every cached `source_block` for backlinks
  pointing at `path`. Used by the TUI's cross-page edit path.
- `WorkspaceIndex::patch_backlink_text(path, target_path, &new_text)`
  for text-only optimistic edits.

## [0.1.0] — 2026-05-25

First public release. Single-device editor; sync transport is on the
roadmap but the algorithm and op-log infrastructure are already in.

### Core (`outl-core`)

- Tree CRDT implementation following Kleppmann et al. 2022
  (`do_op` / `undo_op` / `apply_op` / `creates_cycle`).
- HLC timestamps with actor tiebreak.
- Append-only op log with sqlite backend (`SqliteStorage`).
- `Storage` trait so alternative backends (e.g. ChronDB) can slot in
  without touching the CRDT.
- Workspace file lock via `fs2::flock` — two `outl` processes on the
  same workspace get a clean error, not a race.
- Property-based test of strong eventual consistency over 100+
  randomised op permutations.

### Markdown / sidecar (`outl-md`)

- CommonMark parse + render with the outl dialect (`title::`,
  `icon::`, page/block properties, `[[refs]]`, `#tags`,
  `((block-id))`, fenced code blocks, multi-line block text).
- `.foo.outl` JSON sidecar holding the IDs the `.md` deliberately
  doesn't carry. **The `.md` stays clean** — no `id::`, no UUIDs.
- 3-level matching algorithm (`outl-md::matching`) reconstructs which
  block kept which ID after an external editor saves the file.
- Workspace index (`WorkspaceIndex`) — title, icon, slug, backlinks,
  tag namespace; powers the switcher, autocomplete and backlinks
  panel. Built once on boot, refreshed in a worker thread on save.
- Roundtrip property test: `parse(render(ast)) == ast` over randomly
  generated outlines including multi-line and fenced cases.

### Code-block execution (`outl-exec`)

- `Runtime` trait + `RuntimeRegistry`. Shipped runtimes (each behind
  a Cargo feature for opt-out distributions):
  - `lisp` — Steel (Scheme R5RS-ish in pure Rust).
  - `js` — Boa (ES2015+ in pure Rust).
  - `python` — RustPython (Python 3 subset).
  - `lua` — mlua 5.4 (vendored).
  - `rust` — `rustc → wasm32-wasip1 → wasmtime`. Compiled artefacts
    cached in `~/.cache/outl/runtimes/rust/<hash>.wasm`. ~20× faster
    on a re-run of the same snippet.
- WASM sandbox infrastructure (wasmtime engine + WASI ctx with no
  preopens / no env / no sockets, fuel-based instruction cap,
  epoch-interruption timeout, in-memory stdin/stdout/stderr).
- Idempotent result subblock — re-running the same code overwrites
  the existing `> **result:**` child instead of duplicating it.
- `source-hash::` stamped on each result child so the upcoming auto-run
  loop can short-circuit unchanged sources.

### TUI (`outl-tui`)

- Journal-first: opens on today's date.
- Vim-style modes (Normal / Insert / Visual) with chord support
  (`dd`, `gg`, `gx`, `yy`, `qq`-to-quit).
- Insert mode autocomplete for `[[refs]]`, `#tags`, and `/commands`
  (Notion-style slash menu).
- Slash command system + vim palette share one registry — every
  built-in command shows up in both. Built-ins: `prop-block`,
  `prop-page`, `search`, `run`, `theme`, `today`, `open`,
  `refresh`, `write`, `quit`, `help`. The registry is the
  plugin-extension point.
- `gx` runs the code block under the cursor through `outl-exec`.
- `auto-run::` property runs a block automatically on page open
  (cache-aware via SHA-256 of the source).
- `icon::` page property surfaces in every place the title shows
  (header, switcher, backlinks panel, search results, autocomplete,
  inline `[[refs]]`).
- Multi-line blocks via `Alt+Enter` / `Ctrl+J` / `Shift+Enter`
  (Shift+Enter only on terminals that speak the kitty keyboard
  protocol); plain `Enter` auto-detects an open code fence and
  inserts a soft newline inside it.
- Vertical scroll with `PgUp`/`PgDn`/`Ctrl+D`/`Ctrl+U`/`gg`/`G` and
  auto-scroll when the selection moves off-screen.
- Hot reload on external `.md` edits (polls mtime every 750ms; warns
  instead of clobbering when you're mid-Insert).
- Error modal overlay for multi-line failures (rustc compile errors,
  traps, missing toolchain), keeping the status line for short
  successes.
- Themes: 11 presets, switchable with `/theme <name>` at runtime.

### CLI (`outl-cli`)

- `outl` (no subcommand) opens the TUI in `$PWD`.
- `outl init <path>` scaffolds a workspace.
- `outl serve [--once]` reconciles `.md` files into the op log
  (one-shot or watch mode).
- `outl import logseq <src> <dst>` and `outl import roam <backup.json>
  <dst>` strip `id::` lines, slugify, seed sidecars.
- `outl doctor` and `outl reconcile` placeholders for the integrity
  and orphan-resolution flows.

### Tooling / DX

- Workspace MSRV: rustc 1.88.
- CI: `fmt` + `clippy -D warnings` + `cargo test --workspace --all-targets`
  on Linux and macOS.
- Bench CI: `small` / `medium` / `large` on every PR + push;
  `xlarge` (10k+ files) on weekly cron + manual dispatch.
- File-size guard hook (`.claude/hooks/file-size-guard.sh`) blocks
  Rust files past ~900 LOC unless the change is intentional —
  forces a refactor conversation before drift accumulates.
- Background workspace-index build: `App::new` paints the journal
  immediately and spawns a worker thread for the global index;
  backlinks/icons fill in within ~ms of boot.

### License

MIT.

[0.1.0]: https://github.com/avelino/outl/releases/tag/v0.1.0
