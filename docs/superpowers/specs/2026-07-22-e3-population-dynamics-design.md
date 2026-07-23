# E3 — Population Dynamics Completion — Design Spec

**Date:** 2026-07-22
**Status:** Approved
**Milestone:** E3 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crate:** `anabios-core` (detectors), wiring in `anabios-headless` + `game/`. First roadmap milestone to touch core: serialized layout grows → `FORMAT_VERSION` 8→9, golden hashes regenerated once (behavior unchanged; event stream grows).

## 1. Goal & success criteria

Deliver the design §4.1 detector set on the existing plant/predator substrate — the cheapest emergence left unclaimed. Four new event types:

| EventType | discriminant | Fires when |
|---|---|---|
| `PopulationCycleDetected` | 23 | a species' population oscillates with a regular period |
| `BoomAndBust` | 24 | a cycle with ≥3× peak/trough amplitude |
| `CarryingCapacityReached` | 25 | a species' population variance collapses at a sustained plateau |
| `TrophicCascade` | 26 | predator crash → herbivore boom → plant crash, in order |

Success criteria:

1. Each detector has a handcrafted positive test (fires exactly once) **and** a negative test (a near-miss that must not fire) — these are statistical claims, false positives are the risk (roadmap §9).
2. `scenarios/trophic-cascade.toml` fires the cascade in a meaningful minority of a 16-seed sweep (evidence recorded in the plan).
3. Full wiring: summary CSV columns, scorecard names/weights, codex panel names/colors (boot assert keeps them coupled).
4. Determinism discipline: single golden-hash regeneration, documented; no `HashMap` in detector paths; detectors run on the fused per-species aggregation pattern (budget ≤1 ms at 10k agents).

## 2. Cycle & boom detection (`codex/cycles.rs`)

Two analysis levels, because per-species population lines churn under 200-tick reclustering (a "species" rarely persists coherently for a full 400-tick window):

- **Per-species:** `cycle_history` (400-tick window, u32 counts — separate from the 200-tick `pop_history` so crash-detector semantics and golden behavior stay untouched).
- **Guild/world:** three world-scalar series — herbivore guild, carnivore guild (mean `effective_diet_carnivory` per species, threshold 0.5; SpeciesAgg gains `diet_sum`), and world total. This is the ecologically meaningful oscillator (Lotka-Volterra is a guild phenomenon) and the path that actually fires in real runs.

Events from guild series carry the **largest member species at fire time** as `species_id` (loc = its centroid) so they remain inspectable; the latch sets are keyed by guild (0=herb, 1=carn, 2=total).

Every `CYCLE_CHECK_INTERVAL = 10` ticks per series with a full window:

- Detrend: `x[t] = count[t] − mean(window)`.
- **Cycle:** sign changes of `x[t]` (zeros skipped) number ≥ 4; intervals between crossings all within `[CYCLE_PERIOD_MIN = 40, CYCLE_PERIOD_MAX = 200]`; interval coefficient of variation < 0.5; peak absolute deviation ≥ `CYCLE_MIN_AMPLITUDE = 0.25 × mean`. Fires once per species (latched in `cycle_active`; re-arms when the checks fail for a full window). `value` = mean interval (the period).
- **Boom:** same machinery; if cycle criteria pass **and** `peak ≥ BOOM_AMPLITUDE × trough` (`BOOM_AMPLITUDE = 3.0`, trough ≥ 1), fire `BoomAndBust` instead of/in addition to the cycle event — latched separately in `boom_active`. `value` = peak/trough ratio.

O(window) per species per check, amortized ×10 — negligible.

## 3. Carrying capacity

Over the same `cycle_history` window (a plateau is the cycle detector's null result): species alive for the whole window, `mean ≥ CARRYING_MIN_POP = 20`, and `std/mean < CARRYING_MAX_CV = 0.05` → fire `CarryingCapacityReached` (latched in `carrying_active`, re-arms if CV rises above 0.10). `value` = mean population. Negative test: a slow monotonic ramp (low CV *of the window* is impossible on a ramp — std scales with the ramp — so a ramp must not fire; also a noisy steady state with CV 0.2 must not fire).

## 4. Trophic cascade

World-scalar staged state machine on three aggregated series, sampled per tick:

- **Carnivore population**: sum over active species with mean `effective_diet_carnivory ≥ 0.5` (SpeciesAgg gains `diet_sum`).
- **Herbivore population**: mean carnivory < 0.5.
- **Plant biomass**: `world.plant_biomass_total()`.

State machine (armed → predator-crash → herbivore-boom → plant-crash):

1. **Armed:** track the 150-tick carnivore peak. If carnivores drop ≥ `CASCADE_CRASH_FRAC = 0.5` below that peak (and peak ≥ `CASCADE_MIN_PREDATORS = 5`) → stage 1, recording the herbivore level.
2. **Stage 1** (≤ `CASCADE_LAG = 300` ticks): herbivores rise ≥ `CASCADE_HERB_RISE = 0.3` over their stage-entry level → stage 2, recording the plant level **at boom confirmation** — the claim is "plants crash once the released herbivores graze them down", measured from the release point.
3. **Stage 2** (≤ `CASCADE_PLANT_LAG = 900` ticks — the plant leg is by far the slowest): plant biomass falls ≥ `CASCADE_PLANT_DROP = 0.3` below the boom-confirmation level → fire `TrophicCascade`, re-arm.

Timeouts re-arm. `value` = carnivore drop fraction; loc = 0,0 (world-scale). The lagged ordering is what makes it a *cascade* rather than three independent fluctuations — the negative test feeds the same three fluctuations out of order and must not fire. (Tuning note: the plant reference and lag were set from instrumented predator-prey runs — measured from stage-1 entry with a 300-tick budget, real cascades timed out on the plant leg every time.)

## 5. Core changes

- `EventType` +4 variants (23–26); `EVENT_TYPE_COUNT` derives automatically.
- `CodexState` += `cycle_history: BTreeMap<u32, VecDeque<u32>>`, `herb_cycle_history`/`carn_cycle_history`/`total_cycle_history: VecDeque<u32>`, `cycle_active`/`boom_active`/`carrying_active: BTreeSet<u32>`, `guild_cycle_active`/`guild_boom_active`/`guild_carrying_active: BTreeSet<u8>`, cascade scratch (`cascade_carn_history: VecDeque<u32>`, `cascade_stage: u8`, `cascade_stage_tick: u64`, `cascade_carn_peak: u32`, `cascade_herb_entry: u32`, `cascade_plant_entry: f32`).
- `SpeciesAgg` += `diet_sum: f64` (fused pass; ascending-id accumulation).
- `snapshot.rs`: `FORMAT_VERSION` 8→9 + changelog line; regenerate `GOLDEN`/`INVENTIONS_GOLDEN` once (`UPDATE_HASHES=1`) — serialized layout + event-buffer contents change, agent behavior does not.
- Constants in `codex/mod.rs` with doc comments, matching house style.

## 6. Wiring

- `score.rs`: `ALL_EVENT_NAMES` (now 27) + `DEFAULT_WEIGHTS` (+4 at `NOVELTY_BONUS` — unseen in the E1 corpus) with names `pop_cycle`, `boom_bust`, `carrying_capacity`, `trophic_cascade`.
- `sweep.rs`: `event_name` +4 and CSV header/row +4 columns (appended after `dowry_birth`, before the scorecard columns).
- `codex_panel.gd`: +4 names (`PopCycle`, `BoomBust`, `CarryingCap`, `TrophicCascade`) and colors; boot assert validates the count.

## 7. Testing & evidence

- **Unit (in `cycles.rs`):** stuffed-history tests — sine window fires cycle once; ramp does not; irregular spikes do not; deep sine fires boom; flat plateau fires carrying; noisy plateau does not; cascade state machine driven through in-order (fires) and out-of-order (does not) sequences.
- **Integration (`tests/`):** `trophic-cascade.toml` run long enough to fire ≥1 of the new events; replay-verify the new event types through `anabios-headless replay`.
- **Sweep evidence:** 16 seeds × 6000 ticks of `trophic-cascade.toml`; report per-type fire counts in the plan completion notes.
- **Gallery:** codex panel capture showing the new chapter names live.

## 8. Deferred

Population-chart period annotations (roadmap E3 viewer item) — the codex panel + CSV surface the events for now; chart markers land with E5's trait-drift chart work, which builds the shared annotation plumbing.
