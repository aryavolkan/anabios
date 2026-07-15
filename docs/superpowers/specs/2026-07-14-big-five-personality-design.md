# Big Five (OCEAN) Personality — Design Spec

**Date:** 2026-07-14
**Status:** Approved (brainstorming) → ready for implementation plan
**Baseline:** `main` @ merge of PR #17 + #18 (co-evolution time-series)

## Motivation

Agents should have stable, heritable **personalities** that measurably shape how
they act. We model the Big Five (OCEAN): Openness, Conscientiousness,
Extraversion, Agreeableness, Neuroticism. Each is a genetically-encoded trait in
`[-1,+1]`, **normally distributed across the initial population** and heritable
(so personality *evolves*), and each **dictates concrete behavior** through
hard-coded modulation of an agent's actions.

Exploration finding that grounds this design: the genome already declares
personality-flavored slots (`Aggression`, `Fearfulness`, `Curiosity`,
`SocialAffinity`, `RiskTolerance`, …) but they are **completely inert** — declared
and never read by any behavior code. Initial genomes are `Genome::neutral()` (all
slots = `0.5`, no distribution). Behavior is a hybrid of evolved node-programs
(`program.rs`) that produce an `ActionRegister`, consumed by hard-coded tick
rules. Only `Altruism` (slot 24) is currently wired (energy sharing).

## Design decisions (locked in brainstorming)

- **Storage:** repurpose 5 existing *inert* genome slots (rename in place — indices
  never move, so genome layout stays compatible). Not new slots.
- **Mechanism:** hard-coded behavior modulation (guaranteed, legible, testable) —
  not "expose to evolved programs and hope."
- **Distribution:** `N(0.5, σ)` clamped `[0,1]` for these 5 slots, applied to
  **every** scenario by default.
- **Scope:** sim-only this cycle. No Godot visualization (deferred).

## Trait → slot mapping

Each trait is a **signed axis** in `[-1,+1]`, exposed as `2·g − 1` from the stored
`[0,1]` value (neutral `0.5` → `0.0`).

| OCEAN trait | Repurposed slot (index) | −1 pole ↔ +1 pole |
|---|---|---|
| Openness | `Curiosity` (12) | routine/exploit ↔ novelty/explore |
| Conscientiousness | `RiskTolerance` (21) | impulsive/reckless ↔ careful/prudent |
| Extraversion | `SocialAffinity` (13) | solitary/avoidant ↔ social/seeking |
| Agreeableness | `Aggression` (10) | antagonistic/aggressive ↔ cooperative/peaceful |
| Neuroticism | `Fearfulness` (11) | stable/bold ↔ anxious/reactive |

Notes:
- **Agreeableness reuses slot 10 (`Aggression`)**, folding old aggression onto its
  **negative** pole — the standard Agreeableness-vs-Antagonism axis.
- `Altruism` (slot 24) stays as-is (the live energy-share gate). Agreeableness
  *scales* sharing; it does not replace `Altruism`.
- The old slot names are renamed to the OCEAN names in `GenomeSlot`. Remaining
  inert slots (`KinPreference` 14, `Territoriality` 15, `ExploreVsExploit` 20,
  `AmbushPreference` 22, `CommunicationStrength` 23, `SpeedMax` 25, reserved) are
  left untouched.

## Trait → behavior (hard-coded modulation)

A single isolated pass, `apply_personality(action, genome, sensors)`, runs **after**
the evolved program produces its `ActionRegister` and **before** the tick rules
consume it (integrate/interact/reproduce). It nudges the action intents using the
agent's traits and current percepts. Let each trait `t ∈ [-1,+1]` be the signed
value; `K_*` are tuning gains.

| Trait | Concrete effect on `ActionRegister` / tick params | Testable signature |
|---|---|---|
| **Openness** (O) | scale locomotion magnitude: `move_x,move_y ×= (1 + K_O·O)` (clamped ≥ 0) — open agents range/disperse; closed stay local | high-O pop → larger mean per-agent displacement/dispersal |
| **Conscientiousness** (C) | raise effective reproduction energy threshold `×(1 + K_C·C)` **and** boost `feed_intent` when energy below a comfort fraction — prudent provisioners vs impulsive breeders | high-C pop → higher mean energy at reproduction |
| **Extraversion** (E) | add movement bias toward `sensors.nearest_same_dir` scaled by `K_E·E` (when `has_neighbor`) **and** scale `broadcast_intent ×= (1 + K_E·max(0,E))` | high-E pop → higher mean same-species crowding |
| **Agreeableness** (A) | scale `share_intent ×= max(0, A)` **and** scale same-species `fire_intent ×= clamp(1 − K_A·A, 0, 1)` (negative pole → attacks kin more) | high-A pop → more sharing, fewer intra-species combat deaths; low-A → more combat |
| **Neuroticism** (N) | when a bigger/other-species neighbor is within perception, amplify flee: add move-away along `−sensors.nearest_other_dir` scaled by `K_N·max(0,N)`, and dampen `feed_intent`/`mate_intent` under that threat | high-N pop → stronger displacement away from threatening neighbors |

All modulation reads only existing `SensorRegister` fields (`nearest_same_dir`,
`nearest_other_dir`, `nearest_rel_size`, `has_neighbor`, `crowding`, etc.) and
existing `ActionRegister`/genome data — no new `World` state, no new sensors.

## Architecture

New/changed units, each with one clear responsibility:

- **`crates/anabios-core/src/personality.rs` (new).** Home of:
  - The tuning constants `K_O, K_C, K_E, K_A, K_N, INIT_SIGMA` and a comfort
    fraction for C.
  - `apply_personality(action: &mut ActionRegister, genome: &Genome, sensors: &SensorRegister)`
    — the modulation pass. The reproduction-threshold part of C is applied where
    the threshold is computed (reproduce.rs), reading a helper here.
  - `personality_reproduction_factor(genome: &Genome) -> f32` = `1 + K_C·C`,
    consumed by reproduce.rs.
- **`crates/anabios-core/src/genome.rs`.** Rename slots 10/11/12/13/21 to the OCEAN
  names in `GenomeSlot`. Add signed accessors:
  `openness()/conscientiousness()/extraversion()/agreeableness()/neuroticism() -> f32`
  each returning `2·get(slot) − 1`. Add
  `sample_personality_in_place(&mut self, rng: &mut Rng)` that overwrites the 5
  personality slots with `gaussian(0.5, INIT_SIGMA).clamp(0,1)`.
- **`crates/anabios-core/src/scenario.rs`.** In `instantiate`, after
  `neutral()` + archetype + `TraitOverrides::apply`, call
  `sample_personality_in_place(&mut world.rng)` for each spawned agent at a fixed
  point in the per-agent loop (deterministic). Extend `TraitOverrides` with
  optional `openness/conscientiousness/extraversion/agreeableness/neuroticism`
  fields (each an `Option<f32>` in stored `[0,1]` space); when present they win over
  the Gaussian draw (so scenarios/tests can pin a population's personality).
- **`crates/anabios-core/src/tick.rs`.** In the decide stage, after program
  evaluation fills each agent's `ActionRegister` and before integrate/interact,
  call `apply_personality(&mut action, genome, sensors)` per alive agent.
- **`crates/anabios-core/src/reproduce.rs`.** Multiply the effective reproduction
  energy threshold by `personality_reproduction_factor(genome)`.

Data flow per tick (unchanged except the new modulation step):

```
sense → program.evaluate → ActionRegister
      → apply_personality(action, genome, sensors)   [NEW]
      → integrate (move) / interact (feed/fire/share) / reproduce (mate)
```

## Determinism

This is a **deterministic-core change** (behavior + init both change). The three
golden hashes in `crates/anabios-core/tests/determinism.rs` WILL change. Plan
includes a deliberate **golden-hash refresh**: implement, run the sim to capture
the new hashes, update the pinned values, and note the refresh in the commit. The
personality RNG draws use the scenario's seeded `world.rng` at a fixed code point,
so the new state is fully deterministic and reproducible.

## Testing

- **Unit (`personality.rs`, `genome.rs`):**
  - Signed mapping: `neutral` slot (0.5) → `0.0`; `1.0` → `+1.0`; `0.0` → `−1.0`.
  - `sample_personality_in_place`: over many draws with a fixed seed, mean ≈ 0.5,
    values clamped to `[0,1]`, non-degenerate spread.
  - Each modulation effect on a hand-built `(ActionRegister, Genome, SensorRegister)`:
    e.g. high-O scales move magnitude up; high-A raises `share_intent` and lowers
    same-species `fire_intent`; high-N with a nearby larger other-species neighbor
    adds move-away; assert direction/sign, not exact magnitudes.
- **Behavioral integration (proves "agents act according to them"):** for each
  trait, instantiate two populations pinned high vs low via `TraitOverrides`, run N
  ticks, assert the *testable signature* from the table (high-E → higher mean
  same-species crowding; high-A → fewer intra-species combat deaths; high-N →
  greater displacement from threats; high-C → higher mean energy at reproduction;
  high-O → greater dispersal). Tolerances chosen to be robust to RNG.
- **Determinism:** golden test passes against the **refreshed** hashes.
- **CI gate:** `rustup run stable` fmt/clippy/doc with `-D warnings`; commit fmt
  output; escape `[0,1]`/`[N]` in doc comments.

## Out of scope (deferred to later cycles)

- Godot/frontend visualization of personality (body-color-by-trait overlay,
  inspector fields).
- A codex detector for personality niches / assortative behavior.
- Program-exposed personality (letting evolved programs read the traits as inputs)
  beyond what `SenseGenome` already allows.
- Re-tuning the other still-inert drive slots.

## Success criteria

Every scenario spawns a population with normally-distributed OCEAN personalities;
the five traits are heritable and evolve; and trait-pinned populations exhibit the
predicted behavioral differences in integration tests — all deterministically,
with the golden hashes refreshed and CI green.
