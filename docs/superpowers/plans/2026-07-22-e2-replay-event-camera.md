# E2 — Replay & Event Camera — Implementation Plan

**Goal:** Event replay + run-until-event + event camera, per `docs/superpowers/specs/2026-07-22-e2-replay-event-camera-design.md`. Verification core is the headless `replay` subcommand (strict hash + refire equality); viewer features ride the existing `snapshot.rs` save/load via new gdext bindings.

**Determinism:** zero `anabios-core` changes. Golden suites untouched; the replay harness *strengthens* the gate by asserting bit-identical re-simulation.

---

## Task R1: gdext snapshot bindings
**Files:** `crates/anabios-godot/src/lib.rs`.
- `snapshot_bytes() -> PackedByteArray` wrapping `anabios_core::snapshot::save_to_bytes` (empty + `godot_error!` when no world).
- `restore_snapshot(bytes: PackedByteArray) -> bool` wrapping `load_from_bytes`; on success `self.inner = Some(w)` + `self.reset_history()`; false on error.
- `state_hash() -> i64` — `anabios_core::snapshot::state_hash` bit-cast (`as i64`); document equality-only use.

## Task R2: headless `replay` subcommand
**Files:** `crates/anabios-headless/src/replay.rs` (new), `main.rs`.
- `pub struct EventRecord { index, event_type: EventType, tick, hash: u64 }`, `pub struct ReplayOutcome { record, hash_ok: bool, refired: bool }`.
- `pub fn record_run(text, seed, ticks, every) -> (Vec<(u64, Vec<u8>)>, Vec<EventRecord>)` — snapshot at tick 0 + every `every` ticks; drain per tick; hash recorded per event tick.
- `pub fn verify(snaps, rec) -> ReplayOutcome` — nearest snapshot `tick ≤ rec.tick`, re-sim draining events, compare hash + `(type, tick)` presence.
- CLI: `replay --scenario S [--seed N] --ticks T [--snapshot-every 250] [--event K | --all]`; per-event `PASS`/`FAIL` lines, summary, `ExitCode` 1 on any FAIL. Default selection: all events (cap log spam at 50 lines, still verify all).
- Tests (`replay.rs`): predator-prey scenario (path via `env!("CARGO_MANIFEST_DIR")/../../scenarios/`), 500 ticks, every 100 → assert ≥3 events, all PASS. Negative: flip `scenario.seed += 1` in the verify re-sim → hash_ok false. Keep under ~15 s.

## Task R3: viewer replay manager
**Files:** `game/scripts/replay_manager.gd` (new), `main.gd` (spawn it), `codex_panel.gd` (expose latest event), `legend_panel.gd` (key hints).
- Node child of `Main` (created in `main._ready`, no `.tscn` edit), `PROCESS_MODE_ALWAYS`.
- Ring: `_process` checks `sim.tick() - last >= RING_EVERY (250)`; entries `{tick, bytes}`; cap 16 (`pop_front`).
- **[R]** replay latest event (from `sim.codex_events_since` cursor or codex panel's `_recent.back()`): store live `{tick, bytes}`; find ring entry with max tick ≤ event tick (fallback: oldest); `restore_snapshot`; set `main.paused = true`; self-step `sim.step_n(64)` per frame until `sim.tick() >= ev.tick`; pause; camera to `ev.loc` (zoom 1.5); spawn highlight `Node2D` (pulsing ring `_draw`, 24-unit radius, gold); banner `Label` ("REPLAY t=%d %s · [R] resume"). R/Esc → restore live bytes, resume prior pause state, free highlight/banner.
- **[U]** run-until-event: record `codex_event_count`, set `main.paused=false`, force `main.ticks_per_frame=64`; each frame, if count grew → restore prior speed, pause, camera to newest event loc. U/Esc cancels (restores speed, keeps pause state).
- **[V]** event camera: every 15 s ease camera (`create_tween`) to next recent event loc (newest-first cycle, zoom 2.0); banner "EVENT CAM · [V] exit". V/Esc or WASD/mouse exits.
- Keys via `_unhandled_key_input` (matches camera/overlay convention).

## Task R4: capture harness + gallery
**Files:** `game/scripts/debug_capture.gd`, `gallery/README.md`, new PNGs.
- `ANABIOS_EVENT_CAM=1` → after the tick jump, enable event camera on the replay manager.
- Captures: predator-prey at the t≈150 hunt (event cam close-up) + a replay banner shot on arms-race t≈31 raid. Honest captions in `gallery/README.md`.

## Task R5: README + gate + PR
- README viewer section: document R/U/V keys + `anabios-headless replay` under testing.
- Gate: `cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`; `cargo build -p anabios-godot`; capture-harness run (windowed, self-quitting) exercises all GDScript.
- Branch `e2-replay-event-camera`, PR linking the roadmap spec.

---

## Completion notes (2026-07-22)

All tasks complete. Evidence:

- **Headless replay:** `replay` on weapons-arms-race seed 3, 200 ticks — 11/11 events PASS (`hash_ok=true refired=true` for each, incl. the t=27 `combat_raid`). Unit tests cover all-events PASS on predator-prey + a tampered-hash negative.
- **Tick convention found & pinned:** detectors run before `world.tick += 1` (`tick.rs`), so an event stamped `T` is emitted by the step ending at `T+1`; both the headless verifier and the viewer fast-forward use `target+1`, and the viewer clamps its final `step_n` so the pause lands exactly on the event tick (first capture overshot to t=143 before the clamp).
- **Viewer:** ring 250/16; honest bail "event older than the snapshot ring" when the latest event predates the oldest snapshot; zero-loc events don't move the camera.
- **Captures:** `gallery/e2-event-camera.png` (event cam parked on t=113 Predation), `gallery/e2-replay-t080.png` (paused exactly at tick 80 with the highlight ring on the t=79 Territory centroid; codex panel re-accumulated showing the refired event).
- **Determinism:** no `anabios-core` changes; golden suites untouched and green.
