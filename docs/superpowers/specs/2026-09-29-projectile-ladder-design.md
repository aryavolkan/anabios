# Projectile ladder: Throwing Stones → Hafted Spears → Archery → Steel Arms → Gunpowder — design (2026-09-29)

> Hominids innovate projectiles in order, starting from the thrown stone. One
> new invention (**Throwing Stones**, id 22, era 1) roots the existing military
> line: Hafted Spears is re-rooted onto it, every thrown rung extends weapon
> reach, and a scenario can no longer seed a rung without its foundations.
> `INVENTION_COUNT` 22→23, `MEME_CHANNELS` 32→33, `FORMAT_VERSION` 44→45.
> No new flag, no new codex event, no new RNG draw in any flag-off world.

## Motivation

The weapons branch (`2026-09-07-weapons-branch-design.md`) gave the tree a
military line — Hafted Spears, Archery, Fortifications, Steel Arms — and X1
capped it with Gunpowder. Two things were missing for it to read as *hominids
innovating projectiles*:

1. **The line started at a melee weapon.** The first standoff reach arrived
   only at era-2 Archery; there was no era-1 projectile at all, although the
   thrown stone (manuports, the knapper's spheroids) is the oldest hominin
   weapon in the record and predates the hafted point by a wide margin.
2. **Reach was not a ladder.** `range_multiplier` carried two independent
   terms (Archery, Gunpowder). Nothing made reach climb rung by rung, and a
   culture could hold Gunpowder-range weapons having never thrown anything.

The design adds the missing root and makes reach monotone along the chain.
Spears stay below bows, as in the existing tree and the archaeological record
(hafted and thrown spears long precede the bow); the request's "stones →
arrows → spears → gunpowder" wording is read as the projectile progression it
names, not as a reordering of the two middle rungs.

## The invention (appended id 22)

| id | Invention | Key | Era | Prereqs | Materials [salt,obsidian,amber,spice] |
|----|-----------|-----|-----|---------|----------------------------------------|
| 22 | Throwing Stones | `throwing_stones` | 1 | Stone Tools | [0,1,0,0] |

- **Buff:** `THROWING_STONES_RANGE = 0.25` on the weapon-reach multiplier (the
  first standoff: a bare `Weapon` module's 2.0 contact reach becomes 2.5) and
  `THROWING_STONES_DAMAGE = 0.10` on the weapon-damage multiplier — enough to
  cross `combat_pass`'s `> 1.05` tool-boosted bar, so a stone-thrower's hits
  count toward `EvolvedTool` like every other weapon tech's.
- **Debuff:** none (era-1 entry tech, like Stone Tools and Pottery).
- **Affinity:** Extraversion (coeff 0.8), Archery's slot — mobbing a predator
  with stones is band work, so the two thrown-projectile rungs select the same
  gene, the way the melee/metal rungs (Hafted Spears, Metalworking, Steel
  Arms) share Territoriality. **Gene gate:** none (era-1 entry tech).
- **Basket:** one worked stone — the cheapest basket in the tree. The O3
  findings decomposed the era gate to materials, so the ladder's entry rung is
  deliberately near-free; per-era average basket size stays non-decreasing
  (era 1 falls to 2.2, era 2 is 3.4).

## Re-rooting Hafted Spears

`INVENTIONS[HAFTED_SPEARS].prereqs` changes from `bit(STONE_TOOLS)` to
`bit(THROWING_STONES)` (Stone Tools is implied transitively), and the spear
gains `SPEARS_RANGE = 0.15` — the thrown javelin — on top of its damage and
spoils. The ladder is now a chain, pinned by `PROJECTILE_LADDER` and the
`projectile_ladder_is_a_chain_with_monotone_reach` test:

| Rung | Reach term | Cumulative reach × | Note |
|------|-----------:|-------------------:|------|
| Throwing Stones | +0.25 | 1.25 | the hand-hurled standoff |
| Hafted Spears | +0.15 | 1.40 | thrown javelin; damage + spoils as before |
| Archery | +0.50 | 1.90 | unchanged |
| Steel Arms | — | 1.90 | metallurgy: damage, not reach |
| Gunpowder | +0.30 | 2.20 | unchanged |

Downstream the chain is untouched: Archery still needs Hafted Spears,
Fortifications still needs Farming + Hafted Spears, Steel Arms still needs
Metalworking + Archery, Gunpowder still needs Steel Arms — so every one of
them now transitively needs the stone.

**Append-only ids meet a re-rooted graph.** Ids are append-only (scenario
`starting_inventions` keys, codex `value = id` payloads in recorded decks, and
viewer arrays all depend on it), so the ladder's root sits at the *end* of the
table and is a prereq of a *lower* id. The old table test required "prereqs
reference earlier ids"; that was only ever a sufficient condition for the real
invariant, acyclicity. `prereq_chain_shape` now computes each invention's
transitive prereq closure and asserts (a) it never contains the invention
itself and (b) it always contains Stone Tools (one root). `candidates()` and
the atrophy loop never depended on id order — both read a held mask that is
fixed for the whole walk — and their doc comments now say so.

**Atrophy follows the new root.** Knowledge atrophy already decays any held
invention whose prereqs the holder lacks; a lineage that loses the stone now
loses the spear, then the bow, at `ATROPHY_RATE` per tick. The loader closes
the one hole this opens: `Scenario::parse_toml` rejects a
`starting_inventions` list that seeds an invention without its prerequisites
(`ScenarioError::UnsupportedStartingInvention`), because such a seed would
silently fade over the first few hundred ticks. Every shipped world and
fixture seeds complete chains already; the Godot bridge's inlined replica of
the retired `weapons.toml` now seeds `throwing_stones` beside `hafted_spears`.

## Save format and layout

- `MEME_CHANNELS` 32→33 (`program/mod.rs`), an **exact fit**:
  `INVENTION_CHANNEL_BASE` 8 + 23 inventions + 2 practices. No spare lane is
  left on purpose — `culture::inherit_meme` jitters (one RNG draw each) every
  channel that is neither an invention nor a practice channel, so headroom
  would add draws to every Communicator birth in every world, flag-off ones
  included. `PRACTICE_CHANNEL_BASE` derives to 31.
- serde derives `Serialize`/`Deserialize` for arrays only up to 32 lanes, so
  `AgentBuffers.meme_vector` and `meme_lineage` ride a new
  `#[serde(with = "serde_rows::fixed_rows")]` adapter (`src/serde_rows.rs`),
  which writes each row as a fixed tuple inside a length-prefixed sequence —
  byte-identical to the derive below 32 lanes (pinned by
  `matches_the_derived_layout_below_32`). `state_hash` semantics are unchanged;
  only the row width grew. No other serialized field carries a
  `MEME_CHANNELS`-wide array (the action register, evaluator context, codex
  aggregate and culture scan are per-tick scratch).
- `FORMAT_VERSION` 44→45 with a changelog entry; all eight golden tables and
  the two flag-off trajectory pins are re-pinned at this head. In the same
  change, at the maintainer's request, **golden validation is turned off by
  default**: the ten pinned-hash tests are `#[ignore]`d (run with
  `--ignored`; `UPDATE_HASHES=1` still re-pins), so a hash drift no longer
  fails CI — the self-consistency tests, the save→load→step round-trips and
  the headless `replay` verifier remain the determinism gate.

### What moves and what does not

- **Worlds with `inventions_enabled = false`** (the flag-off test fixtures;
  no shipped world, since the schema defaults the knob on): the new lane is
  an invention channel, jittered only under the flag, so the RNG draw count
  and the trajectory are byte-identical. Only the serialized layout grew (one
  lane per agent), which moves the hashes.
- **Every world with the flag on** changes trajectory: each Communicator
  birth jitters one more lane (`reproduce::inherit_child_meme` gates on the
  child's Communicator module, not on apes — the ape strip runs afterwards
  precisely so the draw count is unchanged), so ape-free worlds such as
  `predator-prey` and `riverlands` move too. Ape worlds additionally get an
  extra era-1 candidate reweighting the discovery table under the same
  single draw, and Hafted Spears now waits on the stone.

## Explicitly out of scope

- **No new codex event.** `InventionDiscovered`/`InventionAdopted`/
  `MaterialLearning` carry `value = 22` generically; a standoff hit already
  counts toward `EvolvedTool`. A dedicated "first ranged kill" detector would
  widen the viewer's parallel arrays, the headless scorer's exhaustive
  matches and the sweep CSV, and is a separate cycle if wanted.
- **No projectile entities, ammunition or accuracy rolls.** Combat stays
  deterministic and RNG-free; a projectile in this engine *is* reach beyond
  contact (the same mechanic that lets `Spines` kill before contact weapons
  close), and the streak layers in both front ends already draw every hit as
  an attacker→target volley.
- **No new sprite pose.** The Godot hominin atlas has spear, bow and blade
  cells keyed to those techs (`fx_math.weapon_action`); a stone-thrower keeps
  the brawl cells. Table growth only: workshop sign, HUD icon, event tint,
  coevolution series, id mirror.

## Testing

1. Table well-formedness: the existing loops (affinity coeffs, gene-gate range
   and era monotonicity, basket bounds and era monotonicity) extend
   automatically; `prereq_chain_shape` is generalised to the closure check;
   `candidates_respect_prereqs` expectations updated (Stone Tools opens Fire,
   Pottery and the stone; the stone opens the spear);
   `is_invention_channel_covers_exactly_the_tree` pins the new top of the
   block; `throwing_stones_tree_shape` spot-checks the entry.
2. Multipliers: `projectile_ladder_is_a_chain_with_monotone_reach` (each rung
   requires the one below; reach non-decreasing, strictly increasing on every
   thrown rung, flat on Steel Arms) and
   `throwing_stones_extend_the_weapon_and_range_stacks` (exact terms, coupling
   identity at neutral, unrelated bits inert).
3. Combat integration (`tests/inventions.rs`): `throwing_stones_extend_weapon_reach`
   (a bare attacker cannot reach 2.3, a stone-thrower can, and the hit is
   tool-boosted), `projectile_rungs_stack_their_reach` (spears alone stop
   short of 2.5, stone + javelin reach it),
   `hafted_spears_atrophy_without_throwing_stones`, and the end-to-end
   `projectile_ladder_climbs_from_the_stone` (skilled, open ape communicators
   seeded with Stone Tools discover the stone before the spear and never hold
   the spear without it).
4. Loader: `parse_toml_rejects_a_seeded_invention_missing_its_prerequisite`
   (and the complete chain instantiates held on an ape kit).
5. Serialization: `serde_rows` round-trips 33-wide rows, matches the derived
   layout below 32, and rejects a truncated row; the existing save→load→step
   round-trips run at `FORMAT_VERSION` 45.

## Measured (tribes, seed 60623 and seeds 0–7, 5000 ticks, release)

Measured with `anabios-headless demo` / `sweep` before and after the change
(same binary configuration otherwise; `runs/` outputs not committed).

- **Scenario seed (60623), demo, 5000 ticks.** Before: Stone Tools is the
  tick-0 seed; the first climbed inventions were Hafted Spears (tick 1903) and
  Fire (1919), no non-seeded adoption. After: Pottery (1104) and Fire (3566);
  the trajectory moved for the reasons above, and this seed happens not to
  roll the stone within the horizon.
- **Seeds 0–7, sweep, 5000 ticks, first discovery of each non-seeded tech.**

  | | before | after |
  |---|---|---|
  | Throwing Stones discovered | — | 5/8 seeds (ticks 121, 210, 286, 382, 850) |
  | Throwing Stones adopted (≥ half a species) | — | 4/8 seeds |
  | Hafted Spears discovered | 5/8 (ticks 125–2332) | 2/8 (2035, 4810), both after the stone |
  | Archery discovered | 1/8 (tick 773) | 0/8 |
  | Fire discovered | 3/8 | 5/8 |
  | any non-seeded invention adopted | 2/8 | 4/8 |

  The ladder behaves as designed: the stone is now the common first
  projectile, cheap enough to be the most frequent first discovery in the
  world, and the spear waits on it — so within a 5000-tick horizon Archery no
  longer appears, which is the price of a real first rung. The `tribes` row in
  `docs/scenarios.md` records the adoption figure (4/8, still under the 5/8
  bar); the README's tribes paragraph quotes the same numbers.
