# The Out-of-Africa Initiative — Feature Showcase Plan

One narrative showcase, framed as the human journey **out of Africa**, in which
every anabios subsystem appears as a *milestone* of that journey — the exodus,
the tool and fire, farming and settlement, **the invention of writing**, and
**the domestication of animals** — all in the Godot viewer, culminating in
livestock herds.

This revises the earlier "grand tour + spotlights" framing: the showcase is the
out-of-africa initiative, and the features are its chapters. But building it
surfaced a hard finding that reshapes the plan — read §2 before §3.

---

## 1. What already exists

- **`scenarios/out-of-africa.toml`** (seed 318): a grand-theater world with
  *every* opt-in flag on, including `sexual_dimorphism_enabled` and
  `domestication_enabled`. Themed geography: an equatorial "Africa" (Cradle hub
  at 150,440; obsidian Quarry at 240,430; megafauna belt at 200,620), a mid-map
  ocean with two desert crossings, a Sahel DIT relay, and a cold "Eurasia" north
  held by archaics.
- **Domesticated animals ship** as the E13 feature
  (`crates/anabios-core/src/domestication.rs`): Husbandry holders tame juvenile
  herbivores into penned, milk-yielding livestock; born-tamed offspring inherit
  it. Detector `codex/domestication.rs` fires `AnimalDomesticated` /
  `LivestockHerd`.
- **The invention of writing** is a real milestone: Writing is invention #4,
  **era 3**, prereq Farming (`invention/mod.rs`). The tech tree is a 4-era climb:
  - **Era 1:** Stone Tools → Fire
  - **Era 2:** Farming, Metalworking
  - **Era 3:** **Writing** (← Farming), Medicine (← Writing), **Husbandry** (← Farming)
  - **Era 4:** Machinery, Electricity, Nuclear Power
  So Writing and Husbandry are era-3 siblings both gated on Farming — the natural
  twin climax of the journey. Domestication needs Husbandry.
- **Viewer + capture harness** exist (Godot `game/`, `debug_capture.gd`).

---

## 2. The hard finding: the grand run does NOT climb to the milestones

The out-of-africa initiative, *as currently tuned*, cannot reach the era-3
milestones (Writing, Husbandry) and never domesticates an animal. This was
measured, not assumed:

| Run | Pop | Ticks | Tech reached | `AnimalDomesticated` |
|---|---|---|---|---|
| `out-of-africa` | 3000 | 8,000 | Stone Tools (797), Fire (2086) — **era 1** | 0 |
| `out-of-africa` | 3000 | **40,000** | still only Stone Tools + Fire — **era 1** | **0** |
| `out-of-africa-saga` (throttled, §4) | 900 | 20,000 | **no inventions at all** | 0 |
| `domestication` (focused reference) | 400 | 12,000 | **Husbandry — era 3** | **70** |

Two structural blockers, plus a third discovered while trying to fix them:

1. **Malthusian churn starves the climb.** With births ungated at pop 3000, the
   world crests the cap and thrashes (18,664 `PopulationCrash` events over 40k
   ticks); no lineage banks the surplus to climb past Fire. The base scenario's
   own "honest expectations" comment already admits the tech arc stops at Fire
   and recommends only 2–4k ticks.
2. **Geographic separation.** The innovators camp at the Quarry (240,430); the
   tameable herds graze ~190 units south at (200,620) — far outside the taming
   range (4.0). Even *with* Husbandry, taming couldn't happen.
3. **Discovery is gated on the material economy, which needs population.**
   Throttling the population to 900 to fix (1) *backfired*: `MaterialLearning`
   dropped from 213 to **0** and the world discovered **nothing**. Invention
   requires gathering and spending a material basket, so a small population
   starves the tech economy entirely.

**Conclusion:** the era-3 climb needs *focused, dense, low-stress* conditions
(exactly what `domestication.toml` provides — a small world where "the tech
race, not Malthusian collapse, dominates"), and those are in direct tension with
the *scale and churn* that make the grand exodus theater compelling. **No amount
of extra ticks fixes this, and there is no starting-tech seeding in the scenario
schema** (`scenario.rs`) to shortcut the climb — every lineage discovers from
scratch. So delivering the user's vision requires a real decision, below.

---

## 3. The decision: how to make the milestones part of the initiative

Three ways to get Writing + domestication into the out-of-africa story. They
trade off *scale*, *engine work*, and *certainty*.

### Option A — Seed starting tech (small engine feature) · **recommended**
Add a scenario field to pre-seed a lineage's invention mask, e.g.:
```toml
[[agents]]
archetype = "innovator"
starting_inventions = ["stone_tools", "fire", "farming"]  # NEW
```
Then the innovators begin at the doorstep of era 3, and the *grand, full-scale*
world can reach Writing → Husbandry → domestication within a watchable budget
**without sacrificing the teeming exodus**. This is the only option that
delivers the user's exact vision — all milestones inside one grand initiative.

- **Work:** a bounded `anabios-core` change — a new `Option<Vec<String>>` field
  on the agent spec that ORs the named invention bits into the lineage's mask at
  instantiate. ~1 file + scenario plumbing + a test.
- **Determinism:** the field is absent from `minimal.toml`, so the golden hashes
  are untouched (the established "flag/field off in golden ⇒ byte-identical"
  pattern). No `FORMAT_VERSION` bump if it only affects instantiation of
  flag-on scenarios. Verify with the determinism suite before merge.
- **Risk:** era-3 *learning* still consumes material baskets, so the seeded
  world must sustain a material surplus; validate that a seeded grand run
  actually fires `AnimalDomesticated` before committing the showcase to it.

### Option B — Two-Act showcase (no engine change) · **runnable today**
Keep the initiative as a narrative frame over two proven runs:
- **Act I — The Exodus** (`out-of-africa`, ~4k ticks): the teeming world —
  worldgen, migration/corridors, trade/markets, dialects, DIT, war, speciation,
  sexual dimorphism, and the era-1 tech dawn (Stone Tools, Fire).
- **Act II — Civilization's Milestones** (`domestication`, ~12k ticks, reframed
  as "zoom in on the lineage that reached the threshold"): the full climb Stone
  Tools → Fire → Farming → **Husbandry**, then the **domestication** of the
  herds — 70 `AnimalDomesticated` + `LivestockHerd` confirmed. Optionally add
  `inventions` for the Writing/era-3 tech-tree beat.

The "one initiative" is the narration that binds Act I's exodus to Act II's
milestones. Delivers every feature *including writing and domestication* with
zero code and zero risk — just not in a single continuous run.

### Option C — Tune a focused saga into one run (no engine change) · **uncertain**
Iterate a variant toward `domestication.toml`'s density/low-stress recipe until
the milestones emerge in a single grand-ish run. My first attempt
(`out-of-africa-saga.toml`, §4) regressed by starving the economy; getting the
population/economy/stress balance right is real iteration and *will* cost the
teeming scale that defines out-of-africa. Higher effort, uncertain payoff.

**Recommendation:** **A** as the definitive product (a small, clean engine
feature unlocks the exact vision at full scale), with **B** as the showcase you
present *today* while A is built and validated. Do **not** invest in C — it
fights the scenario's own identity.

> This is a genuine product decision (ship an engine feature vs. narrate two
> runs). It needs your call before implementation — see "Open decision" at the
> end.

---

## 4. `out-of-africa-saga.toml` — the Option-C experiment (documented negative result)

`scenarios/out-of-africa-saga.toml` is a tuning experiment: out-of-africa's
geography + full flag stack, but throttled to pop 900 with boosted, co-located
innovators + a Quarry herd. **It currently under-delivers** — throttling starved
the material economy and it discovered no inventions (§2, row 3). It is retained
as the honest starting point for Option C and as evidence for why a naive
population throttle is the wrong lever. It is **not** the showcase scenario.

---

## 5. The milestone arc (once a delivery option is chosen)

However the milestones are delivered (A or B), the showcase narration is the
same era-climb story. Each chapter maps to subsystems, screen cues, viewer
controls, and the codex events that prove it fired.

| Chapter | Milestone | Subsystems on show | Watch for | Viewer control | Codex events |
|---|---|---|---|---|---|
| **The Cradle** | A world is born | climate/Whittaker biomes, worldgen, living water, terrain relief | latitude bands, ocean belt, relief shading | **G** → biome | — |
| **First Steps** | Tools & fire | 8-bit apes, body modules, material economy, invention era 1 | hominin sprites; tech panel lights Stone Tools→Fire | **C** → species; tech panel | `InventionDiscovered`, `MaterialLearning` |
| **The Exodus** | Out of Africa | migration, corridors, biome adaptation, speciation | agents funnel through the desert crossings; cline forms | **G** → env-optimum | `Migration`, `CorridorUse`, `RangeExpansion`, `SpeciationEvent` |
| **Settling Down** | Farming & trade | era-2 tech, settlement, markets, trade goods | settlements anchor; trade lanes drawn | **G** → markets | `SettlementFormed`, `MarketEmerged`, `ResourceTraded` |
| **The Word** | ★ Invention of writing | era-3 tech, DIT/culture, dialects, institutions | tech panel reaches **Writing**; dialect-hue clusters | tech panel; **C** → dialect | `InventionDiscovered` (Writing), `MemeSweep`, `InstitutionalRatchet`, `DialectFormed` |
| **The Herd** | ★ Domestication | Husbandry, livestock, herd cohesion | penned stock near owners; `livestock of agent N` in inspector | click herder + neighbors | `AnimalDomesticated`, `LivestockHerd`, `HerdCohesion` |
| **Blood & Kin** | Conflict & cooperation | combat, war, territory, kin, cognition, dimorphism | combat glow, raids, territory borders, two sexes | **Y** coevolution; inspect | `CombatRaid`, `WarOrRaid`, `AllianceFormed`, `TerritoryFormation`, `SexualSelection` |

**The two ★ chapters — Writing and the Herd — are the payoff**, and §2 is why
they need a delivery decision.

---

## 6. Viewer controls (verified from `game/scripts/*.gd`)

Launch: `scripts/emergence.sh view <scenario> --seed 318` (needs Godot 4.x).

| Key | Effect |
|---|---|
| **G** | Cycle ground overlay: biome → env-optimum → succession → markets → pheromone 0–3 (auto-skips inactive subsystems). |
| **C** | Cycle agent coloring (diet / dialect-hue / energy / species …). |
| **T** / **Y** | Evolution panel / coevolution panel. **H** legend. |
| **Click agent** | Pin in inspector (species, lineage, OCEAN personality, modules, `livestock of agent N`). |
| **WASD / arrows / F** | Pan / follow camera. **R/U/V/Esc** replay & menu. |

---

## 7. Gallery capture pipeline

Env-gated harness in `debug_capture.gd` (needs a **windowed** run):
`ANABIOS_SHOT` (output path/switch), `ANABIOS_SHOT_TICKS` (evolved state),
`ANABIOS_INSPECT` (pin agent), `ANABIOS_COEVO`/`ANABIOS_EVO` (reveal panels).
Naming: `<scenario>-t<tick>-<label>.png` (matches existing `gallery/`).

Capture one still per chapter, e.g. the domestication money shot:
```
ANABIOS_SHOT="$PWD/gallery/domestication-t4000-livestock.png" \
ANABIOS_SHOT_TICKS=4000 ANABIOS_INSPECT=1 \
scripts/emergence.sh view domestication --seed 0
```

---

## 8. Phased execution plan

**Phase 0 — done.** Verified the milestone reachability (§2). Domestication
confirmed firing on `domestication` (70 `AnimalDomesticated`); grand run confirmed
stuck at era 1; throttle regression documented.

**Phase 1 — resolve the Open decision (blocking).** Choose A / B / C (§3).

**Phase 2 — deliver the run.**
- *If A:* implement `starting_inventions` seeding, validate a seeded grand run
  fires Writing + `AnimalDomesticated`, then it becomes the single showcase run.
- *If B:* finalize the two-act narration bridging `out-of-africa` (Act I) and
  `domestication`/`inventions` (Act II).

**Phase 3 — narration script + gallery.** Minute-by-minute viewer walk-through
per §5, then capture the chapter gallery per §7.

**Phase 4 — one-pager (optional).** Feature × evidence table combining the
`run`/`sweep` event counts with the gallery stills.

---

## Open decision (needs your call)

**How should the milestones be delivered?** — **A** (small engine feature:
`starting_inventions` seeding, so the full-scale grand run reaches Writing +
domestication — recommended), **B** (two-act showcase using proven scenarios,
runnable today, no code), or **C** (keep iterating the throttled saga — not
recommended). My recommendation is **A for the product, B to demo now.** I need
this call before Phase 2, because it decides whether we touch engine code.

## Scope boundaries (YAGNI)

- Only Option A touches engine code, and only a bounded, determinism-safe field.
- No web frontend (none exists); the Godot viewer is the surface.
- Keep the tour to the seven chapters above; don't beat every codex detector.
