#!/usr/bin/env bash
# Godot viewer smoke checks, run headless against a pre-imported `game/` project.
#
#   godot-smoke.sh scenes  res://scenes/menu.tscn ...   — load each scene, require
#       a clean log and a registered gdextension.
#   godot-smoke.sh scripts test_building_sprites ...    — run each SceneTree test
#       script, require its own success line.
#
# Both modes exist because Godot exits 0 on a script that fails to parse, so an
# exit code alone can green-light a broken viewer. Every check therefore asserts
# on log *content*, not just status. The `dummy` rendering driver keeps this off
# a Vulkan device — GPU-less runners crash under software (llvmpipe) Vulkan, and
# nothing here needs to actually render.
set -uo pipefail

ERRORS='SCRIPT ERROR|Parse Error|Cannot open library|Failed to load GDExtension|Can.t open dynamic library'

fail() {
    echo "::error::$1"
    rc=1
}

smoke_scene() {
    local scene=$1 log
    log=$(mktemp)
    echo "== smoke: $scene =="
    godot --headless --rendering-driver dummy --path game "$scene" --quit-after 120 \
        >"$log" 2>&1 || fail "godot exited non-zero loading $scene"
    cat "$log"
    grep -qE "$ERRORS" "$log" && fail "viewer smoke found errors loading $scene"
    # Positive check: a silently-unloaded extension would otherwise pass on
    # absence-of-errors alone (e.g. on a scene that never calls into Rust).
    grep -q "Initialize godot-rust" "$log" || fail "gdextension did not initialize for $scene"
}

smoke_script() {
    local name=$1 log
    log=$(mktemp)
    echo "== $name =="
    if ! godot --headless --rendering-driver dummy --path game -s "res://scripts/$name.gd" \
        >"$log" 2>&1; then
        cat "$log"
        fail "$name failed"
        return
    fi
    cat "$log"
    if grep -qE "$ERRORS|FAIL:" "$log"; then
        fail "$name emitted script errors"
    elif ! grep -q "$name: all passed" "$log"; then
        fail "$name never reported success"
    fi
}

rc=0
mode=${1:?usage: godot-smoke.sh <scenes|scripts> <target>...}
shift
for target in "$@"; do
    case $mode in
        scenes) smoke_scene "$target" ;;
        scripts) smoke_script "$target" ;;
        *) echo "::error::unknown mode: $mode"; exit 2 ;;
    esac
done
exit $rc
