# The Out-of-Africa Initiative — Feature Showcase Plan

One narrative showcase, framed as the human journey **out of Africa**, in which
every anabios subsystem appears as a *milestone* of that journey — the exodus,
the tool and fire, farming and settlement, **the invention of writing**, and
**the domestication of animals** — all in the Godot viewer, culminating in
livestock herds.

This revises the earlier "grand tour + spotlights" framing: the showcase is the
out-of-africa initiative, and the features are its chapters. Building it surfaced
a hard finding — the grand run can't climb to the era-3 milestones on its own
(§2) — which is now **resolved**: a small engine feature (`starting_inventions`,
§3/§4) lets the full-scale run hold Writing and Husbandry from tick 0, and a
validated scenario `out-of-africa-saga.toml` delivers the invention of writing
**and** domesticated livestock in one continuous run. Read §2 → §4.

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
| `out-of-africa-saga` (throttled — abandoned) | 900 | 20,000 | **no inventions at all** | 0 |
| `domestication` (focused reference) | 400 | 12,000 | **Husbandry — era 3** | **70** |
| **`out-of-africa-saga` (Option A, seed Farming)** | 3000 | 15,000 | seeded era 2 → **Metalworking** emergent, era-3 IQ-gated | 0 |
| **`out-of-africa-saga` (Option A, seed Writing+Husbandry) ✓** | 3000 | 8,000 | **Writing + Husbandry held at t0** | **21 (+6 herds)** |

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

**Conclusion:** the *emergent* era-3 climb needs *focused, dense, low-stress*
conditions (exactly what `domestication.toml` provides), in direct tension with
the *scale and churn* of the grand exodus theater. No amount of extra ticks
fixes this. The resolution (§3) was **not** to sacrifice scale but to add the
missing lever — starting-tech seeding — so the full-scale run can begin already
holding the milestone tech.

---

## 3. The decision (RESOLVED): Option A — seed starting tech · **built & validated**

The chosen fix is a small engine feature, `AgentSpec.starting_inventions`: a
lineage can begin tick 0 already **holding** named inventions, so the *grand,
full-scale* world reaches the era-3 milestones without sacrificing the teeming
exodus. Shipped in this branch:

```toml
[[agents]]
archetype = "innovator"
starting_inventions = ["stone_tools", "fire", "farming", "writing", "husbandry"]
```

- **Engine:** `AgentSpec.starting_inventions: Vec<String>` (default empty) +
  `invention::id_from_name`; `instantiate` seeds the named invention meme
  channels to fully adopted after spawn. No RNG is drawn. Unknown names are
  rejected at `parse_toml` with a `ScenarioError::UnknownInvention` naming the
  offender, so a typo fails at load instead of deep inside instantiation.
- **Determinism:** the field is absent from `minimal.toml`, so seeding is a
  no-op there and the golden hashes are **byte-identical** — verified: the full
  `anabios-core` suite (incl. the 123 s determinism/golden test) passes, clippy
  clean, no `FORMAT_VERSION` bump. TDD: test written and watched fail first.
- **Validated (seed 318):** seeding just Farming lifts the grand run from era 1
  to discovering **Metalworking emergently at tick 4465** — but era-3 stays
  IQ-gated (no cognition trait to seed high IQ), so Writing/Husbandry/domestication
  still don't emerge. Seeding **Writing + Husbandry** directly then delivers the
  showcase: **Writing held at t0**, **21 `AnimalDomesticated` + 6 `LivestockHerd`**
  at the co-located Quarry herd, with the full grand-theater spread intact
  (6249 Migration, 142 MarketEmerged, 20 DialectFormed, 16 SettlementFormed,
  10 WarOrRaid). Downstream techs still emerge on their own from the seeded base.

Seeding the milestone tech is legitimate for a *showcase* — the goal is to
reliably DISPLAY the features, not to prove they emerge from a cold start (the
focused `domestication.toml` and the §2 analysis already establish emergence).

**Alternatives considered and rejected:** *Option B* — a two-act narration over
`out-of-africa` (exodus) + `domestication` (milestones), zero code but two runs;
kept as the fallback if a single-run showcase is ever undesirable. *Option C* —
tune a throttled saga until milestones emerge; abandoned (it starved the economy,
§2) and fights the scenario's identity.

---

## 4. `out-of-africa-saga.toml` — the showcase scenario (Option A, validated)

`scenarios/out-of-africa-saga.toml` **is** the showcase run. It is the full
`out-of-africa` world — pop 3000, material economy intact, every subsystem on,
same geography and seed (318) — with exactly two changes:

1. **Seeded milestone tech.** The Quarry innovators carry
   `starting_inventions = ["stone_tools", "fire", "farming", "writing",
   "husbandry"]`, so the invention of Writing and the Husbandry that enables
   domestication are on display from tick 0 instead of stranded behind the
   IQ-gated era-3 climb.
2. **Co-located herd.** A tameable grazer+herd sits *with* the innovators at the
   Quarry (250,445), inside taming range — the base scenario's herds are ~190u
   south, out of reach. So Husbandry holders tame juveniles on-camera.

Run it: `scripts/emergence.sh view out-of-africa-saga --seed 318` (live) or
`scripts/emergence.sh run out-of-africa-saga --ticks 8000 --seed 318` (tally).
Validated output is in §3. The throttled-population experiment that preceded
this (pop 900 → zero inventions) is gone; keeping full scale + seeding is the
fix.

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

**The two ★ chapters — Writing and the Herd — are the payoff**, and §3/§4 show
how `out-of-africa-saga` delivers them in one run.

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

Capture one still per chapter from the showcase run, e.g. the domestication
money shot (pin a herder to show the `livestock of agent N` line):
```
ANABIOS_SHOT="$PWD/gallery/saga-t4000-livestock.png" \
ANABIOS_SHOT_TICKS=4000 ANABIOS_INSPECT=1 \
scripts/emergence.sh view out-of-africa-saga --seed 318
```

---

## 8. Phased execution plan

**Phase 0 — done.** Verified milestone reachability (§2): domestication confirmed
on `domestication` (70 `AnimalDomesticated`); grand run confirmed stuck at era 1;
throttle regression documented.

**Phase 1 — done.** Decision resolved: Option A.

**Phase 2 — done.** Built `starting_inventions` (engine + test, determinism-clean)
and the `out-of-africa-saga` showcase scenario; validated it fires Writing +
21 `AnimalDomesticated` + 6 `LivestockHerd` at full scale (§3/§4).

**Phase 3 — next.** Narration script (minute-by-minute viewer walk-through of
`out-of-africa-saga` per §5) + capture the chapter gallery per §7.

**Phase 4 — optional.** One-pager: feature × evidence table combining the
`run`/`sweep` event counts with the gallery stills.

---

## Scope boundaries (YAGNI)

- Option A is the only engine change — a bounded, determinism-safe field.
- No web frontend (none exists); the Godot viewer is the surface.
- Keep the tour to the seven chapters above; don't beat every codex detector.
- Milestone tech is *seeded* for a reliable showcase; cold-start emergence lives
  in the focused `domestication.toml`, not here.
