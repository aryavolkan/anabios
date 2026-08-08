# Continental worldgen (large-scale "actual world")

**Date:** 2026-08-07
**Status:** Approved design → implementation
**Branch:** `claude/2d-world-scale-159e3b`

## Context: the larger arc

This spec is **sub-project A** of a four-part effort to build "an actual world with
good scale in 2D." The full decomposition (each its own spec → plan → PR):

- **A · Continental worldgen (this spec)** — geography realism: continents, oceans,
  mountain ranges, rain-shadow, rivers. The foundation the rest sits on.
- **B · Scale-up (engine capacity)** — largely subsumed: the target is a **large
  world with a small population**, which makes the biome grid (not agent count) the
  only cost surface, and that is cheap (see §Performance). B shrinks to "profile if
  the flagship scenario ever feels slow."
- **C · Viewer at scale** — multi-zoom, minimap, shaded relief + river rendering,
  agent LOD. Consumes the `elevation`/`river_flow` fields this spec stores.
- **D · Emergence room (experiment)** — test whether geographic separation + scale
  unblocks the culture-exclusion problem (the OoA climb). Hypothesis-driven and
  historically fragile; starts as a cheap throwaway probe, comes last.

Only **A** is designed here.

## Motivation

The current generator (`BiomeField::generate_with` in
`crates/anabios-core/src/biome.rs`) is a good climate-driven pipeline — gradient-noise
fBm with domain warping for elevation, latitude temperature/moisture bands, Whittaker
classification into 9 terrains, all seamless on the torus. But land/water comes from a
**single sea-level threshold on fBm elevation**, so:

- Land and ocean are **speckled** — there are no distinct continents and oceans.
- High elevation (→ Rock) is **scattered**, not organized into mountain **ranges**.
- There are **no rivers** — the single most recognizable "real planet" feature.
- Moisture ignores terrain: **no rain-shadow** (dry lee slopes behind ranges).

This spec adds four geography passes so a world reads as a recognizable planet, and
ships a flagship **large** world (a small population dwarfed by a continent-scale map).

## Scope decisions (locked)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Topology | **Keep the torus** | Distance/movement/spatial-hash/seamless-noise all assume wrap; continents in ocean with open-ocean wrap edges read fine. Changing topology is enormous blast radius for no gameplay gain. |
| Integration | **`ClimateParams` knobs, OFF by default** (opt-in per scenario) | Continental features only make sense at large scale; existing scenarios stay behavior-identical. Mirrors the ClimateParams precedent. |
| Rivers | **Moisture corridors, not terrain** (passable) | A small population in a huge world must never be stranded/fragmented by impassable water. |
| Rivers hydrology | **Descending-elevation flow accumulation, no pit-filling** | Deterministic (no RNG), simple; endorheic pits terminate as sinks. Priority-flood is a later refinement. |
| Rain-shadow | **Single global prevailing wind** | Simple, deterministic. Latitude-varying trade winds are a later refinement. |
| Scale | Default stays **1024/128**; flagship **`continental.toml` = 4096/512** | Existing scenarios untouched; the "large world" is a new opt-in scenario. |
| Ambition | Continents + mountains + rain-shadow + rivers | No erosion, no lakes/pit-filling, no plate simulation this pass. |

Out of scope (possible follow-ups): hydraulic erosion, pit-filling/lakes, latitude-
varying winds, rivers as impassable water or drinkable resources, plate simulation,
`continent_id` for continent-aware seeding (deferred to sub-project D).

## Generation pipeline additions

Layered onto the existing `generate_with`. New noise fields are **appended to the
fixed draw order** (draw order is part of the determinism contract). Each geography
pass runs **only when its knob is > 0.0**, so a scenario with knobs off draws the
**identical RNG stream** as today and generates a behavior-identical world.

New `ClimateParams` knobs (all default `0.0` = today's behavior):

```
continentality   # 0 = fBm speckle (today); >0 pulls land into continents
mountain_uplift  # 0 = scattered peaks (today); >0 builds linear ranges
rain_shadow      # 0 = no orographic drying; >0 dries lee slopes
river_threshold  # 0 = no rivers; >0 = min flow accumulation for a river cell
                 #   (a "rivers off" sentinel; positive turns hydrology on)
```

### 1. Continent mask
A very-low-frequency fBm (`continent_noise`) sampled on warped coordinates yields a
continentality field. Blend it into elevation so land clusters:

```
mask      = continent_noise.sample(warp(p))            # ~[0,1], low frequency
elevation = lerp(DEEP_OCEAN_ELEV, elevation, lerp(1.0, mask, continentality))
```

At `continentality = 0` this is exactly today's elevation; at `1.0`, ocean basins open
between a handful of landmasses. Tune the continent-noise period so a 4096 world holds
~2–5 continents.

### 2. Mountain ranges
Ridged noise concentrates uplift into linear belts, weighted toward continent interiors
(so ranges sit on land, not mid-ocean):

```
ridge     = 1 - |2 * mountain_noise.sample(warp(p)) - 1|   # ridged, in [0,1]
elevation = clamp(elevation + mountain_uplift * ridge * mask, 0, 1)
```

Raising elevation before classification means ridge crests cross `ROCK_LINE` in
connected lines → mountain **ranges** (and colder, via the existing lapse term).

### 3. Rain-shadow
One global prevailing wind direction `WIND` (e.g. westerly). Each cell samples upwind
elevation at a short offset and loses moisture behind higher terrain:

```
upwind_elev = elevation at (p - WIND * SHADOW_DIST)      # torus-wrapped sample
moisture   -= rain_shadow * max(0, upwind_elev - own_elev)
moisture    = clamp(moisture, 0, 1)
```

Windward slopes stay wet; lee slopes dry out. Applied **before** classification, so it
shifts terrain (lee Forest→Grass→Desert). Deterministic, no RNG.

### 4. Rivers (hydrology)
Deterministic post-process on the finished elevation field — **no RNG**:

1. **Downhill routing:** for each land cell, pick the steepest-descent neighbor among
   the 8 (torus-wrapped). Cells with no lower neighbor are sinks (endorheic).
2. **Flow accumulation:** process cells in **descending elevation order** (stable
   tie-break by cell index for determinism). Each cell adds its accumulated flow
   (starting at 1 per cell, optionally weighted by moisture) to its downhill neighbor.
3. **River cells:** land cells with accumulation ≥ `river_threshold`. Store the
   normalized accumulation in `river_flow`.
4. **Riparian moisture + reclassify:** hydrology is a **whole-grid post-pass** (it needs
   every cell's final elevation), so it runs *after* the per-cell classification loop.
   River cells and their immediate neighbors gain moisture, then those affected cells
   are **re-run through `classify`** so terrain reflects the river (emergent riparian
   forests along the banks). Cells untouched by rivers keep their first-pass terrain.

Rivers stay **passable** — no `carrying_capacity` change, no new `TerrainType` variant,
so the ~17-file exhaustive-terrain-match churn is avoided entirely.

## Schema & determinism (blast radius)

### BiomeCell schema
Add **two serialized `f32` fields**:

- `elevation: f32` — already computed during generation; now **stored** (behavior-
  neutral) so the viewer (sub-project C) can render shaded relief without a second
  format bump. `#[serde(default)]`.
- `river_flow: f32` — normalized flow accumulation; `0.0` when rivers are off.
  `#[serde(default)]`.

Both **feed hashed state** (NOT `#[serde(skip)]` — skipping a field that varies by
world would break `save→load→step` replay; see the serde-skip determinism footgun).

### FORMAT_VERSION + goldens
- **Bump `FORMAT_VERSION`** in `snapshot.rs` + changelog line.
- `state_hash` is FNV1a over the full bincode-serialized world, so the two new fields
  rehash **all three golden suites** (`determinism.rs`, `inventions.rs`, `cognition.rs`)
  and the `trade.rs` hashes. Regenerate via `UPDATE_HASHES=1`. **Expected and accepted.**
- **Existing scenarios are behavior-identical**: with knobs off, terrain / moisture /
  plant_biomass trajectories are byte-identical in *value*; only the added bytes
  (real `elevation`, `river_flow = 0`) move the hashes. Every determinism/behavior test
  still passes after the one-time regen.

### No new exhaustive terrain matches
Because rivers are a field, not a `TerrainType`, the generator is the only code that
changes. `carrying_capacity`, `regrowth_rate`, `biome_colors`, and all `match`-on-
terrain sites are untouched.

## Flagship scenario & scale

New **`scenarios/continental.toml`**:

- `world_size = 4096`, `biome_res = 512` (cell_size 8, matching today; 262,144 cells).
- `continentality`, `mountain_uplift`, `rain_shadow`, `river_threshold` turned on.
- A **small founder cohort clustered on one continent near a river.** A placement
  helper scans for a high-carrying-capacity land region (near river cells) and seeds
  founders there, so the tiny population isn't spawned into ocean and can actually
  find food and each other.
- `hash_res` scaled so `world_size / hash_res` stays ~16 (perception-cap invariant),
  e.g. `hash_res = 256`.

The compile-time defaults (1024/128) are unchanged; all ~40 existing scenarios keep
their world and behavior.

## Performance

With **few agents in a large world**, agent-side cost (sense/decide/interact/spatial
hash) is negligible; the only new cost surface is the biome grid:

- **Generation (one-time):** continent + mountain fBm (low frequency, cheap),
  rain-shadow upwind sample (O(cells)), river routing + accumulation (sort 262k cells +
  one pass). Sub-second at 512².
- **Per tick:** `river_flow`/`elevation` are static, read only by the viewer.
  Regrowth/recolonize already run every `BIOME_STEP_INTERVAL = 10` ticks; pheromone
  decay short-circuits on an empty field (which a sparse population leaves it). No new
  per-tick work.
- **Memory/snapshot:** +2 f32/cell = +2 MB at 262k cells (~9 MB biome total). Negligible.

## Testing

New/updated tests (in `biome.rs` and/or `tests/`):

- **Continents:** at `continentality > 0`, land forms a few large connected components
  (4-neighbour flood fill) vs. many speckle components in a `continentality = 0` control.
- **Mountains:** with `mountain_uplift > 0`, Rock/high-elevation cells form connected
  ranges (component length distribution); ranges appear across a sample of seeds.
- **Rain-shadow:** mean moisture on the lee side of a range < mean on the windward side.
- **Rivers:** ≥ K river cells; every river cell has a strictly-downhill downstream
  neighbor whose chain terminates in Water or a sink; river cells raise local moisture
  vs. matched non-river cells.
- **Guarantee across seeds:** every continental-config world contains continents,
  ≥1 mountain range, and ≥1 river reaching the sea.
- **Identity (knobs off):** a `continentality=mountain_uplift=rain_shadow=
  river_threshold=0` world has byte-identical terrain/moisture/plant_biomass **values**
  to the pre-change generator (RNG stream unchanged), with `river_flow = 0` everywhere.
- **Determinism:** same seed → byte-identical `BiomeField`; `save → load → step` stable
  (both new fields survive the round-trip and don't perturb the hash path).

**Iteration tool:** extend `examples/biome_scout.rs` to dump a **PNG** (terrain colors +
rivers overlaid + optional relief shading from `elevation`) so the world can be
eyeballed while tuning knobs — a cheap visual probe in the measure-first spirit, before
committing to constant values.

Then: `cargo test` (regenerate goldens via `UPDATE_HASHES`), `cargo fmt --check`,
`cargo clippy`, and a smoke run of `continental.toml` confirming a stable, capped
population and a deterministic `state_hash`.

## Constants (initial values, tunable in impl via the PNG probe)

```
DEEP_OCEAN_ELEV   = 0.15      # elevation land is pulled down to in ocean basins
continent period  ~ 2         # low-frequency continent-mask octave base period
SHADOW_DIST       = 4 cells   # upwind sample distance for rain-shadow
WIND              = (+1, 0)   # global prevailing wind (westerly)
river accumulation flow-per-cell = 1.0 (optionally moisture-weighted)
```

Flagship `continental.toml` knob values (tuned via the PNG probe during impl):

```
continentality ~ 0.8   mountain_uplift ~ 0.5   rain_shadow ~ 0.4
river_threshold: min accumulation for a river (tuned to a legible river density)
```

## Milestone / delivery

Follows the repo's determinism-milestone discipline: implement in ordered,
independently-testable units (continent mask → mountain ranges → rain-shadow →
river hydrology + fields → schema/FORMAT_VERSION + golden regen → PNG scout →
flagship scenario), TDD per unit, single PR on the branch.
