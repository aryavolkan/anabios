# Procedural Climate Biome + Genetic Local Adaptation — Design Spec

**Date:** 2026-07-17
**Status:** Approved (brainstorming) → ready for implementation plan
**Baseline:** `main` @ merge of PRs #20/#21/#22 (perf/refactor batches).

## Motivation

Give the biome a continuous **climate gradient** and let species **genetically
adapt to their local climate**, so different regions evolve divergent, locally-
adapted lineages (biome-driven speciation). This is a spatial + genetic twin of
the existing DIT `env_optimum` mechanic (which is global + temporal + cultural).

Current state (from exploration): the biome is a 128×128 2-octave value-noise
mosaic of 5 terrains + `plant_biomass`; the generator computes a continuous noise
scalar per cell and **discards it**. Terrain reaches agent fitness only through
biomass availability at the grazed cell. The DIT mechanic already matches a scalar
trait against a scalar optimum via a triangular kernel, multiplying feeding yield
in `feed_pass` — this design reuses that kernel and application point.

## Decisions (from brainstorming)

- **Biome depth:** a continuous per-cell **climate** field from a dedicated noise
  (semi-independent of terrain). Terrain stays the 5 existing types; world stays
  1024². No new terrain types, no size change.
- **Adaptation:** **in-place local adaptation** via a feeding differential only —
  no movement pull. Agents still roam; the affinity gene evolves to match local
  climate.
- **Gating:** the adaptation feeding bonus is **opt-in** via a World flag, off by
  default, so existing scenarios stay behavior-identical.

## Component 1 — Biome climate field

- Add `pub env: f32` (`[0,1]`) to `BiomeCell` (`biome.rs`).
- In `BiomeField::generate`, add a **dedicated climate `NoiseGrid`** using its own
  octave(s), drawn from the biome-gen `Rng` *after* the terrain noise (deterministic).
  Sample per cell → store `cell.env`. Using a separate noise (not the terrain
  elevation scalar) makes climate semi-independent of terrain, so a Forest cell can
  be warm or cold — a genuinely diverse gradient. Recommended: two-octave value
  noise like the terrain path (e.g. coarse 6 + fine 18), weights ~0.7/0.3, result
  clamped to `[0,1]`.
- `env` is **static after generation** (like terrain) — it does not fluctuate.
- A read accessor `BiomeCell` already exposes fields directly; `BiomeField::sample(pos)`
  returns the cell, so consumers read `cell.env`.

**Determinism:** adding a `BiomeCell` field changes the serialized `World` layout
(the biome is in `state_hash`). This requires a **one-time golden refresh** and a
`snapshot::FORMAT_VERSION` bump (1 → 2). The extra `NoiseGrid` draws from the
biome-gen `Rng` (seeded from the world seed, separate from `world.rng`), so
generation stays deterministic.

## Component 2 — `EnvAffinity` gene + local-adaptation feeding bonus

- Rename inert slot `_SensoryReserved41` → **`EnvAffinity = 41`** in `GenomeSlot`
  (index unchanged). Value in `[0,1]`. It is a normal genome slot: heritable,
  mutates via `mutate_in_place`, crosses over, and — deliberately — **counts toward
  `genome.distance`** (it is NOT added to the personality-exclusion list). This is
  the engine of biome-driven speciation: regions that converge to different
  affinities become genetically distant and split into species.
- New constants (in `culture.rs` alongside the DIT constants, or a small
  `biome`/`interact` const block): `ENV_AFFINITY_BONUS: f32 = 0.5`,
  `ENV_AFFINITY_TOLERANCE: f32 = 0.25`.
- Reuse the triangular match kernel. Either call the existing
  `culture::technique_match` (tolerance 0.2) or add a sibling
  `env_affinity_match(affinity, env) = (1 - |affinity-env|/ENV_AFFINITY_TOLERANCE).clamp(0,1)`
  with its own tolerance. **Decision:** add the sibling with its own tolerance
  (keeps the two mechanics independently tunable).
- In `feed_pass` (`interact.rs`), when the adaptation flag is on, apply after the
  base/DIT bite computation:

  ```rust
  if world.biome_adaptation {
      let env = world.biome.sample(pos).env;
      let affinity = world.agents.genome[i].get(GenomeSlot::EnvAffinity);
      let m = env_affinity_match(affinity, env);
      desired_bite *= 1.0 + ENV_AFFINITY_BONUS * m;
  }
  ```

  This multiplies the same `desired_bite` the DIT bonus does (they compose
  multiplicatively; both can be active). It reads no RNG.

## Component 3 — Gating & determinism

- Add `pub biome_adaptation: bool` to `World` with `#[serde(default)]` (mirrors
  `env_period`), initialized `false` in `World::new`, settable from the scenario
  TOML (add a `biome_adaptation: Option<bool>` to `Scenario`, applied in
  `instantiate`).
- **Off by default → every existing scenario is behavior-identical** (the affinity
  read is skipped in `feed_pass`; `EnvAffinity`'s slot value existed before as
  `_SensoryReserved41` and already mutated, so no genome-layout or RNG change).
  The only hash movement for existing scenarios is the cosmetic new `env` field —
  covered by the single golden refresh.
- On (opted-in) scenarios get the adaptation dynamics.

## Component 4 — Scenario

Add `scenarios/biome-adaptation.toml`: a single uniform population (~200 agents,
grazer archetype) with `biome_adaptation = true`, run long enough for a climate→
affinity cline to form. `EnvAffinity` starts near neutral (0.5 via `neutral()` +
mutation drift) and selection pulls each region toward its local `env`.

## Testing

- **Unit** (`biome.rs`, `interact.rs`/`culture.rs`, `genome.rs`):
  - Climate field: over a generated biome, every `cell.env ∈ [0,1]`, and the field
    has non-trivial spatial variance (max − min > 0.3) and isn't identical to the
    terrain-derived value everywhere.
  - `env_affinity_match`: `match(x,x)=1`; `match` falls to 0 beyond `TOLERANCE`;
    bounded `[0,1]`.
  - `EnvAffinity` accessor reads slot 41.
- **Behavioral proof (local adaptation):** instantiate `biome-adaptation.toml`,
  run N ticks, bucket alive agents by their local `cell.env` (low half vs high
  half), assert **mean `EnvAffinity` differs between the buckets** by a robust
  margin (a cline formed) — i.e. agents in high-climate regions carry higher
  affinity than those in low-climate regions. Direction, not exact magnitude.
- **Flag-off invariance:** a flag-off run of the same population is behavior-
  identical to a pre-feature run (the affinity read is skipped). Practically: the
  refreshed golden covers `minimal` (flag off), and the full suite must stay green
  — existing scenarios must sustain exactly as before.
- **Determinism:** golden refreshed once (new `BiomeCell.env` field);
  `determinism.rs` re-baselined; runs reproducibly (twice → same hash).
- **CI gate:** `rustup run stable` fmt/clippy/doc `-D warnings`; commit fmt output;
  escape `` `[0,1]` `` in doc comments.

## Out of scope (deferred)

- Frontend visualization of the climate field (ground overlay) and `EnvAffinity`
  (body-color mode) — sim-only this cycle (like the personality first pass).
- Active habitat selection (an affinity-driven movement pull → spatial
  segregation) — a possible follow-up; higher risk (movement change).
- More terrain types / larger world / temperature×moisture Whittaker model.
- Domestication, writing/meme-persistence.

## Success criteria

Every biome cell carries a climate value; with `biome_adaptation` on, populations
evolve a spatial `EnvAffinity` cline matched to local climate and diverge into
biome-adapted species (NichePartitioning fires); with the flag off, every existing
scenario behaves identically; determinism holds against the refreshed golden; CI
green.
