# Weapons branch + demographic payoff — design (2026-09-07)

> Four new inventions extend the tech tree with a military line whose payoffs
> are routed at the **birth ledger**, not just energy — the mechanism cycle the
> O3 handoff prescribes ("inventions don't pay demographically"; buff
> economics must give invention-holding lineages a demographic edge).
> `INVENTION_COUNT` 10→14, `MEME_CHANNELS` 20→24, `FORMAT_VERSION` 39→40.
> No new flag, no new codex events.

## Motivation

Two threads converge:

1. **The tech→combat channel is one number.** Weapons are evolvable body
   modules (`Weapon`/`Jaws`/`Spines`); the *entire* cultural contribution to
   combat is Metalworking's +50% damage multiplier
   (`invention::weapon_multiplier_coupled`, read at `interact.rs`
   `combat_pass`). There is no ranged-tech, no defensive tech, and combat is a
   pure drain for the attacker (`energy_cost` paid, nothing gained; payoff only
   via later corpse scavenging).
2. **Inventions don't pay demographically** (O3 handoff,
   `2026-09-02-o3-ape-composition-findings.md` §"the remaining gap, named";
   ROADMAP O3 status). Cultural dominance and invention activity
   anti-correlate; the corrected apparatus measured that **energy does not
   bind founder share at the cap — the margin lives on the birth ledger**
   (practices tax it). Every existing invention buff is energy-shaped, which
   is why the package loses the intra-cultural contest.

The design marries them: a weapons/security branch whose top payoff is a
**birth-ledger subsidy** — symmetric with practices, which are birth-ledger
taxes — plus combat payoffs (spoils, range, defense) that make the branch
worth fighting with along the way.

## The four inventions (appended ids 10–13)

New inventions are **appended** to `INVENTIONS`; ids 0–9 and their meme
channels are untouched (renumbering would break scenario `starting_inventions`
keys, codex `value = id` payloads in recorded decks, and viewer arrays). The
`candidates()` doc comment "visits ids ascending (era order)" becomes "visits
ids ascending" — the prereq-DAG test only requires prereqs to reference
earlier ids, which appending satisfies.

| id | Invention | Key | Era | Prereqs | Materials [salt,obsidian,amber,spice] |
|----|-----------|-----|-----|---------|----------------------------------------|
| 10 | Hafted Spears | `hafted_spears` | 1 | Stone Tools | [0,1,1,0] |
| 11 | Archery | `archery` | 2 | Hafted Spears | [0,1,2,0] |
| 12 | Fortifications | `fortifications` | 3 | Farming + Hafted Spears | [1,1,0,1] |
| 13 | Steel Arms | `steel_arms` | 3 | Metalworking + Archery | [1,2,1,0] |

Basket sizing is deliberate: the O3 findings decomposed the era gate to
**materials** (~0% of apes hold a basket under the trade freeze), so the
era-1/2 entries stay cheap (totals 2–3). Verified against the existing table
tests: per-good ≤ `STOCK_TARGET`, totals ≤ `INVENTORY_BASE_CAP`, and per-era
average basket size stays non-decreasing (2.33 / 3.33 / 3.8 / 5.33).

### Affinities and gene gates

| Invention | Affinity (coeff 0.8) | GeneReq | Rationale |
|-----------|----------------------|---------|-----------|
| Hafted Spears | Territoriality | none (era-1 entry tech stays free) | hunting/holding ground; shares Metalworking's slot |
| Archery | Extraversion | Extraversion ≥ 0.40 | band hunting is social coordination; wires a tree-unused personality slot |
| Fortifications | Conscientiousness | Conscientiousness ≥ 0.45 | building/planning; shares Farming's slot |
| Steel Arms | Territoriality | Territoriality ≥ 0.50 | the Metalworking line escalated |

Slot sharing has precedent (Openness serves Fire and Electricity). The
gene-gate era-monotonicity test still passes: era maxima stay 0.30 / 0.40 /
0.50 / 0.65.

### Effects (constants in `invention/params.rs`)

| Invention | Buff | Debuff |
|-----------|------|--------|
| Hafted Spears | `SPEARS_DAMAGE = 0.25` (weapon damage), `SPEARS_SPOILS = 0.30` (fraction of net damage recovered as attacker energy) | none |
| Archery | `ARCHERY_RANGE = 0.50` (weapon reach), `ARCHERY_DAMAGE = 0.15` | `ARCHERY_UPKEEP = 0.003`/tick |
| Fortifications | `FORT_DEFENSE = 0.25` (incoming net damage reduced 25%), `FORT_BIRTH_SUBSIDY = 0.15` (effective reproduction threshold ×0.85) | `FORT_SPEED = -0.10` (locomotor) |
| Steel Arms | `STEEL_DAMAGE = 0.60` (stacks additively with Metalworking's 0.50 inside the same multiplier), `STEEL_SPOILS = 0.20` (added to the spoils fraction) | `STEEL_UPKEEP = 0.10` (module upkeep, stacks with Metalworking's) |

## New effect hooks

All follow the existing identity-at-mask-0 multiplier pattern in
`invention/mod.rs` — exact `1.0`/`0.0` when unheld, so every scenario without
the new techs (including all flag-off worlds) is byte-identical in the
arithmetic. No hook consumes RNG.

### 1. Spoils transfer — `combat_pass` (attacker side)

After `net` is computed and subtracted from the target:

```rust
world.agents.energy[i] += invention::spoils_fraction(mask_i) * net;
```

`spoils_fraction(mask) = SPEARS_SPOILS·held(SPEARS) + STEEL_SPOILS·held(STEEL)`.
`net` here is the **final** net — after armor subtraction AND the target's
defense multiplier (hook 2) — i.e. exactly what the target lost, so the
conservation invariant holds against fortified targets too.
This is an energy **transfer** bounded by damage actually dealt — never
creation (the drinking-never-adds-energy invariant class applies: spoils may
only redistribute what the target lost; an invariant test asserts
`attacker gain ≤ target loss` for all masks). Combat is already
cross-species-only (`nearest_other_id`), so spoils cannot farm conspecifics.
Coupled variant: spoils scale with the attacker's affinity gene under
`gene_tech_coupling`, same shape as the damage multiplier.

### 2. Defense multiplier — `combat_pass` (target side)

The first *target-side* invention read in combat:

```rust
let net = (damage - armor).max(0.0) * invention::defense_multiplier(mask_t);
```

`defense_multiplier(mask) = 1.0 − FORT_DEFENSE·held(FORTIFICATIONS)` (floor
well above 0; single term). The target's held mask is computed from its meme
vector at hit time. Combat stays deterministic and RNG-free.

### 3. Weapon-range multiplier — `combat_pass` (range check)

The `weapon.range` comparison becomes
`dist < weapon.range * invention::range_multiplier(mask_i)` with
`range_multiplier = 1.0 + ARCHERY_RANGE·held(ARCHERY)`. Only the combat reach
check changes; `sense` radii and the culture-threat register are untouched
(the anthro-race threat signal stays `damage/DAMAGE_MAX`).

### 4. Birth subsidy — `reproduce::is_eligible`

The effective breeding threshold (currently
`SPAWN_ENERGY × ReproductionThreshold × REPRO_ENERGY_MULT × personality ×
affect`) gains one factor:

```rust
* invention::repro_threshold_multiplier(held_mask(&agents.meme_vector[i]))
```

`repro_threshold_multiplier(mask) = 1.0 − FORT_BIRTH_SUBSIDY·held(FORTIFICATIONS)`.
A Fortifications-holding lineage breeds earlier and more often — the direct,
pre-registered attack on the birth-ledger margin. Uncoupled in v1 (no
affinity scaling; `is_eligible` has no access to the coupling flag and the
causal story is cleaner with one moving part). `mask = 0 → ×1.0` exactly, so
inventions-off worlds need no flag plumbing and stay byte-identical.
`held_mask` is cheap and the eligibility path is already per-candidate
(benchmarked before: caching held masks is not a win).

### 5. Existing multiplier extensions

- `weapon_multiplier_coupled` gains the Spears/Archery/Steel damage terms
  (additive, same FMA shape).
- `speed_multiplier_coupled` gains the Fortifications −10% term (negative
  coeff; the multiplier stays strictly positive).
- `module_upkeep_multiplier` gains Steel Arms' +10%.
- `flat_upkeep_coupled` gains Archery's per-tick upkeep.

## Save format and layout

- `MEME_CHANNELS` 20→24 (`program/mod.rs`); `INVENTION_CHANNEL_BASE = 8`
  unchanged; `PRACTICE_CHANNEL_BASE` 18→22 (`practice.rs` derives or is
  updated; both compile-time asserts still hold).
- `FORMAT_VERSION` 39→40 (`snapshot.rs`) with a changelog entry (meme vector
  widened; four inventions appended).
- `held_mask` stays `u32` (14 ≤ 32).
- All golden families rehash (determinism / inventions / cognition) via the
  unified `UPDATE_HASHES` regen in `tests/common`; save→load→step round-trip
  must pass at the new format.

## Explicitly out of scope

- **No new codex events.** `InventionDiscovered`/`InventionAdopted`/
  `MaterialLearning` carry `value = id` and fire generically for ids 10–13.
  (Appending events has a known blast radius: viewer parallel arrays,
  headless `score.rs` exhaustive matches.)
- **No weapon artifacts/crafting/inventory items.** Weapons remain body
  modules; the branch stays a knowledge layer, consistent with the tree.
- **No new opt-in flag.** Effects are held-gated like every other invention;
  `inventions_enabled = false` remains byte-identical (no RNG, exact-1.0
  multipliers).
- **No sense/viewer mechanic changes** beyond table growth (below).

## Ripples

- **Viewer/showcase tables**: any Godot or web-player array keyed to the
  invention list (names, colors, building sprites; `game/showcase/
  inventions.json`) must grow to 14 entries — enumerate during planning; the
  viewer asserts parallel-array lengths at boot.
- **`tool_boosted` widens**: `combat_pass` flags a hit tool-boosted when
  `inv_weapon_mult > 1.05`, so Spears/Archery/Steel hits now count toward the
  `EvolvedTool` detector alongside Metalworking — semantically correct
  ("invention-boosted hit") and requires no code change; noted so the detector
  shift isn't mistaken for a regression.
- **Discovery probability tables change** for every `inventions_enabled`
  scenario (new candidates alter the summed probability under the same single
  RNG draw), so invention-scenario goldens change content, not just format —
  expected and covered by the regen.
- **`scenarios/weapons.toml`**: a probe scenario (armed ape culture vs plain
  forager competitor) for the follow-up measurement; also exercises
  `starting_inventions = ["hafted_spears", ...]` name resolution.

## Testing

1. Table well-formedness: the existing loops over `INVENTIONS` (affinity
   coeffs, gene-gate range + era monotonicity, basket bounds + era
   monotonicity, prereq DAG) extend automatically; add spot-checks for the
   four new entries and update `candidates_respect_prereqs` expectations.
2. Oracle identity tests for each new multiplier (coupled vs base at
   `coupling = false`; exact 1.0 at mask 0), matching the existing pattern.
3. **Spoils conservation invariant**: for all masks and damage values,
   attacker energy gain ≤ target energy loss (spoils ≤ 1.0 enforced
   structurally; test asserts it).
4. Combat integration: a Spears-holding attacker vs Fortified target scenario
   step — damage, spoils, and defense all observable in energy deltas;
   Archery extends the effective reach past the bare module range.
5. Birth subsidy integration: two agents at energy just below the base
   threshold — ineligible without Fortifications, eligible with it.
6. Round-trip + goldens at `FORMAT_VERSION` 40.

## Success metric (pre-registered, follow-up cycle)

The O3 apparatus, unchanged: invention-holding lineages win the
intra-cultural contest — **culture-dominant AND era-active in the same
world**, distributionally (n≈10 seeds; OoA is bistable, smaller n does not
read). This design ships the mechanism; the measurement sweep is the next
cycle and its negative result would be recorded in place per roadmap
convention.
