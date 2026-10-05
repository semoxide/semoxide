#!/usr/bin/env bash
# Clean release build of each metrics crate: wall time, stripped binary size, normal-dep crate count,
# and whether openssl-sys is in the graph. Usage: metrics/measure.sh [extra cargo args]
set -euo pipefail
cd "$(dirname "$0")"
exe=""; [[ "${OS:-}" == "Windows_NT" ]] && exe=".exe"
printf '| crate | clean release build (s) | binary (bytes) | crates (normal deps) | openssl-sys |\n|---|---|---|---|---|\n'
for m in m-https m-libssh2 m-russh; do
  (cd "$m"
   cargo fetch -q
   rm -rf target
   t0=$(date +%s)
   cargo build --release -q "$@"
   t1=$(date +%s)
   size=$(stat -c %s "target/release/$m$exe" 2>/dev/null || wc -c < "target/release/$m$exe")
   crates=$(cargo tree -e normal --prefix none "$@" | sed 's/ (\*)//; s/ (proc-macro)//' | sort -u | grep -vc '^$')
   ossl=$(cargo tree -e normal -i openssl-sys "$@" 2>/dev/null | grep -q openssl-sys && echo yes || echo no)
   printf '| %s | %s | %s | %s | %s |\n' "$m" "$((t1-t0))" "$size" "$crates" "$ossl")
done
