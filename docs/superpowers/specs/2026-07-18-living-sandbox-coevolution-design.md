# Living Sandbox for Culture-Gene Coevolution — Design Spec

**Date:** 2026-07-18
**Status:** Design (pre-plan)
**Topic:** Procedurally-generated living biome + large-sandbox demonstration of a culture-gene lineage selection differential.

## 1. Goal & success criterion

Demonstrate a **robust lineage selection differential**: in a large, procedurally-generated, *living* biome, a seeded **culture-carrying lineage** (Communicator module + learned cumulative foraging skill, i.e. experiment C's mechanism) **reliably out-reproduces a non-cultural control lineage** over a long run.

**Success is measured, not asserted by construction.** The deliverable is the mechanisms + an experiment harness + a run that reports the effect. Concretely (Phase 3):

- Over `T` ticks across `N` seeds (target `N = 10`), at run end the culture lineage's living-descendant count (and cumulative births) exceeds the control lineage's in **≥ 7 of 10 seeds**, with a positive mean log-ratio.
- The differential is **stronger with `living_biome` ON than OFF** on the same world — this is the load-bearing evidence that the living biome (renewal + seasonality) is what rescues the advantage from the density-dependence trap, not just a re-run of the small-world result.

This is the **lineage-differential** bar, deliberately below the full gene-frequency-sweep bar (which the prior first-principles experiment B failed and which needs a genetic-assimilation channel — out of scope here).

## 2. Why this is hard (the failure modes we are attacking)

Prior work (`tests/gene_culture.rs`, the DIT boundary suite, and the two project memories) established that culturally-adaptive behaviour does **not** durably translate into gene-level selection here, for three compounding reasons:

1. **Density-dependence.** The cultural feeding bonus is a *multiplier* on the grazing bite (`desired_bite *= 1 + SKILL_BONUS * skill`, `interact.rs`). At carrying capacity the biome is depleted, so there is nothing to multiply, exactly where selection pressure is highest.
2. **Panmixia.** Freely-roaming agents experience the global-mean environment; a fitness differential alone produces no spatial structure (established by the biome-adaptation work — the fix there was `best_env_direction` habitat selection).
3. **No genetic assimilation (Baldwin) channel.** Confirmed absent in code; learned skill never biases inheritance. (Out of scope — deferred.)

The chosen bar (lineage differential, not gene sweep) sidesteps #3. This design attacks **#1 directly** (a renewing + larger world keeps productive resource available at density, so the multiplier keeps paying) and gives **#2** a temporal analog (seasonally migrating productive zones reward tracking, which culture — social learning — supports).

## 3. Non-negotiable constraints

- **Existing scenarios stay byte-identical.** The 14 scenarios + the DIT/gene-culture validated findings are calibrated to today's 1024-unit world. Every new capability is **opt-in**: gated behind a flag (`living_biome`) or a runtime field whose **default equals today's compile-time constant**. Flag-off / default runs must produce byte-identical agent trajectories (only the serialized `World` layout grows).
- **Golden-hash discipline.** `state_hash` bincode-serializes the entire `World`, so any new persistent field moves every golden hash even with identical behaviour. Each phase that adds a `World`/`BiomeField` field budgets **one deliberate, reviewed golden refresh** in `tests/determinism.rs`, and must independently verify (a separate check) that default-config trajectories are unchanged — a moved hash alone is not evidence of safety.
- **Perception/spatial invariant.** `PERCEPTION_MAX_RADIUS` must remain ≤ the spatial-hash cell size (`world_size / hash_res`), which is `debug_assert`-enforced at every spatial query. Runtime world dimensions must preserve this invariant by deriving the perception radius from the runtime values.
- **Determinism of new mechanics.** No new RNG draws in the hot path unless seeded through the existing `Rng`; recolonization/seasonal updates use fixed scan order and (where diffusion is involved) double-buffering so results are order-independent.
- **`FORMAT_VERSION`** (`snapshot.rs`) is bumped when the serialized layout changes.

## 4. Approach

**Runtime-configurable world (opt-in), NOT a global constant bump.** `WORLD_SIZE` (1024), `BIOME_RES` (128), `HASH_RES` (64) are compile-time consts wired into biome, spatial hash, and perception math. Promote them to **runtime fields on `World`** defaulting to today's values. Existing scenarios omit the knobs → unchanged; only the large-sandbox scenario opts into a bigger world. (Rejected alternative: enlarging the constants globally — less code, but changes/breaks every calibrated scenario and its golden findings.)

Three phases, one plan, built in sequence.

## 5. Phase 1 — Runtime world dimensions (enabler)

**Intent:** make world size, biome resolution, and spatial-hash resolution per-`World` runtime values, defaulting to today's constants, so a scenario can request a larger world without disturbing anyone else.

**Changes:**
- `World` gains `world_size: f32`, `biome_res: usize`, `hash_res: usize`, each `#[serde(default = "…")]` returning today's constant (1024.0 / 128 / 64). Keep the current consts renamed as `*_DEFAULT`.
- `BiomeField` stores `res: usize`, `world_size: f32`, `cell_size: f32` (computed once) instead of reading module consts. `generate(seed)` → `generate(seed, res, world_size)`. Every method (`cell_coords`, `graze`, `regrow_step`, `best_env_direction`) uses `self.*`. The `NoiseGrid` corner-sampling is unchanged, so at default `res` the generated field is **byte-identical**.
- `UniformSpatialHash` stores `res: usize`, `cell_size: f32`; construction derives them from `world_size`/`hash_res`. `PERCEPTION_MAX_RADIUS` becomes a runtime value = `hash_cell_size`, threaded to the `.min()` clamps in `culture.rs`/`interact.rs`/habitat code (or read back from the hash). `debug_assert!(radius ≤ cell_size)` uses the runtime cell size.
- `Scenario` TOML gains optional `world_size`, `biome_res`, `hash_res`; `instantiate()` applies them (falling back to defaults).
- Frontend accessors `world_size()` / `biome_resolution()` (`anabios-godot/src/lib.rs`) return `world.world_size` / `world.biome_res` instead of the consts. Frontend GDScript already reads these dynamically → no structural break.

**Determinism:** at default dimensions, generation and every per-tick computation are numerically identical (defaults equal the old consts; `cell_size = world_size/res` yields the same float). Only the `World` struct grows → **one golden refresh**; separately verify (assert) that the tick-1000 agent buffers match the pre-change run at defaults.

**Out-of-scope for P1:** any behaviour change; only plumbing.

## 6. Phase 2 — Living biome (flag-gated)

**Intent:** make the biome renew and shift so grazed patches recover and productive zones migrate — the environment that keeps the cultural multiplier valuable at density. Gated behind `World.living_biome: bool` (`#[serde(default)]` = false) plus `World.season_period: u32` (`#[serde(default)]` = 0 = off).

**6a. Renewing resources (recolonization + spread).** Today `regrow_step` leaves depleted cells (`biomass ≤ 0`) permanently dead. When `living_biome`:
- A depleted cell with positive carrying capacity (`K > 0`: Grass/Forest/Desert) recolonizes from vegetated 4- or 8-neighbours: `new_biomass += RECOLONIZE_RATE * mean_vegetated_neighbour_biomass`, capped at `K`.
- Computed with a **double buffer** (read the pre-step field, write the next) so the diffusion is order-independent and deterministic; fixed scan order.
- Runs on the existing `BIOME_STEP_INTERVAL` (every 10 ticks). Flag-off path is the current single-pass logistic regrowth, untouched.

**6b. Seasonal climate.** Today `BiomeCell.env ∈ [0,1]` is static. When `season_period > 0`, a global **season phase** `φ(tick) = triangle/sine over season_period` sweeps `[0,1]`. Per-cell regrowth (and effective capacity) is boosted when the cell's static `env` matches the current phase, via a triangular kernel `season_match(env, φ)` (peak 1, zero beyond a tolerance): `regrow *= 1 + SEASON_AMPLITUDE * season_match(env, φ)`. Effect: the band of most-productive cells **migrates across the world** as the season sweeps, so foragers must track it — a temporal niche that rewards social learning. `env` itself stays static (heritable-affinity semantics preserved); only its *expression* (productivity) is seasonal. Flag-off (`season_period == 0`) skips it.

**Determinism:** both paths are pure functions of `(field, tick)` with no RNG and fixed order. Flag-off (`living_biome == false`, `season_period == 0`) is byte-identical → the golden refresh here is purely the two new serialized fields; verify default trajectories unchanged.

**Parameters** (`RECOLONIZE_RATE`, `SEASON_AMPLITUDE`, season tolerance) start modest (avoid boom/bust and competitive exclusion — hard-won lessons: modest bonuses, per-tick not per-graze) and are tuned in Phase 3.

## 7. Phase 3 — Large-sandbox coevolution experiment (the payoff)

**Scenario** `scenarios/living-sandbox-coevolution.toml`: large world (target `world_size = 2048`, `biome_res = 256`, `hash_res = 128` — preserves `hash_cell_size = 16`), `living_biome = true`, `season_period` set (target ~2000 ticks), `max_population` raised (target ~8000). Two seeded agent specs, each a distinct cluster/lineage:
- **Culture lineage** — starter kit **+ Communicator** module (skill archetype, e.g. `skilled_forager`), so learned-by-doing + social-copy skill is active.
- **Control lineage** — identical kit **without** Communicator (no skill channel), matched count.

Both from the same standing genome distribution; the only manipulated variable is the Communicator/skill capacity. Lineages are distinguishable by their seed `lineage_id` cohort (reuse the head-to-head tracking already in `tests/gene_culture.rs`).

**Harness** `tests/living_sandbox.rs` (`#[ignore]`, release-run), mirroring the `gene_culture.rs`/DIT-boundary pattern:
- Run the scenario for `T` ticks across `N = 10` seeds.
- At run end, tally living descendants (and cumulative births) per seed cohort.
- Report the per-seed differential and the aggregate: culture wins in ≥ 7/10, positive mean log-ratio.
- **Control comparison:** run the same scenario with `living_biome = false` and confirm the differential is weaker/absent there — isolating the living biome as the cause.

**Frontend hooks (minimal):** extend the existing co-evolution sample (`coevo.rs` / `coevo_metrics`) with the two-lineage living-share so the differential is watchable live in the `[Y]` chart; add the large-sandbox scenario to the menu. No visual redesign.

## 8. Testing strategy

- **Per phase:** a determinism check that flag-off/default config reproduces pre-change tick-1000 agent state byte-for-byte (beyond the golden hash), plus the reviewed golden refresh for the new fields.
- **Phase 1:** unit coverage that `BiomeField`/`UniformSpatialHash` at non-default dimensions satisfy their invariants (cell coords in range, `radius ≤ cell_size`), and that a 2048/256/128 world generates and steps without panics.
- **Phase 2:** targeted tests — a depleted patch surrounded by vegetation recolonizes over N steps (flag-on) and stays dead (flag-off); the seasonal productive band's centroid moves as `φ` sweeps.
- **Phase 3:** the harness itself is the success metric; also add it to `tests/all_scenarios.rs` as a smoke regression (short run, no-panic).
- Existing `cargo test` + golden gate run throughout; `cargo fmt --check` + rustdoc `-D warnings` (the repo CI gates) must pass.

## 9. Risks & open questions (research uncertainty)

- **The differential may still be weak or negative** even with the living biome. This is a genuine research bet. The harness reports effect size; levers if fragile: tune `RECOLONIZE_RATE`/`SEASON_AMPLITUDE`/regrowth/population, then (last resort, separate project) the deferred Baldwin channel.
- **Performance at scale** (2048 world, 256² biome, 8000 agents): `decide_all` is rayon-parallel but `feed_pass`/`culture_step`/`regrow_step`/`species_step` are serial and become the bottleneck. Plan includes a throughput check; run length/population tuned to keep harness runtime practical. `best_env_direction` scan cost grows as `cell_size` shrinks — keep habitat selection off (or reach-scaled) in this scenario.
- **Boom/bust instability** from recolonization + seasonality — mitigated by modest parameters and flag isolation.
- **Frontend marshalling** (`biome_colors()` walks all `res²` cells per call) gets heavier at 256²; throttle/dirty-track if the frontend stutters (frontend-only, non-blocking for the experiment).

## 10. Out of scope

Genetic-assimilation / Baldwin channel (deferred — the next lever if the differential is fragile, and the requirement for the full gene-sweep bar); domestication; writing/meme persistence; frontend visual redesign; multi-threading the serial passes (only if perf blocks the harness).
