# Mutation-Gated Cumulative Cultural Inventions — Design Spec

**Date:** 2026-07-18
**Status:** Design (pre-plan)
**Builds on:** the living-sandbox finding (PR #29) that a *saturating* foraging-skill benefit does not durably beat the Communicator's persistent upkeep in an abundant biome.

## 1. Goal & success criteria (staged)

Give culture a **robust, cumulative** benefit — a mutation-gated cultural *invention* ratchet whose payoff (a) does not saturate with food abundance and (b) converts to fitness *above* the reproduction threshold — so culture-gene coevolution can finally hold. Two stages:

- **Stage 1 — Lineage differential (the fix).** In the *abundant living biome* (the condition PR #29 failed), an **inventive-culture** cohort (carries the `Inventiveness` gene **+** a Communicator) reliably out-reproduces a **control** cohort (neither), across ≥ 7/10 seeds with a positive mean log-ratio. Because the cohorts now differ in the **genome** (the `Inventiveness` gene), speciation keeps them apart — this also fixes the cohort-tally leak flagged in PR #29's review.
- **Stage 2 — Gene sweep (the reach).** In one interbreeding population seeded ~15% with `Inventiveness` (+ Communicator), the gene frequency rises toward fixation across seeds — the full gene-culture coevolution the prior first-principles experiment B failed.

Both are `#[ignore]` **reporting** harnesses (print a verdict; do not assert a forced pass — parameter-hunting a pass is p-hacking). A negative Stage-2 is a legitimate result; Stage 1 is the primary deliverable.

## 2. Why this should work (mechanism, corrected)

PR #29's review established the real failure mechanism: it is **fitness-threshold**, not per-bite saturation. `graze` returns `desired.min(biomass)`, so a bigger (skill-boosted) desired bite actually takes *more* in abundance. The problem is that under abundance **both cohorts clear the reproduction-energy threshold regardless of skill**, so the extra intake does not convert into extra offspring — while the Communicator module's upkeep persists as a net drag. A benefit that fixes this must **lower a persistent cost or the breeding threshold**, so it produces offspring even when food is plentiful. The invention tech-tree is built from exactly such benefits.

## 3. Mechanism

### 3.1 The gene — `Inventiveness`
- Rename the currently-inert genome slot **42** (`_SensoryReserved42`) to `Inventiveness` (`f32 ∈ [0,1]`).
- **Threshold-gated:** an agent is *inventive* iff `genome[Inventiveness] > 0.5`.
- It is **NOT** in `PERSONALITY_MASK`, so it counts toward `Genome::distance` / speciation — inventive vs non-inventive lineages diverge genetically (fixes the cohort leak). Mutates via the ordinary Gaussian `mutate_in_place`.

### 3.2 The ratchet — an invention-level meme channel
- Reuse meme **channel 7** as `INVENTION_CHANNEL` (`meme_vector[i][7] ∈ [0,1]`, the *invention level*). No new per-agent field (meme_vector is already `[f32; 8]`, serialized).
- **Invent (solo, slow):** each foraging tick, an inventive agent **with a Communicator** raises its level: `inv += INVENT_RATE * (1 - inv)` (`INVENT_RATE ≈ 0.01`).
- **Copy (social, fast):** in `culture_step`, an inventive Communicator copies the highest-level inventive neighbor: `inv += INVENT_SOCIAL_RATE * max(0, best_neighbor_inv - inv)` (`INVENT_SOCIAL_RATE ≈ 0.15`) — social outpaces solo, the ratchet.
- **Retention across generations:** the level rides in `meme_vector`, inherited via the existing `inherit_meme` (parent-average + jitter), so culture *accumulates* (the cumulative-culture ratchet). Non-inventive agents (no gene) never raise or apply the level, even if they inherit a nonzero value — the benefit is gated on the gene at application time.

### 3.3 The tech-tree — stacking robust benefits
The invention level unlocks cumulative benefits at thresholds; **all gated on `inventive && has(Communicator)`** and on `World.cultural_inventions`. Each is robust (non-saturating) and converts to fitness above the reproduction threshold:
1. `inv ≥ 0.34` → **Efficiency** — a per-tick metabolic discount `EFFICIENCY_DISCOUNT` (≈ 0.004) subtracted from the agent's upkeep in `module::upkeep_all` (directly offsets the Communicator upkeep, `UPKEEP_BASE=0.005`, that sank culture before). Clamped so upkeep never goes negative.
2. `inv ≥ 0.67` → **Tooling** — a flat **additive** `TOOL_BONUS` (≈ 0.3) energy on a successful graze in `feed_pass` (added to the energy gained, NOT a multiplier on the bite → cannot saturate with abundance).
3. `inv ≥ 1.0` → **Provisioning** — the effective reproduction-energy threshold (`GenomeSlot::ReproductionThreshold`, slot 30) is reduced by `PROVISION_DISCOUNT` (≈ 0.05) in `reproduce.rs` (breed sooner → more offspring, converts in abundance).

Benefits **stack** (an agent at `inv = 1.0` gets all three). Starting values are modest and tuned in the experiment.

### 3.4 Flag
- `World.cultural_inventions: bool` (`#[serde(default)]`, default false) + a matching `Scenario` knob. Every new codepath early-exits when off → existing scenarios byte-identical.

## 4. Determinism

- Uses existing `genome`/`meme_vector` fixed arrays (no new per-agent fields). The only new serialized field is `World.cultural_inventions: bool` → one reviewed golden refresh; flag-off is byte-identical (verified by the `default_dims_byte_identical` trajectory fingerprint + golden).
- All invention math is deterministic: fixed iteration order (ascending id in `culture_step`), no new RNG in the hot path. Renaming slot 42 does not change its value semantics (still `0.5` neutral default), so flag-off generation/behaviour is unchanged.
- Bump `FORMAT_VERSION` for the new `World` field.

## 5. Experiment

- **Scenario** `scenarios/cultural-inventions.toml` (Stage 1): living biome (reuse the 2048 living-sandbox settings), `cultural_inventions = true`, two cohorts — culture = a new `inventive_forager` archetype (`starter_kit` + Communicator, `Inventiveness` gene set high) vs control = `asocial_forager`. Cohorts differ in the genome (Inventiveness) → clean speciation.
- **Stage-1 harness** extends/reuses `tests/living_sandbox.rs` structure: culture-vs-control descendant differential across seeds in the living biome; report verdict vs the ≥7/10 bar.
- **Stage-2 harness** `tests/inventions_sweep.rs`: one interbreeding population (species 0), ~15% seeded with `Inventiveness` + Communicator, run N seeds × T ticks, track the `Inventiveness`-gene frequency over time (rose/fell), report.

## 6. Testing

- Flag-off byte-identity (golden refresh for the new bool; `default_dims_byte_identical` stays green).
- Unit tests: the ratchet (level rises with the gene+Communicator, stays 0 without; social copy outpaces solo); each tech-tree benefit applies only at/above its threshold and only when inventive (Efficiency reduces upkeep, Tooling adds graze energy, Provisioning lowers the effective threshold); benefits stack.
- The two staged harnesses (reporting).
- CI gates (`fmt`, `clippy -D warnings`, `doc`, `test --workspace`) green throughout.

## 7. Risks & open questions

- **Stage 2 (gene sweep) may still fail** — it is the ambitious bar; even a robust benefit can be defeated by the Communicator upkeep + invention ramp-up lag if the gene's carriers can't establish the ratchet before selection acts. That's a research result. Stage 1 (differential) is the primary, more-likely-to-succeed deliverable.
- **Parameter tuning** (`INVENT_RATE`, `INVENT_SOCIAL_RATE`, the three benefit magnitudes, the thresholds) will need a sweep; the harnesses parameterize via env like the living-sandbox one.
- **Balance:** benefits too strong → runaway/competitive-exclusion again; too weak → no differential. Start modest.
- **Inheritance of level into non-inventive offspring:** harmless (gated at application), but means a non-inventive child of inventive parents carries a latent level that reactivates if it later mutates inventive — a mild Baldwin-adjacent effect worth watching, not a bug.

## 8. Out of scope

The full genetic-assimilation (Baldwin) channel as a separate mechanism; domestication; writing/meme persistence; a frontend visualization of invention level (a follow-up once the mechanism validates); more than three tech-tree tiers.
