# Trade-Economy Freeze — Corrected Diagnosis (2026-08-02)

Investigation for the Phase-2 roadmap item *Trade-economy redesign*. Before
building the [redesign plan](../plans/archive/2026-08-01-trade-economy-redesign.md)'s
perishability fix, the freeze was measured directly. **The freeze is real, but
the plan (and `ROADMAP.md`, and prior notes) misdiagnosed its cause** — so the
planned fix cannot work. This doc records the corrected diagnosis and the
evidence, so the next attempt starts from the right cause.

> **⚠️ Read the [Update](#update-2026-08-02-six-candidate-fixes-eliminated--the-freeze-is-a-structural-barter-equilibrium) at the bottom first.** Sections §1–§3 correctly show the freeze is *not* demand-satiation and that perishability fails. But their conclusion that it is a pure **supply-side** problem with a supply-side fix was **also subsequently disproven** — conservation-on-death and 6× harvest access *both* still froze. The current best understanding (bottom Update) is that the freeze is a **structural bilateral-barter equilibrium**, not a supply *or* demand bug alone. The §1–§3 measurements stand; the "target supply" prescription in §"Root cause"/§"Implications" is superseded.

## TL;DR

- The trade freeze is a **supply-side starvation**: over a long run, agent
  inventories collapse to **empty**, so no agent can spare a `TRADE_UNIT` to
  give and `pick_swap` returns `None`. It is **not** the *demand-satiation
  "absorbing state"* (everyone saturated at `STOCK_TARGET`, `want→0`) that the
  plan and roadmap describe.
- Because the diagnosis was wrong, **both** mechanisms the plan proposed target
  the wrong thing and are empirically ineffective:
  - **Perishability** removes goods — but frozen agents already hold none; it
    freezes `biome-trade` at the *same* tick and collapses it *earlier* mid-run,
    and *reduces* `geographic-trade` throughout.
  - **Non-satiating `want`** addresses `want=0` — but at the freeze `want` is
    already maxed (full deficit); it changes nothing.

## Method

Release `anabios-headless`; long-horizon runs counting `world.trade_routes`
(per-tick swaps) in windows, plus a frozen-state inventory dump. Measurements
from throwaway probe tests (not committed).

## Evidence

### 1. Where/when the freeze bites (20 000 ticks, trades per 2 000-tick window)

| window end | `biome-trade` | `geographic-trade` |
|---:|---:|---:|
| 2 000 | 54 588 | 152 341 |
| 4 000 | 96 811 | 71 102 |
| 6 000 | 70 512 | 21 123 |
| 8 000 | 2 627 | 24 327 |
| 10 000 | **0** | 4 698 |
| 12 000 | **0** | 16 |
| 14 000 | **0** | 807 |
| 16 000 | **0** | 689 |
| 18 000 | **0** | 392 |
| 20 000 | **0** | 1 574 |

- **`biome-trade` freezes permanently at ~t9 000–10 000** (`total_trades` stuck
  at 224 538; population stable ~1 997). This is the real freeze.
- **`geographic-trade` never fully freezes** — it decays to ~1 % of peak but
  *sputters on*, because `terrain_habitat` keeps agents migrating and re-sorting,
  continually re-creating imbalances. (The 2 000-tick window that the plan's
  Task-3 test happened to check still showed healthy trade — which is why that
  test was a false positive.)

### 2. The frozen state is EMPTY inventories, not saturated ones

`biome-trade` @ t12 000 (frozen), all 1 995 alive agents:

- per-good mean inventory = **`[0.0, 0.0, 0.0, 0.0]`**
- agents holding ≥ `STOCK_TARGET` on all goods: **0 / 1995**
- agents holding < `TRADE_UNIT` on any good: **1995 / 1995**
- agents with `want > 0` on some good: **1995 / 1995** (they want everything)
- resource nodes present: **160 nodes, 1 744.9 units available**

So the world is *not* saturated — it is **starved**. Goods are available in the
biome; agents just don't hold any. `pick_swap` gates on `inv[give] ≥ TRADE_UNIT`
(`interact.rs:442`), so empty agents cannot trade regardless of how much they want.

### 3. Perishability does not fix it (it makes it worse)

`biome-trade`, base vs. `perishable_goods=true` (`PERISH_RATE=0.001`), trades per
2 000-tick window:

| window end | base | perishable |
|---:|---:|---:|
| 2 000 | 54 588 | 69 126 |
| 4 000 | 96 811 | 49 658 |
| 6 000 | 70 512 | **4 460** |
| 8 000 | 2 627 | 214 |
| 10 000 | 0 | 0 |

Both freeze at t10 000; perishability collapses *earlier* mid-run. On
`geographic-trade`, perishability *lowered* trade at every window (late/early
ratio 0.545 vs. non-perishable 0.628 at 2 000 ticks; 0.015 vs. 0.143 at 4 000).
Uniform multiplicative decay drains the give-side below the `TRADE_UNIT` gate —
accelerating starvation — and preserves basket symmetry, so it never re-arms a
beneficial swap.

## Root cause

`biome-trade` runs with `resources_enabled` only — **inventions and cognition
are off**, so `consume_materials` is not the drain. Goods leave the population
via **death-churn**: a dead agent's inventory is lost (not transferred), and
ungated reproduction floods the world with **empty newborns**
(`spawn_zeroes_inventory`). At ~2 000 agents dispersed across the clusters, the
tiny `HARVEST_RANGE = 2.0` refills far too few agents per tick to keep pace, so
the population's total goods bleed to zero and stay there. Trade — being pure
redistribution — cannot restart with nothing to redistribute.

## Implications for the redesign

The redesign must target **supply**, not demand. Candidate levers (each opt-in
by scenario flag, golden-tested, per the determinism contract):

- **Keep goods in the population across death** — transfer a dead agent's
  inventory to a nearby agent / scavenge, instead of destroying it.
- **Sustain harvest access at scale** — larger `HARVEST_RANGE`, or node spawning
  that tracks the *dispersed* population rather than one cluster centroid.
- **Temper the churn dilution** — the empty-newborn flood under ungated
  reproduction is a major sink; a per-scenario cap or a small inherited basket
  would slow the bleed.

What will **not** help (measured): perishability, and non-satiating `want`.

## Status of the abandoned perishability work

The `perishable_goods` flag + `perish_step` + the Task-3 test were implemented
on a local branch before this investigation, then abandoned (they don't fix the
real cause). They are not merged and not in this PR. This PR corrects the
diagnosis in `ROADMAP.md` and the redesign plan so a future attempt targets the
supply side.

## Reproduce

Build release, then run the trade scenarios for ≥12 000 ticks and count
`world.trade_routes` per tick (or read `world.total_trades` — it plateaus at the
freeze). Dump `world.agents.inventory` at t12 000 on `biome-trade` to see the
empty baskets; `world.resources` shows nodes still available.

---

## Update (2026-08-02): six candidate fixes eliminated — the freeze is a structural barter equilibrium

Before/while attempting the supply-side redesign, six candidate mechanisms were
measured against `biome-trade` at 16 000 ticks. Throughout the freeze the world
is *healthy*: population ~2 000, **163–231 distinct species alive** (largest
share ~13 %), resource nodes available. Trade thrives early (~50–150 k swaps in
the first 2 k ticks) then collapses to **0 by ~t8–10 k, permanently.**

| # | Candidate | Result |
|---|---|---|
| 1 | Perishability (demand decay) | Freezes at the same tick; *worse* mid-run (drains give-side below `TRADE_UNIT`). |
| 2 | Conserve goods on death (lever A) | Goods conserved but **concentrated** on ~7 "nearest-living" hoarders (mean 3–5/good, yet 1987/1995 below `TRADE_UNIT`). Still 0. |
| 3 | Harvest access ×6 (`HARVEST_RANGE` 2→12, nodes 40→120, lever B) | B-only late=0; A+B late=0. Not harvest access. |
| 4 | Species-collapse check | **Disproven** — 163+ species remain; cross-species partners exist. |
| 5 | Invention material sink (`inventions_enabled`+`cognition`) | late=0. (Caveat: grazers may not *learn*, so the sink may never activate — flag-flip alone doesn't help.) |
| 6 | (baseline, no fix) | Freezes ~t10 k. |

**Best current framing (a structural equilibrium, not a supply *or* demand bug
alone):** `pick_swap` is **bilateral barter** requiring both parties to spare a
`TRADE_UNIT`, and `want` saturates at `STOCK_TARGET`. Trade therefore drives
every agent toward the *same* balanced `STOCK_TARGET` basket and then **stops** —
once holdings equalize (or saturate, or deplete) there are no mutual want-gains
left, and nothing in the scenario regenerates the asymmetry/demand that made
early trade thrive (the original reproduction-dowry sink was removed; the
invention sink is off, and even enabling it did not restart trade in this test).
Harvest keeps *individual* home-good surpluses flowing early, but once the
cross-sectional distribution equalizes, bilateral barter has nothing left to do.

**Implication:** a durable fix likely requires changing the *demand model*
itself (e.g. per-agent heterogeneous, non-saturating, continuously-regenerated
demand — a real consumption loop), not a supply tweak or a single flag. That is a
research spike, not a quick fix. Note that the trade economy **is** functional as
an early/mid-game mechanic (thousands of ticks of vigorous trade) — the open
question is whether indefinite late-game trade is even a goal, or whether
bounded-duration trade is acceptable.

### Update 2 (2026-08-02): comparative-advantage consumption also fails — the barter *primitive* is the problem

Seventh candidate: a per-tick consumption loop (each agent consumes a little of
every good, regenerating deficits — the textbook "produce one, consume many,
trade" model), prototyped and swept on `biome-trade` (16k ticks):

| ZZ_CONSUME/good/tick | biome-trade late | +terrain_habitat late |
|---|---|---|
| 0 (baseline) | 0 | 319 |
| 0.02 | 0 | 0 |
| 0.05 | 0 | 0 |

Consumption *lowered* trade even early (54.6k→14.6k→5.5k) and still froze late —
because it drains the **give-side** below `TRADE_UNIT` (it consumes the home good
too; sparse harvest can't refill), the exact trap perishability hit.

**Conclusion — the freeze is intrinsic to the exchange *primitive*, not any
supply/demand parameter.** `pick_swap` is **bilateral barter**: both parties must
hold ≥ `TRADE_UNIT` of what they give, and `want` saturates at `STOCK_TARGET`.
This makes trade a pure *redistribution-to-equilibrium* process that
self-terminates, and it is squeezed from both sides:
- anything that regenerates demand (perishability, consumption) also drains the
  give-side below `TRADE_UNIT` → trade dies;
- anything that adds supply (harvest ×6, conserve-on-death) → saturation or
  hoarding → trade dies.

Seven mechanisms were eliminated (perishability, conserve-on-death, harvest ×6,
species-diversity [not the cause], invention-sink flag, comparative-advantage
consumption, and combinations). A durable fix requires **redesigning the exchange
primitive itself** — e.g. dropping the bilateral both-must-give constraint
(allow unilateral sale / one-sided transfer), or a **price/market-mediated**
exchange with continuous fractional quantities instead of a fixed `TRADE_UNIT`
barter. That is a substantial engine redesign, not a lever or a spike. The trade
economy remains a compelling **early/mid-game** mechanic (thousands of ticks of
vigorous trade); indefinite late-game trade needs the primitive rework.
