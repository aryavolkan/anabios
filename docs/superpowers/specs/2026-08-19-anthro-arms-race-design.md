# Anthropogenic Arms Race — design (2026-08-19)

> One subsystem, opt-in per scenario: **wild animals evolve aimed defenses
> against a scenario-declared culture-bearing ("human") lineage, and the codex
> measures the trait race.** Flag: `anthro_race_enabled`. New codex event:
> `HuntedAdaptation` (id 59).

## Motivation

anabios already has both sides of a human–animal arms race but nothing that
*aims* them at each other:

- Humans (the emergent `is_ape` niche: large omnivore + Communicator + IQ +
  inventions) escalate through Metalworking-boosted weapons, Fire, and
  Husbandry; every cross-species hit already feeds the lineage-rooted war
  substrate (`codex/war.rs`).
- Animals have the full defensive repertoire — armor/speed modules, FEAR and
  the freeze-flight-fight hijack, `SenseHostility`, herd cohesion — but their
  perception cannot distinguish a tool-bearing hunter from any other
  neighbor, and no detector pairs cultural advance against wild-trait
  adaptation (the existing `ArmsRace` detector is world-global,
  weapon↔armor only).

This subsystem closes the aim gap with four components and one measurement,
following the repo's opt-in recipe (flag → scenario plumbing → tick hook →
codex detector → tests → determinism rehash).

## Design decision: explicit species tag

"Human" is an **explicit scenario tag**, not the emergent `is_ape` predicate
(rejected: a morphological check would silently re-tag any lineage that
drifts into the primate band mid-run, making experiments non-comparable).
The tag is set per founder `AgentSpec` and inherited through speciation via
lineage-root descent, so emergent splinters of a tagged founder stay "human"
without the author annotating anything but the founder.

## Component A — culture-lineage tag

- `AgentSpec.culture_bearer: bool` (default `false`).
- `World.culture_roots: BTreeSet<u32>` (serialized) — species ids of tagged
  founders, collected at `instantiate`.
- `species::is_culture_lineage(world, sid)`: `culture_roots` contains
  `codex::war::lineage_root(world, sid)` — the same ancestor walk war uses,
  so speciation splinters inherit the tag.
- Validation: `culture_bearer` without `anthro_race_enabled = true` is a
  `parse_toml` error (fail-fast like `KnowledgeNeedsInventions`).

## Component B — threat perception

- `SensorRegister.culture_threat: f32` (`#[serde(skip)]` scratch, like
  `hostility`): `0.0` unless the nearest *other-species* neighbor is a
  culture-lineage agent carrying a weapon; then its weapon damage normalized
  by `module::DAMAGE_MAX`, in `[0,1]`. The lineage check reads a per-species
  `#[serde(skip)] Vec<bool>` mask refreshed each tick before `sense_all`
  (rebuilt from `culture_roots` + `species_parents`; empty when the flag is
  off → all registers exactly 0.0, byte-identical).
- New program input node `Node::SenseCultureThreat` — appended at the END of
  the `Node` enum (bincode positional stability), `node_kind` 47, pushed
  from `EvalContext.culture_threat`. Joins the `random_node` mutation pool
  only when `anthro_race_enabled` (gated extras stay ordered: hostility →
  anchors → culture-threat, so flag-off pools are byte-identical). Prey can
  now evolve "flee the tool-users" programs directly.
- Affect read path (only when `affect_enabled` is also on): `fear_trigger`
  gains a `K_FEAR_CULTURE_THREAT × culture_threat` term, applied before the
  Boldness gain. Gated on a weight parameter that is exactly `0.0` with the
  flag off (term skipped → bit-identical).

## Component C — Vigilance gene (genome slot 43)

- `_SensoryReserved43` → `Vigilance`: heritable wariness in `[0,1]`. Read
  only when `anthro_race_enabled`: scales the FEAR input by `2 × v` (neutral
  genome 0.5 ⇒ exact identity gain 1.0; 1.0 ⇒ hair-trigger; 0.0 ⇒ fearless).
  The survival hijack responds indirectly through elevated FEAR arousal.
- Counts toward speciation distance (non-personality slot) — a hunted herd
  that drifts wary can speciate from its unwary cousins, which is the
  animal side of the race made visible.
- `TraitOverrides.vigilance` lets scenarios seed the distribution.
- No hijack-threshold term in v1: FEAR gain already modulates arousal, and a
  single read site keeps the causal story clean.

## Component D — `HuntedAdaptation` detector (`codex/anthro.rs`)

The measurement: a hunted prey lineage answers its culture predator's rise.

- **Pairing**: iterate the war substrate's `hostility` map (already keyed by
  lineage-root pair). A pair qualifies as hunter/hunted when exactly one
  root ∈ `culture_roots` and the current hostility episode has
  `kills ≥ HUNTED_MIN_KILLS` (3).
- **Lineage aggregation**: current species are folded into roots via
  `lineage_root` each tick (same walk as war).
- **Culture power index** (per hunter root): `era + weapon_mean /
  DAMAGE_MAX`, where `era` is the highest invention era at ≥50% lineage
  adoption (from `SpeciesAgg.invention_counts`; 0 when the tree is off) —
  tech climb and weapon escalation both register.
- **Prey defense channels** (per prey root): normalized armor
  (`armor_mean / PROTECTION_MAX`), raw mean speed (`speed_sum / count`), and
  mean Vigilance (`genome_sums[43] / count`) — tracked as THREE separate
  channels, not a sum (see the evidence note: a sum lets one channel's
  decline mask another's rise).
- **Signal**: when a pair qualifies, both sides' indices are latched as a
  **sticky baseline** (`CodexState.hunted_baselines`), kept across war
  episodes until a lineage goes extinct. Fire when
  `culture_now − culture_base ≥ HUNTED_MIN_CULTURE_DELTA` (0.05) AND
  `max(Δarmor, Δspeed, Δvigilance) ≥ HUNTED_MIN_PREY_DELTA` (0.05). One shot
  per pair per continuous qualification (`hunted_active` latch; pruned only
  on extinction).
- Event payload: `species_id` = prey lineage's lowest active species id,
  `value` = the winning prey-channel rise, `loc` = prey centroid.
- Gated in `observe_all` on `world.anthro_race_enabled` (like the affect
  detectors); zero state touched when off.

### Why not a rolling co-rise window (measured)

The first cut compared windowed first-vs-last samples of a summed prey index
(`HUNTED_WINDOW` = 200 ticks). Diagnostics on the reference scenario showed
two structural mismatches, and the design above fixes both:

1. **Timescale**: the culture climbs in ~1k-tick era bursts then sits flat
   (era 1→4 by ~t15k, flat within any 200-tick window thereafter), while
   prey traits grind slowly — the two signals never peak inside one short
   window. Divergence-from-baseline is the actual race outcome.
2. **Episode reset**: hostility kill counts reset at every `WarEnded` and
   cooled records are pruned, so a kills≥3 qualification latched the baseline
   only *after* both sides had already risen. Baselines must be sticky
   across episodes.

The detector credits **correlation** (hunted + culture rose + prey trait
rose), not causation — the same standard as the existing world-global
`ArmsRace` detector.

## Plumbing & tables

- `World.anthro_race_enabled` + `Scenario.anthro_race_enabled` (both
  `#[serde(default)]`), copied in `instantiate` with the other flags.
- `EventType::HuntedAdaptation = 59` appended; `EVENT_TYPE_COUNT` follows.
- `anabios-headless/src/score.rs`: `ALL_EVENT_NAMES` += `"hunted_adaptation"`;
  `DEFAULT_CORPUS_NT` += `("hunted_adaptation", 0)` (novel → novelty bonus).
- Godot `codex_panel.gd`: `CHAPTER_NAMES` += `"HuntedAdapt"`, colors += 1.
- Serialized `World`/`CodexState` change ⇒ `FORMAT_VERSION` 33 → 34, and the
  minimal-scenario golden hashes are regenerated (`UPDATE_HASHES=1`) — the
  schema change alone perturbs `state_hash` even though flag-off *behavior*
  is bit-identical.

## Scenario

`scenarios/anthro-race.toml`: a tagged `innovator` culture (seeded
`stone_tools`, `inventions_enabled` so the era climb is live) sharing a range
with wild `herd` grazers and a `mammal_pursuer` predator;
`affect_enabled` + `war_enabled` + `anthro_race_enabled` on. Auto-smoke-tested
by `tests/all_scenarios.rs`; mapped in `docs/scenarios.md`.

## Tests

- Flag-off inertness: `culture_threat` register stays 0.0; `SenseCultureThreat`
  never enters the mutation pool; `fear_trigger` unchanged; detector silent.
- Tag: `is_culture_lineage` true for founder + reassigned splinter species,
  false for wild founders; scenario validation rejects untagged flag-off use.
- Sense: threat = damage/`DAMAGE_MAX` with an armed tagged neighbor; 0 for
  unarmed/untagged/none.
- Detector: hand-driven ring histories fire once per pair, latch, and re-arm
  (shape copied from `codex/domestication.rs` tests).
- Integration (`crates/anabios-core/tests/`): the anthro-race scenario steps
  1k ticks; with the flag on the sensor/node/detector paths are exercised;
  two flag-on runs hash identically (determinism).

## Evidence plan

A/B sweep, flag on vs off on the same scenario: `hunted_adaptation` must
fire only in the on arm; prey-lineage defensive traits must diverge vs the
off arm; the emergence scorecard counts the new event as novel (corpus
count 0).

## Measured results (2026-08-19, seed-pinned runs)

Reference scenario `scenarios/anthro-race.toml`, 20k ticks:

- **The event fires emergently**: `HuntedAdaptation` in **6/8** seeds
  (`runs/anthro-on-20k`). The off arm structurally cannot fire (detector
  gated; no culture roots).
- **Hunting pressure is real**: the tagged lineage logged 174 kills on herd
  A by t5k in the diagnostic run (seed 7).
- **The prey answer is speed, not vigilance**: at t20k the hunted herd's
  mean speed was **0.63 vs 0.25 in the flag-off arm** (drift), while its
  mean Vigilance *fell* slightly (0.47 → 0.41 on-arm vs a drift rise to
  0.67 off-arm — the FEAR response costs feeding time and wasn't selected).
  Hunters also suppressed the herd below the off-arm population cap
  (~100–250 vs 300). This channel asymmetry is why the detector uses a
  per-channel max rather than a summed index.
- **Scenario lethality**: half the 20k runs end in total collapse (alive=0)
  — the overhunted world dies. The event fires before collapse; tuning the
  scenario for longer coexistence is follow-up work, not a detector fix.
- Without the `stone_tools` seed the culture lineage failed to establish
  (0 kills in 20k) — the seed is load-bearing in this scenario, mirroring
  the Out-of-Africa climb problem (ROADMAP Phase 2).

## Explicit non-goals (v1)

- No man-eater detector, livestock raiding, overkill attribution, carcass
  contests, or fire ecology — those are the follow-on slate this subsystem's
  tag + perception substrate is designed to enable.
- No metabolic cost for Vigilance beyond the existing FEAR/hijack behavior
  costs (a fear-driven agent flees instead of feeding — the cost is
  behavioral, not a new upkeep term).
