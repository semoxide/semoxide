#!/bin/sh
# Every crate root except semoxide-git forbids unsafe code (CLAUDE.md Rust rule 6).
set -eu
missing=""
for root in crates/*/src/lib.rs crates/*/src/main.rs; do
  [ -f "$root" ] || continue
  case "$root" in crates/semoxide-git/*) continue ;; esac
  grep -q '^#!\[forbid(unsafe_code)\]' "$root" || missing="$missing $root"
done
[ -z "$missing" ] && exit 0
echo "Missing #![forbid(unsafe_code)] in:$missing" >&2
exit 1
