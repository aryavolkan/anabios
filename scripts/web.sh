#!/usr/bin/env bash
#
# web.sh — build, test and serve the three.js frontend (web/).
#
# The page is static: an import map resolves `three` to a vendored copy, the
# simulation is `anabios-core` compiled to WebAssembly (crates/anabios-wasm),
# and the curated scenarios are staged next to the page. Nothing here needs a
# bundler; node is used only to copy files and run the smoke test.
#
# Usage:
#   scripts/web.sh build [--debug]  # wasm → web/wasm, three → web/vendor, scenarios → web/scenarios
#   scripts/web.sh wasm  [--debug]  # just the wasm module
#   scripts/web.sh test  [scenario] [ticks] [seed]  # node smoke test + native/wasm fingerprint check
#   scripts/web.sh serve [port]     # python http.server on 127.0.0.1 (default 8080)
#   scripts/web.sh clean            # remove every generated file under web/
#
# Prerequisites: rustup target `wasm32-unknown-unknown`, node ≥ 18, npm.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB="$ROOT/web"
WASM_OUT="$WEB/wasm/anabios.wasm"

die() { echo "error: $*" >&2; exit 1; }

ensure_target() {
  rustup target list --installed 2>/dev/null | grep -q '^wasm32-unknown-unknown$' \
    || rustup target add wasm32-unknown-unknown
}

build_wasm() {
  local profile=release dir=release
  if [ "${1:-}" = "--debug" ]; then profile=dev; dir=debug; fi
  ensure_target
  ( cd "$ROOT" && cargo build -p anabios-wasm --profile "$profile" --target wasm32-unknown-unknown ) >&2
  mkdir -p "$(dirname "$WASM_OUT")"
  cp "$ROOT/target/wasm32-unknown-unknown/$dir/anabios_wasm.wasm" "$WASM_OUT"
  # wasm-opt is optional: it trims ~10-15% when present, the module is valid without it.
  if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -O2 --enable-bulk-memory --enable-sign-ext "$WASM_OUT" -o "$WASM_OUT" >&2 || true
  fi
  echo "wasm: $WASM_OUT ($(du -k "$WASM_OUT" | cut -f1) KiB)"
}

vendor() {
  command -v npm >/dev/null || die "npm not found (node ≥ 18 is required for the frontend build)"
  ( cd "$WEB" && npm install --no-audit --no-fund --silent && node scripts/vendor.mjs ) >&2
}

manifest() {
  ( cd "$WEB" && node scripts/manifest.mjs ) >&2
}

cmd="${1:-}"; shift || true
case "$cmd" in
  build)
    build_wasm "$@"
    vendor
    manifest
    echo "web build complete → serve with: scripts/web.sh serve"
    ;;
  wasm)
    build_wasm "$@"
    ;;
  test)
    # Runs the node smoke test against the built module and asserts the wasm
    # trajectory fingerprint equals the native one for the same scenario/seed.
    scn="${1:-predator-prey}"; ticks="${2:-300}"; seed="${3:-7}"
    [ -f "$WASM_OUT" ] || build_wasm
    fp=$( cd "$ROOT" && cargo run --release -q -p anabios-wasm --example fingerprint -- \
            "scenarios/$scn.toml" "$ticks" "$seed" | grep -o 'fingerprint=0x[0-9a-f]*' | cut -d= -f2 )
    [ -n "$fp" ] || die "native fingerprint run failed"
    node "$WEB/test/wasm-smoke.mjs" --wasm "$WASM_OUT" --scenario "$ROOT/scenarios/$scn.toml" \
      --ticks "$ticks" --seed "$seed" --expect-fingerprint "$fp"
    ;;
  serve)
    port="${1:-8080}"
    [ -f "$WASM_OUT" ] || echo "note: no wasm built yet — run 'scripts/web.sh build' first" >&2
    echo "serving $WEB at http://127.0.0.1:$port/ (Ctrl-C to stop)" >&2
    ( cd "$WEB" && exec python3 -m http.server "$port" --bind 127.0.0.1 )
    ;;
  clean)
    rm -rf "$WEB/node_modules" "$WEB/vendor" "$WEB/wasm" "$WEB/scenarios" "$WEB/replays" "$WEB/package-lock.json"
    echo "cleaned generated web/ assets"
    ;;
  *)
    sed -n '3,20p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
    ;;
esac
