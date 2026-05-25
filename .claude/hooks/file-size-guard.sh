#!/usr/bin/env bash
# PostToolUse hook: warn (and at higher thresholds, *block*) when a
# Rust source file grows past sensible limits.
#
# The point isn't a hard line-count cap — it's to force a conversation
# about responsibility. A 700-line file is usually a design smell
# (multiple concerns sharing a module); a 1000-line file always is.
#
# Thresholds (lines of Rust source, blank/comment included):
#   < 400     OK, no signal
#   400..600  notice (informational; printed to stderr, doesn't block)
#   600..900  warning (exit 2; reminds Claude to consider extracting)
#   >= 900    refuse-with-rationale (exit 2 + strong note; Claude must
#             propose a refactor before the next big edit)
#
# Reads tool_input.file_path from stdin JSON. Skip non-Rust files,
# files outside /crates/, and target/ artifacts.

set -uo pipefail

event_json=$(cat)

file_path=$(printf '%s' "$event_json" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

case "$file_path" in
  *.rs) ;;
  *) exit 0 ;;
esac

case "$file_path" in
  */crates/*) ;;
  *) exit 0 ;;
esac

case "$file_path" in
  */target/*) exit 0 ;;
esac

if [ ! -f "$file_path" ]; then
  exit 0
fi

lines=$(wc -l < "$file_path" | tr -d ' ')

# Pretty filename for messages.
rel=${file_path#"${CLAUDE_PROJECT_DIR}/"}

if [ "$lines" -lt 400 ]; then
  exit 0
fi

if [ "$lines" -lt 600 ]; then
  # Informational only — no exit 2, no blocking.
  printf 'note: %s is %d lines. Watch for accumulation; extract when responsibilities diverge.\n' \
    "$rel" "$lines" >&2
  exit 0
fi

if [ "$lines" -lt 900 ]; then
  printf 'WARNING: %s is %d lines. This is past comfortable single-module size.\n' \
    "$rel" "$lines" >&2
  printf 'Consider extracting one of the concerns into a sibling module before the next big edit.\n' >&2
  printf 'See docs/architecture.md and the per-crate CLAUDE.md for the layering principle.\n' >&2
  exit 2
fi

# >= 900 — strong push.
printf 'STOP: %s is %d lines. A Rust file that big has accreted multiple\n' "$rel" "$lines" >&2
printf 'concerns and is hard to read, hard to test, and hard to evolve.\n' >&2
printf '\n' >&2
printf 'Before the next non-trivial edit to this file:\n' >&2
printf '  1. Identify 2-4 distinct responsibilities inside it (state, IO,\n' >&2
printf '     rendering, key handling, AST helpers, ...).\n' >&2
printf '  2. Extract each into a sibling module (`mod x;` in the crate).\n' >&2
printf '  3. Re-export from the parent only the names other modules need.\n' >&2
printf '\n' >&2
printf 'Invoke the `refactor-architect` agent (.claude/agents/) to propose\n' >&2
printf 'a concrete split, or stop and ask the user for guidance.\n' >&2
exit 2
