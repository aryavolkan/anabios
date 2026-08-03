# Out-of-Africa Invention-Climb Experiment — Findings (2026-08-02)

Executes the [OoA climb experiment plan](../plans/2026-08-01-out-of-africa-climb-experiment.md)
(Phase-2 roadmap item R). **Question:** can the grand-scale `out-of-africa` run
reach the era-3 milestones (Writing, Husbandry) by *emergence*, or is seeding
them (`starting_inventions`) the honest framing?

**Answer:** seeding is honest. The grand run cannot climb to era-3 emergently.
The plan's hypothesis (the *era-3 IQ ceiling* is the blocker) is **disconfirmed**;
the real block is **upstream and ecological** — the cognitive/culture-bearing
lineages are competitively excluded by a fast, non-cognitive forager before they
can climb past era-1. Removing that competitor is *necessary-ish* progress (era 1
→ 2 for a minority of seeds) but nowhere near *sufficient* (still 0% era-3).

Method: release `anabios-headless sweep`, 16 seeds × 20 000 ticks per condition.
Era reached is derived from `InventionDiscovered` events (`value` = invention id;
id→era `[1,1,2,2,3,3,3,4,4,4]`: 0 StoneTools · 1 Fire · 2 Farming · 3 Metalworking
· 4 Writing · 5 Medicine · 6 Husbandry). All conditions determinism-clean (no
core code changed — variant is a TOML-only agent-count edit).

## Knob × outcome table

| Condition | Knob (one variable) | max era (any seed) | seeds ≥ era-2 | seeds ≥ era-3 | any domestication | median PopulationCrash |
|---|---|---|---|---|---|---|
| **baseline** `out-of-africa` | — | **1** | **0/16 (0%)** | **0/16 (0%)** | 0/16 | ~14 000 (range 9.5k–57k) |
| **`ooa-noasocial`** | `asocial_forager` founding 610 → 50 | **2** | **2/16 (12%)** | **0/16 (0%)** | 0/16 | ~9 900 (range 6.8k–21k) |

Decision-gate criterion (plan Task 3): promote an emergent scenario only if a
single knob yields **≥50% of seeds at era-3 without ecosystem collapse**. Neither
condition comes close (0% era-3). → **document seeding; keep `starting_inventions`
in the saga.**

## What the baseline actually does (mechanism)

Narrated single-seed run (`demo --scenario out-of-africa --seed 12
--report-every 2000`):

- The climb *starts*: the `innovator` lineage **DISCOVERED STONE_TOOLS (t5112)**
  then **FIRE (t5198)**. The discovery engine works.
- Then it is *selected out*. `asocial_forager` — a cheap-breeding
  (`reproduction_threshold = 0.2`), non-cognitive archetype — explodes
  (2125 → 2782 → 2956 → 2996 alive) and competitively excludes every
  culture-bearing lineage. The `innovator` dwindles 26 → 5 → 1 → **EXTINCT
  (t8570)**; traditionalist, communicator, cultural_cooperator, cultural_forager,
  and the imitators are **all extinct by ~t9000**.
- By t10 000 the world is a tech-less `asocial_forager` monoculture (era 0,
  0/10 techs, ~2996 alive, ~50 energy each) and stays there to t20 000.

IQ (metabolically costly, `IQ_METABOLIC_COST = 0.25`) and culture make the
cognitive lineages slower breeders; under Malthusian churn at grand scale the
r-selected forager wins the ecological race before the k-selected innovators can
bank the surplus to climb. The **era-3 IQ gate (`IQ_REQ_BY_ERA[2] = 0.55`,
`invention/mod.rs:365`) is never even approached** — no lineage survives to
era-2, so the gate the plan targeted is downstream of the actual failure.

## The confirmatory knob: remove the competitor

`ooa-noasocial` is `out-of-africa.toml` with the five `asocial_forager` founding
blocks cut from counts {220,70,150,110,60}=610 to 10 each (=50). Nothing else
changed (same cognitive lineages, geography, flags, seed set).

- **Effect is real and directional:** max era 1 → 2; **2/16 seeds reach
  Metalworking** (era-2, seeds 7 & 8), one reaches Farming; PopulationCrash falls
  ~2–3× (the forager was a major churn driver).
- **But insufficient:** still **0/16 era-3, 0 domestication**. And the climb
  trades against survival — seed 8 reached Metalworking but ended at **45 alive**.

So competitive exclusion is a *confirmed, major* contributor to the stall, but
removing it does not unlock the era-3 climb. The remaining blockers (residual
Malthusian churn, the material-economy/population coupling, geographic skill
pooling, and — for the seeds that now reach era-2 — the era-2→3 IQ gate) compound;
no single knob clears them. This matches and mechanistically explains the
"focused/dense/low-stress vs. scale/churn" tension recorded in
[`docs/showcase-plan.md`](../../showcase-plan.md) §2: `domestication.toml` climbs
to era-3 precisely because it has *no* fast asocial competitor and a dense
cognitive founding population.

## Decision (RESOLVED)

Seeding the milestone tech (`starting_inventions`) is the honest framing for the
grand-scale showcase. The emergent era-3 climb is not reachable at grand scale by
single-knob tuning; it requires the focused conditions of `domestication.toml`,
which are in direct tension with the exodus's scale and churn — consistent with,
and now systematically evidenced beyond, the prior §2 finding.

**Not pursued (and why):** the plan's Task 2 code-change variants (lower the
era-3 IQ gate; boost cognitive-potential genome init) were *deprioritized once
the baseline showed 0/16 seeds reach era-2* — both target the era-2→3 stage that
the grand run never reaches, so neither could move the gate that actually binds
(era-1 survival of culture). A future attempt at emergence should target the
*ecological* stage: protect or subsidize cognitive lineages against r-selected
competitors (e.g. a culture fitness floor, niche separation, or a competitor cap),
not the discovery/IQ math.

## Reproduce

```sh
cargo build --release -p anabios-headless
# baseline
./target/release/anabios-headless sweep --scenario scenarios/out-of-africa.toml \
    --seeds 16 --ticks 20000 --out runs/ooa-baseline
# competitor-removed variant (asocial_forager counts → 10 each)
./target/release/anabios-headless sweep --scenario <variant>.toml \
    --seeds 16 --ticks 20000 --out runs/ooa-noasocial
# single-seed mechanism narration
./target/release/anabios-headless demo --scenario scenarios/out-of-africa.toml \
    --seed 12 --ticks 20000 --report-every 2000
```

Era reach is derived from each seed's `*.events.jsonl` (`InventionDiscovered`
`value` → era). Runs write under `runs/` (git-ignored scratch); the tables above
are the durable record.
