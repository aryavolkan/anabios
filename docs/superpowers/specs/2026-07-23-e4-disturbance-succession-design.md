# E4 — Disturbance & Succession — Design Spec

**Date:** 2026-07-23
**Status:** Approved
**Milestone:** E4 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crate:** `anabios-core` (substrate + detectors), `anabios-godot` (overlay binding), `game/` (overlay mode), `anabios-headless` (wiring). Behavior-altering → everything behind `disasters_enabled` (off by default: baseline scenarios byte-identical); `FORMAT_VERSION` 9→10 + one golden regeneration (layout only).

## 1. Goal & success criteria

The world pushes back. Delivers design §6.2 disasters and unlocks spatial succession dynamics — the substrate E5's shock-correlated adaptation detector will lean on. Four new event types: `RangeExpansion` (27), `SegregationEmerged` (28), `CorridorUse` (29), `Succession` (30).

Success criteria:

1. Fire / drought / freeze disasters strike a deterministic Poisson schedule, mutate the biome over their duration, and leave recoverable scars.
2. Biome cells carry a succession state (bare → pioneer → climax) that shapes regrowth: pioneer regrows fast to half capacity, bare reseeds slowly.
3. Each detector has a positive + negative handcrafted test; `scenarios/disturbance.toml` fires ≥3 of the 4 new types across a 16-seed sweep.
4. Viewer: succession ground overlay + disaster tints; gallery before/after of a burn scar being re-colonized.
5. Flag off ⇒ byte-identical trajectories (proved: golden suites pass with the flag off in all golden scenarios).

## 2. Disaster scheduler & effects (`disaster.rs`, core)

`World.disasters_enabled: bool` + `World.disasters: DisasterState` (scheduler, ≤4 active disasters, ≤8 recent sites). All draws from the single world RNG stream, in documented order (interval, kind, epicenter col, epicenter row, severity).

- **Schedule:** next disaster at `tick + 800 × expovariate` (mean interval `DISASTER_MEAN_INTERVAL = 800`, realized via `-ln(1-u)`).
- **Kinds:** `Fire` (radius grows linearly over `DISASTER_DURATION = 120` ticks to `severity × 24` cells: vegetated cells inside burn — biomass → 0, succession → Bare, Forest scorches to Grass), `Drought` (fixed disk `severity × 32` cells for 400 ticks: biomass ×(1 − severity×0.004)/tick), `Freeze` (fixed disk `severity × 24` cells for 200 ticks: biomass ×(1 − severity×0.03)/tick).
- One `disaster_step(world)` per tick, inserted after pheromone decay (deterministic serial stage; no agent effects in E4 — biome only).

## 3. Succession (`biome.rs`)

`BiomeCell.succession: u8` — 0 Climax (default; existing semantics), 1 Pioneer, 2 Bare. Transitions evaluated in `regrow_step*` (every 10 ticks):

- **Bare → Pioneer** when biomass > 5% of carrying capacity. Bare cells reseed spontaneously at 0.5% of capacity per biome step (wind-blown seed — otherwise burns never recover without `living_biome`).
- **Pioneer → Climax** when biomass ≥ 60% of capacity.
- **Regrowth:** Pioneer gets ×1.5 rate but half effective capacity (fast, weedy, low standing crop); Bare gets only the reseed increment; Climax unchanged.

## 4. Detectors (`codex/disturbance.rs`)

- **`RangeExpansion`** — per-species occupied-cell count (`SpeciesAgg` gains a scratch occupied-cell set) grows ≥50% over a 400-tick window (min 20 cells) AND centroid displaces ≥60 units (existing `centroid_history`). Latched, re-arms on contraction.
- **`SegregationEmerged`** — a species (≥20 members, ≥20 occupied cells) whose occupied-cell overlap against the union of all other species stays <10% for a 200-tick streak **and which was observed spatially mixed at least once**. The prior-mixed requirement is what makes it *emerged* segregation — species founded apart are founder geography, not events (the first implementation auto-fired at t=199 from starter placement in every run). Species-vs-rest (not pairwise) so it scales to speciation-maelstrom worlds; `value` = 1 − overlap.
- **`CorridorUse`** — a species logs ≥`CORRIDOR_MIN_MIGRATIONS = 4` migrations with pairwise direction agreement within ~14° (`cos ≥ 0.97`), spanning ≥400 ticks (a habit, not a burst), and with ≥6 aggregated barrier-terrain (water/rock) sample hits across the legs. The migration detector records `(tick, direction, barrier_hits)` per species, sampling 16 points along each unwrapped displacement. Tuning history: direction+span alone fired ~1000×/run (every persistent drifter qualifies); the barrier requirement is the actual corridor semantics — passage through hostile ground.
- **`Succession`** — per fire site (scorched cell list, cap 200 cells, 8 sites): ≥50% of scorched cells **vegetated again (Pioneer or Climax)** → fire once with the epicenter as loc. Pioneer counts because grazers crop pioneer growth below its ceiling — requiring full Climax never completes under real grazing pressure (observed 0 completions before the refinement).

## 5. Wiring & viewer

- `EventType` 27–30; `score.rs` names 31 + weights at bonus; `sweep.rs` CSV +4; `codex_panel.gd` names/colors +4.
- gdext: `succession_colors() -> PackedColorArray` (per-cell: bare → scorched umber, pioneer → light green, climax → transparent; active disaster cells tint fire-orange / drought-sepia / freeze-pale). `game/`: new ground overlay mode appended to `GROUND_NAMES` ("succession"), overlay manager + biome renderer branch.
- `scenarios/disturbance.toml`: fire-prone grassland — two starter archetypes (generalist + specialist grazer), `disasters_enabled = true`. Menu entry.

## 6. Determinism & perf

- Flag off ⇒ `disaster_step` is a no-op, succession stays Climax everywhere, `regrow_step` arithmetic unchanged (succession multiplier resolves to ×1 on the Climax path — same operation order).
- `FORMAT_VERSION` 9→10: `BiomeCell.succession`, `World.{disasters_enabled, disasters}`, CodexState detector scratch. Golden regen once; behavior in golden scenarios (flag off) unchanged.
- Perf: disaster step touches ≤ π×32² cells worst case; detector set unions are per-tick scratch on the fused agg pass.

## 7. Testing & evidence

- Unit: scheduler determinism (same seed → same disaster sequence), fire scorch + Forest→Grass conversion, succession state machine transitions, each detector positive + negative.
- Integration: disturbance.toml long run fires ≥1 new type; replay-verify one Succession event.
- Sweep: 16 seeds × 6000 ticks; per-type counts in completion notes.
- Gallery: burn scar right after a fire vs the same region re-colonized ~800 ticks later (viewer capture, disaster overlay on).
