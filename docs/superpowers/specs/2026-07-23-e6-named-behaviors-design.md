# E6 — Named Behaviors Completion — Design Spec

**Date:** 2026-07-23
**Status:** Approved
**Milestone:** E6 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crate:** `anabios-core` (instrumentation + detectors). `FORMAT_VERSION` 11→12 (`CombatHit` context fields + detector scratch), goldens regenerated once, behavior unchanged. Inspector "signature view" deferred to E10's codex screen (noted; detectors + codex text carry the information meanwhile).

## 1. Goal & success criteria

Finish the design §4.3 named-behavior codex chapter — the most *legible* emergence for players. Four new event types: `EvolvedAmbush` (34), `EvolvedTool` (35), `EvolvedFlight` (36), `StructuredSignaling` (37).

Success criteria:

1. Every combat hit carries behavioral context (was the attacker lying in wait? was the damage invention-boosted?) recorded at fire time.
2. Each detector has positive + negative handcrafted tests — negative tests are the point here (a random shooter is not an ambusher; a slow wader is not a flier; a broadcast followed by nothing is not a signal).
3. 16-seed sweep evidence on combat-heavy scenarios; replay verification.
4. Golden suites pass (layout-only regen).

## 2. Instrumentation

- `World.still_ticks: Vec<u32>` (`#[serde(skip)]` scratch, like the other per-agent scratch): consecutive ticks the agent's speed was below `STILL_SPEED_FRAC ×` its effective max speed. Updated once per tick in the serial interact stage (before combat), zero when the agent moves.
- `CombatHit` += `ambush: bool` (attacker's `still_ticks ≥ AMBUSH_STILL_MIN = 40` at fire time) and `tool_boosted: bool` (`invention::weapon_multiplier > 1.05` — Metalworking-boosted damage).

## 3. Detectors (`codex/signatures.rs`)

Rolling per-species counters over the combat-hit window (400 ticks):

- **`EvolvedAmbush`** — ≥`AMBUSH_MIN_HITS = 10` hits and ≥30% ambush-flagged → fire once per species (latched, re-arms below 15%). `value` = ambush share. Long stillness followed by prey-proximate fire is the ambush signature; a species that fires on the move never accumulates it.
- **`EvolvedTool`** — ≥`TOOL_MIN_HITS = 10` hits and ≥30% tool-boosted → fire. `value` = boosted share. Naturally silent without the invention tree (no Metalworking, no boost).
- **`EvolvedFlight`** — per-species count of agent-ticks spent on barrier terrain (water/rock cell) at ≥70% of the agent's effective max speed, over a 400-tick window ≥ `FLIGHT_MIN_CROSSINGS = 20` → fire. Slow waders never qualify (speed gate); fast open-ground runners never qualify (terrain gate).
- **`StructuredSignaling`** — mirrors the AlarmCall machinery but for *convergence*: a Communicator broadcast (any meme channel) after which ≥3 same-species receivers move **toward** the caller's position within 30 ticks. ≥`SIGNAL_MIN_RESPONSES = 10` cumulative → fire once per species. A broadcast followed by no directed response is not a signal.

## 4. Wiring

- `EventType` 34–37; `score.rs` (38 names, +4 bonus); `sweep.rs` CSV +4; `codex_panel.gd` +4 (Ambush / ToolUse / Flight / Signaling).

## 5. Testing & evidence

- Unit: scripted ambusher vs mobile shooter; metalworking hunter vs unboosted; fast barrier-crosser vs slow wader vs fast open-ground runner; broadcast-with-convergence vs broadcast-alone.
- Integration: weapons-arena long run fires ≥1 new type; replay one event.
- Sweep: 16 seeds × 6000 ticks on `weapons-arena.toml`; counts in completion notes.
- Gallery: close-up of an ambush kill (streak from a stationary attacker) per the arms-race capture convention.
