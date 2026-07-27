# Domesticable Animals (E13) — Design Spec

**Date:** 2026-07-27
**Status:** Approved → implementation
**Baseline:** `main` through E12 (sexual dimorphism).
**Depends on:** the invention tree (`inventions_enabled`); taming is gated on
holding **Husbandry**.

## Motivation

Husbandry (era-3 invention) currently exists only as a scavenge-energy buff —
there are no *animals* to husband. This milestone adds the taming + livestock
pens model chosen at the E12 brainstorm: Husbandry holders capture wild
juveniles, pen them, and draw passive yields; penned stock breeds, so herds
grow. Domestication becomes a visible coevolutionary event (a cultural
lineage reshaping another species' life), catalogued by the codex.

## Decisions

- **Model:** taming + livestock pens (per E12 brainstorming), riding the
  Husbandry invention. No slaughter mechanic (owners can already hunt; the
  Husbandry scavenge buff covers carcass use).
- **New opt-in flag** `domestication_enabled` (World + Scenario), default
  off. A separate flag — not piggybacking `inventions_enabled` — so every
  existing invention scenario stays byte-identical (zero new RNG draws, zero
  new behavior with the flag off).
- **Livestock is per-agent state**, not a species trait: a new serialized
  `AgentBuffers::livestock_of: Vec<AgentId>` column (`AGENT_NULL` = wild).
  (A population-level tameness gradient was the rejected alternative.)

## Mechanics

### Taming (`domestication::husbandry_step`, tick stage 6e)

Each tick, for every agent that **holds Husbandry** (meme level ≥
`HELD_THRESHOLD`) and owns fewer than `MAX_LIVESTOCK_PER_OWNER` (8) animals:

1. Find the nearest eligible animal within `TAME_RANGE` (4.0) via the spatial
   hash — min distance, tie-break lowest id (order-independent, deterministic).
   Eligible = alive, **wild** (`livestock_of == AGENT_NULL`), **juvenile**
   (`age < TAME_MAX_AGE` = 100, the maturation window), **other species**, and
   **herbivorous** (`effective_diet_carnivory < 0.5`) — the domesticable-animals
   rule: you pen grazers, not predators.
2. Roll `rng.f32_unit() < TAME_CHANCE` (0.05/tick). The draw happens only when
   a candidate exists, so pen-free worlds draw zero RNG.
3. On success: `livestock_of[animal] = owner`; latch + push
   `AnimalDomesticated` (once per livestock species, `value` = owner species).

### Pens (movement override in `decide_all`)

Livestock with a living owner has its program movement **overridden**
(feeding, sensing, combat intents are untouched — grazing is position-based):
beyond `PEN_RADIUS` (6.0) of the owner it is pulled straight back (unit
torus-aware delta); inside the radius it stands and grazes. This suppresses
flee-from-owner behavior — taming is exactly the domestication of the flight
response. Dead owner ⇒ the override lapses and the orphan sweep (below)
returns the animal to wild.

### Yields (milking, same stage)

Each adult (`age ≥ MILK_MIN_AGE` = 100) livestock with a living owner and
surplus energy (`> MILK_MIN_ENERGY` = 30) pays `MILK_RATE × size` (0.02)
energy per tick to its owner, costing the animal `MILK_COST_MULT` (1.25)× the
transfer (conversion loss). Owner income scales with herd size; the herd pays
for itself by grazing its pen cell.

### Born domesticated (`reproduce_all`)

A newborn whose **both parents are livestock of the same living owner** is
born tamed (`livestock_of = that owner`). Pen herds grow themselves — no
extra draws. Otherwise newborns are wild (`spawn` resets the column to
`AGENT_NULL`).

### Orphans + combat exemption

- `husbandry_step` pass 1 returns livestock with a dead owner to wild, and
  builds the per-owner herd counts used for the cap (single ascending pass).
- `combat_pass` exemption: an attacker skips a target that is its own
  livestock (no energy cost, no damage) — herders don't hunt their own stock.
  Other agents (including the owner's conspecifics) are not exempt.

## Codex (2 new event types, `EVENT_TYPE_COUNT` 51 → 53)

- **`AnimalDomesticated` (= 51)** — first tame of a member of a livestock
  species (latched per livestock species in
  `CodexState::domesticated_species`; `value` = owner species id). Pushed
  directly from `husbandry_step` (like `DowryBirth` from `reproduce_all`).
- **`LivestockHerd` (= 52)** — a livestock species sustains ≥ `HERD_MIN` (6)
  living tamed members for `HERD_WINDOW` (500) consecutive ticks, counted via
  a new `livestock_count` accumulator in `SpeciesAgg` (streak +
  edge-trigger latch, `codex/domestication.rs`; re-arms on drop below
  `HERD_MIN`).

Headless `score.rs` gains `animal_domesticated` / `livestock_herd`
(`ALL_EVENT_NAMES` 51 → 53, corpus `n_t = 0` post-corpus). Viewer
`codex_panel.gd` gains the two names/colors.

## Viewer + scenario

- `agent_detail` adds `livestock_of` (i64, −1 = wild) and
  `domestication_enabled`; the inspector shows `livestock of agent N` /
  `wild` when the flag is on.
- Menu entry `E13 — Domestication (husbandry pens)`.
- `scenarios/domestication.toml`: `inventions_enabled` +
  `domestication_enabled`; an innovator culture cluster (fast tech climb to
  Husbandry via Stone Tools → Fire → Farming) sharing a range with a wild
  grazer herd — the future livestock.

## Serialization & determinism

- `AgentBuffers.livestock_of` + `World.domestication_enabled` + codex
  latch/streak fields ⇒ `FORMAT_VERSION` 20 → 21; golden hashes regenerated
  (flag-off trajectories byte-identical: the tick stage early-returns, the
  decide override is flag-gated, zero RNG with the flag off).

## Testing

- **Unit:** tame eligibility (wild/juvenile/other-species/herbivore/cap),
  tame roll latches the event, pen pull override (beyond/inside radius),
  milk transfer (surplus only, cost multiplier), orphan sweep, born-
  domesticated inheritance, owner combat exemption, flag-off inertness.
- **Integration (`tests/domestication.rs`):** scenario instantiates; hand-
  seeded Husbandry holder tames a juvenile grazer over ticks; herd breeds;
  save→load→step bit-identity; flag-off scenario has zero livestock.
- **Emergence (release-gated):** the domestication scenario across seeds —
  innovators reach Husbandry and `AnimalDomesticated` fires in a floor
  fraction of seeds; livestock present at horizon in the taming seeds.
