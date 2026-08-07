# Climate-driven procedural world generation

**Date:** 2026-07-27
**Status:** Approved design → implementation
**Branch:** `worktree-desert-tropical-world` (extends PR #74)

## Motivation

The current generator (`BiomeField::generate` in `crates/anabios-core/src/biome.rs`)
assigns terrain from a **single elevation noise value** via hard bands
(`elevation_to_terrain`): `0.30–0.45 → Desert`, `0.65–0.85 → Forest`, etc. The
separate per-cell climate field (`env`) is **completely decoupled** from terrain.
Consequences:

- "Desert" means *mid elevation*, not *hot and dry*. Biomes have no physical basis.
- No way to *request* a desert or tropical zone — PR #74 had to seed-scout for one.
- Raw **value noise + bilinear interpolation** → blocky, grid-aligned, seam-at-edges
  features (the world is a torus, but the noise does not wrap).
- Two octaves only; no fractal detail, no organic landmasses.

This spec replaces the generator with a **climate-driven pipeline**: compute
elevation, temperature, and moisture per cell, then classify biome with a
**Whittaker matrix**. The key realism insight is that deserts and rainforests are
**latitude phenomena** — a latitude profile (wet hot equator, dry hot subtropics,
wet temperate, cold dry poles) makes *every* seed contain both deserts and tropics
by construction.

## Scope decisions (locked)

| Decision | Choice |
|----------|--------|
| Integration | **Replace** the default generator (not opt-in) |
| Terrain set | **Expand** the `TerrainType` enum |
| Ambition | **Core realistic pipeline** (no erosion/rivers/rain-shadow) |
| Biome borders | **Hard thresholds** (discrete Whittaker cells) |
| Climate params | **Fixed compile-time constants** (no scenario knobs this pass) |

Out of scope (possible follow-ups): hydraulic erosion, rivers, orographic
rain-shadow, plate/continent structure, scenario-tunable climate knobs.

## Generation pipeline

All noise is hand-rolled and deterministic from the world seed, using the existing
`Rng` — **no new dependencies**. Draw order is fixed; changing it rehashes goldens.

### 1. Gradient noise + fBm (replaces value noise)

`GradientNoise`: random unit gradient vectors at integer grid corners; value at a
point = smoothstep-interpolated dot products of corner gradients with offset vectors
(Perlin construction). `fbm(p, octaves, lacunarity, persistence)` sums octaves.

**Seamless on the torus:** corner indices wrap modulo the grid period so the field is
continuous across `x=0↔WORLD_SIZE` and `y=0↔WORLD_SIZE` (fixes today's edge seam).

### 2. Domain warping

`warp(p) = p + WARP_AMP * (fbm(p + off_a), fbm(p + off_b))` (one or two iterations,
Inigo-Quilez style). Elevation/moisture are sampled at warped coordinates so
landmasses are organic rather than grid-aligned.

### 3. Elevation

`elevation = redistribute(fbm(warp(p)))`, normalized to `[0,1]`. `redistribute` is a
power curve (`e^k`) that flattens lowlands and sharpens peaks. `SEA_LEVEL` and
`ROCK_LINE` constants gate Water (below sea) and Rock (above rock line).

### 4. Temperature

```
lat(v)      = 1 - |2v - 1|                    # 1 at equator (v=0.5), 0 at poles
temperature = clamp( lat(v)
                     - LAPSE * max(0, elevation - SEA_LEVEL)
                     + TEMP_NOISE * fbm_temp(p), 0, 1 )
```

`lat(v)` is toroidally continuous (both y-edges are poles, temperature 0). Higher
land is colder (lapse rate). A small noise term breaks up perfectly straight bands.

### 5. Moisture

```
mlat(v)  = 0.5 + 0.5*cos(3π(2v-1))            # wet equator, dry subtropics, wet temperate, dry poles
moisture = clamp( 0.5*mlat(v)
                  + 0.35*fbm_moist(warp(p))
                  + 0.15*coastal(p), 0, 1 )
```

`coastal(p)` = fraction of Water cells in a small neighborhood (cheap; wetter near
coasts). The `mlat` band model is what produces **subtropical deserts** (dry ~30°)
and an **equatorial rainforest belt** (wet ~0°).

## Terrain set (expanded)

`TerrainType` keeps existing discriminants (Water=0, Grass=1, Forest=2, Desert=3,
Rock=4) and appends new land biomes to avoid renumbering:

```
Water = 0, Grass = 1, Forest = 2, Desert = 3, Rock = 4,
Savanna = 5, Rainforest = 6, Taiga = 7, Tundra = 8
```

Semantics: `Grass` = temperate grassland, `Forest` = temperate forest,
`Rainforest` = tropical (hot+wet), `Savanna` = hot+moderate, `Taiga` = boreal
forest, `Tundra` = cold+dry, `Rock`/`Water` = barren.

### Productivity (`carrying_capacity`, energy units) and `regrowth_rate`

| Terrain | carrying_capacity | regrowth_rate |
|---------|------|--------|
| Water | 0 | 0 |
| Rock | 0 | 0 |
| Desert | 3 | 0.002 |
| Tundra | 4 | 0.002 |
| Savanna | 8 | 0.006 |
| Grass | 10 | 0.01 |
| Taiga | 12 | 0.004 |
| Forest | 20 | 0.003 |
| Rainforest | 28 | 0.004 |

(Existing values for Water/Grass/Forest/Desert/Rock are unchanged; new biomes
interpolate sensibly. Final numbers may be tuned during implementation but stay in
this ordering.)

## Whittaker classification

Applied per cell after computing (elevation, temperature, moisture):

```
if elevation < SEA_LEVEL        -> Water
else if elevation > ROCK_LINE   -> Rock
else classify by (temperature, moisture) bands:   # cutoffs at 0.33 / 0.66
    hot  (t>0.66):  arid->Desert     moderate->Savanna   wet->Rainforest
    temp (0.33..):  arid->Grass      moderate->Forest    wet->Forest
    cold (t<0.33):  arid->Tundra     moderate->Taiga     wet->Taiga
```

Hard thresholds — no border dithering. Cutoffs are named constants.

## Downstream integration (blast radius)

### BiomeCell schema
- Add field `moisture: f32`. **Repurpose `env` to mean temperature** (the climate
  axis read by season/`biome_adaptation`/`culture::env_optimum_at`); document the new
  semantics. Net: **one new serialized field**. Not `#[serde(skip)]` (it feeds the
  hashed state; skipping would break replay — see the serde-skip determinism footgun).
- **Bump `FORMAT_VERSION` 19 → 20** (`snapshot.rs`); add a changelog line.

### Golden hashes
`state_hash` = FNV1a over the full bincode-serialized world, so both the new field
and the changed terrain distribution rehash **all three golden suites**
(`determinism.rs`, `inventions.rs`, `cognition.rs`) and the `trade.rs` hashes.
Regenerate via the `UPDATE_HASHES` env flow. This is expected and accepted.

### Exhaustive `TerrainType` matches to update (~17 files)
- `biome.rs`: `carrying_capacity`, `regrowth_rate`, ASCII/debug if any.
- `resource.rs`: `Good::from_terrain` / `home_terrain` (see trade remap below).
- `crates/anabios-godot/src/lib.rs`: `biome_colors` (add colors for the 4 new biomes).
- `disaster.rs`, `sense.rs`, `iq.rs`, `codex/*` (`population.rs`, `signatures.rs`,
  `mod.rs`): any `match` on terrain gains arms for the new variants.

### Trade / goods remap
Keep **4 goods** (Salt, Obsidian, Amber, Spice). `from_terrain` maps all 7 land
terrains onto them; `home_terrain` returns the **representative** terrain so the
`home_terrain ∘ from_terrain == identity-on-goods` invariant (test
`home_terrain_inverts_from_terrain`) holds:

```
from_terrain:  Desert->Salt   Rock->Obsidian
               Forest,Rainforest,Taiga->Amber
               Grass,Savanna,Tundra->Spice     Water->None
home_terrain:  Salt->Desert   Obsidian->Rock   Amber->Forest   Spice->Grass
```

`preferred_good` (4 equal bands) and `terrain_affinity`→terrain are unchanged
(agents still target the 4 representative terrains). New biomes can host resource
nodes but are not new `terrain_affinity` targets.

### Scenarios
- **`scenarios/experiments/desert-tropical.toml`**: drop seed-scouting. Biomes are now guaranteed
  by latitude, so place the desert cohort in the subtropical band (`v≈0.33` →
  `y≈341`) and the tropical cohort at the equator (`v≈0.5` → `y≈512`). Update the
  header comment to describe the climate model, not the scout.
- **Other ~40 scenarios**: worlds change but scenarios still run. Spot-check that
  none assert on specific terrain via tests; behavioral tests referencing terrain
  (`nutrient_fertility.rs`, `dims.rs`, `trade.rs`, `combat_predation.rs`,
  `module_gating.rs`, `cognition_evolution.rs`) are re-run and updated where they
  encode old-terrain assumptions.
- **`examples/biome_scout.rs`**: update the terrain legend/scoring for the new set;
  it becomes a "view this seed's biomes + latitude bands" tool (still useful),
  scoring tropical as `Rainforest` directly rather than `Forest`+env.

## Testing

New unit tests (in `biome.rs` and/or `tests/`):
- **Noise**: gradient-noise output is bounded `[0,1]`, continuous, and **tileable**
  (value at `x=0` equals value at `x=WORLD_SIZE`); fBm bounded; domain warp
  deterministic.
- **Temperature**: equatorial mean > polar mean; high-elevation cells colder than
  low at the same latitude.
- **Moisture**: subtropical-band mean < equatorial-band mean (dry ~30°, wet ~0°).
- **Classification**: hot+arid classifies Desert; hot+wet classifies Rainforest;
  cold classifies Tundra/Taiga.
- **Guarantee**: for a sample of seeds, every world contains **both Desert and
  Rainforest**, and all 7 land terrains appear across seeds.
- **Determinism**: same seed → byte-identical `BiomeField`; `save → load → step`
  is stable (moisture survives the round-trip and does not perturb the hash path).

Then: `cargo test` (regenerate goldens via `UPDATE_HASHES`), `cargo fmt --check`,
`cargo clippy`, and a smoke run of `desert-tropical.toml` confirming a stable,
capped population and a deterministic `state_hash`.

## Constants (initial values, tunable in impl)

```
SEA_LEVEL      = 0.40     ROCK_LINE   = 0.82
LAPSE          = 0.55     TEMP_NOISE  = 0.10
WARP_AMP       = 0.35     OCTAVES     = 5   (lacunarity 2.0, persistence 0.5)
TEMP_BAND cutoffs 0.33 / 0.66     MOIST_BAND cutoffs 0.33 / 0.66
```

## Milestone / delivery

Follows the repo's determinism-milestone discipline: implement in ordered,
independently-testable units (noise → warp → fields → classification → terrain enum
& matches → trade remap → scenario/scout → golden regen), TDD per unit, single PR
on top of #74.
