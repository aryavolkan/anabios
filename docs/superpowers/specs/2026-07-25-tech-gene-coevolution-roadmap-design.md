# Tech–Gene Coevolution Roadmap — Design Spec

**Date:** 2026-07-25
**Status:** Draft (awaiting review)
**Depends on:** the inventions tree (`invention.rs`), cognitive gene–culture (`iq.rs`, cognition gate), the `[Y]` coevolution time-series panel (`coevolution_panel.gd`, `coevo.rs`), and the headless capture / gallery pipeline.

## 1. Goal

Turn the invention tree from a system that genes *gate one-way* into a genuine **gene↔tech coevolutionary loop**, and make that loop **legible** — both live in the Godot viewer and offline as a shareable artifact.

Today the coupling is one-directional and under-instrumented:

- **Genes → tech (exists):** `Openness` scales discovery rate; `CognitivePotential → IQ` gates acquisition per era; the `Communicator` module is required to innovate. So the gene distribution decides *whether* tech advances.
- **Tech → genes (missing):** holding an invention applies fitness buffs/debuffs, but those effects are **gene-independent** — a high-`Openness` holder and a low-`Openness` holder get the identical multiplier. Culture therefore exerts almost no *directional selection* on the genome. There is no feedback arm, so "coevolution" is currently two curves plotted side by side, not a coupled system.
- **Legibility gap:** `tech_panel.gd` is a flat text table (no DAG structure), the `[Y]` panel wires only 5 of the 10 inventions and never pairs a tech with the gene it couples to, and nothing computes or shows the **lead–lag** relationship that would demonstrate coevolution (did the gene rise *before* the tech, or did the tech drag the gene up after?).

This roadmap closes both arms across **five milestones, TG1–TG5**, one PR each, following the same commitments as the E-series roadmap.

### 1.1 What "coevolution" means here (the mechanic in one paragraph)

Each invention gains an optional **affinity gene** with a **selection coefficient**. When the invention is held, its fitness-relevant buff is made *conditional on that gene* (a holder with a higher affinity value gets a larger buff). Because the buff feeds reproduction, holders drift the gene mean up over generations — culture now selects the genome. The invention still *spreads* as a meme, so the loop is: gene distribution enables discovery (arm 1, exists) → adoption applies gene-conditional selection → gene mean rises → discovery/effectiveness of dependent tech improves (arm 1 again). If the meme collapses (atrophy, extinction, dark age) the selection pressure vanishes and the gene can drift back — a coevolution loop **with hysteresis**, which is exactly what the lead–lag view is built to show.

## 2. Cross-cutting invariants

These hold for **every** TG-milestone; restated per-milestone only when at risk.

1. **Determinism gate stays green.** Behavior-altering milestones (TG1, TG2) regenerate golden hashes deliberately (`UPDATE_HASHES=1 …`, values copied into `tests/determinism.rs`) and call it out in the milestone plan. Pure-viewer/analysis milestones (TG3, TG4, TG5) must change **no** hash.
2. **No new hashed hidden state.** The coupling mechanic is driven by a **static const table** (affinity metadata) and folds into *existing* deterministic multipliers — **no new `#[serde(skip)]` accumulators** that feed hashed state (the known save→load→step replay footgun), and **no new RNG draws** on any existing scenario. Instrumentation reads state; it never mutates it.
3. **Identity when disabled.** All new behavior sits behind a scenario flag (`gene_tech_coupling` for TG1, `MEME_CHANNELS` growth guarded by the existing `inventions_enabled` for TG2). With the flag off, every multiplier is bit-identical to today and baseline scenarios stay unchanged.
4. **Perf budget.** ≤10% tick-time regression at 10k agents on the criterion suite per behavior milestone. The per-tick coupling instrumentation reuses the existing `SampleScratch` buffers — no per-tick allocation in the render loop.
5. **Evidence trio per phenomenon.** Every milestone ships (a) handcrafted unit/integration tests, (b) ≥1 headless seed demonstrating the effect (for TG1: a measurable selection differential; for TG4/TG5: a run where the gene demonstrably leads or lags a tech), and (c) ≥1 gallery capture with an honest caption.
6. **Independently shippable & tagged.** Each milestone lands as its own PR tagged `tg1`…`tg5`, with a spec+plan pair under `docs/superpowers/{specs,plans}/` named `YYYY-MM-DD-tgN-<slug>.md`. This roadmap is the index, not the plan.

## 3. Milestones

The order is deliberate: **the mechanic ships first** (TG1) because every visualization downstream points at the data it produces. Content expansion (TG2) is separated from the mechanic because it carries the expensive `MEME_CHANNELS` layout change and golden regen, and should not be entangled with the selection logic. Then two lenses (TG3 DAG, TG4 coupling) and the offline artifact (TG5).

```
TG1 (mechanic) ──┬──> TG2 (content, more nodes/branches)
                 ├──> TG3 (Godot DAG view)
                 ├──> TG4 (Godot coupling / lead-lag view)
                 └──> TG4 ──> TG5 (headless capture + HTML artifact)
```

TG3/TG4/TG5 depend on TG1's affinity metadata + instrumentation, not on each other; they can ship in any order after TG1. TG2 is independent of the viewers.

### TG1 — Gene↔tech feedback loop *(mechanic)* — **detailed spec: `2026-07-25-tg1-gene-tech-coupling-design.md`**

**Goal:** close the tech→gene arm and instrument both arms.

- **Core:** add `affinity: Option<GeneAffinity>` to `Invention` (`{ slot: GenomeSlot, coeff: f32 }`). Behind a new `World::gene_tech_coupling` flag, make each affinity-bearing invention's buff gene-conditional (e.g. `Writing` spread bonus scales with the currently-inert `CommunicationStrength`; `Medicine` lifespan scales with `CognitivePotential`; `Electricity` discovery scales with `Openness`). Generalize the discovery-rate gate so per-tech innovation probability reads the invention's affinity gene, not just global `Openness`.
- **Instrumentation:** extend `CoevoSample` with, per affinity-bearing invention, the **affinity-gene mean over holders vs non-holders** (the selection differential Δ) and the world affinity-gene mean. This is the raw material TG4/TG5 correlate.
- **Evidence:** a headless seed where, with the flag on, an invention's affinity-gene mean rises measurably above the flag-off control run (directional selection), and reverts after an induced extinction of the meme (hysteresis).
- **Determinism:** identity when flag off (golden hashes unchanged); flag-on scenarios get freshly-regenerated goldens. No new RNG on flag-off paths.

### TG2 — Expand & branch the tree *(content)*

**Goal:** grow the 10-node mostly-linear chain into a branching DAG with competing paths, each new node carrying a TG1 affinity gene so it feeds the coevolution story.

- **Constraint (the real cost):** `MEME_CHANNELS = 20` is **full** — channels 8–17 are the 10 inventions, 18–19 the two practices. Adding inventions requires bumping `MEME_CHANNELS`, shifting `PRACTICE_CHANNEL_BASE` up, regenerating goldens, and auditing every hardcoded channel index (`coevolution_panel.gd` markers/series, `SenseMeme` clamps, `practice.rs`). This is why content is its own PR.
- **Core:** bump `MEME_CHANNELS` by the number of new inventions; add ~4–6 inventions forming **two competing branches** (e.g. a *social/knowledge* path culminating in Writing→Printing→… vs an *industrial* path Metalworking→Machinery→…), each with distinct affinity genes so different branches select different genomes → branch choice becomes visible as genetic divergence.
- **Optional (YAGNI-gated):** a tech→module unlock so a late invention grants a morphology module. Deferred unless it earns its keep after TG1.
- **Evidence:** a run where two lineages commit to different branches and diverge genetically along the branches' affinity genes.
- **Determinism:** deliberate golden regen (layout change); flag-off baseline still identity because invention channels stay zero when `inventions_enabled` is off.

### TG3 — Interactive tech-tree DAG view *(lens, Godot)*

**Goal:** replace the flat `tech_panel.gd` table with a real DAG.

- **Viewer:** nodes laid out by era (columns) × branch (rows), edges = prereqs, node fill = world adoption fraction, per-node buff/debuff tooltip, and an **affinity-gene badge** (which gene this node selects for, colored by current selection differential sign). Hover a node → highlight its prereq chain. Live, updates from `species_stats()` / a new adoption-frac accessor.
- **Also:** wire all 10 (post-TG2: all) inventions into the `[Y]` panel's adoption chart (only 5 are shown today).
- **Evidence:** gallery capture of the DAG mid-run with a partially-unlocked tree.
- **Determinism:** view-only, **no hash change**.

### TG4 — Gene↔tech coupling / lead–lag view *(lens, Godot)*

**Goal:** the hero analytical view — make the feedback *explicit*.

- **Core/bridge:** add pure cross-correlation helpers in `coevo.rs` (lagged Pearson between an invention's adoption series and its affinity-gene-mean series), unit-tested in isolation.
- **Viewer:** a new panel (or `[Y]` sub-mode) that, per affinity-bearing invention, shows adoption(t) and affinity-gene-mean(t) overlaid, plus the **lag at peak correlation** and its sign — i.e. "gene led tech by ~N ticks" vs "tech dragged gene up over ~N ticks." A small correlation-vs-lag sparkline per pair.
- **Evidence:** a captured run whose lead–lag readout matches the hand-computed value from the JSONL.
- **Determinism:** view-only + post-hoc math, **no hash change**.

### TG5 — Headless capture + shareable HTML artifact *(lens, offline)*

**Goal:** rigorous offline analysis of a finished run, sharable like the gallery.

- **Headless:** extend the run capture (JSONL) to emit per-tick the TG1 coupling series (adoption + affinity-gene means/differentials) so an analysis pass has everything it needs without re-simulating.
- **Artifact:** a self-contained HTML page (inline CSS/JS, no external hosts — CSP-safe) rendering the DAG (TG3 layout) plus the lead–lag coupling analysis (TG4 math) over one captured run. Published via the Artifact tool; theme-aware; horizontally-scrolling wide charts.
- **Evidence:** one published artifact over a showcase seed; the DAG and lead–lag figures reproduce the Godot panels' readouts for the same seed.
- **Determinism:** offline, **no sim impact**.

## 4. Open questions for review

1. **Affinity-gene assignments (TG1):** the concrete gene↔invention pairings in the TG1 spec are a first proposal (Writing↔CommunicationStrength, Medicine↔CognitivePotential, Electricity↔Openness, Farming↔Conscientiousness). Do these match the intended narrative, or should the coupling target different slots?
2. **Selection strength:** how strong should the tech→gene coefficient be — subtle (a few % over hundreds of ticks, realistic drift) or dramatic (visible sweep within a run, good for demos)? This sets `coeff` magnitudes and whether the effect is a fitness multiplier vs a discovery-rate multiplier.
3. **TG2 branch count:** is a 2-branch, +4–6-node expansion the right size, or do you want a wider tree? This drives the `MEME_CHANNELS` bump and golden-regen cost.
4. **Reuse existing `CommunicationStrength`:** slot 23 is declared-but-inert and is the natural "make tech spread select for it" hook. Confirm we may finally wire it (it currently does not count toward speciation distance — TG1 must decide whether an adaptive, tech-selected gene should).
```
