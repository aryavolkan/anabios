# Geographic Trade Routes — Implementation Plan

**Goal:** Species sort into different biome terrains (each terrain yields a distinct trade good) via a `TerrainAffinity` gene + a terrain-based habitat pull, so specialization is geographic and trade happens where territories border. Builds on the shipped biome-trade economy.

**Approach:** Mirror the existing climate habitat selection (`EnvAffinity` + `best_env_direction` under `biome_adaptation`) for terrain. Rename reserved genome slot 42 → `TerrainAffinity` (no serialization change). Add a `best_terrain_direction` pull applied in `decide_all` under a new `terrain_habitat` flag. Pin distinct per-species affinities in a new scenario. All new reads gated on the flag → existing scenarios byte-identical.

**Determinism:** `TerrainAffinity` at slot 42 is already mutated (reserved-but-perturbed) and stays neutral (0.5) in existing scenarios → speciation `distance()` and golden hashes unchanged. The ONE layout change is `World.terrain_habitat: bool` → `FORMAT_VERSION` 6→7 + regenerate the two golden suites ONCE (Task G3). Every terrain-pull read is gated on `terrain_habitat`.

## Global Constraints
- Single RNG stream; iterate `iter_alive()` ascending; no `HashMap`/`HashSet` in sim paths.
- `terrain_habitat` off ⇒ zero behavior change (byte-identical trajectory); only the serialized layout grows (Task G3).
- Genome array length stays 50; slot 42 renamed only (positional serde).
- Terrain→good mapping: Desert=Salt, Rock=Obsidian, Forest=Amber, Grass=Spice (Water=none), matching the shipped `Good::from_terrain`.

---

## Task G1: `TerrainAffinity` slot + Good↔terrain mapping
**Files:** `genome.rs` (rename slot 42), `resource.rs` (`Good::home_terrain`, `preferred_good`). No serialization/golden impact.
- Rename `GenomeSlot::_SensoryReserved42 = 42` → `TerrainAffinity = 42`.
- `Good::home_terrain(self) -> TerrainType`: Salt→Desert, Obsidian→Rock, Amber→Forest, Spice→Grass.
- `preferred_good(affinity: f32) -> Good = Good::ALL[((affinity * GOOD_COUNT as f32) as usize).min(GOOD_COUNT - 1)]` (4 equal bands over [0,1]).
- Tests: `home_terrain` round-trips with `from_terrain`; `preferred_good` band boundaries (0.0→Salt, 0.3→Obsidian, 0.6→Amber, 0.9→Spice, 1.0→Spice clamped).

## Task G2: `best_terrain_direction` + pull constants
**Files:** `biome.rs` (`best_terrain_direction`), `culture.rs` (constants). No golden impact (unused until G4).
- `pub fn best_terrain_direction(biome: &BiomeField, pos: Vec2, target: TerrainType, radius: f32) -> Vec2` — mirror `best_env_direction` exactly, but the per-cell error is `0.0` if `cell.terrain == target` else `1.0`; return unit direction toward the nearest matching cell (or `Vec2::ZERO` if the current cell already matches / none found). Deterministic, no RNG, strict-min with the same tie-break shape as `best_env_direction`.
- Constants in `culture.rs`: `TERRAIN_HABITAT_REACH: f32 = 48.0`, `TERRAIN_HABITAT_PULL: f32 = 1.0` (tunable in G5).
- Test: on a synthetic/real biome, an agent off its target terrain gets a non-zero direction; an agent already on target gets `ZERO` (or toward-nearest per the mirror's semantics — match `best_env_direction`'s exact behavior).

## Task G3: `terrain_habitat` flag + FORMAT_VERSION bump + regen goldens
**Files:** `world.rs` (`terrain_habitat: bool` `#[serde(default)]`), `scenario.rs` (`Scenario.terrain_habitat` + wire in `instantiate`; `TraitOverrides.terrain_affinity: Option<f32>` + `apply`), `snapshot.rs` (`FORMAT_VERSION` 6→7 + doc line), `determinism.rs`/`inventions.rs` (regenerate goldens once, add changelog note). This is the single golden-regen task.
- Add `World.terrain_habitat`, init false in `new`/`with_dims`.
- `Scenario.terrain_habitat` (`#[serde(default)]`) → `w.terrain_habitat = self.terrain_habitat` in `instantiate`.
- `TraitOverrides.terrain_affinity: Option<f32>` + `if let Some(v) { g.set(GenomeSlot::TerrainAffinity, v) }` in `apply`.
- Bump `FORMAT_VERSION` 6→7; regenerate `GOLDEN`/`INVENTIONS_GOLDEN` with `UPDATE_HASHES=1`; document the bump (behavior unchanged with flag off — only the serialized `terrain_habitat` byte grew).
- Test: `terrain_habitat`/`terrain_affinity` parse + wire; flag defaults false; snapshot roundtrip.

## Task G4: apply terrain habitat pull in `decide_all`
**Files:** `tick.rs`. Gated on `terrain_habitat` (capture like `biome_adaptation`). Flag off ⇒ goldens unchanged.
- In `decide_all`'s closure, after the existing `biome_adaptation` block:
```rust
if terrain_habitat {
    let aff = agents.genome[i].get(crate::genome::GenomeSlot::TerrainAffinity);
    let target = crate::resource::preferred_good(aff).home_terrain();
    let pull = crate::biome::best_terrain_direction(biome, agents.position[i], target, crate::culture::TERRAIN_HABITAT_REACH);
    action.move_x += crate::culture::TERRAIN_HABITAT_PULL * pull.x;
    action.move_y += crate::culture::TERRAIN_HABITAT_PULL * pull.y;
}
```
- Capture `terrain_habitat` in the closure. Test: with the flag on and a pinned affinity, an agent's `desired_direction` biases toward its target terrain over a few ticks (or unit-test the pull is applied).

## Task G5: `geographic-trade.toml` + integration test + tune
**Files:** new `scenarios/geographic-trade.toml`, new/extended `tests/trade.rs` cases.
- Scenario: 4 species, each pinned to a distinct terrain via `terrain_affinity` (~0.12 Salt, ~0.37 Obsidian, ~0.62 Amber, ~0.87 Spice), `terrain_habitat = true`, `resources_enabled = true`, spread placement (uniform or broad clusters) so they can sort. Keep in-bounds.
- Tests: (a) determinism (two runs' `state_hash` match at tick N); (b) turnover — ≥1 `ResourceTraded` AND ≥1 `DowryBirth` honestly; (c) geographic sorting — after M ticks, each species' members are predominantly on their target terrain (mean terrain-match above a threshold), proving the cline formed.
- Tune `TERRAIN_HABITAT_PULL`/reach, population, affinity spread so sorting AND border trade both happen. Effort-bounded; if trade can't co-occur with sorting, report and we reconsider pull strength (sorting vs. contact tension).

## Verify (each task)
`cargo test -p anabios-core` green; `--test determinism --test inventions` unchanged after G3's regen and untouched by G1/G2/G4/G5; fmt + clippy clean.
