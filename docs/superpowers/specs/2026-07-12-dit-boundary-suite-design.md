# DIT boundary suite — design

**Date:** 2026-07-12
**Branch:** `experiment/gene-culture-coevolution`
**Status:** approved, ready for implementation

## Goal

Characterize *where gene–culture coevolution (dual inheritance theory) works and where
it fails* in anabios, as an explicit test matrix. Prior work on this branch established:

- **A** — seeded meme→behavior triggers are neutral-to-maladaptive (the baseline meme is a
  scalar trigger redundant with the genetic program).
- **C** — a cumulative, gene-gated cultural *skill* (learned-by-doing + socially copied) is
  adaptive head-to-head (14/20 seeds).
- **B** — that gene does not reliably sweep from standing variation (no assimilation channel).

This suite adds the canonical DIT axis — **environmental variability** (Rogers' paradox,
Boyd–Richerson) — plus the full four-strategy taxonomy, and pins the boundary with directional
assertions rather than printouts.

## The predicted boundary

| Env change rate | Genetic (innate) | Cultural (learner) | DIT verdict |
|---|---|---|---|
| Static | evolves to fixed optimum, cheap | pays learning cost for no edge | culture **FAILS** |
| Slow / intermediate | lags (mutation too slow) | tracks via learn+copy | culture **WORKS** |
| Fast (period ≪ copy time) | lags | copies stale info, also lags | culture **FAILS** |

Rogers' paradox refines the cultural column: a *pure imitator* (copy, never individually learn)
invades an individual-learner population because it is cheaper, but as imitators become common
they copy each other's stale info and gain no adaptive edge — culture fails to raise mean
fitness. Coupling imitation with individual correction (a *critical learner*) resolves it.

## Substrate additions (determinism-gated on `env_period == 0` being inert)

1. **`World.env_period: u32`** (serde default `0`). The only new persistent field. `0` = the
   env mechanism is fully off, so every existing scenario (including `minimal.toml`) is
   behaviorally unchanged. Because `state_hash` bincode-serializes the whole `World`, adding
   this field grows the payload → a **single one-time golden refresh** of `determinism.rs`
   (minimal.toml keeps `env_period = 0` and stays deterministic).
2. **`env_optimum` is derived, not stored** — a pure function `env_optimum_at(tick, period)`
   returning a value in `[0,1]` (a fixed deterministic square wave / sequence). No RNG, no
   second field. A `period` sentinel (e.g. `u32::MAX`) means "active but static" (never shifts).
3. **Three reserved genome slots renamed** (index-stable, bincode-neutral; all default `0.0`):
   - `InnateTechnique` (slot 40) — the genetic strategy's fixed, heritable, mutable technique.
   - `IndividualLearning` (slot 28) — `> 0.5` ⇒ learns-by-doing toward the current optimum,
     paying `LEARN_COST` (costly, always current).
   - `SocialLearning` (slot 29) — `> 0.5` ⇒ copies the best-matched neighbour's `meme[6]`.
4. **`TECH_CHANNEL = meme[6]`** — the cultural technique (SKILL = `meme[5]` untouched).
5. **`feed_pass` (only when `env_period > 0`)**: `match = 1 − |technique − env_optimum|`;
   `bite *= 1 + ENV_BONUS · match.clamp(0,1)`. Technique source is the genome `InnateTechnique`
   for genetic agents, `meme[6]` for cultural agents. Env-mode and C's monotonic-skill bonus
   are **mutually exclusive** (a cultural agent in env mode uses the match bonus, not the C
   skill bonus) so the two mechanisms never compound.
6. **`culture_step`**: the technique copy toward the best-matched neighbour is gated on
   `SocialLearning > 0.5` (and Communicator, for perception realism), independent of the
   existing meme-channel lerp.

## The four strategies (corners of the 2×2, plus innate)

| Strategy | Communicator | IndivLearn | SocialLearn | Technique source |
|---|---|---|---|---|
| innate (genetic) | – | 0 | 0 | genome `InnateTechnique` (mutates slowly) |
| individual_learner (IL) | ✓ | 1 | 0 | `meme[6]`, self-sampled each life, costly |
| pure_imitator (SL) | ✓ | 0 | 1 | `meme[6]`, copied only (Rogers variant) |
| critical_learner | ✓ | 1 | 1 | `meme[6]`, copy **and** self-correct |

New archetypes in `scenario.rs`: `innate_forager`, `individual_learner`, `pure_imitator`,
`critical_learner`. New scenarios under `scenarios/` for each test cell.

## Tests — `crates/anabios-core/tests/dit_boundary.rs`

All `#[ignore]`-gated analysis harnesses (run with `--release --ignored --nocapture`), each
asserting the predicted direction so "works/fails" is encoded, not merely printed:

1. `dit_env_static_genes_win` — static env ⇒ innate lineage out-grows the learner lineage.
2. `dit_env_slow_culture_wins` — intermediate period ⇒ learner tracks, innate lags; learner wins.
3. `dit_env_fast_culture_stale` — period ≪ copy time ⇒ learner is no better than innate.
4. `dit_rogers_imitator_invades_no_gain` — imitators rise in frequency yet carry *worse* mean
   technique-match than the ILs they copy (invasion without adaptive gain).
5. `dit_rogers_critical_learning_resolves` — critical_learners reach higher mean match / out-grow ILs.
6. `dit_social_clustered_vs_dispersed` — existing C mechanism: clustered (copy channel open) beats
   dispersed (solo only).
7. `dit_scarcity_vs_abundance` — existing C mechanism: culture edge binds under scarcity, vanishes
   under abundance.

## Determinism & CI plan

- One golden refresh (new `env_period` field). Ticks 0/100 expected unchanged in spirit but the
  whole-world bincode payload grows, so all three golden entries are regenerated and verified
  stable across two runs.
- New nodes/slots excluded from any mutation grammar that would perturb `minimal.toml` behavior;
  the env mechanism is inert at `env_period = 0`.
- Full CI-accurate gate on the stable toolchain: `fmt --all --check`,
  `clippy --workspace --all-targets -- -D warnings`, rustdoc `-D warnings`, workspace tests, and
  the release emergence + `dit_boundary` harnesses.

## Out of scope (YAGNI)

- No new modules — strategies are pure genome/meme configurations.
- No spatial environmental gradients (env_optimum is global per tick).
- No change to the C skill mechanism or the A/B harnesses.
