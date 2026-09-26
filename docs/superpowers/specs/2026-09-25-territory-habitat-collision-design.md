# Territory, Habitat & Collision — Design

**Date:** 2026-09-25
**Status:** Approved design, pending implementation plan
**Branch:** `claude/territory-habitat-collision`

## Goal

Animals should (1) never overlap each other, (2) roam within a defined
species territory, and (3) respect their habitat: land animals stay on
land, water animals stay in water, and birds (air) may range over both.

## Current state (2026-09-25, `main` @ bf563ea)

- **No collision/separation anywhere in the sim.** The only de-overlap is
  the viewer's draw-time crowd cap (`game/scripts/agent_layer.gd`), which
  hides sprites rather than spacing them.
- **Per-agent home ranges exist (E8).** `agents.anchor` + `Territoriality`
  gene (slot 15) + constant-magnitude anchor pull (`settlement.rs`), gated by
  `settlement_enabled`. No species-level range, no free-roam band.
- **No locomotion distinction.** `TerrainType::Water` exists but has zero
  carrying capacity and is walkable by every agent.
- `FORMAT_VERSION = 43`; latest `EventType` = 62.

## Decisions

| Question | Decision |
|---|---|
| Locomotion assignment | Heritable gene (genome slot 7), rare class flips |
| Water food | Aquatic biomass on Water cells, grazable by Water class only |
| Territory model | Per-species range (centre + radius), soft edge, overlap allowed |
| Collision | Steering separation **plus** hard post-integrate min-gap resolve |
| Gating | New opt-in `territory_enabled` flag; off ⇒ byte-identical |

## Architecture

All behavior below is active only when `World::territory_enabled` (mirrored
on `Scenario`, copied in `instantiate`). Flag off ⇒ zero extra RNG draws,
all existing goldens byte-identical.

### 1. Locomotion gene

- Rename `GenomeSlot::_Reserved7` → `Locomotion` in place (`GENOME_LEN`
  stays 50; old saves remain readable). Counts toward speciation distance.
- Bands: `v < 0.4` ⇒ **Land**, `0.4 ≤ v < 0.7` ⇒ **Water**, `v ≥ 0.7` ⇒ **Air**.
- Mutation: slot 7 uses the same RNG draw as today, but the delta is scaled
  ×0.1 when the flag is on (no extra draws; class stable across generations,
  flips rare).
- Spawn: flag on and no archetype pin ⇒ slot 7 written to `0.2` (Land),
  no RNG draw. Scenario archetypes may pin `locomotion = "land"|"water"|"air"`
  (writes band midpoints 0.2 / 0.55 / 0.85).
- New `AgentBuffers.locomotion: Vec<u8>` cache, derived from the genome at
  spawn/birth (added to spawn push, slot-reuse reset, kill reset). Not
  serialized; recomputed from genomes on load.
- Spawn/birth relocation: if the spawn cell is invalid for the class, move to
  the nearest valid cell via a deterministic ring search over biome cells
  (no RNG). If no valid cell exists (e.g. water class on an all-land map),
  the agent spawns in place and the habitat gate simply keeps it stationary;
  scenarios are responsible for providing the habitat they pin (a scenario
  validation warning is emitted at `instantiate`).

### 2. Habitat constraint

- Validity: Land ⇔ cell terrain ≠ Water; Water ⇔ terrain = Water; Air ⇔ any.
- Integrate gate (`integrate_all`, per-agent, own-slot write): compute the
  proposed wrapped position; if invalid, try x-only move, then y-only move
  (coastline slide); if both invalid, stay put (velocity zeroed).
- Decide masking: with the flag on, the water pull (for Water class — they
  are already in water), EnvAffinity habitat pull and terrain habitat pull
  are zeroed when their target direction's `REACH` probe cell is invalid for
  the agent's class. Land/Air agents keep the water pull (drinking at shore).

### 3. Aquatic biomass

- Flag on ⇒ `TerrainType::Water` carrying capacity = `AQUATIC_CAPACITY`
  (initial value 0.4 × Grass; tuned by the probe). Regrowth uses the existing
  plant path unchanged.
- Grazing gate: Water-class agents graze only Water cells; Land and Air
  agents graze only non-Water cells. Predation is unrestricted by class but
  physically limited by reach (a land predator can take a water agent only
  within contact range at the shoreline).

### 4. Species territory

- New persisted `World.species_territories: Vec<Territory>`, parallel to the
  other per-species vectors:
  ```rust
  pub struct Territory { pub cx: f32, pub cy: f32, pub r: f32, pub class: u8 }
  ```
- `territory_step` runs immediately after `species_step` (every
  `SPECIES_STEP_INTERVAL` = 200 ticks):
  - centre: EMA (`TERRITORY_CENTRE_RATE`) toward the torus-aware circular
    mean of members' `anchor` positions;
  - radius: `clamp(TERRITORY_K · √n, R_MIN, R_MAX)`;
  - class: members' majority locomotion class;
  - new species inherit the parent species' record (no parent ⇒ seeded from
    the members' current centroid); extinct species' records retained but
    inert (index-aligned with `species_*` vectors).
- Anchors are maintained under `territory_enabled` even if
  `settlement_enabled` is off (anchor learn step runs when either flag is on;
  with both off behavior is unchanged).
- Pull (in `decide_all`, next to the anchor pull):
  `d = torus_distance(pos, centre)`; `d ≤ r` ⇒ 0 (free roam);
  `d > r` ⇒ `min((d − r) / r, 1) · TERRITORY_PULL · Territoriality ·
  unit(centre − pos)`.
- Ranges of different species may overlap; exclusion stays emergent
  (existing `TerritorialRage`).

### 5. Separation

- Body radius `r_i = BODY_R_BASE + BODY_R_SIZE · Size` (so the pair gap
  `r_i + r_j` is capped at `MIN_GAP_MAX` = 1.5, below the 2-unit
  `MATING_RANGE` / `SHARE_RANGE` / `HARVEST_RANGE`).
- Collision rule: Air collides only with Air; Land and Water collide with
  each other and themselves; nothing collides with Air except Air.
- **Steering (sense → decide):** `NearestNeighbors::consider` accumulates
  `sep += (r_i + r_j − d) · unit(self − other)` for overlapping, colliding
  neighbours, into a new non-serialized `SensorRegister` field. `decide_all`
  adds `sep · SEP_PULL` before normalization.
- **Hard resolve (new stage 4d, after integrate / needs):** snapshot
  positions; K = 2 Jacobi passes; each agent sums push-out from overlapping
  colliding neighbours (one-ring `query` on the stage-1 hash — positions are
  stale by ≤ `SPEED_MAX_CAP` = 4, within the one-ring guarantee), applies
  half the correction, drops the push if the resulting cell is invalid for
  its class, writes only its own slot. Fixed iteration order, no RNG ⇒
  deterministic and rayon-safe.

## Per-tick data flow (flag on)

| Stage | New work |
|---|---|
| spawn / reproduce | locomotion cache; habitat relocation; archetype pin |
| mutate | slot-7 delta ×0.1 |
| 2 sense | separation vector accumulation |
| 3 decide | + separation pull, + territory pull, habitat masking of pulls |
| 4 integrate | habitat gate with coastline slide |
| 4d resolve (new) | Jacobi min-gap resolve |
| 5 interact / eat | class-gated grazing |
| 8 species (÷200) | `territory_step` |
| 10 biome | Water carrying capacity > 0 |

## Persistence

- `species_territories` serialized ⇒ `FORMAT_VERSION` 43 → 44 with history
  note in `snapshot.rs`. `territory_enabled` field added to `World`.
- `locomotion` cache, separation vector, and resolve snapshot are
  non-serialized scratch (derived or per-tick; none path-dependent).
- No new `EventType` in v1.

## Viewer

- New `game/scripts/territory_layer.gd` ground overlay: species territory
  circles in species colour (bridge accessor `species_territories()` in
  `crates/anabios-godot/src/lib.rs`). Registered in `overlay_manager.gd`.
  Keep `main.gd` untouched where possible (line cap).
- Air agents: lifted shadow offset in `agent_layer.gd` (bridge exposes the
  per-agent locomotion class alongside positions).
- Crowd cap stays.

## Testing

**Unit (TDD, local):**
- Locomotion band edges (0.39 / 0.4 / 0.7); mutation scale only with flag;
  deterministic spawn relocation.
- Habitat gate: land-into-water blocked; coastline slide; water can't leave
  water; air crosses freely. Property test on `continental.toml`, 2,000
  ticks: zero Land-on-Water and zero Water-on-Land agents.
- Territory: pull exactly 0 inside `r`, increasing outside; torus-seam
  centroid; radius formula; child species inherits parent record.
- Separation: two stacked agents end ≥ `min_gap` apart after resolve;
  Air↔Land do not push; push never crosses invalid habitat; mating still
  succeeds within `MATING_RANGE`.
- Aquatic biomass: flag on ⇒ Water regrows and only Water class grazes it;
  flag off ⇒ capacity 0.

**Determinism:**
- Flag off: all existing goldens (determinism / affect / cognition /
  inventions) unchanged — primary gating guard.
- Flag on: new `territory` golden in `determinism.rs`; save→load→step entry
  in `save_load_roundtrip.rs`; 1-thread vs N-thread identity;
  `snapshot_size.rs` updated.
- Full golden suite on PR CI; locally `cargo fmt --check`, clippy, rustdoc
  `-D warnings`, fast unit tests.

**Performance:** ≤ 10% tick-time overhead at 10k agents, measured with the
isolated stage bench against a saved baseline.

**Measurement probe (before freezing constants):**
`scenarios/territories_habitat.toml` — continental worldgen, pinned
Land/Water/Air archetypes; 20k ticks × 8 seeds, headless. Report:
- pairwise overlap rate (target: 0 colliding-pair overlaps at tick end),
- % members inside own territory (target ≥ 80%),
- habitat violations (must be 0),
- Water and Air population persistence,
- territory centre drift.
If aquatic lineages collapse, tune `AQUATIC_CAPACITY` / `R_MIN`, not the
architecture.

**Viewer:** GDScript tests for pure circle/shadow helpers; screenshots at 1×
and 3× zoom via the `running-the-viewer` skill.

## Tunable constants (initial values)

| Constant | Initial |
|---|---|
| `AQUATIC_CAPACITY` | 0.4 × Grass capacity |
| `TERRITORY_CENTRE_RATE` | 0.1 per species step |
| `TERRITORY_K` | 12.0 |
| `R_MIN` / `R_MAX` | 48 / 256 |
| `TERRITORY_PULL` | 1.0 |
| `SEP_PULL` | 2.0 |
| `BODY_R_BASE` / `BODY_R_SIZE` | 0.35 / 0.13 (⇒ pair gap ≤ 1.5 at Size ≤ ~3) |
| Resolve passes K | 2 |

## Out of scope (v1)

- Exclusive / mutually repelling territories.
- New codex events (e.g. `TerritoryShift`, `LandfallTransition`).
- Flight energy costs, diving, amphibious class.
- Viewer-side sprite spreading beyond the existing crowd cap.
