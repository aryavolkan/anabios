# Open-Ended Complexity Arc (O1–O8) — Design Spec

**Date:** 2026-08-02
**Status:** Approved (framing + arc + cross-cutting approved in brainstorming)
**Supersedes-in-scope:** the single open research item in `ROADMAP.md` (the Out-of-Africa
climb problem) and the `2026-08-01-out-of-africa-climb-experiment.md` plan, which are
folded into O1/O3 as prior work.
**Depends on:** the completed **E1–E10 emergence arc** (53 codex event types shipped:
population dynamics, disturbance/succession, trait evolution, named behaviors, war/kin,
settlements, traditions, maladaptation, dimorphism, domestication) and the E1 emergence
scorecard / novelty archive.

---

## 1. Goal & thesis

E1–E10 answered *"can we see emergence?"* — yes, across 53 detectors. This arc answers the
next question: **can emergence keep going — climb, compound, and cross levels of
organization — on its own?**

Today it cannot. The Out-of-Africa grand run stalls at era-1, the novelty of long runs
decays, no new levels of organization appear, and one strategy dominates rather than many
niches coexisting. The single root cause behind all four failures:

> **In anabios today, sustained complexity doesn't pay for itself.** Fast, simple, asocial
> strategies win because the selective landscape never rewards cumulative investment.

Every one of the four requested north stars is a facet of that one problem:

| North star | The same problem, seen from one angle |
|---|---|
| **Emergent era-3 climb** | Culture is competitively excluded before it can ratchet. |
| **Non-decaying novelty** | No standing pressure rewards continued innovation; runs stationarize. |
| **Major transitions** | No payoff to becoming a higher-level unit, so none forms. |
| **Adaptive radiation** | One strategy dominates instead of many niches persisting. |

So this arc is **not** "more substrate + more detectors" (that was E1–E10). It is:
**engineer the selective landscape and cognitive capacity so that sustained complexity
becomes a winning long-run strategy** — then prove it, on the hardest concrete target first.

### 1.1 Structure decision (why climb-first, general-machinery)

Considered three structures: (A) climb-first with general machinery, (B) open-endedness
engine first, (C) four parallel pillars. **Chose A.** It is the only structure that yields
*falsifiable evidence early* — emergent era-3 is a yes/no that cannot be fudged — and
because cracking the climb genuinely exercises lifetime learning + cultural niche
construction + transmission fidelity at scale, **the machinery built to crack the climb is
the open-endedness engine**; each later phase points the same levers at the next north star.
(C) was rejected because it misses the shared root cause — it would build four systems that
are really one. (B) was rejected because it risks machinery that never actually produces the
climb, with evidence arriving late.

### 1.2 Relationship to the invariants

Brainstorming re-negotiated anabios's hard invariants for this arc:

- **Hand-engineered-cognition wall: DOWN.** Lifetime (within-life) learning is explicitly
  on the table (O2). Deep neural cognition stays out **unless O1 proves it is required**.
- **Determinism: negotiable per-mechanism** (see §3). Default stays deterministic; a
  per-mechanism, flagged escape hatch exists where it demonstrably buys open-endedness.
- **Opt-in / baseline-stable: HOLDS.** Every O-mechanism ships off-by-default behind a
  scenario flag; existing scenarios and goldens stay stable; each milestone is tagged
  (`o1`…`o8`) per the `m*`/`e*` convention.

---

## 2. The arc

Eight milestones, **O1–O8**, in seven phases. Odd emphasis on *lens* (measure/adjudicate),
even emphasis on *substrate* (new causal machinery) — but the binding order is the
**climb-attack dependency chain** O1 → O2 → O3 → O4, after which the same machinery is
re-aimed at the remaining north stars.

| # | Phase | Milestone | Kind | North star advanced | New events (est.) |
|---|---|---|---|---|---|
| **O1** | I. Diagnose | Competitive-exclusion autopsy | lens | *(de-risks all)* | — (analysis) |
| **O2** | II. Cognition | Lifetime learning substrate | substrate | climb, transitions | +2 |
| **O3** | III. Make culture pay | Cultural niche construction | substrate | **emergent era-3 climb** | +2 |
| **O4** | III. Make culture pay | Transmission fidelity at demographic scale | substrate | climb (collective brain) | +1 |
| **O5** | IV. Keep pressure on | Co-evolving environment (POET-lite) | substrate | **non-decaying novelty** | +1 |
| **O6** | V. Jump levels | Major transitions (higher-level individuality) | substrate | **major transitions** | +2 |
| **O7** | VI. Fill niches | Adaptive radiation & niche depth | substrate | **adaptive radiation** | +2 |
| **O8** | VII. Capstone | Open-ended soak & discovery meta-loop | lens | all four, together | — (meta) |

Event-type total at arc completion: **≈63** (53 current + ~10 new).

### Phase I — Diagnose

#### O1 — Competitive-exclusion autopsy *(lens)*

**Goal:** before building anything, quantify *why* culture loses, and name **the** dominant
lever. This adjudicates a live disagreement in the existing docs: the
`out-of-africa-climb-experiment` plan hypothesizes the **era-3 IQ ceiling** is the blocker,
while the recorded project finding is that the block is **ecological competitive exclusion of
culture by a fast asocial forager** — and that the IQ ceiling is never even approached. O1
settles this with measurement, not priors.

- **Instrumentation:** a per-strategy **fitness ledger** (lifetime reproductive success
  decomposed by cost/benefit component: foraging return, learning cost, teaching cost,
  invention buff/debuff, mortality) computed post-hoc over event/telemetry streams — zero
  sim impact.
- **Invasion analysis (the core tool, both directions):** seed a rare cultural mutant into an
  asocial-forager-dominated world and measure whether its lineage frequency grows from rare;
  seed a rare asocial mutant into a cultural world and measure the same. Exclusion is
  confirmed only when culture *cannot* invade **and** asocial *can* re-invade.
- **Lever sweep:** one-variable scans over the candidate knobs the existing plan already
  enumerates (learning/teaching cost, transmission fidelity, resource ceiling, founding
  density/geography, IQ gate) — each attributed, confound-guarded.
- **Deliverable:** `docs/superpowers/specs/2026-08-…-o1-exclusion-findings.md` with the
  fitness ledger, the bidirectional invasion result, the timescale/margin culture loses on,
  and a named dominant lever with a single confirming intervention.
- **Done when:** the diagnosis predicts *and* a single targeted intervention confirms which
  knob moves the invasion margin. The IQ-ceiling-vs-exclusion question is answered with data.
- **Determinism/perf:** post-hoc analysis + existing scenario variants — no sim change, no
  hash change.

### Phase II — Cognition

#### O2 — Lifetime learning substrate *(substrate)*

**Goal:** cut the up-front cost of culture. A naive juvenile who *learns* within its lifetime
(from experience and from observing others, with error) pays less of the culture tax that
O1 shows is fatal — and within-life adaptation is itself a lever for later major transitions.

- **Substrate (smallest viable learner):** a bounded, deterministic within-life update over
  the **existing** postfix behavior program or a tiny tabular action-value layer — **no
  neural nets** unless O1 proved they are required. Learning draws from a dedicated seeded RNG
  substream so bit-identity holds. Opt-in via `lifetime_learning_enabled`.
- **Two modes:** individual reinforcement (update from own outcomes) and social learning
  (imitation-with-error of high-payoff neighbors) — the second is the bridge to O3/O4.
- **Detectors:** `LearnedBehavior` (an agent's action-policy measurably diverges from its
  genome-specified default toward higher payoff within one lifetime), `ImitationCascade` (a
  learned behavior spreads by observation faster than genetic inheritance could carry it).
- **Scenario:** `scenarios/lifetime-learning.toml` — a task with a learnable optimum a
  genome-only agent cannot pre-specify.
- **Evidence:** handcrafted world where a learner reaches the optimum and a genome-only
  control does not; sweep firing both detectors; **measured drop in naive-juvenile culture
  cost vs the genome-only baseline** (feeds O3).
- **Determinism/perf:** deterministic by default (seeded substream, golden-tested,
  save→load→step round-trip for the new learned state — the `#[serde(skip)]` footgun applies).
  Per-species buffers only; joins the fused aggregation pass.

### Phase III — Make culture pay

#### O3 — Cultural niche construction *(substrate)* — **the climb crack**

**Goal:** the anti-exclusion engine. Culture must **reshape the environment** so that
cultural bearers create the very conditions under which more culture pays — an autocatalytic
loop that builds a *protected niche* for culture instead of competing head-to-head with fast
foragers on the forager's terms.

- **Substrate:** invention/practice holders modify biome cells (farming raises local carrying
  capacity → food surplus → higher local density) and **vertical resource tiers** gate access
  by era — asocial foragers hit a hard ceiling on era-0 resources while cultural strategies
  unlock era-gated food/materials they physically cannot reach otherwise. The loop:
  *culture → niche modification → surplus → density → more transmission partners → higher tech
  ceiling → new resource tiers → more culture.*
- **Detectors:** `NicheConstructed` (a lineage's activity durably raises local carrying
  capacity above the unmodified baseline), `ResourceTierUnlocked` (a species accesses an
  era-gated resource class for the first time).
- **Scenario:** `scenarios/out-of-africa-emergent.toml` — the grand run with niche
  construction on and **nothing seeded** (`starting_inventions` empty).
- **Done when (north-star bar):** **the grand Out-of-Africa run reaches an era-3 milestone
  (Writing/Husbandry/Metalworking) with nothing seeded** — validated by O1's invasion
  analysis (a rare cultural mutant *invades*; a fast forager *cannot* re-invade the resulting
  cultural world) — **OR** the seeding decision is documented with that same invasion-analysis
  evidence showing why first-principles emergence is not reachable. A documented negative is a
  valid deliverable; a hand-tuned "you-win-era-3 button" is not.
- **Determinism/perf:** behavior-altering → golden regeneration expected; new scenario keeps
  shipped scenarios' goldens stable. Anti-self-deception gate: the invasion test, not the
  raw climb curve, is what counts.

#### O4 — Transmission fidelity at demographic scale *(substrate)*

**Goal:** Henrich's collective brain — the sustainable tech ceiling should rise with the size
of the connected, transmitting population, and the ratchet should survive turnover and
bottlenecks. This is what turns O3's local loop into a durable climb.

- **Substrate:** transmission fidelity depends on the number of available same-culture models
  (more connected minds → less ratchet slippage), building on the E9 meme-lineage fidelity
  tracking. Ties institutional memory (E9 `InstitutionalRatchet`) to O3 density.
- **Detector:** `CollectiveBrainThreshold` (a culture's sustainable tech era rises past a
  bar only once connected population exceeds a measured size).
- **Scenario:** `scenarios/collective-brain.toml` — matched worlds differing only in
  connected-population size.
- **Done when:** a measured **monotone relationship between connected-population size and
  sustainable tech ceiling**, and the ratchet **survives a forced bottleneck** (population
  crash) without era regression (ties to E9).
- **Determinism/perf:** deterministic; per-species; golden-refresh for the fidelity coupling.

### Phase IV — Keep pressure on

#### O5 — Co-evolving environment (POET-lite) *(substrate)*

**Goal:** once complexity pays (O3/O4), stop the selective landscape from stationarizing —
the mechanism behind north star #2. A co-evolving environment keeps posing new problems so
novelty does not decay.

- **Substrate:** environment features (climate drift — already seeded — plus resource-layout
  and disturbance-regime parameters) that **track agent capability**: as agents master the
  current regime, the regime shifts (minimal-criterion coevolution — keep agents meeting a
  minimal bar as the environment moves). This is the **one candidate for the non-deterministic
  research escape hatch** (a wall-clock-seeded environment search), evaluated against a
  deterministic seeded-schedule variant first.
- **Detector/instrument:** `OpenEndedArmsRace` (a capability↔environment metric pair that
  keeps climbing without settling over a long window).
- **Scenario:** `scenarios/coevolving-world.toml` vs a static-environment control.
- **Done when (north-star bar):** the **novelty-per-100k-ticks decay curve flattens** on a
  matched soak relative to the static-environment control, on a **pre-registered** metric and
  fixed soak length (no post-hoc curve-fitting).
- **Determinism/perf:** deterministic-schedule mode ships first and golden-tested; any
  non-deterministic search mode is flagged and governed by the §3 statistical-reproducibility
  contract; soak must hold the perf floor (§3).

### Phase V — Jump levels

#### O6 — Major transitions: higher-level individuality *(substrate)*

**Goal:** a genuine jump in level of organization — aggregation → division of labor →
heritable reproduction *as a unit* — reusing O2 (learning), O3 (niche/surplus), and the E8
settlement substrate. The bar is Darwinian dynamics **at the new level**, not a new pattern.

- **Substrate:** agents can bind into a persistent aggregate (coloniality / settlement-as-
  unit) with role differentiation; the aggregate acquires a heritable, selectable group-level
  trait and can seed daughter aggregates.
- **Detectors:** `AggregationBound` (a persistent multi-agent unit with role differentiation
  forms and endures), `MajorTransition` (heritable variation **and** a measured selection
  differential exist on a group-level trait — the transition is Darwinian at the new level).
- **Scenario:** `scenarios/major-transition.toml` — built on `settlement.toml`, aggregation
  enabled.
- **Done when:** `MajorTransition` fires with a demonstrated **selection differential on a
  group-level trait** (heritability × differential > 0), not merely a co-location pattern.
- **Determinism/perf:** behavior-altering, flagged, golden-refresh; group-level state gets a
  save→load→step round-trip test.

### Phase VI — Fill niches

#### O7 — Adaptive radiation & niche depth *(substrate)*

**Goal:** one seed lineage diversifies into many coexisting, persistent niches — north star
#4 — by deepening niche space and adding the selection that *maintains* diversity.

- **Substrate:** a higher-dimensional niche space (resource types × spatial heterogeneity ×
  trophic role, extending the biome/trade substrates) plus **negative-frequency-dependent
  selection** (a strategy's payoff falls as it becomes common) so radiations don't collapse
  back to a single winner. Key-innovation triggers (an invention/learned behavior opening a
  new axis) seed the radiation.
- **Detectors:** `AdaptiveRadiation` (one lineage → k persistent descendant forms occupying
  distinct niches within a window), `CharacterDisplacement` (two sympatric lineages'
  trait distributions diverge where they overlap).
- **Scenario:** `scenarios/adaptive-radiation.toml` — one generalist starter in a
  niche-rich, frequency-dependent world.
- **Done when:** **sustained multi-niche richness from one starter**, held elevated vs a
  single-niche control over the window, with `CharacterDisplacement` confirming the niches are
  maintained by selection (not drift).
- **Determinism/perf:** flagged, golden-refresh; richness metric joins the fused pass.

### Phase VII — Capstone

#### O8 — Open-ended soak & discovery meta-loop *(lens + capstone)*

**Goal:** run everything together and report, honestly, whether the four north stars hold at
once — extending the E10 open-ended-engine capstone.

- **Core/headless:** one documented **million-tick soak** with O2–O7 levers on: novelty
  archive (E1) wired in to report the novelty-per-100k-ticks decay curve; per-north-star
  telemetry (max emergent era; novelty-decay slope; count/level of major transitions;
  sustained niche richness). Memory/perf telemetry with no pathological state growth.
- **Meta-loop:** the cross-world codex/novelty archive becomes the discovery driver — runs
  firing corpus-novel events are surfaced for inspection (E1 `--archive` + E2 replay).
- **Done when:** a single soak-run artifact set reports **each of the four north stars as hit
  or honestly missed**, with the evidence discipline of §3, committed to the O8 plan; README
  updated with the full event roster and a "reproduce a north star" guide.
- **Perf:** soak holds the §3 perf floor end-to-end; any growth leak is a blocker.

---

## 3. Cross-cutting invariants (hold for every O-milestone)

### 3.1 Determinism — negotiable per-mechanism, spent deliberately

Determinism is what makes the codex, replay, and golden tests *mean* something; most of this
arc doesn't need to break it (O2 learning and O5 env-generation both run off seeded RNG
substreams and stay bit-identical). The rule:

1. **Default: determinism holds.** Every mechanism is deterministic + golden-tested unless it
   buys open-endedness that determinism *provably blocks*.
2. **Escape hatch, per-mechanism, flagged.** A milestone may add a **non-deterministic
   research mode** (e.g. async/parallel learning, wall-clock-seeded env search) only behind
   its own flag, and only after a deterministic variant is shown to underperform. Shipped and
   showcase scenarios stay deterministic.
3. **Statistical-reproducibility contract** replaces bit-identity for any such mode: same seed
   → same *distribution* of the north-star metric across N runs, within a stated CI; the full
   run is archived. O5 is the only pre-identified candidate; O1 confirms whether O2–O4 need it
   (prior: they do not — we likely reach emergent era-3 fully deterministic).

### 3.2 Evidence discipline (the honesty gate)

Carried forward from the gene-culture record (cumulative-skill culture *did* sweep
Inventiveness; a first-principles culture gene *did not* sweep from standing variation;
winner-take-all caps mask differentials). Every "complexity now pays" claim ships with:

1. **Invasion analysis, both directions** — a rare cultural/complex mutant *invades* a
   simple-dominated world **and** the simple mutant *cannot* re-invade. That, not a growth
   curve, is the definition of a stable win.
2. **Seeded vs first-principles A/B** — every climb/transition result states plainly whether
   it emerged from standing variation or needed seeding, with the measured margin. "Honestly
   documented as seeded" is an acceptable milestone outcome; "vibes" is not.
3. **Negative controls & confound separation** — module-vs-gene disentangled before crediting
   a sweep; every statistical detector ships a scripted near-miss that must **not** fire.
4. **Scorecard-driven & pre-registered** — O-scenarios are picked off the E1 scorecard corpus;
   open-endedness metrics (O5 novelty decay, O7 richness) are pre-registered before looking.

### 3.3 Determinism gate, perf budget, shippability

- **Determinism gate stays green** where §3.1 default applies: behavior-altering work
  regenerates golden hashes deliberately (`UPDATE_HASHES=1 …`), values copied into
  `tests/determinism.rs`, called out in the milestone plan; new persistent state (learned
  policies O2, group-level state O6) gets a **save→load→step round-trip test** (the
  `#[serde(skip)]`-feeds-hashed-state footgun is the recurring failure mode).
- **Perf budget:** ≤10% tick-time regression at 10k agents per milestone on the criterion
  suite; O8 soak holds ≥30 ticks/s at 10k agents end-to-end. History buffers are per-species,
  not per-agent, and join the fused aggregation pass.
- **Independently shippable:** each milestone lands behind a scenario flag, keeps baseline
  scenarios unchanged, and is tagged `o1`…`o8`. Each gets its own spec + plan pair under
  `docs/superpowers/{specs,plans}/` named `2026-…-oN-<slug>.md`; this roadmap is the index.
- **Cross-cutting meter:** the **novelty-per-100k-ticks decay curve** is reported at **every**
  phase on a soak run — north star #2 is the arc's continuous vital sign, not just O5's target.

---

## 4. Sequencing & dependencies

```
I     O1 autopsy ─────────────► (names the dominant lever; settles IQ-ceiling vs exclusion)
II    O2 lifetime learning ─┐
III   O3 niche construction ─┼─► EMERGENT ERA-3 (climb ✅)   [O3 done-when = the crack]
      O4 fidelity@scale ─────┘   (collective brain; ratchet survives bottleneck)
IV    O5 co-evolving env ──────► NON-DECAYING NOVELTY ✅
V     O6 major transitions ────► MAJOR TRANSITIONS ✅    ┐ reuse O2+O3+E8 machinery
VI    O7 adaptive radiation ───► ADAPTIVE RADIATION ✅   ┘
VII   O8 soak + meta-loop ─────► all four, measured together
```

**Why this order:** O1 first because building before diagnosing is how the arc fools itself.
O2→O3→O4 is a strict chain — learning cuts culture's cost, niche construction makes culture
pay, fidelity-at-scale makes the payoff durable; era-3 is unreachable until all three land.
O5 only matters once complexity pays (nothing to keep novel otherwise). O6/O7 deliberately
*reuse* O2/O3 machinery rather than adding new cognition — they re-aim it at a new level and a
new axis. O8 is unstartable before O1/E1/E2 and is last by construction.

## 5. Risks & mitigations

- **Lifetime learning explodes scope / erodes determinism (O2).** Smallest viable learner
  over the existing program; deterministic-first; opt-in; golden round-trip. No neural nets
  unless O1 proves them necessary.
- **Niche construction becomes a scripted "win era-3" button (O3) — highest self-deception
  risk.** The invasion-analysis gate (§3.2.1) is the guard: a hand-tuned world that favors
  culture without a rare cultural mutant *invading* does not count.
- **Non-decaying novelty is unfalsifiable / curve-fitted (O5).** Pre-registered metric +
  static-environment control + fixed soak length before looking.
- **Major transitions / radiation collapse into "just another detector" (O6/O7).** Each must
  show heritable variation **and** selection at the new level/axis — Darwinian or it doesn't
  count.
- **Determinism erosion creeps in silently.** The escape hatch is per-mechanism, flagged, and
  every milestone names its tick-pipeline insertion point + RNG substream before
  implementation.
- **Heavy sweeps/soaks are expensive.** Pilot at reduced seeds/ticks before full runs; run on
  the release binary over rayon; let PR CI run the heavy determinism/emergence suite (don't
  run the full golden/soak suite locally every commit).

## 6. Explicitly out of scope

- Rewriting the viewer in another engine; real-time multiplayer / a playable game loop.
- A WASM live-in-browser simulation fork (flagged for a later arc).
- Deep neural cognition — O2 stays a minimal learner **unless O1 explicitly demands otherwise**.
- New agent-level "objects" (buildings, government): O6 settlements/aggregates stay field- and
  lineage-property based, per the E-arc discipline.

## 7. Open questions (resolved as phases close)

1. **IQ ceiling vs competitive exclusion** — which actually blocks the climb? **RESOLVED:**
   competitive exclusion, not the IQ ceiling (baseline never exceeds `max_era=0`, so the
   era-3 IQ gate is never even tested); dominant lever = the cognition/IQ subsystem itself,
   confirmed 4/4 seeds. See
   [2026-08-03-o1-exclusion-findings.md](2026-08-03-o1-exclusion-findings.md).
2. **Does emergent era-3 need seeding?** (O3's done-when accepts either outcome, with the
   invasion evidence.)
3. **Does O5 need the non-deterministic escape hatch, or does a seeded env schedule suffice?**
4. **Is one connected-population "collective brain" enough, or does the ratchet need
   institutions (E9) to survive bottlenecks?** (O4 measures it.)

---

_Living document — revise as O-milestones close. This roadmap is the index; each milestone
gets its own spec + plan. Sizing is sequencing intent, not commitment._
