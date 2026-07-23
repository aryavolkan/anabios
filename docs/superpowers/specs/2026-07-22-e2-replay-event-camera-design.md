# E2 — Replay & Event Camera — Design Spec

**Date:** 2026-07-22
**Status:** Approved
**Milestone:** E2 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crates:** `anabios-headless` (replay harness), `anabios-godot` (snapshot bindings), `game/` (replay UI). No `anabios-core` changes — snapshot save/load already exists (`snapshot.rs`).

## 1. Goal & success criteria

Close design §4.5/§6.3/§6.4: the player can *return to* emergence, not just read about it. Emergence events become re-visitable moments, and the same machinery doubles as a detector-regression harness.

Success criteria:

1. `anabios-headless replay` re-simulates any codex event from a periodic snapshot and asserts (a) bit-identical state hash at the event tick and (b) the same event type re-firing at the same tick. Non-zero exit on mismatch — CI-usable detector regression gate.
2. The viewer keeps a ring of periodic snapshots; pressing **[R]** replays the most recent codex event: rewind to the nearest snapshot, fast-forward to the event tick, pause with a highlight at the event location, and resume the live world on demand.
3. **[U]** run-until-next-event: max-speed forward run that auto-pauses when a codex event fires and jumps the camera there.
4. **[V]** event camera: auto-cut tour of recent event locations (~15 s each), toggleable, screensaver-friendly.
5. Golden hashes untouched (no core changes); GDScript exercised end-to-end via the `ANABIOS_SHOT` capture harness, which also produces the gallery evidence.

## 2. Headless replay (the verification core)

```
anabios-headless replay --scenario S [--seed N] --ticks T [--snapshot-every M] [--event K | --all]
```

- **Pass 1 (record):** run ticks `0..T`. Store an in-memory snapshot at tick 0 and every `M` ticks (default 250). At every tick, drain codex events and record `(index, type, tick, state_hash_at_tick)`.
- **Pass 2 (verify):** for each selected event: load the nearest snapshot with `tick ≤ ev.tick`, re-sim to `ev.tick` draining events, then assert hash equality against pass 1 and that the re-simmed stream contains the same `(event_type, tick)` pair.
- Determinism (design §7.2) makes this a strict equality check — any detector or tick-pipeline regression that perturbs the trajectory flips a PASS to FAIL.
- Output: one line per event (`PASS seed=… event=#K type=tick …`), a summary, exit code 1 on any FAIL.

## 3. gdext bindings

- `snapshot_bytes() -> PackedByteArray` — `snapshot::save_to_bytes`; empty array on failure.
- `restore_snapshot(bytes) -> bool` — `snapshot::load_from_bytes`; on success replaces the world and calls `reset_history()` (the codex panel already shrink-resets on a shorter event log, and the coevo chart restarts from the snapshot tick).
- `state_hash() -> i64` — bit-cast of the FNV-1a hash; equality comparison only.

## 4. Viewer features (`game/scripts/replay_manager.gd`, child of `Main`)

- **Snapshot ring:** every `RING_EVERY = 250` ticks (checked with `>=` since `step_n` advances up to 64/frame), capped at 16 entries (≈16 × snapshot size; a 2k-agent world snapshot is ~1 MB). Pure GDScript — no core/gdext ring type.
- **Replay [R]:** saves the live world, restores the nearest ring snapshot at or before the latest event's tick, sets `main.paused` and self-steps at 64 ticks/frame until the event tick, pauses, moves the camera to the event location, and shows a pulsing highlight ring + a "REPLAY t=N · [R] resume" banner. Pressing **R** again (or Esc) restores the live snapshot and resumes.
- **Run-until-event [U]:** toggles a 64× forward run; auto-pauses and jumps the camera when `codex_event_count` grows.
- **Event camera [V]:** cycles the most recent events (newest first), easing the camera to each location at zoom 2.0 for 15 s; any manual camera key or V/Esc exits.

## 5. Capture harness extension

`debug_capture.gd` gains `ANABIOS_EVENT_CAM=1` (enable event camera after the tick jump) so gallery shots can show the event camera mid-tour with its banner. Gallery: event-camera close-up on a predator-prey hunt event + replay banner shot.

## 6. Testing & evidence

- **Headless unit/integration (`replay.rs` tests):** short predator-prey run (500 ticks, snapshot every 100); assert ≥1 event and every replayed event PASSes. Negative: replaying with a perturbed seed must FAIL the hash check.
- **gdext:** bindings exercised by the capture harness run (windowed, self-quitting).
- **Gate:** fmt, clippy `-D warnings`, `cargo test --workspace` — determinism suite untouched and green (no core diff).
- **Gallery:** `gallery/e2-event-camera.png`, `gallery/e2-replay.png` with honest captions in `gallery/README.md`.

## 7. Non-goals

- No persistent cross-session event→snapshot DB (that is E10's codex DB).
- No timeline scrubber UI (design §6.4) — the ring is in-memory per session; scrubbing arrives with the codex screen in E10.
- No event-keyed core-side snapshots: the ring cadence bounds rewind error to `RING_EVERY` ticks, which the fast-forward covers in ~4 frames at 64×/frame-stepping.
