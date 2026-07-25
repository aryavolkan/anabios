#!/usr/bin/env bash
#
# emergence.sh — run, replay, and inspect anabios emergence scenarios from the CLI.
#
# A thin wrapper over `anabios-headless` (built in release). Scenarios can be
# given by short name (`predator-prey`), file name (`predator-prey.toml`), or a
# full path. Extra flags after the scenario pass straight through to the binary.
#
# Usage:
#   scripts/emergence.sh list                       # list available scenarios
#   scripts/emergence.sh info    <scenario>         # print a scenario summary
#   scripts/emergence.sh run     <scenario> [flags] # run once, tally emergent events
#   scripts/emergence.sh replay  <scenario> [flags] # deterministic event replay/verify
#   scripts/emergence.sh sweep   <scenario> [flags] # multi-seed emergence scorecard
#   scripts/emergence.sh soak    <scenario> [flags] # long run, novelty-decay curve
#   scripts/emergence.sh demo    <scenario> [flags] # narrated invention race
#
# Common passthrough flags: --ticks N  --seed N  --seeds N  --window N  --out DIR
#
# Examples:
#   scripts/emergence.sh run    predator-prey --ticks 5000
#   scripts/emergence.sh replay weapons-arms-race --seed 3
#   scripts/emergence.sh sweep  traditions --seeds 16 --ticks 12000
#   scripts/emergence.sh soak   drifting-climate --ticks 300000 --window 50000

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCN_DIR="$ROOT/scenarios"
BIN="$ROOT/target/release/anabios-headless"
OUT_DIR="${ANABIOS_OUT:-${TMPDIR:-/tmp}/anabios}"

die() { echo "error: $*" >&2; exit 1; }

build() {
  # Build only the headless crate (skips the heavy godot crate). Fast when
  # already up to date.
  ( cd "$ROOT" && cargo build --release -p anabios-headless ) >&2
}

# Resolve a scenario argument to a .toml path.
resolve() {
  local s="${1:-}"
  [ -n "$s" ] || die "no scenario given (try: $0 list)"
  for cand in "$s" "$SCN_DIR/$s" "$SCN_DIR/$s.toml"; do
    [ -f "$cand" ] && { echo "$cand"; return; }
  done
  die "unknown scenario '$s' — see: $0 list"
}

cmd="${1:-help}"
[ $# -gt 0 ] && shift || true

case "$cmd" in
  list)
    ls "$SCN_DIR"/*.toml | xargs -n1 basename | sed 's/\.toml$//' | sort | column 2>/dev/null \
      || ls "$SCN_DIR"/*.toml | xargs -n1 basename | sed 's/\.toml$//' | sort
    ;;

  info)
    scn="$(resolve "${1:-}")"; build
    "$BIN" info --scenario "$scn"
    ;;

  run)
    scn="$(resolve "${1:-}")"; shift || true; build
    mkdir -p "$OUT_DIR"
    events="$OUT_DIR/$(basename "$scn" .toml)-events.jsonl"
    "$BIN" run --scenario "$scn" --events-jsonl "$events" "$@"
    echo
    echo "--- emergent behaviors fired ($scn) ---"
    if [ -s "$events" ]; then
      grep -o '"event_type":"[A-Za-z]*"' "$events" \
        | sed 's/.*:"//; s/"//' | sort | uniq -c | sort -rn
      echo "(full event stream: $events)"
    else
      echo "(no codex events — try more --ticks, e.g. --ticks 5000)"
    fi
    ;;

  replay)
    scn="$(resolve "${1:-}")"; shift || true; build
    "$BIN" replay --scenario "$scn" "$@"
    ;;

  sweep)
    scn="$(resolve "${1:-}")"; shift || true; build
    out="$OUT_DIR/sweep-$(basename "$scn" .toml)"
    # Let an explicit --out passthrough win; otherwise default it.
    case " $* " in *" --out "*) "$BIN" sweep --scenario "$scn" "$@" ;;
      *) "$BIN" sweep --scenario "$scn" --out "$out" "$@"; echo "summary: $out/summary.csv" ;;
    esac
    ;;

  soak)
    scn="$(resolve "${1:-}")"; shift || true; build
    out="$OUT_DIR/soak-$(basename "$scn" .toml).csv"
    case " $* " in *" --out "*) "$BIN" soak --scenario "$scn" "$@" ;;
      *) "$BIN" soak --scenario "$scn" --out "$out" "$@"; echo "curve: $out" ;;
    esac
    ;;

  demo)
    scn="$(resolve "${1:-}")"; shift || true; build
    "$BIN" demo --scenario "$scn" "$@"
    ;;

  help|-h|--help|*)
    sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    ;;
esac
