# Basic Needs (thirst + sleep) — design

**Date:** 2026-09-02
**Flag:** `basic_needs_enabled` (opt-in, default off)
**Status:** approved for implementation (single-PR milestone)

## 1. Problem and shape

The user asked for "basic needs: hunger + sleep + thirst." Hunger already
exists twice over: `energy` is the homeostatic variable (starvation death in
`age_and_starve`), and the affect layer's Layer-0
`homeostatic_drive(energy)` (`affect.rs`) is hunger-as-drive feeding SEEKING.
The affect spec (§3.2, 2026-08-02) explicitly reserves widening Layer 0 from
a scalar to a drive *vector* (thirst, fatigue) as an additive milestone.

This subsystem adds the two missing drives as a self-contained opt-in module
(`needs.rs`), leaving hunger/energy untouched. It follows the house gating
contract: flag off ⇒ strict no-op, zero RNG drawn, zero state written,
byte-identical trajectory (layout growth only).

## 2. New state

Per-agent serialized columns on `AgentBuffers` (spawn/grow_one/kill wired,
NOT `#[serde(skip)]` — they are path-dependent accumulators feeding hashed
movement; still_ticks v13 footgun):

- `thirst: Vec<f32>` — [0,1]. Rises each tick (base + movement component),
  falls while drinking. Stays 0.0 when the flag is off.
- `fatigue: Vec<f32>` — [0,1]. Rises with activity, falls while asleep.
- `asleep: BitVec` — hysteresis sleep state: falls asleep at
  `fatigue >= SLEEP_ONSET` (0.9), wakes at `fatigue <= WAKE_AT` (0.2).

Genome: reserved slots renamed **in place** (indices unchanged, snapshot- and
helix-panel-safe): slot 8 `_BodyReserved8` → `ThirstTolerance`, slot 9
`_BodyReserved9` → `SleepNeed`. Neutral 0.5 ⇒ exactly ×1.0 rate scaling.
Read only when the flag is on; both count toward speciation distance
(adaptive, non-personality).

CodexState: `dehydration_fired: BTreeSet<u32>` (once-per-species latch).

No new `#[serde(skip)]` fields anywhere.

## 3. Mechanics

**Water.** A cell is *drinkable* if its terrain is `TerrainType::Water` or
its `river_flow >= RIVER_DRINK_MIN`; an agent can drink when its own cell or
a 4-neighbour is drinkable (shoreline drinking). No `BiomeCell` schema
change, no worldgen change — reuses static fields only.

**Thirst.** `needs_step` (new tick stage directly after `integrate_all`,
serial ascending-id loop, zero RNG):
`thirst += (THIRST_RATE_BASE + THIRST_RATE_MOVE × |velocity|) × (1.5 − ThirstTolerance)`
per tick, clamped to [0,1]; if drinkable-adjacent, `thirst −= DRINK_RATE`
instead. Dehydration kills through the *existing* starvation path: in
`integrate_all`, basal metabolism is multiplied by
`1 + DEHYDRATION_DRAIN × thirst²` — exactly 1.0 at thirst 0, so flag-off is
identity, and drinking never *adds* energy (the
`energy_plus_biomass_does_not_grow` invariant is untouched).

**Water-seeking.** In `decide_all`, gated on the flag: when
`thirst > WATER_SEEK_MIN`, add `WATER_PULL × thirst × best_water_direction()`
to the movement intent — the same additive-bias pattern as the
habitat/anchor/hub pulls. `best_water_direction` is a bounded cell scan
(radius `WATER_SEEK_REACH`) modeled on `best_terrain_direction`, matching
drinkable cells; deterministic tie-break on lowest (dy,dx).

**Sleep.** `fatigue += (FATIGUE_RATE_BASE + FATIGUE_RATE_MOVE × |velocity|) ×
(0.5 + SleepNeed)` while awake. While asleep: movement fully suppressed in
`integrate_all` (velocity zeroed before the move path, so no move cost),
basal metabolism × `SLEEP_METABOLISM_FACTOR` (0.6), feeding skipped in
`feed_pass`, and `fatigue −= FATIGUE_RECOVERY`. The cost of sleep is lost
foraging/mating time; sleeping agents remain attackable. Hysteresis
thresholds prevent flapping.

## 4. Observability

- `EventType::Dehydration = 60` (appended; append-only invariant respected).
  Detector `codex/needs.rs`: fires once per species (latched via
  `dehydration_fired`) when the species' mean thirst ≥
  `DEHYDRATION_EVENT_MIN` (0.8) with ≥ `DEHYDRATION_MIN_COUNT` live members.
  Inert unless the flag is on.
- Viewer: `codex_panel.gd` `CHAPTER_NAMES`/`CHAPTER_COLORS` gain the
  Dehydration row (boot assert keeps them in lockstep with
  `EVENT_TYPE_COUNT`); `agent_detail` exposes thirst/fatigue/asleep.

## 5. Serialization / determinism bookkeeping

- `FORMAT_VERSION` 34 → 35 with a changelog line: "basic needs — layout
  growth only; flag off ⇒ byte-identical behavior." **Known collision risk:**
  the unmerged PR #145 also claims v35; whichever merges second re-bumps
  (same protocol as the v31/v32 merge).
- All 7 golden hash tables regenerate (bincode layout growth rehashes
  everything even flag-off) via `UPDATE_HASHES=1`, each with a dated
  layout-growth comment. A golden moving for any non-layout reason is a
  determinism bug — stop.
- `save_load_roundtrip` gains one macro line over the new scenario.

## 6. Scenario + tests

- `scenarios/basic-needs.toml`: flagship — rivers (`climate.river_threshold`)
  + lakes, grazer founders, flag on. Auto-covered by `all_scenarios`.
- Unit tests (needs.rs / agent.rs): flag-off inertness (state-hash equality),
  thirst accumulation + drinking, dehydration raises basal drain, sleep
  hysteresis (onset/wake), asleep suppresses movement + feeding,
  spawn/reuse/kill column resets.
- Integration test `tests/basic_needs.rs`: with the flag on, an agent placed
  by water outlives an identical agent in a dry world region; save→load→step
  identity.

## 7. Explicitly deferred (YAGNI)

Program `SenseThirst`/`SenseFatigue` nodes (affect spec §3.4 sanctions adding
them later without touching goldens); coupling thirst/fatigue into affect
SEEKING; sleep vulnerability modifiers (extra combat damage); dynamic
per-cell water levels; a thirst-specific death cause code.
