#!/bin/sh
# Fails when a LOCKED test file (rust-testing skill) is modified or deleted.
# Usage: test-lock.sh --staged        (pre-commit: staged changes vs HEAD; honours SEMOXIDE_TESTS_UNLOCKED=1)
#        test-lock.sh <base-ref>      (CI: changes since the merge base with <base-ref>)
set -eu

if [ "${1:-}" = "--staged" ]; then
  # Maintainer unlock for a session (see CODE-ARCHITECTURE A2); CI ignores it and needs the label.
  if [ "${SEMOXIDE_TESTS_UNLOCKED:-}" = "1" ]; then
    echo "test-lock: unlocked by SEMOXIDE_TESTS_UNLOCKED=1" >&2
    exit 0
  fi
  base=HEAD
  changed=$(git diff --cached --name-only --diff-filter=MDR)
else
  base=$(git merge-base "${1:?base ref required}" HEAD)
  changed=$(git diff --name-only --diff-filter=MDR "$base" HEAD)
fi

locked=""
for file in $changed; do
  if git show "$base:$file" 2>/dev/null | grep -q '^// LOCKED:'; then
    locked="$locked $file"
  fi
done

[ -z "$locked" ] && exit 0
echo "Locked tests changed:$locked" >&2
echo "A locked test changes only with the maintainer's approval (rust-testing skill)." >&2
exit 1
