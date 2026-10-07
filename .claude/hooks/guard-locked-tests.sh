#!/bin/sh
# Claude Code PreToolUse hook: refuses edits to LOCKED test files (rust-testing skill).
# JSON escapes Windows backslashes ("C:\\x"); the second sed turns them back into one.
file=$(sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 | sed 's/\\\\/\\/g')
[ -n "$file" ] && [ -f "$file" ] || exit 0
grep -q '^// LOCKED:' "$file" || exit 0
echo "Blocked: $file is a LOCKED test approved by the maintainer. Do not edit it; stop and report why it should change (rust-testing skill)." >&2
exit 2
