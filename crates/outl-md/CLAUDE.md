# CLAUDE.md — outl-md

The boundary between **what the user sees** (clean markdown) and **what the
core processes** (op log with stable IDs).

If this crate misroutes a block during matching, the user perceives "outl
deleted my work" — even if the op log still has it. Treat matching with the
same paranoia as the CRDT.

## What this crate owns

- Parse `.md` (clean, no IDs) → outline AST
- Render outline AST → `.md` (clean, no IDs)
- Read/write `.outl` sidecar (JSON, dotfile)
- The 3-level matching algorithm (external edit → reconstruct IDs)
- Diff (old AST + new AST) → minimal sequence of `Op`s
- **Inline tokenization** (`inline.rs`) — `**bold**`, `[[refs]]`,
  `#tags`, etc — and `ref_at_cursor`. **UI-agnostic.** TUI, future
  Tauri GUI, and mobile clients all consume the same `InlineTok` /
  `RefTarget` types and map them to their own primitives (`Span`,
  HTML, `AttributedString`, `AnnotatedString`).
- **Slugify** (`slug.rs`) — `[[Avelino]]` → `pages/avelino.md`. The
  user-facing name is preserved verbatim in the page's `title::`
  property.

## What this crate does NOT own

- The op log → `outl-core`
- File watching / debounce → `outl-cli`
- Reconcile TUI → `outl-tui`
- Network sync → `outl-sync` (phase 2)

## The 3-level matching algorithm

When an external save lands on `pages/foo.md`:

1. **Parse** new `.md` → AST without IDs.
2. **Load** `.foo.outl` → AST with old IDs and content hashes.
3. **Match** new ↔ old blocks at 3 confidence levels:

| Level | Confidence | Criteria | Action |
|-------|-----------|----------|--------|
| 1 | High | `content_hash` exact match, same parent (by hash) or identical structure | Preserve ID, emit `Move` if position changed |
| 2 | Medium | Normalized Levenshtein similarity > 80%, same parent OR position within ±2 lines | Preserve ID, emit `Edit` (+ `Move` if needed), log warning |
| 3 | Low / no match | Falls through | New ULID for new block; old block becomes `Delete` (`Move` to `TRASH_ROOT`); record in `.outl/orphans.log` |

**Hard rule:** a block that drops to level 3 must appear in `orphans.log`
before being deleted. **Silent deletion is a P0 bug.**

## Sidecar format

```json
{
  "version": 1,
  "page_id": "01HXY8KJZQ9T8M7VN3P2R6S4A0",
  "last_synced_hash": "sha256:...",
  "blocks": [
    {
      "id": "01HXY8KJZQ9T8M7VN3P2R6S4A1",
      "line": 1,
      "indent": 0,
      "content_hash": "sha256:..."
    }
  ]
}
```

- `content_hash` = SHA-256 of the **block's textual content** (not children).
- Sidecar is replicated between devices alongside the `.md`. Don't gitignore by default.

## Outl markdown dialect

```markdown
title:: example
status:: active
tags:: #project

- top level block
  priority:: high
  - child block with [[page reference]]
  - child block with ((block-reference))
- another top level
```

- `key:: value` lines at top of file = page properties (frontmatter outliner-style).
- `key:: value` lines nested as children of a block = block properties.
- `[[name]]` = page reference (bidirectional link).
- `[[2026-05-24]]` = journal reference (renders as date).
- `#tag` = tag (page reference with classification semantics).
- `((block-id))` = block embed (shows content of referenced block).
- `{{query: ...}}` = saved query (phase 3, parse as opaque for now).

**No `id::`, no UUID, no HTML comments** — IDs go in the sidecar only.

## Files

```
src/
├── lib.rs
├── parse.rs       # md → AST (no IDs)
├── render.rs      # AST → md (clean)
├── sidecar.rs     # read/write .outl JSON
├── matching.rs    # 3-level matching algorithm
└── diff.rs        # AST diff → Op sequence

tests/
├── roundtrip.rs           # render(parse(md)) == md (property test)
├── external_edit.rs       # light external edit preserves IDs
├── duplicate_block.rs     # Ctrl+D in vscode → first keeps ID, second gets new
├── identical_blocks_swap.rs # two identical blocks change parents
└── heavy_edit.rs          # >20% content change → level 2 warning
```

## Invariants

1. **Roundtrip stability.** `render(parse(md))` produces a semantically
   identical `.md` (same tree, properties, content; whitespace may normalize).
2. **No silent block loss.** A block falling to level 3 is always in `orphans.log`.
3. **Sidecar is JSON-valid.** Always. If you can't write valid JSON, you fail.
4. **Sidecar `version` field always present.** Future migrations.
5. **`content_hash`** is `sha256(block.content_text())` consistently. Same hash function across read and write.

## Things to never do here

- ❌ Write IDs into the `.md` file (use sidecar)
- ❌ Delete a block in matching without recording in `orphans.log` first
- ❌ Match on similarity > 80% across **different parents** without warning
- ❌ Skip the property test in `roundtrip.rs`
- ❌ Use a different hash function in sidecar read vs write
- ❌ Drop sidecar version 1 support when adding version 2 (always backward read)
- ❌ Block on a corrupt sidecar — fall back to "regenerate from op log" via `outl doctor`

## When you're done

1. `cargo fmt`
2. `cargo clippy -p outl-md -- -D warnings`
3. `cargo test -p outl-md`
4. `/roundtrip` to invoke the markdown-roundtrip-tester agent
5. Manual smoke: create a fixture md, render it back, diff
