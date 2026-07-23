# E4 — Disturbance & Succession — Implementation Plan

**Goal:** Disasters + succession substrate and +4 spatial detectors per `docs/superpowers/specs/2026-07-23-e4-disturbance-succession-design.md`.

**Determinism:** `disasters_enabled` off ⇒ byte-identical trajectories. One `FORMAT_VERSION` 9→10 bump + golden regen (Task D3). RNG draw order in the scheduler documented and fixed.

---

## Task D1: succession in `biome.rs`
- `BiomeCell.succession: u8` (0 Climax default / 1 Pioneer / 2 Bare) + consts (`PIONEER_RATE_MULT=1.5`, `PIONEER_CAPACITY_MULT=0.5`, `BARE_RESEED_FRAC=0.005`, `PIONEER_ENTRY_FRAC=0.05`, `CLIMAX_ENTRY_FRAC=0.6`).
- `regrow_step`/`regrow_step_seasonal`: Climax path byte-identical; Pioneer half-cap ×1.5-rate; Bare reseed-only; transitions after regrowth.
- Unit tests: transitions + Climax arithmetic unchanged.

## Task D2: `disaster.rs` scheduler + effects
- `DisasterKind` (repr u8), `ActiveDisaster`, `DisasterSite`, `DisasterState` (serde).
- `DisasterState::new(rng) -> schedule first`; `disaster_step(world)` per tick (spawn at schedule, propagate fire ring / drought / freeze disks, expire, register fire sites).
- `World.disasters_enabled: bool` + `World.disasters: DisasterState`; scenario flag wiring.
- Tick insertion: after pheromone decay (stage 8c), before species_step; no-op when flag off.
- Unit tests: deterministic sequence, fire scorch + Forest→Grass, drought/freeze decay, expiry.

## Task D3: FORMAT_VERSION 9→10 + golden regen
- `snapshot.rs` bump + changelog; regen all three golden suites (`UPDATE_HASHES=1`), notes: layout only, golden scenarios run with the flag off.

## Task D4: `codex/disturbance.rs` detectors
- EventType 27–30 + `EVENT_TYPE_COUNT`; `SpeciesAgg` += scratch `occ_cells: BTreeSet<u32>`; CodexState += `range_occ_history: BTreeMap<u32, VecDeque<u32>>`, `range_active: BTreeSet<u32>`, `segregation_streak: BTreeMap<u32, u32>`, `segregation_active: BTreeSet<u32>`, `migration_dirs: BTreeMap<u32, VecDeque<(f32, f32)>>`, `corridor_active: BTreeSet<u32>`, `succession_fired_sites` tracked via `DisasterSite.succession_fired: bool`.
- `detect_range_expansion` / `detect_segregation` / `detect_corridor_use` / `detect_succession` (site scan runs on the biome-step cadence).
- `population::detect_migration`: append direction to `migration_dirs` on fire (cap 4).
- observe_all wiring; unit tests positive + negative each.

## Task D5: wiring + viewer overlay
- `score.rs` (31 names, +4 bonus weights), `sweep.rs` (event_name +4, CSV +4), `codex_panel.gd` (+4: RangeExpand / Segregation / Corridor / Succession).
- gdext `succession_colors()`; `game/` ground mode "succession" in `overlay_manager.gd` + `GROUND_NAMES` + `biome_renderer.gd` branch; legend line.
- `scenarios/disturbance.toml` + menu entry.

## Task D6: evidence + gate + PR
- Integration test (disturbance.toml, ≥1 new type, cap-pinned for speed); replay one Succession event; 16-seed sweep counts; gallery burn-scarcar before/after pair.
- fmt / clippy / workspace tests / gdext build; branch `e4-disturbance-succession` stacked on `e3-population-dynamics`; PR.

---

## Completion notes (2026-07-23)

All tasks complete. Evidence:

- **Sweep (16 seeds × 6000 ticks, `disturbance.toml`):** RangeExpansion 14/16, SegregationEmerged 7/16, CorridorUse 16/16, Succession 15/16.
- **Replay verification:** `replay --event 1194` on disturbance (2500 ticks) → `PASS succession tick=2460 hash_ok=true refired=true` (fire at t=2231, scar re-vegetated ~230 ticks later).
- **Detector honesty iterations (all observed in real runs, fixed same-day):**
  - CorridorUse fired ~1000×/run on direction+span alone — every persistent drifter qualified. Now requires ≥6 aggregated barrier (water/rock) sample hits across 4 agreeing legs over ≥400 ticks (24×/6000 on the showcase seed).
  - SegregationEmerged auto-fired at t=199 in 16/16 runs from starter placement. Now requires a prior spatially-mixed observation ("emerged", not "founded apart") — honest 7/16 minority.
  - Succession never completed under grazing pressure when requiring full Climax — fires at re-vegetation (Pioneer counts).
- **Substrate:** Poisson scheduler (fire ring growth, drought/freeze disks; debug-instrumented spawn/expire traces used for tuning), succession state machine with byte-identical Climax path (unit-pinned against the pre-E4 arithmetic).
- **Tests:** 12 disturbance detector unit tests (positive + negative incl. founded-apart negative and burst negative), 4 disaster unit tests, 4 succession biome unit tests, `tests/disturbance.rs` integration (10 s debug, asserts scheduler fired + scars exist + ≥1 new event).
- **Determinism:** `FORMAT_VERSION` 9→10; all three golden suites regenerated once (layout only; flag off in golden scenarios).
- **Viewer:** succession ground overlay (climax/pioneer/bare + active disaster tints) gated on `disasters_active`; gallery pair `e4-fire-ring.png` (burn ring mid-expansion, t=2321) / `e4-succession.png` (same scar re-vegetated, t=2631, `Succession: 1` in the tally).
