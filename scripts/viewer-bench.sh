#!/usr/bin/env bash
#
# viewer-bench.sh — frame-time / scale bench for the Godot viewer.
#
# Boots a real windowed run of a scenario, lets the [F3] readout
# (game/scripts/perf_readout.gd) sample ANABIOS_BENCH_FRAMES frames without
# pausing the sim, then writes + prints the mean row — the number every phase
# of docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md quotes
# against the Phase 0 baseline (§6, §7).
#
# Usage:
#   scripts/viewer-bench.sh <scenario-name> [frames] [seed]
#
# Mirrors scripts/emergence.sh's `capture`/`view` launch: same Godot binary
# lookup, same `--path game` + `res://scenes/main.tscn` boot, same
# ANABIOS_SCENARIO/ANABIOS_SEED overrides applied by debug_capture.gd. Needs a
# real window (the readout reads live RenderingServer/Performance monitors) —
# do not add --headless/--rendering-driver dummy.
#
# Examples:
#   scripts/viewer-bench.sh predator-prey
#   scripts/viewer-bench.sh continental 600 3
#
# Env overrides:
#   ANABIOS_BENCH_OUT   output CSV path (default runs/viewer-bench.csv)
#   GODOT               path to a Godot 4.x binary

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCN_DIR="$ROOT/scenarios"

die() { echo "error: $*" >&2; exit 1; }

# Locate a Godot 4 binary — same lookup as emergence.sh's godot_bin().
godot_bin() {
  local g="${GODOT:-}"
  [ -n "$g" ] || g="$(command -v godot || true)"
  [ -n "$g" ] || g="/Applications/Godot.app/Contents/MacOS/Godot"
  command -v "$g" >/dev/null 2>&1 || [ -x "$g" ] \
    || die "godot not found — install Godot 4.x or set GODOT=/path/to/godot"
  echo "$g"
}

# Resolve a scenario argument to a .toml path — same lookup as emergence.sh's resolve().
resolve() {
  local s="${1:-}"
  [ -n "$s" ] || die "no scenario given (try: $ROOT/scripts/emergence.sh list)"
  for cand in "$s" "$SCN_DIR/$s" "$SCN_DIR/$s.toml"; do
    [ -f "$cand" ] && { echo "$cand"; return; }
  done
  die "unknown scenario '$s' — see: $ROOT/scripts/emergence.sh list"
}

[ $# -ge 1 ] || die "usage: $0 <scenario-name> [frames] [seed]"
scn="$(resolve "$1")"
name="$(basename "$scn" .toml)"
frames="${2:-300}"
seed="${3:-}"

godot="$(godot_bin)"
# Build the gdext cdylib the frontend loads (debug build, same as view/capture).
( cd "$ROOT" && cargo build -p anabios-godot ) >&2

out="${ANABIOS_BENCH_OUT:-$ROOT/runs/viewer-bench.csv}"
# perf_readout.gd writes wherever it's told; absolutize a relative path
# against the caller's cwd — Godot's own cwd differs once it launches, same
# reasoning as emergence.sh's capture/record --out handling.
case "$out" in /*) ;; *) out="$PWD/$out" ;; esac
mkdir -p "$(dirname "$out")"
rm -f "$out"

envs=(
  "ANABIOS_SCENARIO=res://../scenarios/$(basename "$scn")"
  "ANABIOS_BENCH_FRAMES=$frames"
  "ANABIOS_BENCH_OUT=$out"
)
[ -n "$seed" ] && envs+=("ANABIOS_SEED=$seed")
echo "[viewer-bench] $name frames=$frames${seed:+ seed=$seed} → $out — windowed" >&2
env "${envs[@]}" "$godot" --path "$ROOT/game" res://scenes/main.tscn

[ -f "$out" ] || die "bench CSV not produced at $out (did the viewer boot? see stderr above)"

# Re-derive the printed summary from the CSV's own mean row rather than
# trusting Godot's console output, which is buried in engine startup noise.
mapfile -t hdr < <(head -n1 "$out" | tr ',' '\n')
mapfile -t vals < <(tail -n1 "$out" | tr ',' '\n')
summary="frames=$frames"
for i in "${!hdr[@]}"; do
  [ "$i" -eq 0 ] && continue  # column 0 is "tick"; its mean row says "mean"
  summary+=" ${hdr[$i]}=${vals[$i]:-}"
done
echo "viewer-bench: $summary"
echo "csv: $out" >&2
