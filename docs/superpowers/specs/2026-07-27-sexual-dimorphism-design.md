# Sexual Dimorphism (E12) — Design Spec

**Date:** 2026-07-27
**Status:** Approved (brainstorming) → implementation
**Baseline:** `main` through E11 (climate maladaptation) + geographic-trade batch.

## Motivation

Two gaps between the original design (§genome, §reproduction) and the shipped
substrate:

- **Reproduction is sexless.** `reproduce.rs` pairs any two eligible
  same-species agents. Genome slot 33 `SexualDimorphism` and slot 32
  `MateChoosiness` are declared but explicitly unread.
- There is no **sexual selection** axis: nothing couples morphology (size,
  weapon damage) to mating success, so the codex has no way to observe runaway
  sexually-selected traits — a classic emergence story the discovery sandbox
  is built to catalogue.

This milestone adds binary sex, female mate choice, and a dimorphism knob that
expresses sex-linked morphology/physiology differences, all behind an opt-in
flag. Domesticable animals (taming + livestock pens riding the Husbandry
invention) are a **separate follow-up milestone**, not covered here.

## Decisions (from brainstorming)

- **Binary sex + dimorphism knob.** Each agent is female or male; mating
  requires one of each. The `SexualDimorphism` gene (slot 33) is a heritable
  magnitude `d ∈ [0,1]` scaling sex-linked trait expression.
- **Female choice, deterministic.** `MateChoosiness` (slot 32) on the *female*
  partner (whichever side initiates) sets a minimum male display-quality bar.
  No RNG in the choice rule — deterministic threshold, so no new draw-order
  reasoning inside the mating pass.
- **Opt-in flag.** `sexual_dimorphism_enabled` on `World`/`Scenario`, default
  off. Flag off = zero extra RNG draws and identity (×1.0) stat factors, so
  every existing scenario stays behavior-identical; only the serialized
  layout grows (new `AgentBuffers.sex` column + new `World` flag →
  `FORMAT_VERSION` bump, golden hashes regenerated).
- **Speciation distance:** slot 33 counts toward genome distance like every
  other non-personality slot (only the 5 Big-Five slots are excluded).
  Dimorphism divergence can therefore contribute to lineage splits — intended.

## Sex model

- `AgentBuffers` gains a serialized `sex: BitVec` column; `false` = female,
  `true` = male. `AgentBuffers::spawn` takes a `sex: bool` argument.
- Founders (`World::spawn_agent` / `spawn_seeded`): when the flag is on, one
  `rng.f32_unit() < 0.5` draw per founder; when off, no draw and `sex` stays
  `false` (unread).
- Births (`reproduce_all`): one draw per newborn, placed immediately before
  `agents.spawn(...)`, gated on the flag so flag-off birth RNG streams are
  unchanged.

## Mating constraint + female choice

In `find_mate` (reproduce.rs):

1. When the flag is on, candidates of the **same sex as the initiator** are
   skipped (zero cost when off — the check is compiled behind the flag value
   passed in as `Option<bool>`).
2. **Choosiness test** (flag on only): identify the female side of the
   candidate pair (initiator or candidate). The pair is rejected iff

   ```
   male_quality < CHOOSINESS_QUALITY_SCALE * female_choosiness
   ```

   where `male_quality = Size_gene × (1 + DIMORPHISM_MALE_DISPLAY × d_male)`
   and `CHOOSINESS_QUALITY_SCALE = 0.8`. Neutral calibration: size 0.5, d 0.5,
   choosiness 0.5 → quality 0.575 ≥ 0.4, so neutral populations mate freely;
   choosiness 1.0 demands quality ≥ 0.8 (top-display males only); choosiness
   0 accepts any male. This creates directional sexual selection on male Size
   and on `d` itself (higher `d` widens the male display distribution),
   paid for by the male upkeep penalty below — the runaway/cost tension the
   milestone wants.

## Dimorphism expression (3 touchpoints, helpers in new `dimorphism.rs`)

Let `d = genome[SexualDimorphism]` of the agent being evaluated.

1. **Basal metabolism** (`integrate.rs`, both locomotor and sessile paths):
   - males: `× (1 + DIMORPHISM_MALE_UPKEEP × d)` (`DIMORPHISM_MALE_UPKEEP = 0.30`)
   - females: `× (1 − DIMORPHISM_FEMALE_EFFICIENCY × d)` (`= 0.20`)
2. **Combat damage** (`interact.rs` weapon pass): males
   `× (1 + DIMORPHISM_MALE_DAMAGE × d)` (`= 0.40`); females ×1.
3. **Mate display quality** (the choosiness rule above): males
   `Size × (1 + DIMORPHISM_MALE_DISPLAY × d)` (`= 0.60`).

All factors read `d` **only when the flag is on**; flag off → exact identity.

Net selection pressure: high `d` makes males better fighters and more
attractive but costlier to run, and females cheaper to run — so `d` evolves
under the tug between sexual/predation advantage and metabolic cost.

## Codex detectors (2 new event types)

New `EventType` variants (appended; `EVENT_TYPE_COUNT` 49 → 51):

- **`SexualSelection` (= 49)** — per-species, gated on the flag and the
  `CYCLE_CHECK_INTERVAL` cadence, reading the existing E5 genome-moment ring:
  fires when slot-33 mean rises ≥ `SEXSEL_MIN_DELTA` (0.15) across the ring
  **and** the newest mean ≥ `SEXSEL_MIN_MEAN` (0.65). Latched per species;
  re-arms if the mean falls below 0.55. `value` = newest mean `d`.
- **`SexRatioCollapse` (= 50)** — per-species with `count ≥ 12`: fires when
  the minority sex count drops below `SEXRATIO_MIN_MINORITY` (3) — the
  population is one bad tick from losing a sex and going reproductively
  extinct. Latched per species; re-arms on recovery ≥ 4. `value` = minority
  fraction. Runs every tick off a new `male_count` accumulator in
  `SpeciesAgg` (added alongside the other per-species sums; `reset()` updated
  — the agg drift-guard test covers this).

Headless: `score.rs` gains `sexual_selection` / `sex_ratio_collapse` in
`ALL_EVENT_NAMES` + `DEFAULT_CORPUS_NT` (`n_t = 0`, post-corpus) +
`event_name`. Viewer: `codex_panel.gd` gains the two names/colors (its boot
assertion pins the arrays to `EVENT_TYPE_COUNT`).

## Viewer + inspector

- `agent_detail` (gdext) adds `sex` (bool), `dimorphism` (slot-33 value), and
  `dimorphism_enabled` (world flag); the inspector panel renders a
  `sex male/female  dimorphism d=…` line only when the flag is on.
- Scenario menu gains `E12 — Sexual dimorphism`.

## Scenario

`scenarios/dimorphism.toml`: `sexual_dimorphism_enabled = true`, a grazer herd
(cluster, 60) under stalker predation (8) — predation makes male damage/size
matter, female choice amplifies display, so `d` and male Size evolve visibly.
Trait overrides `sexual_dimorphism` / `mate_choosiness` are added to
`TraitOverrides` for scenario tuning.

## Testing

- **Unit:** sex assignment ~50/50 and flag-off all-female; opposite-sex
  mating constraint; choosiness rejects a low-quality male and accepts a
  high-quality one; male/female basal factors in integrate; male damage
  factor; detector latch/re-arm logic.
- **Integration (`tests/dimorphism.rs`):** scenario instantiates with both
  sexes present; flag-off behavior of an existing scenario is untouched.
- **Emergence (release-gated):** dimorphism scenario across seeds — prey
  persists, `SexualSelection` or sustained `d` drift observed in a floor
  fraction of seeds.
- **Determinism:** flag-off `minimal.toml` golden hashes are regenerated
  (serialized layout grew; behavior unchanged). Save→load→step round-trip
  covers the new serialized columns (the sex buffer and codex latches are
  plain serde fields).

## Follow-up (not in this milestone)

Domesticable animals: Husbandry-gated taming of juvenile prey, livestock pens,
passive yields (milk/scavenge-style), breeding of tamed stock. Scoping choice
recorded from brainstorming: **taming + livestock pens** model.
