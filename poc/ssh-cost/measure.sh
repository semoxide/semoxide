#!/usr/bin/env bash
# Clean release build of each variant in its own fresh target dir (RUNS times, default 2):
# wall time per run, stripped binary size, unique normal-dep crates, openssl-sys presence.
# Usage: ./measure.sh [extra cargo args, e.g. --target x86_64-unknown-linux-musl --features vendored-openssl]
set -euo pipefail
cd "$(dirname "$0")"
exe=""; [[ "${OS:-}" == "Windows_NT" ]] && exe=".exe"
runs=${RUNS:-2}
cargo fetch -q
printf '| variant | build s (each run) | binary bytes | crates | openssl-sys via |\n|---|---|---|---|---|\n'
for v in https-only libssh2 russh; do
  times=()
  for i in $(seq "$runs"); do
    rm -rf "target-$v"
    t0=$(date +%s)
    cargo build --release -q --no-default-features --features "$v" --target-dir "target-$v" "$@"
    t1=$(date +%s)
    times+=("$((t1-t0))")
  done
  bin=$(find "target-$v" -path '*/release/ssh-cost'"$exe" -type f | head -1)
  size=$(stat -c %s "$bin")
  crates=$(cargo tree -e normal --prefix none --no-default-features --features "$v" "$@" \
           | sed 's/ (\*)//; s/ (proc-macro)//' | grep -v '^$' | sort -u | wc -l)
  ossl=$(cargo tree -e normal -i openssl-sys --prefix none --depth 1 --no-default-features --features "$v" "$@" 2>/dev/null \
         | grep -v '^openssl-sys' | awk '{print $1}' | sort -u | tr '\n' ' ' || true)
  printf '| %s | %s | %s | %s | %s |\n' "$v" "${times[*]}" "$size" "$crates" "${ossl:-none}"
  echo "$bin" >&2
done
