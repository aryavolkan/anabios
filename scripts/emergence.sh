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
#   scripts/emergence.sh view   [scenario] [flags]  # WINDOWED Godot sandbox (watch it live)
#   scripts/emergence.sh record  <scenario> [flags] # record a cinematic showcase MP4
#   scripts/emergence.sh run     <scenario> [flags] # run once, tally emergent events
#   scripts/emergence.sh replay  <scenario> [flags] # deterministic event replay/verify
#   scripts/emergence.sh sweep   <scenario> [flags] # multi-seed emergence scorecard
#   scripts/emergence.sh sweep-archived <scenario> [flags] # sweep scored against $ANABIOS_CORPUS
#   scripts/emergence.sh soak    <scenario> [flags] # long run, novelty-decay curve
#   scripts/emergence.sh demo    <scenario> [flags] # narrated invention race
#
# Common passthrough flags: --ticks N  --seed N  --seeds N  --window N  --out DIR
#
# `view` opens a real window (needs Godot; set GODOT=/path/to/godot to override
# the default lookup). With a scenario it boots straight in; with none it opens
# the picker menu. Only `--seed N` is honored for view.
#
# Examples:
#   scripts/emergence.sh view   predator-prey --seed 3   # watch it in a window
#   scripts/emergence.sh record out-of-africa-saga --seed 318   # cinematic MP4
#   scripts/emergence.sh view                            # menu: pick a scenario
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

# Locate a Godot 4 binary for the windowed `view` command.
godot_bin() {
  local g="${GODOT:-}"
  [ -n "$g" ] || g="$(command -v godot || true)"
  [ -n "$g" ] || g="/Applications/Godot.app/Contents/MacOS/Godot"
  command -v "$g" >/dev/null 2>&1 || [ -x "$g" ] \
    || die "godot not found — install Godot 4.x or set GODOT=/path/to/godot"
  echo "$g"
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

  view|window|watch)
    # Windowed Godot sandbox. With a scenario it boots straight into the sim;
    # with none it opens the picker menu. Reads scenario/seed via env overrides
    # that debug_capture.gd applies to GameConfig before main.tscn loads.
    godot="$(godot_bin)"
    # Build the gdext cdylib the frontend loads (debug — a CLI/editor run uses
    # the macos.debug library entry).
    ( cd "$ROOT" && cargo build -p anabios-godot ) >&2

    # Optional leading scenario (anything not starting with '-').
    env_scn=""
    if [ $# -gt 0 ] && [ "${1#-}" = "$1" ]; then
      scn="$(resolve "$1")"; shift
      # Frontend paths are relative to game/: res://../scenarios/<name>.toml
      env_scn="res://../scenarios/$(basename "$scn")"
    fi

    # Pull a --seed N out of the remaining flags; the rest go to Godot verbatim.
    seed=""; rest=()
    while [ $# -gt 0 ]; do
      case "$1" in
        --seed) seed="${2:-}"; shift 2 || die "--seed needs a value" ;;
        --seed=*) seed="${1#--seed=}"; shift ;;
        *) rest+=("$1"); shift ;;
      esac
    done

    if [ -n "$env_scn" ]; then
      envs=("ANABIOS_SCENARIO=$env_scn")
      [ -n "$seed" ] && envs+=("ANABIOS_SEED=$seed")
      echo "[view] $env_scn${seed:+ (seed=$seed)} — windowed" >&2
      env "${envs[@]}" "$godot" --path "$ROOT/game" res://scenes/main.tscn ${rest[@]+"${rest[@]}"}
    else
      echo "[view] opening scenario picker (no scenario given)" >&2
      "$godot" --path "$ROOT/game" ${rest[@]+"${rest[@]}"}
    fi
    ;;

  record)
    # Cinematic showcase recording: plays a scripted beat timeline (the
    # showcase director) over the live sim and captures it with Godot's Movie
    # Maker (--write-movie, fixed 60 fps), then converts to MP4 with ffmpeg.
    # Flags: --seed N  --timeline FILE  --out FILE.mp4  --fps N (default 60)
    #        --max-seconds N  # safety quit if the timeline never reaches "end"
    scn="$(resolve "${1:-}")"; shift || true
    name=$(basename "$scn" .toml)
    seed=""; timeline="$ROOT/game/showcase/$name.json"; fps=60
    out="$ROOT/runs/showcase/$name.mp4"; max_seconds=""
    while [ $# -gt 0 ]; do
      case "$1" in
        --seed) seed="${2:-}"; shift 2 || die "--seed needs a value" ;;
        --seed=*) seed="${1#--seed=}"; shift ;;
        --timeline) timeline="${2:-}"; shift 2 || die "--timeline needs a value" ;;
        --timeline=*) timeline="${1#--timeline=}"; shift ;;
        --out) out="${2:-}"; shift 2 || die "--out needs a value" ;;
        --out=*) out="${1#--out=}"; shift ;;
        --fps) fps="${2:-}"; shift 2 || die "--fps needs a value" ;;
        --fps=*) fps="${1#--fps=}"; shift ;;
        --max-seconds) max_seconds="${2:-}"; shift 2 || die "--max-seconds needs a value" ;;
        --max-seconds=*) max_seconds="${1#--max-seconds=}"; shift ;;
        *) die "unknown record flag '$1'" ;;
      esac
    done
    [ -f "$timeline" ] || die "no timeline at $timeline (pass --timeline)"
    # Godot resolves --write-movie relative to --path (game/), so absolutize.
    case "$out" in /*) ;; *) out="$PWD/$out" ;; esac
    case "$timeline" in /*) ;; *) timeline="$PWD/$timeline" ;; esac
    godot="$(godot_bin)"
    command -v ffmpeg >/dev/null || die "ffmpeg not found (brew install ffmpeg)"
    ( cd "$ROOT" && cargo build -p anabios-godot ) >&2
    mkdir -p "$(dirname "$out")"
    avi="${out%.mp4}.avi"
    echo "[record] $name${seed:+ (seed=$seed)} timeline=$(basename "$timeline") → $out" >&2
    envs=("ANABIOS_SCENARIO=res://../scenarios/$(basename "$scn")"
          "ANABIOS_SHOWCASE=$timeline")
    [ -n "$seed" ] && envs+=("ANABIOS_SEED=$seed")
    # Movie Maker needs a real window (the capture reads rendered frames);
    # the director quits the app at the timeline's "end" beat. --quit-after
    # is the safety valve for a stalled trigger (no event, missing end beat).
    godot_args=(--write-movie "$avi" --fixed-fps "$fps")
    [ -n "$max_seconds" ] && godot_args+=(--quit-after "$max_seconds")
    env "${envs[@]}" "$godot" --path "$ROOT/game" "${godot_args[@]}" res://scenes/main.tscn
    [ -s "$avi" ] || die "movie write failed — $avi is empty (windowed run required)"
    ffmpeg -y -loglevel error -i "$avi" -c:v libx264 -crf 20 -pix_fmt yuv420p -movflags +faststart "$out"
    rm -f "$avi"
    echo "[record] wrote $out" >&2
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

  sweep-archived)
    # Convenience wrapper: sweep scored against a maintained reference
    # corpus (see `sweep --archive`), so the weekly archive-weighted run
    # doesn't need hand-passing --archive. Corpus-unseen runs land in
    # <out>/novel/.
    scn="$(resolve "${1:-}")"; shift || true; build
    corpus="${ANABIOS_CORPUS:-runs/corpus}"
    out="$OUT_DIR/sweep-$(basename "$scn" .toml)-archived"
    # Let an explicit --out passthrough win; otherwise default it.
    case " $* " in *" --out "*) "$BIN" sweep --scenario "$scn" --archive "$corpus" "$@" ;;
      *) "$BIN" sweep --scenario "$scn" --archive "$corpus" --out "$out" "$@"
         echo "summary: $out/summary.csv   novel: $out/novel/" ;;
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
    sed -n '2,31p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    ;;
esac
