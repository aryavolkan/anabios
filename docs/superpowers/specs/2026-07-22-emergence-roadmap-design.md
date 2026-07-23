# Emergence Grand Roadmap — Design Spec

**Date:** 2026-07-22
**Status:** Approved
**Depends on:** everything shipped through the geographic-trade batch (M1–M15 tags plus coevolution, personality, biome-climate, inventions, living-sandbox, cognitive gene-culture, trade-goods, and geographic-trade batches).

## 1. Goal

anabios exists to surface emergence. This roadmap is the **full remaining arc** — ten milestones, **E1–E10**, that take the project from its current state (23 codex event types) to open-ended, civilization-scale emergence, with two commitments held throughout:

1. **Interleave lens and substrate.** Odd-numbered milestones mostly *measure and reveal* what's already emerging (detectors, scoring, replay, charts); even-numbered milestones mostly *add new substrates* that unlock never-before-seen phenomena (disturbance, war, settlements, traditions). Neither runs away from the other: new substrates immediately get instruments, and new instruments immediately get something worth pointing at.
2. **Every milestone proves its phenomenon.** No milestone ships on vibes. Each one delivers detectors with handcrafted tests, a showcase scenario, headless sweep evidence that the events actually fire, and gallery captures.

### 1.1 Gap analysis (what this arc closes)

From the original design (`2026-05-23-anabios-design.md`):

- **Promised but undelivered detectors (§4):** PopulationCycle, BoomAndBust, CarryingCapacityReached, TrophicCascade, CorridorUse, SegregationEmerged, RangeExpansion, ConvergentEvolution, TraitFixation, RapidAdaptation, EvolvedFlight, EvolvedAmbush, EvolvedTool, KinNetworkStable, WarOrRaid, TraditionPreserved.
- **Promised but undelivered player systems (§4.5, §6.3–6.5):** event-snapshot replay, event camera, run-until-next-event, persistent cross-world codex DB, codex chapters with discovery progress.
- **Promised but undelivered substrate (§6.2):** climate disasters (fires, droughts, freezes).
- **Missing lens:** no emergence scoring, novelty ranking, or sweep objective for codex coverage (§10 names codex coverage as the metric to optimize; nothing computes it yet).

## 2. Cross-cutting invariants

These hold for **every** E-milestone and are restated per-milestone only when at risk:

1. **Determinism gate stays green.** Behavior-altering work regenerates golden hashes deliberately (`UPDATE_HASHES=1 …`, values copied into `tests/determinism.rs`) and calls this out in the milestone plan.
2. **Perf budget.** ≤10% tick-time regression at 10k agents on the criterion suite (`cargo bench -p anabios-core`) per milestone. Detector work stays inside the fused per-species aggregation pattern.
3. **Evidence trio per phenomenon.** Every new detector ships with: (a) one handcrafted minimal-world unit/integration test that fires it exactly once, (b) ≥1 seed firing it in a long headless run recorded in the milestone plan, (c) ≥1 gallery capture with an honest caption.
4. **Independently shippable.** Each milestone lands behind scenario flags where it changes behavior, keeps baseline scenarios unchanged, and is tagged (`e1`…`e10`) per the `m1`–`m10` convention.
5. **Process.** Each E-milestone gets its own spec + plan pair under `docs/superpowers/{specs,plans}/` named `YYYY-MM-DD-eN-<slug>.md`. This roadmap is the index, not the plan.

## 3. Phase I — Make emergence legible

### E1 — Emergence scorecard & novelty archive *(lens)*

**Goal:** compute what the sweep CSV currently only hints at — how *much* and how *rarely* a world emerges.

- **Core/headless:** per-run **emergence score** = sum over fired event types of a rarity weight (empirical: inverse frequency across a reference seed corpus, shipped as a versioned table in `anabios-headless`). Extend `summary.csv` with `emergence_score`, `novel_events` (types never before seen in the corpus), and `coverage` (fraction of all event types fired).
- **Novelty archive:** `anabios-headless sweep` accepts `--archive runs/corpus/` so each run is scored against everything seen before; runs that fire corpus-novel events are flagged and their event streams copied to `runs/<name>/novel/`.
- **Sweep objective:** document `--metric emergence_score` as the W&B sweep optimization target (design §10).
- **Evidence:** 32-seed `divergent` + `inventions` sweeps; demonstrate score ordering matches human judgement on a hand-picked sample; corpus containing ≥1 novel event flagged.
- **Determinism/perf:** scoring is post-hoc over event streams — zero sim impact, no hash change.

### E2 — Replay & event camera *(lens)*

**Goal:** close design §4.5/§6.3/§6.4 — the player can *return to* emergence, not just read about it.

- **Core:** event-keyed compact snapshots already exist in `snapshot.rs`; add a ring buffer of recent snapshots (every N ticks, N scenario-tunable) so any codex event has a rewind point within N ticks.
- **Viewer:** **Replay this moment** button on codex entries (rewind to nearest snapshot, deterministic re-sim forward, highlight overlay on the event's `location_bbox`); **run-until-next-event** time mode; **event camera** mode that auto-cuts to recent events ~15 s each.
- **Headless:** `anabios-headless replay --events runs/foo/events.jsonl --event 42` re-simulates and re-asserts the event fires at the same tick — this doubles as the detector regression harness.
- **Evidence:** replay of a `CombatRaid` and an `InventionDiscovered` event captured frame-by-frame into the gallery; determinism asserted by replay reproducing the same state hash at event tick.
- **Determinism:** replay correctness *depends on* the golden-hash discipline; add a replay test to the gate.

## 4. Phase II — Ecological depth

### E3 — Population dynamics completion *(detectors)*

**Goal:** deliver the design §4.1 detector set on the existing plant/predator substrate — the cheapest emergence left unclaimed.

- **Detectors:** `PopulationCycleDetected` (spectral peak in a per-species population rolling window with period in a plausible band), `BoomAndBust` (cycle with amplitude ≥ threshold), `CarryingCapacityReached` (variance collapse at a sustained plateau), `TrophicCascade` (predator removal crash → grazer boom → plant crash, ordered-lag correlation across three trophic levels).
- **Scenario:** `scenarios/trophic-cascade.toml` — plants → grazers → stalkers, tuned so a 32-seed sweep fires the cascade in a meaningful minority of runs.
- **Viewer:** population chart annotations marking detected cycle periods.
- **Evidence:** handcrafted three-trophic test world firing the cascade exactly once; sweep stats reported in plan; gallery chart captures.
- **Perf:** detectors must join the fused per-species aggregation pass; no new per-agent loops.

### E4 — Disturbance & succession *(substrate)*

**Goal:** the world pushes back. Delivers design §6.2 disasters and unlocks spatial succession dynamics.

- **Substrate:** disaster scheduler (fire, drought, freeze) with Poisson frequency and severity fields; disasters mutate biome cells (plant biomass destruction, terrain conversion, temperature shock) and propagate over a few ticks; biome cells gain **succession state** (bare → pioneer → climax) governing regrowth rate and nutrient recovery.
- **Detectors:** `RangeExpansion` (species centroid displacement + occupied-cell count growth over a window), `SegregationEmerged` (two species' spatial overlap drops below threshold and stays), `CorridorUse` (recurrent migration along a narrow terrain band), `Succession` (post-disaster cell traverses bare→pioneer→climax while being re-colonized).
- **Scenario:** `scenarios/disturbance.toml` — fire-prone grassland with one generalist and one specialist starter; archipelago variant for corridor use.
- **Viewer:** disaster overlay (scorch/frost tint), succession tint on biome layer, disaster entries in the event ticker.
- **Evidence:** per-detector handcrafted worlds; 32-seed disturbance sweep; before/after gallery captures of a burn scar being re-colonized.
- **Determinism:** behavior-altering → golden-hash regeneration expected and documented.

## 5. Phase III — Evolutionary depth

### E5 — Trait-evolution instruments *(lens)*

**Goal:** make *evolution itself* visible, per design §4.3.

- **Detectors:** `TraitFixation` (genome slot variance within a species collapses below threshold after being polymorphic), `RapidAdaptation` (slot mean moves ≥ k standard deviations within a short window, correlated with an environmental or competitive shock), `ConvergentEvolution` (two lineages with no recent common ancestor independently fix the same slot signature or module motif).
- **Core:** per-species genome-moment history (mean/variance per slot, ring buffer) feeding both detectors and charts; phylogeny export (already tracked) exposed to the viewer.
- **Viewer:** trait-drift charts (slot means over time per species, shock markers from E4 disasters), **phylogeny tree view** panel.
- **Scenario:** `scenarios/convergent.toml` — two geographically isolated identical starters in matched niches.
- **Evidence:** handcrafted fixation/adaptation worlds; convergent sweep showing the detector fires between (not within) lineages; phylogeny gallery capture.

### E6 — Named behaviors completion *(detectors)*

**Goal:** finish the design §4.3 named-behavior codex chapter — the most *legible* emergence for players.

- **Detectors:** `EvolvedFlight` (sustained locomotion across hostile terrain-affinity barriers at high speed — signature: repeated barrier crossing no starter program performs), `EvolvedAmbush` (long stillness + prey-proximity-triggered weapon fire, statistically separated from random firing), `EvolvedTool` (weapon/jaws damage output materially boosted by held invention, used in successful hunts), `StructuredSignaling` (pheromone/meme broadcast patterns that reliably precede a coordinated group action).
- **Core:** per-species **behavior-signature** accumulator (action-conditioned-on-context counts) shared by all four detectors; signatures stamped on `NovelBehaviorPattern` events for richer codex text.
- **Viewer:** inspector shows the agent's current behavior signature vs species mean; codex entries name the signature.
- **Scenario:** extend `weapons-arena.toml` lineage with an ambush-bait layout; `scenarios/signaling.toml` built on the alarm-call substrate.
- **Evidence:** per-behavior handcrafted worlds (a scripted ambusher firing the detector, a scripted non-ambusher *not* firing it — negative tests matter here); gallery close-ups per the arms-race capture convention.

## 6. Phase IV — Social & civilizational emergence

### E7 — Kin networks & war *(substrate)*

**Goal:** group-level conflict and durable kin structure, per design §4.4.

- **Substrate:** **war state** — sustained inter-species combat exceeding a rolling front threshold creates an explicit per-species-pair hostility record readable by behavior programs (`SenseHostility` input); hostility decays without reinforcement and can terminate in `WarEnded`. Kin clusters gain persistence tracking (lineage co-location over generations).
- **Detectors:** `KinNetworkStable` (a kin cluster maintains co-location and cooperation above thresholds across N generations), `WarOrRaid` upgraded: sustained war state with territorial front movement (distinct from the existing one-off `CombatRaid`), `AllianceFormed` (two species with shared meme signature sustain mutual non-aggression + cooperation near each other).
- **Scenario:** `scenarios/war.toml` — two armed territorial species contesting one resource corridor, plus a third neutral meme-compatible species as alliance candidate.
- **Viewer:** war-front overlay (hostility heat + front line), kin-network view in the inspector lineage panel.
- **Evidence:** handcrafted war/peace worlds; 32-seed sweep reporting war duration distribution; gallery war-front captures.
- **Determinism:** behavior-altering (new sense input) → behind `war_enabled` scenario flag; hashes regenerated.

### E8 — Settlements & economy *(substrate)*

**Goal:** emergence of *place* — the step from roaming trade to anchored civilization, building directly on the trade-goods and geographic-trade batches.

- **Substrate:** **home-range anchoring** (genome slot + program input lets agents bias movement toward a learned anchor point; anchors are heritable with drift); **market nodes** emerge where trade-swap density crosses a threshold and are reinforced by continued use (no hardcoded marketplace object — the node is a field property); **specialization**: trade goods production costs drop with practice (`practice.rs` reuse), so agents drift into producer roles.
- **Detectors:** `SettlementFormed` (a cluster of anchors persists with density and longevity thresholds), `MarketEmerged` (a market node sustains cross-species swap volume over a window), `SpecializationSplit` (within-species bimodal distribution of production profiles correlated with trade role).
- **Scenario:** `scenarios/settlement.toml` — resource-heterogeneous map (extension of `geographic-trade.toml`) with anchoring enabled.
- **Viewer:** settlement/market overlay (anchor density + node glow), trade-route rendering already present gets market endpoints; per-settlement stats in inspector.
- **Evidence:** handcrafted settlement world; sweep showing market nodes concentrate swaps vs baseline; gallery economy captures per the geotrade convention.

### E9 — Traditions & institutions *(substrate)*

**Goal:** culture that *outlives* its carriers, per design §4.4 — the ratchet beyond individual memory.

- **Substrate:** **meme lineages** — memes carry lineage ids and mutation history; transmission already exists, add fidelity tracking so a meme's descent can be traced across generations. **Institutional memory**: invention/practice holders teach with higher fidelity inside settlements (E8), so culture anchors to place.
- **Detectors:** `TraditionPreserved` (a meme lineage persists above adoption threshold across ≥ N generations with fidelity above threshold), `CulturalRadiation` (one ancestral meme diversifies into ≥ k distinct descendant variants across species), `InstitutionalRatchet` (a culture's effective era never regresses over a long window despite holder turnover — measured on invention adoption).
- **Scenario:** `scenarios/traditions.toml` — long-run (≥ 20k ticks) gene-culture + settlement world.
- **Viewer:** meme-lineage tree view (same panel family as E5 phylogeny), tradition badges on species pages.
- **Evidence:** multi-generation handcrafted test with forced turnover; long-run sweep demonstrating ratchet vs acultural control; gallery meme-tree capture.

## 7. Phase V — Open-endedness capstone

### E10 — Open-ended world engine & codex completion *(lens + capstone)*

**Goal:** the game the original design promised: a persistent cross-world codex and worlds that keep generating novelty indefinitely.

- **Core/headless:** million-tick stability profile — soak runs (`sandbox-xlarge`, 1M ticks) with memory/perf telemetry and no pathological state growth; **drifting climate** (slow secular temperature/rainfall drift on top of seasons) so selection pressures never stationarize; novelty archive (E1) wired into soak harness to report novelty-per-100k-ticks decay curves.
- **Codex meta-game (design §6.5):** persistent cross-world codex DB (SQLite, app-support path, WAL per §11); chapters by family with `???` hidden entries; per-entry screenshot/replay (E2) and linked species page; discovery-completion progress surfaced in the menu.
- **Viewer:** codex screen promoted to a top-level peer of Worlds/Viewer per design §6.1; event camera + run-until-next-event (E2) become default discovery loop.
- **Evidence:** 1M-tick soak artifacts (event stream, novelty curve, telemetry) committed to the plan; gallery "codex chapter completed" capture; README updated with the full event-type roster.
- **Perf:** soak run must hold ≥30 ticks/s at 10k agents end-to-end; any growth leak is a blocker.

## 8. Milestone summary

| # | Kind | Name | New event types | Key dependencies |
|---|---|---|---|---|
| E1 | lens | Emergence scorecard & novelty archive | — (scoring) | sweep CLI |
| E2 | lens | Replay & event camera | — (viewer) | snapshot.rs |
| E3 | detectors | Population dynamics completion | +4 | — |
| E4 | substrate | Disturbance & succession | +4 | biome field |
| E5 | lens | Trait-evolution instruments | +3 | E4 shocks (optional) |
| E6 | detectors | Named behaviors completion | +4 | behavior signatures |
| E7 | substrate | Kin networks & war | +3 | kin, combat |
| E8 | substrate | Settlements & economy | +3 | trade goods, practice |
| E9 | substrate | Traditions & institutions | +3 | E8 settlements |
| E10 | capstone | Open-ended engine & codex completion | — (meta) | E1, E2, all above |

Event-type total at arc completion: **43** (23 current + 20 new), with `EVENT_TYPE_COUNT` and the viewer's parallel name/color arrays extended per the existing boot-assert convention.

## 9. Risks & mitigations

- **Detector false positives** (cycles, ambush, convergence are all statistical claims): every statistical detector ships with a negative handcrafted test (scripted near-miss that must *not* fire), not just a positive one. E3/E6 carry the most risk.
- **Threshold brittleness across scenarios:** detector thresholds get scenario-level sensitivity multipliers (the design's "codex sensitivity" tweaker, §6.2) rather than per-detector retuning.
- **Perf creep from history buffers** (E3/E5/E6 all add ring buffers): buffers are per-species, not per-agent, and join the fused aggregation pass; each milestone re-runs the criterion gate.
- **Scope creep in E8/E9:** settlement and institution substrates are deliberately minimal (field properties, lineage ids) — no new agent-level objects beyond what SoA buffers already pattern. Anything resembling "buildings" or "government" is explicitly out of scope for this arc.
- **Determinism erosion** as more stages join the tick: every substrate milestone (E4, E7, E8, E9) names its tick-pipeline insertion point and RNG draw order in its own spec before implementation.

## 10. Sequencing rationale

E1–E2 come first because every later milestone is *cheaper to prove* with scoring and replay in hand — the evidence-trio invariant (§2.3) leans on both. E3 precedes E4 so cycle detectors calibrate on a calm world before disasters complicate the signal. E5's shock-correlated adaptation detector is strongest after E4 provides real shocks but is specified to work without them. E7–E9 form a strict dependency chain (war needs kin, settlements need trade, traditions need settlements). E10 is unstartable before E1/E2 and is deliberately last: it instruments everything the arc built.
