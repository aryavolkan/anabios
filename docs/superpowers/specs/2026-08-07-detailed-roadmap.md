# anabios — Detailed Roadmap (2026-08-07)

Companion to the quarterly [`ROADMAP.md`](../../../ROADMAP.md) (sequencing intent,
coarse sizing). This document is the **detailed** layer: capability inventory,
evidence base, milestone-level breakdown per phase, workstream gates, and the
backlog. Revise at each phase exit; delete sections as they ship into the
README status list.

---

## 1. Where we are

### 1.1 Shipped capability inventory

| Layer | What exists | Key docs |
|---|---|---|
| Core sim | Deterministic SoA simulation; uniform-grid spatial hash; evolvable postfix behavior programs; 50-slot float genome; modular morphology; speciation | `docs/superpowers/specs/2026-05-23-anabios-design.md` |
| Interaction substrate (M11–M15) | Combat & predation, carcass scavenging, pheromone fields, meme culture, kin cooperation | archive plans m11–m15 |
| Invention tree | 10-tech cumulative culture (Stone Tools → Nuclear Power); Openness+skill-gated discovery, social diffusion, per-holder buffs/debuffs; opt-in `inventions_enabled` | `2026-07-18-cultural-inventions-design.md` |
| Codex | 21+ emergence detectors on a persistent event timeline | archive plan m5/m8 |
| Affect layer (mA–mF) | Primitive-brain affect (SEEK/FEAR/RAGE/LUST/CARE/PANIC/PLAY), hijack, showcase observability; opt-in `affect_enabled` | `2026-08-02-primitive-brain-affect-layer-design.md` |
| Vertebrate classes | Mammal/reptile founder archetypes (endotherm vs ectotherm profiles) | README status |
| Experiments | DIT gene-culture model; biome climate adaptation; runtime world dimensions; living biomes | `2026-07-12-dit-boundary-suite-design.md` |
| Sexual dimorphism (E12) | Binary sex, female mate choice, sexual-selection codex events; opt-in | `2026-07-27-sexual-dimorphism-design.md` |
| Domestication (E13) | Husbandry → taming, milk yields, born-tamed herds; rides inventions | `2026-07-27-domestication-design.md` |
| Viewer | Godot 4.6+: overlays, inspector, codex panel, coevo charts, tech panel, showcase director (cinematic beats), event camera/replay | archive plan m6/m7 |
| Tooling | Headless sweep CLI (parallel seeds → JSONL+CSV), emergence scorecard (`--archive`), replay gate, record pipeline, criterion benches | README |
| Showcase | Web replay player + `out-of-africa-saga` deck; Movie Maker capture | `2026-07-30-web-showcase-design.md` |

### 1.2 Repo state (post 2026-08-07 cleanup)

- `scenarios/` — 41 curated TOMLs (all test-pinned, menu, or gallery/showcase-backed); `scenarios/experiments/` — 20 archived ablations (O1, DIT, biome variants). Smoke test walks the tree recursively.
- `docs/superpowers/plans/` — 9 active plans + index; `plans/archive/` — 51 completed milestone/batch plans.
- `docs/superpowers/specs/` — 43 design + findings docs (kept flat; findings are dated).
- `runs/` — local only; `corpus-e1` is the scorecard reference corpus.

### 1.3 Evidence base — what the science says so far

| Finding | Result | Implication |
|---|---|---|
| O1 exclusion autopsy (`2026-08-03-o1-exclusion-findings.md`) | Cultural strategy is **net-costly**; asocial foragers competitively exclude culture-bearers | Any "culture wins" story needs a mechanism that pays for cognition |
| Gene-culture coupling pilot (`2026-08-03-gene-culture-finding.md`) | **Negative** — no coupled-gene sweep; Openness selected *down* | Corroborates the cognition-cost mechanism; next attempt needs stable-population scenarios first |
| OoA climb experiment (`2026-08-02-ooa-climb-findings.md`) | Era-3 climb blocked **ecologically** (competitive exclusion), not by IQ ceiling; 0/16 seeds reach era-3 even without the asocial competitor | Seeded `starting_inventions` is the honest saga framing — for now |
| Trade freeze diagnosis (`2026-08-02-trade-freeze-diagnosis.md`) | Freeze is **supply-side starvation**, not demand satiation; perishability measured to hurt | Redesign must preserve goods on death / sustain harvest access |
| O2 instrument gaps (`plans/2026-08-03-o2-step0-instrument-fixes.md`) | Absolute-count invasion fitness confounded by non-stationary populations; per-birth module mutation breaks founder tags | Step-0 headless-only instrument fixes precede any O2 claim |

**Meta-lesson:** every "X should emerge" claim so far has been decided by
measurement, and the naive expectation lost 4 out of 4 times. The roadmap
below budgets *instrument-first* work accordingly.

---

## 2. North star & strategy

**North star (quarter):** a stranger can watch the Out-of-Africa saga in a
browser, and every claim on screen is backed by a reproducible
scenario+seed.

**Strategy:** showcase what is *honestly* emergent, fix the discovery loop
that finds the next story (scorecard sweeps), and buy one new emergence
subsystem with the evidence earned. Cognition-cost is the central scientific
obstacle; the O-track experiments (O1 done, O2 instrumented) exist to find a
mechanism that makes culture pay.

---

## 3. Phase 1 — Ship the showcase (August, in progress)

Plan: `plans/2026-08-01-web-showcase-and-capture.md`. Track V/S/T.

| # | Item | Detail | Done when |
|---|---|---|---|
| 1.1 | Host the web replay player | Deploy `showcase/` (settlements, ghosts, trade lanes, poses already render, #96–#98); saga deck as landing experience | Shareable URL plays saga end-to-end, no console errors, desktop+mobile |
| 1.2 | Deck authoring pass | 2–3 curated decks beyond smoke: predator-prey collapse, dialect/meme sweep, invention race; each pinned to scenario+seed and listed in the player menu | Each deck regenerates from a one-line command |
| 1.3 | Showcase-director cinematic pass | Fold tier-2/3 effects (embers, firelight, climate grade, footstep trails) into `game/showcase/*.json` beat lists | Headless capture produces a clean cinematic with no manual camera work |
| 1.4 | Capture pipeline hardening | One command (`scripts/emergence.sh` or `make showcase`) regenerates every asset: viewer stills (gallery), web-deck JSON, MP4 | Documented; assets reproducible from pinned scenarios |

**Phase-1 exit:** the Out-of-Africa story is watchable by a stranger in a
browser; every asset regenerates from source.

**Dependencies/risks:** hosting choice (static host suffices — player is
replay-only); deck pinning must survive the Phase-3 determinism work (record
`FORMAT_VERSION` alongside decks).

---

## 4. Phase 2 — Close the research gap & deepen emergence (September)

### 4.1 O-track: making culture pay (research spine)

O1 established the exclusion mechanism; O2 tests **payoff-biased learning** as
the counter-mechanism.

| # | Item | Detail | Done when |
|---|---|---|---|
| 2.1 | O2 step 0: instrument fixes — **done 2026-08-07** | Headless-only: share-relative `invasion_fitness` + lineage-locked `FounderTracker`. No core change, no golden impact. Plan archived; validation note at `docs/superpowers/data/o2/step0-validation.md` (module tag was inflating cultural counts ~5×; O1 "invades" headline not reproduced under the founder tag) | Plan tasks 1–4 merged with unit tests; autopsy output a strict superset of today |
| 2.2 | O2a: measurement — **done 2026-08-07** | O1 invasion/lever matrix re-run under `--tag founder` + share fitness. Corrected decomposition: [`2026-08-07-o2a-corrected-decomposition.md`](2026-08-07-o2a-corrected-decomposition.md). Headline: O1's 90% invade-fraction and both "asocial invades" seeds were module-tag artifacts; lever *directions* survive at much weaker magnitude; cultural-*resident* fragility is the real asymmetry | Findings doc with CSVs in `docs/superpowers/data/o2/` |
| 2.3 | O2b: payoff-biased learning — **done 2026-08-07 (negative)** | `payoff_biased_learning` flag shipped (model + content bias, `FORMAT_VERSION` 30) and measured n=10 under the founder tag: **no rescue** (share-r -0.770 vs baseline -0.778, 0/10 invade) and mild skill-adoption suppression. Mechanism of the negative: practice harm is *reproductive* (stillbirth/cull), invisible to the energy proxy — the design's flagged fallback (reproductive-success proxy) is now the main line. Findings: [`2026-08-07-o2b-payoff-biased-findings.md`](2026-08-07-o2b-payoff-biased-findings.md). `SelectiveLearning` detector deferred to the working variant | Either a measured condition where cultural share *grows*, or a documented negative with mechanism |

### 4.2 Discovery loop

| # | Item | Detail | Done when |
|---|---|---|---|
| 2.4 | Scorecard-driven sweeps | Weekly archive-weighted sweep (`--archive runs/corpus-e1`); triage `<out>/novel/`; regenerate corpus **after** any new codex event lands | Weekly shortlist of novel runs worth viewer inspection |
| 2.5 | OoA climb — close out — **done 2026-08-07** | Emergent era-3 is dead at grand scale (findings). Decision recorded in `docs/showcase-plan.md` §2 ("Decision record 2026-08-07"): evidence table consolidated, saga keeps `starting_inventions`, O-track corroboration linked, reopen conditions stated (ecological-stage fix, not discovery/IQ math) | Decision recorded; saga keeps `starting_inventions` |

### 4.3 Economy

| # | Item | Detail | Done when |
|---|---|---|---|
| 2.6 | Trade-economy redesign | Supply-side fix: conserve goods on death (shipped as opt-in mechanism in #103) is *not* the freeze fix — the freeze is a structural barter equilibrium. Candidates: sustain harvest access at scale, temper churn dilution. Plan: `plans/2026-08-01-trade-economy-redesign.md` | `biome-trade`/`geographic-trade` sustain nonzero trade past the freeze tick behind an opt-in flag, determinism rehashed |

**Phase-2 exit:** flagship story is either emergent or honestly documented;
new mechanics chosen by scorecard evidence. Gene-culture experiment is
**resolved negative** (do not resurrect without a stable-population scenario).

---

## 5. Phase 3 — New mechanics & consolidation (October)

| # | Item | Detail | Done when |
|---|---|---|---|
| 3.1 | One new emergence subsystem | Pick by scorecard-corpus gaps: (a) **knowledge ratchet** — Writing → durable per-culture tech memory surviving bottlenecks (scenario `knowledge-ratchet.toml` exists; plan `2026-08-01-new-emergence-subsystem-knowledge.md` adds `KnowledgeRatchet` event 53); (b) disease/epidemiology — Medicine as counter-pressure; (c) climate-refugia migration | Opt-in flag, off by default; integration test + goldens; new codex event observed in a sweep |
| 3.2 | Determinism & save/load hardening | Round-trip test per opt-in subsystem; audit `#[serde(skip)]` accumulators feeding hashed state. Plan: `2026-08-01-determinism-saveload-hardening.md`. Must cover the 3.1 scenario and the trade-redesign scenario | Every subsystem round-trips; determinism suite covers all opt-in flags |
| 3.3 | Perf headroom | Profile tick hot paths at 10k+ agents on top of the opt-in observer cadence (#92); criterion delta required | ≥10% tick improvement at 10k agents, or a written "no cheap win" with profile |
| 3.4 | Docs & onboarding | Fold shipped subsystems into README status; "reproduce a finding" guide linking scenarios → phenomena (README scenario table started 2026-08-07) | Newcomer reproduces an emergence run from docs alone |

**Phase-3 exit:** one evidence-backed subsystem; determinism gate covers every
opt-in path; docs let someone else reproduce the science.

---

## 6. Q4 horizon (flagged, not committed)

1. **Web player fork** (open question 3 in ROADMAP.md): curated replay viewer
   vs WASM-compiled core running live in-browser. Decision inputs: Phase-1
   hosting experience, perf headroom from 3.3.
2. **Disease/epidemiology or climate-refugia** — whichever loses the 3.1 pick
   becomes the leading Q4 candidate.
3. **Cognition-cost arc continuation** — if O2b is positive, a scenario family
   exploring the payoff mechanism (possibly an `o3-*` suite under
   `scenarios/experiments/`); if negative, a written synthesis of the O-track
   and a pivot to ecological framing (niches where culture is rent-free).
4. **Scenario garden** — as decks accumulate (1.2), consider a
   `scenarios/decks/` tier pinned to showcase assets, distinct from the
   test-pinned core set.

---

## 7. Process gates (hold always)

- **Determinism is the contract.** Bit-identical per seed; intentional behavior
  changes regenerate goldens in the same PR (`UPDATE_HASHES=1 …`); schema
  changes bump `FORMAT_VERSION` with a changelog line.
- **Opt-in by scenario flag.** New mechanics ship `*_enabled = false` by
  default so scenarios and goldens stay stable.
- **Evidence before credit.** Sweeps, A/Bs, benchmarks decide — see §1.3 for
  what happens to unmeasured intuitions.
- **Green gates before merge.** `cargo fmt --check`, clippy, rustdoc
  `-D warnings`, `gdformat --check` + `gdlint`. CI runs the heavy
  determinism/emergence suite.
- **Repo hygiene.** New scenarios go to `scenarios/experiments/` unless they
  are test-pinned, menu-worthy, or gallery/showcase-backed; completed plans
  move to `plans/archive/` at phase exit; findings get dated spec docs +
  `docs/superpowers/data/` CSVs.

---

## 8. Sequencing (at a glance)

```
Aug   1.1 host ─► 1.2 decks ─► 1.3 cinematic ─► 1.4 pipeline
Sep   2.1 instruments ─► 2.2 O2a ─► 2.3 O2b
      2.4 scorecard sweeps (weekly)   2.5 OoA close-out   2.6 trade redesign
Oct   3.1 subsystem (needs 2.4 evidence)
      3.2 determinism ‖ 3.3 perf ‖ 3.4 docs
```

## 9. Explicitly out of scope

- Neuroevolution / learned cognition (hand-engineered cognition + evolvable
  postfix programs is the design).
- Real-time multiplayer or a playable game loop.
- Rewriting the viewer in another engine.
- Resurrecting the gene-culture coupling experiment without a
  stable-population scenario (see `2026-08-03-gene-culture-finding.md`).

---

_Living document — detailed layer under `ROADMAP.md`. Update at each phase
exit; when an item ships, delete it here and add it to the README status list._
