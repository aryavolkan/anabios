# Territory, Habitat & Collision — Design

**Date:** 2026-09-25 (text reconciled with the shipped code 2026-09-26, review)
**Status:** Implemented — PR #173
**Branch:** `claude/territory-habitat-collision`
**Plan:** `docs/superpowers/plans/2026-09-25-territory-habitat-collision.md`
**Findings:** `docs/superpowers/specs/2026-09-25-territory-habitat-collision-findings.md`
**Diagnosis:** `docs/superpowers/specs/2026-09-25-territory-pull-diagnosis.md`

## Goal

Animals should (1) be kept apart from each other (separation steering plus a
best-effort min-gap resolve, not a hard overlap guarantee), (2) roam within a
defined species territory, and (3) respect their habitat: land animals stay
on land, water animals stay in water, and birds (air) may range over both.

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
  stays 50, so the genome's serialized layout is unchanged; snapshots are
  still strictly versioned — see Persistence). Counts toward speciation
  distance (only the personality slots are masked out of `Genome::distance`).
- Bands: `v < 0.25` ⇒ **Water**, `0.25 ≤ v < 0.75` ⇒ **Land**, `v ≥ 0.75` ⇒ **Air**.
  `Genome::neutral()` (0.5) is therefore Land.
- Mutation: slot 7 uses the same RNG draw as today, but the delta is scaled
  ×0.1 when the flag is on (no extra draws; class stable across generations,
  flips rare).
- Spawn: no override needed — unpinned agents are Land (neutral 0.5).
  Scenarios pin a spec's class via the existing trait-override table:
  `[agents.traits] locomotion = 0.1` (water) / `0.5` (land) / `0.9` (air).
- No per-agent cache column: the class is `habitat::Locomotion::of(&genome)`
  (one slot read), so `AgentBuffers` keeps its layout.
- Spawn/birth relocation: if the spawn cell is invalid for the class, move to
  the nearest valid cell via a deterministic ring search over biome cells
  (no RNG). If no valid cell exists (e.g. water class on an all-land map),
  the agent spawns in place and the habitat gate keeps it stationary;
  scenarios are responsible for providing the habitat they pin.

### 2. Habitat constraint

- Validity: Land ⇔ cell terrain ≠ Water; Water ⇔ terrain = Water; Air ⇔ any.
- Integrate gate (`integrate_all` → `habitat::gate_move`, per-agent,
  own-slot write): validate the proposed destination; if invalid, try the
  x-only move, then the y-only move (coastline slide); if both are invalid,
  stay put (velocity zeroed). The gate samples the torus-*wrapped*
  destination — the exact coordinate `integrate_all` stores — not the raw
  `pos + v` (review fix: for a sub-3e-5 overshoot past the seam, f32
  `rem_euclid` rounds the stored coordinate to exactly `world_size`, which
  `cell_coords` reads as row/column 0 while the raw value clamps to the last
  row — a violation the gate could not see; see the findings doc's "Review
  caveats"). A displacement longer than half a biome cell is
  sampled along its segment at ≤ half-cell spacing (speed multipliers stack
  to ~9 units/tick against 8-unit cells), so a fast agent cannot tunnel
  through a one-cell strip of forbidden terrain
  (`gate_does_not_tunnel_through_a_one_cell_strip`).
- Decide masking: with the flag on, the EnvAffinity and terrain habitat
  pulls are skipped when the cell one cell-width along the pull is invalid
  for the agent's class (`habitat::pull_allowed`). The water pull is kept:
  Land/Air drink at the shore (`drinkable_near`), Water agents already sit in
  water.
- Plant sensing is class-aware: `best_plant_direction` and
  `local_plant_biomass` ignore cells the agent's class can't graze, so land
  agents don't chase aquatic biomass into the shoreline.

### 3. Aquatic biomass

- Flag on ⇒ Water cells hold aquatic biomass up to `AQUATIC_CAPACITY`
  (0.4 × Grass; tuned by the probe), seeded at instantiate and regrown by a
  dedicated `BiomeField::aquatic_regrow_step` (logistic, with a reseed floor).
  `TerrainType::carrying_capacity` (a `const fn`) stays 0.0 for Water.
- Grazing gate: Water-class agents graze only Water cells; Land agents graze
  only non-Water cells; Air agents graze both (a seabird niche — see the
  amendment note below). Predation is unrestricted by class but physically
  limited by reach (a land predator can take a water agent only within
  contact range at the shoreline).

> **Amended 2026-09-25 after the Task 9 probe: Air grazes land and aquatic
> biomass.** The original "Air agents feed on land only" rule, combined with
> a habitat mask that lets Air occupy any terrain, meant Air spent most of
> its range over ungrazeable sea while directly competing with Land for the
> same land-only forage — the Task 9 measurement probe found Air going
> extinct in every seed tried (24/24 runs across three constant
> configurations, and again after a `max_share` per-lineage population-cap
> fix). `Locomotion::can_graze` now returns `true` for Air on every terrain,
> matching its already-unrestricted `can_occupy`.

### 4. Species territory

- New persisted `World.species_territories: Vec<Territory>`, parallel to the
  other per-species vectors:
  ```rust
  pub struct Territory { pub cx: f32, pub cy: f32, pub r: f32, pub class: Locomotion }
  ```
  `class` is the `habitat::Locomotion` enum (serde-derived; the viewer bridge
  exports its `u8` discriminant). `r == 0.0` marks a row `territory_step` has
  not initialised yet, which applies no pull.
- `territory_step` runs immediately after `species_step` (every
  `SPECIES_STEP_INTERVAL` = 200 ticks):
  - centre: EMA (`TERRITORY_CENTRE_RATE`) toward the members' torus-safe
    mean position (mean offset from a reference point — the current centre,
    or the first member for a new row; no trig);
  - radius: `clamp(TERRITORY_K · √n, R_MIN, R_MAX)`;
  - class: members' majority locomotion class (lowest index wins ties);
  - new species inherit the parent species' record (no parent ⇒ seeded from
    the members' current centroid); extinct species' records retained but
    inert (index-aligned with `species_*` vectors).
- Pull (in `decide_all`, next to the anchor pull):
  `d = torus_distance(pos, centre)`; `inner = TERRITORY_FREE_FRAC · r`;
  `d ≤ inner` ⇒ 0 (free roam); `d > inner` ⇒
  `min((d − inner) / (r − inner), 1) · TERRITORY_PULL · Territoriality ·
  unit(centre − pos)` — full strength is reached at `r` itself, not `2r`.
  Before this pull is added, the move intent accumulated so far in
  `decide_all` is capped to unit length (`territory::apply_territory_pull`,
  only when the pull is non-zero) so an unbounded evolved move intent can't
  swamp a fixed-size pull after normalization.
- Ranges of different species may overlap; exclusion stays emergent
  (existing `TerritorialRage`).
- **Amended 2026-09-25 after the territory pull diagnosis**
  (`docs/superpowers/specs/2026-09-25-territory-pull-diagnosis.md`):
  the pull formula above (free-roam to `r/2`, ramp to full at `r`, and the
  unit-capped intent) and the enlarged `TERRITORY_K` / `TERRITORY_R_MAX`
  below replace the original "free roam to `r`, ramp to `2r`" geometry, which
  measured ≤ 1/8 seeds ≥ 80% `inside_territory` (root cause: an unbounded
  evolved move intent swamps a fixed-size pull, and the old ramp's stall
  point for an outward-steering member sits outside `r` by construction).
  The fix was validated at 6/8 seeds ≥ 80%, zero extinctions — see the
  findings doc's "Round 4" section.

### 5. Separation

- Body radius `r_i = BODY_R_BASE + BODY_R_SIZE · Size` with `Size ∈ [0,1]`,
  so the pair gap `r_i + r_j` is at most 1.5, below the 2-unit
  `MATING_RANGE` / `SHARE_RANGE` / `HARVEST_RANGE`.
- A dedicated fine collision hash (`COLLISION_CELL` = 4-unit cells), rebuilt
  at stage 1 and inside the resolve, serves both layers — the sense hot loop
  is left untouched.
- Collision rule: Air collides only with Air; Land and Water collide with
  each other and themselves; nothing collides with Air except Air.
- **Steering (decide):** `collision::separation_steer` sums
  `unit(self − other) · (reach − d)/reach` over colliding neighbours within
  `reach = STEER_MARGIN·(r_i + r_j)` on the collision hash; `decide_all` adds
  `sep · SEP_PULL` last, before normalization.
- **Hard resolve (new stage 4′, `collision::resolve_overlaps` in `tick.rs`:
  immediately after integrate and *before* `needs_step`, anchor learning and
  interact, so every later stage sees resolved positions):** snapshot
  positions; K = 2 Jacobi passes; each agent sums push-out from overlapping
  colliding neighbours (one-ring `query` on the collision hash rebuilt from
  post-integrate positions; between passes positions move ≤ `MAX_PUSH`),
  applies half the correction, drops the push if the resulting cell is invalid
  for its class, writes only its own slot. Fixed iteration order, no RNG ⇒
  deterministic and rayon-safe.

## Per-tick data flow (flag on)

| Stage | New work |
|---|---|
| spawn / reproduce | habitat relocation (founders + newborns) |
| mutate | slot-7 delta ×0.1 |
| 1 hash | + fine collision hash rebuild |
| 2 sense | class-aware plant sensing |
| 3 decide | + separation pull, + territory pull, habitat masking of pulls |
| 4 integrate | habitat gate with coastline slide |
| 4′ resolve (new; before 4a′ needs / 4c anchor / 5 interact) | Jacobi min-gap resolve |
| 5 interact / eat | class-gated grazing |
| 8 species (÷200) | `territory_step` |
| 10 biome | Water carrying capacity > 0 |

## Persistence

- `species_territories` serialized ⇒ `FORMAT_VERSION` 43 → 44 with history
  note in `snapshot.rs`. `territory_enabled` field added to `World`. Both
  new fields are `#[serde(default)]`, but the snapshot envelope is an exact
  `FORMAT_VERSION` match (`load_from_bytes`), so pre-44 snapshots are
  rejected, not migrated. Flag absent/off in every pre-existing scenario ⇒
  trajectories byte-identical (pinned by the
  `*_trajectory_unchanged_by_territory_substrate` guards in
  `tests/determinism.rs`); only the serialized layout grew.
- `collision_spatial` and the resolve snapshot are non-serialized per-tick
  scratch; the locomotion class is derived from the genome, never stored.
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
- Locomotion band edges (0.25 / 0.75 — `WATER_MAX` / `AIR_MIN`,
  `bands_split_at_quarter_and_three_quarters`); mutation scale only with
  flag; deterministic spawn relocation.
- Habitat gate: land-into-water blocked; coastline slide; water can't leave
  water; air crosses freely; the gate judges the wrapped destination at the
  seam. Invariant test `habitat_classes_never_leave_their_terrain`
  (`tests/invariants.rs`) on `scenarios/habitat-territories.toml`, 2,000
  ticks: zero Land-on-Water and zero Water-on-Land agents, checked every
  tick over several seeds (tightened in review from one seed sampled every
  50 ticks, which had missed a 7-tick seam violation — findings doc, "Review
  caveats").
- Territory: pull exactly 0 out to `TERRITORY_FREE_FRAC · r` (= `r/2`),
  ramping to full strength at `r` and capped beyond it
  (`pull_is_zero_inside_and_ramps_outside`); the unit-cap wins over a huge
  outward program intent in `decide_all`; torus-seam centroid; radius
  formula; child species inherits parent record.
- Separation: two stacked agents end ≥ `min_gap` apart after resolve;
  Air↔Land do not push; push never crosses invalid habitat; mating still
  succeeds within `MATING_RANGE`.
- Aquatic biomass: flag on ⇒ Water regrows and only Water class grazes it;
  flag off ⇒ capacity 0.

**Determinism:**
- Flag off: all existing goldens (determinism / affect / cognition /
  inventions) unchanged — primary gating guard.
- Flag on: `HABITAT_GOLDEN` / `habitat_territories_matches_golden_hashes`
  in `determinism.rs`; `territory_roundtrip` save→load→step entry in
  `save_load_roundtrip.rs`; `parallel_matches_serial_across_thread_counts`
  includes the scenario. `snapshot_size.rs` (a non-pinned size report) was
  not changed.
- Full golden suite on PR CI; locally `cargo fmt --check`, clippy, rustdoc
  `-D warnings`, fast unit tests.

**Performance:** target ≤ 10% tick-time overhead at 10k agents, measured with
the isolated stage bench against a saved baseline. Measured 1.19–1.24× in the
findings doc's Round 1 and accepted as a miss; that figure predates the
Round-4 mechanism and was taken on the 1024-wide flagship only (see the
findings doc's "Review caveats" for the collision-grid-cap cost on ≥ 4096-wide
worlds).

**Measurement probe (before freezing constants):**
`scenarios/habitat-territories.toml` — continental worldgen, pinned
Land/Water/Air archetypes; 20k ticks × 8 seeds, headless
(`territory_measurement_probe` in `tests/invariants.rs`, `--ignored`). Report:
- pairwise overlap rate (target: 0 colliding-pair overlaps at tick end),
- % members inside own territory (target ≥ 80%),
- habitat violations (must be 0),
- Water and Air population persistence,
- territory centre drift (not instrumented in the shipped probe, which
  reports per-class populations, `violations`, `deep_overlaps` /
  `shallow_overlaps`, `inside_territory` overall and per class, and
  `water_cells_with_biomass` — all final-tick readings).
If aquatic lineages collapse, tune `AQUATIC_CAPACITY` / `R_MIN`, not the
architecture.

**Viewer:** GDScript tests for pure circle/shadow helpers; screenshots at 1×
and 3× zoom via the `running-the-viewer` skill.

## Tunable constants (initial values)

| Constant | Initial | Current (2026-09-25, Task 9b) |
|---|---|---|
| `AQUATIC_CAPACITY` | 0.4 × Grass capacity | 0.6 × Grass capacity |
| `TERRITORY_CENTRE_RATE` | 0.1 per species step | unchanged |
| `TERRITORY_K` | 12.0 | **24.0** |
| `R_MIN` / `R_MAX` | 48 / 256 | 48 / **512** |
| `TERRITORY_PULL` | 1.0 | 2.5 |
| `TERRITORY_FREE_FRAC` | (n/a, ramp started at `r`) | **0.5** (new) |
| `SEP_PULL` | 2.0 | unchanged |
| `BODY_R_BASE` / `BODY_R_SIZE` | 0.4 / 0.35 (⇒ pair gap ≤ 1.5 at Size = 1) | unchanged |
| Resolve passes K | 2 | unchanged |

**Amended 2026-09-25 after the territory diagnosis**: `TERRITORY_K` and
`R_MAX` were raised (12→24, 256→512) so a range strong enough to hold members
(via the pull-geometry fix above) can still feed them — see the diagnosis
§9c/§10 and the findings doc's "Round 4" section. `TERRITORY_FREE_FRAC` is new
(§4 above). `TERRITORY_PULL` stays 2.5 (Task 9 round 3's value).

## Out of scope (v1)

- Exclusive / mutually repelling territories.
- New codex events (e.g. `TerritoryShift`, `LandfallTransition`).
- Flight energy costs, diving, amphibious class.
- Viewer-side sprite spreading beyond the existing crowd cap.
