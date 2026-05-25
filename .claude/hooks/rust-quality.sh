#!/usr/bin/env bash
# PostToolUse hook: format + lint Rust files after edit/write.
#
# Reads tool_input.file_path from stdin (JSON). If it ends in .rs:
#   1. cargo fmt on that file (rustfmt directly — fast, doesn't need full workspace)
#   2. cargo clippy --no-deps -q on the owning crate (catches issues fast)
#
# Emits a non-blocking warning (exit 2 with reason) on clippy failure so Claude
# sees the lint output in context and can react in the next turn.
#
# Skip conditions:
#   - file path doesn't end in .rs
#   - file is outside crates/ (e.g. target/, .claude/, docs/)
#   - CARGO is unavailable (CI bootstrap scenarios)

set -uo pipefail

# Read hook event from stdin
event_json=$(cat)

file_path=$(printf '%s' "$event_json" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

# Skip non-Rust files
case "$file_path" in
  *.rs) ;;
  *) exit 0 ;;
esac

# Skip files outside the workspace src tree
case "$file_path" in
  */crates/*) ;;
  *) exit 0 ;;
esac

# Skip target build artifacts
case "$file_path" in
  */target/*) exit 0 ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
  exit 0
fi

if ! command -v rustfmt >/dev/null 2>&1; then
  exit 0
fi

# Format the single file (cheap)
rustfmt --edition 2021 --quiet "$file_path" 2>/dev/null || true

# Detect owning crate from path: crates/<name>/...
crate_dir=$(printf '%s' "$file_path" | sed -n 's|.*/crates/\([^/]*\)/.*|\1|p')

if [ -z "$crate_dir" ]; then
  exit 0
fi

# Run clippy on just this crate. Fast incremental check.
clippy_output=$(
  cd "${CLAUDE_PROJECT_DIR}" 2>/dev/null || cd .
  cargo clippy -p "$crate_dir" --no-deps --quiet --message-format=short 2>&1
)
clippy_status=$?

if [ "$clippy_status" -ne 0 ]; then
  # Emit warning, non-blocking. Claude sees this in next-turn context.
  printf 'clippy emitted warnings in crate %s after edit of %s:\n%s\n' \
    "$crate_dir" "$file_path" "$clippy_output" >&2
  exit 2
fi

exit 0
