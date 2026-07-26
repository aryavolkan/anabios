# Varied Nutrient Value + Varied Soil Fertility — Design Spec

**Date:** 2026-07-25
**Status:** Design — pending review
**Goal:** Introduce spatially-varying food *quality* and soil *fertility* to `anabios-core` as the substrate for a **foraging selection experiment**, following the existing determinism contract.

## 1. Motivation

Today all food yields a fixed energy value (`FOOD_ENERGY_PER_BIOMASS = 4.0`) and the only spatial variation in food supply is discrete terrain carrying capacity plus the static climate `env` field. There is no smooth, cell-level variation in *how nutritious* food is, nor in *how fertile* the ground is.

For a foraging selection experiment we want two independent, spatially-structured landscapes so that agents which forage in richer areas out-reproduce those that do not. Selection acts through **existing sensing and the evolved movement network** — no new gene. The landscapes simply reshape the payoff surface; well-foraging lineages sort themselves toward rich patches.

## 2. Decisions (locked with the user)

| Decision | Choice |
|---|---|
| Nutrient value source | **Per-cell static quality field**, noise-generated at world-gen like `env`. Scales energy-per-bite. |
| Quality vs fertility coupling | **Two independent fields** (separate noise, distinct frequencies). |
| What fertility modulates | **Both** carrying capacity `K` and regrowth rate `r`. |
| Selection lever | **Reuse existing sensing** — no new genome slot; the evolved NN foragers sort themselves out. |
| Experiment framing | Foraging selection: does the population concentrate in rich patches over generations? |

## 3. New state

Two `f32` fields added to `BiomeCell` (`crates/anabios-core/src/biome.rs`):

```rust
pub struct BiomeCell {
    pub terrain: TerrainType,
    pub plant_biomass: f32,
    pub env: f32,
    pub pollution: f32,
    pub succession: u8,
    pub nutrient_quality: f32, // NEW — energy-per-bite multiplier, in [NUTRIENT_QUALITY_MIN, NUTRIENT_QUALITY_MAX]
    pub fertility: f32,        // NEW — capacity & regrowth multiplier, in [FERTILITY_MIN, FERTILITY_MAX]
}
```

Both are **serialized** (not `#[serde(skip)]`) and therefore part of the golden `state_hash`. They are set once at generation and are static thereafter (like `env`).

### Constants (tunables)

```rust
pub const NUTRIENT_QUALITY_MIN: f32 = 0.6;
pub const NUTRIENT_QUALITY_MAX: f32 = 1.4; // mean ~1.0 => global energy economy ~conserved
pub const FERTILITY_MIN: f32 = 0.5;
pub const FERTILITY_MAX: f32 = 1.5;        // mean ~1.0 => global productivity ~conserved
```

Both fields map their `[0,1]` noise sample linearly into `[MIN, MAX]`. Means are centered near 1.0 so that flag-on worlds have roughly the same *total* food/energy as baseline — the experiment is about *spatial redistribution*, not a global buff/nerf.

## 4. Generation (determinism-critical)

In `BiomeField::generate` (`biome.rs:148`), **append** two new `NoiseGrid`s *after* the existing terrain and climate grids, so terrain and `env` generation consume the RNG in the same order as today (the underlying worlds are unchanged; we only overlay two new fields):

```rust
// ... existing: coarse(8), fine(24), climate_coarse(3), climate_fine(9) ...
// Appended AFTER climate so existing draws are byte-identical.
let nutrient_coarse = NoiseGrid::new(&mut rng, 5);
let nutrient_fine   = NoiseGrid::new(&mut rng, 13);
let fertility_coarse = NoiseGrid::new(&mut rng, 4);
let fertility_fine   = NoiseGrid::new(&mut rng, 11);
```

Per cell:

```rust
let nq = 0.8 * nutrient_coarse.sample(u, v) + 0.2 * nutrient_fine.sample(u, v);
let nutrient_quality = lerp(NUTRIENT_QUALITY_MIN, NUTRIENT_QUALITY_MAX, nq.clamp(0.0, 1.0));
let fe = 0.8 * fertility_coarse.sample(u, v) + 0.2 * fertility_fine.sample(u, v);
let fertility = lerp(FERTILITY_MIN, FERTILITY_MAX, fe.clamp(0.0, 1.0));
```

Frequencies (5/13 and 4/11) are distinct from `env` (3/9) and from each other, so the three landscapes are spatially uncorrelated. They are **patchy** (patches larger than a single agent's step) but finer than `env`, so within-lifetime foraging choices are meaningful. Frequencies + ranges are the primary knobs to tune experiment strength.

`plant_biomass` still seeds at `terrain.carrying_capacity()` (unchanged) — `generate()` stays flag-agnostic. When `soil_fertility` is on, standing crop relaxes to `K_eff = capacity * fertility` via the logistic term over the first few biome steps; no need to thread the flag into generation.

## 5. Consumption / integration

### 5.1 Nutrient quality → energy per bite (`interact.rs` `feed_pass`, ~line 129)

`taken` (biomass removed) is **unchanged**. Only the energy conversion is scaled:

```rust
let quality_mult = if world.nutrient_variation {
    world.biome.sample(pos).nutrient_quality
} else {
    1.0
};
let taken = world.biome.graze(pos, desired_bite);
if taken > 0.0 {
    world.agents.energy[i] += taken
        * FOOD_ENERGY_PER_BIOMASS
        * crate::invention::food_energy_multiplier(inv_mask)
        * quality_mult;
    // ... existing skill-learn ...
}
```

Same bite, less energy in poor cells → the foraging pressure. Composes multiplicatively with all existing bonuses.

The same treatment is applied to **carcass/scavenging** (`interact.rs` `scavenge_pass`) only if we decide meat should inherit the local cell's quality — **default: NO**, scavenging keeps flat `FLESH_ENERGY_PER_UNIT`. (Nutrient variation is a *plant/terrain* property; meat energy is set by the carcass. Noted here so the choice is explicit; revisit if the experiment wants it.)

### 5.2 Fertility → carrying capacity + regrowth (`biome.rs`)

Thread the flag into the regrowth entry points and compute a per-cell `fert`:

```rust
pub fn regrow_step(&mut self, soil_fertility: bool) { /* fert = if soil_fertility {cell.fertility} else {1.0} */ }
pub fn regrow_step_seasonal(&mut self, phase: f32, soil_fertility: bool) { ... }
pub fn recolonize_step(&mut self, soil_fertility: bool) { ... }
```

Inside `regrow_succession` / `regrow_climax`, apply `fert` to **both** capacity and rate:

```rust
let base_cap = cell.terrain.carrying_capacity();
if base_cap <= 0.0 { return; }        // Water/Rock stay barren (fert > 0 keeps this valid)
let capacity = base_cap * fert;       // K_eff
let r = cell.terrain.regrowth_rate() * pollution_mult(cell) * rate_mult * fert; // r_eff
```

`recolonize_step` likewise scales its `cap` ceiling by `fert`. Pioneer `pcap = capacity * PIONEER_CAPACITY_MULT` inherits the already-scaled capacity.

### 5.3 Tick wiring (`tick.rs:102-115`)

Pass `world.soil_fertility` into `regrow_step` / `regrow_step_seasonal` / `recolonize_step` at their existing call sites. No cadence change.

## 6. Feature flags (`world.rs`)

```rust
#[serde(default)] pub nutrient_variation: bool, // default false
#[serde(default)] pub soil_fertility: bool,     // default false
```

Separate flags so each landscape can be exercised independently. Doc comments follow the established style: *"Zero RNG. When off, the corresponding multiplier is forced to 1.0; flag-off behavior is identical to baseline apart from the FORMAT_VERSION bump. The `nutrient_quality`/`fertility` fields are always generated and serialized regardless of flag state."*

## 7. Determinism contract — the honest cost

- **This is NOT a byte-identical change.** Adding two `f32` fields to `BiomeCell` changes the serialized payload of *every* scenario (the biome is always in `state_hash`). Therefore:
  - **Bump `FORMAT_VERSION` 18 → 19** (`snapshot.rs`).
  - **Regenerate ALL golden hashes** (`UPDATE_HASHES=1`), because the fingerprint changes for every scenario even with both flags off.
- **No new tick-time RNG.** All new randomness is at world-gen, drawn from the existing biome-generation `Rng`, appended after the current draws so terrain/`env` are unchanged.
- Flag-off arithmetic forces every new multiplier to exactly `1.0` (not "≈1.0") so the flag-off *dynamics* are identical to baseline; only the serialized bytes (and thus the hash) differ.
- `#[serde(default)]` on the flags does not rescue old bincode snapshots — the `FORMAT_VERSION` gate rejects them, as documented in `snapshot.rs`.

## 8. Experiment observability

No genome slot changes, so the selection signal is **spatial**, not a gene-frequency sweep. Add a lightweight, deterministic telemetry metric to the headless harness:

- **`forage_quality_gain`** = (population-weighted mean `nutrient_quality` at live-agent positions) − (global mean `nutrient_quality`).
- **`forage_fertility_gain`** = same for `fertility`.

Interpretation: `> 0` and rising over generations ⇒ foragers concentrate in rich patches (selection is exploiting the landscape); `≈ 0` ⇒ no exploitation. Both are pure functions of world state, zero RNG, cheap (`O(n_agents)` once per report interval).

Deliverables for the experiment tier:
- An experiment scenario with both flags on, plus the flag-off control, wired into the existing `emergence.sh` / E-series harness (candidate id: next free E-number).
- The two metrics exported alongside existing headless telemetry.

## 9. Testing

1. **Determinism / save-load-step identity** with **both flags ON**: generate a world, save, load, and confirm N steps produce an identical `state_hash` from both the original and the reloaded world. Guards against any field not round-tripping. (See the `serde-skip` footgun precedent.)
2. **Range unit tests**: after `generate`, every cell's `nutrient_quality ∈ [MIN, MAX]` and `fertility ∈ [MIN, MAX]`.
3. **Fertility scaling unit test**: a cell with `fertility = 1.5` reaches a higher standing crop and regrows faster than the same terrain with `fertility = 0.5`; Water/Rock stay at 0 regardless.
4. **Quality unit test**: identical grazes on a high-quality vs low-quality cell yield energy in the ratio of their `nutrient_quality`; biomass removed is equal.
5. **Flag-off equivalence**: with both flags off, per-step *dynamics* match a baseline built before the change (compare biomass/energy trajectories, not the hash — the hash legitimately differs due to the new serialized fields).
6. **Golden regen**: `FORMAT_VERSION` 18→19, run `UPDATE_HASHES=1`, commit new goldens. Heavy determinism/golden suite runs on PR CI (per repo workflow), not every local commit.

## 10. Out of scope (explicit)

- **No new genome slot / gene.** (Decided: reuse existing sensing.)
- **No viewer overlay** for the two landscapes in this spec — noted as an optional follow-up (would mirror the existing `env`/`pollution` overlays in the Godot/web viewers) if visualizing the experiment proves useful.
- **Scavenging/meat quality** stays flat (§5.1) unless the experiment later calls for it.
- No change to trade-good resource nodes (`resource.rs`) — that is an economy, not food.

## 11. File touch-list

| File | Change |
|---|---|
| `crates/anabios-core/src/biome.rs` | 2 new `BiomeCell` fields; 4 new consts; 4 appended noise grids + per-cell mapping in `generate`; `fert` threading in `regrow_climax`/`regrow_succession`/`regrow_step`/`regrow_step_seasonal`/`recolonize_step` |
| `crates/anabios-core/src/interact.rs` | `quality_mult` in `feed_pass` energy payout |
| `crates/anabios-core/src/world.rs` | `nutrient_variation`, `soil_fertility` flags (`#[serde(default)]`) + docs |
| `crates/anabios-core/src/tick.rs` | pass `soil_fertility` into regrow/recolonize calls |
| `crates/anabios-core/src/snapshot.rs` | `FORMAT_VERSION` 18 → 19 |
| headless/telemetry module | `forage_quality_gain` / `forage_fertility_gain` metrics |
| `scripts/emergence.sh` + scenario defs | experiment scenario (both flags on) + control |
| golden hashes | regenerate via `UPDATE_HASHES=1` |
| tests | items in §9 |

## 12. Build sequence (for the implementation plan)

1. Add fields + consts + generation (fields present, flags absent) → bump `FORMAT_VERSION`, regen goldens, land determinism/range tests. World is now baseline-equivalent in dynamics.
2. Add `soil_fertility` flag + regrowth/recolonize threading + tick wiring + tests.
3. Add `nutrient_variation` flag + `feed_pass` payout + tests.
4. Add telemetry metrics.
5. Add experiment scenario + control + harness wiring.
