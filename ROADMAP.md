# anabios Roadmap — Q3 2026 (Aug–Oct)

> Greek *ἀναβίωσις* — life arising. anabios is a **discovery engine**: deterministic
> ecosystems where speciation, culture, invention, and domestication *emerge* from
> local rules, and the codex catalogues the first time each phenomenon appears.

This is a mid-term (one-quarter) plan across all four tracks. It assumes the
tier-1→3 viewer animation stack and the web replay parity work (PRs #93–#98)
have just landed on `main`. The quarter has three themed phases; each item
carries a **track tag**, a **rough size** (S ≈ days, M ≈ 1–2 weeks, L ≈ 3+ weeks),
its **dependencies**, and a **done-when** bar.

## Tracks

- **R — Research & Science.** Emergence discovery, the DIT gene-culture line, the
  Out-of-Africa initiative. The reason the engine exists.
- **E — Simulation Engine & Mechanics.** The Rust core (`anabios-core`): agents,
  genome, inventions, codex detectors, new subsystems.
- **V — Viewer & Showcase.** The Godot client (`game/`) and the web replay player
  (`showcase/`) — how the emergence is *seen* and *shared*.
- **T — Tooling, Determinism & Infra.** Headless CLI, sweep pipeline, golden/
  determinism gates, CI, docs.

## North star for the quarter

**Make anabios legibly shippable and scientifically sharper.** Turn the freshly
polished presentation layer into a public-facing Out-of-Africa showcase, close the
one hard research gap that story depends on (the grand run can't climb to the
era-3 milestones on its own), and tighten the emergence/experiment loop so new
mechanics earn their place with evidence rather than vibes.

---

## Phase 1 — Ship the showcase (August)

*Capitalize on the just-merged presentation work while it's fresh. Goal: a
Out-of-Africa demo anyone can watch, in the browser and in the viewer.*

- **[V, M] Publish & host the web replay player.** The web player now renders
  settlements, death ghosts, combat streaks, trade lanes, and action poses
  (#96–#98). Wire `showcase/` up to a real hosted build with the
  `out-of-africa-saga` deck as the landing experience. *Depends:* none.
  *Done when:* a shareable URL plays the saga end-to-end with no console errors on
  desktop + mobile widths.
- **[V, S] Deck authoring pass.** Add 2–3 curated replay decks beyond the smoke
  decks (`game/showcase/*.json`) — e.g. predator-prey collapse, a dialect/meme
  sweep, the invention race. *Depends:* web player host. *Done when:* each deck is
  reproducible from a pinned scenario+seed and listed in the player's menu.
- **[V, S] Godot showcase-director cinematic pass.** The `ANABIOS_SHOWCASE`
  timeline drives camera/overlays/title-cards; fold the tier-2/3 effects (embers,
  firelight, climate grade, footstep trails) into the beat list so the recorded
  demo uses them deliberately. *Depends:* none. *Done when:* a headless capture run
  produces a clean cinematic without manual camera nudging.
- **[T, S] Screenshot/replay capture hardening.** Make `debug_capture.gd` +
  `ANABIOS_SHOT`/`ANABIOS_SHOWCASE` a one-command reproducible artifact pipeline
  (viewer stills + web-deck JSON from the same headless run). *Done when:* `make
  showcase` (or a documented script) regenerates every showcase asset from source.

**Phase-1 exit:** the Out-of-Africa story is watchable by a stranger in a browser,
and every asset regenerates from a pinned scenario.

---

## Phase 2 — Close the research gap & deepen emergence (September)

*The showcase leans on a story the engine can't yet tell unaided. Fix that, and
sharpen the discovery loop that finds the next story.*

- **[R, L] The Out-of-Africa climb problem.** Today the 3000-agent grand run only
  reaches era-1 (Stone Tools, Fire) even at 40k ticks; the saga only hits Writing
  + Husbandry because they're *seeded* at t0 (`starting_inventions`). Investigate
  whether an emergent climb to era-3 is reachable by tuning (IQ gates, discovery
  cadence, population/energy budget, culture diffusion) or whether seeding is the
  honest framing. *Depends:* none. *Done when:* either a documented emergent run
  reaches era-3, or `docs/showcase-plan.md` records the seeding decision with the
  measured evidence behind it.
- **[T, M] Emergence-scorecard-driven sweeps.** Operationalize the E1 scorecard
  (`emergence_score` / `coverage` / `novel_events`): stand up a reference corpus,
  run archive-weighted sweeps (`--archive`), and route corpus-unseen runs from
  `<out>/novel/` into a triage list. *Depends:* none. *Done when:* a weekly sweep
  surfaces a ranked shortlist of novel runs worth inspecting in the viewer.
- **[R, M] Next gene-culture experiment (confound-controlled).** Prior results:
  cumulative-skill culture *does* sweep the Inventiveness gene; a first-principles
  culture gene does *not* sweep from standing variation, and winner-take-all caps
  mask differentials. Design the next test to **disentangle module-vs-gene**
  before crediting any sweep. *Depends:* scorecard sweeps (for scenario picks).
  *Done when:* a scenario + seeded/first-principles A/B lands with a written
  finding (positive *or* negative) and a golden-tested regression.
- **[E, M] Trade-economy redesign.** The biome trade-goods economy freezes over
  long runs (`biome-trade` stops trading permanently by ~t10k). **Corrected
  diagnosis (2026-08-02, measured — see
  [`docs/superpowers/specs/2026-08-02-trade-freeze-diagnosis.md`](docs/superpowers/specs/2026-08-02-trade-freeze-diagnosis.md)):**
  the freeze is a **supply-side starvation** — agent inventories bleed to empty
  (death-churn loses goods; ungated reproduction floods empty newborns; the tiny
  `HARVEST_RANGE` can't refill a dispersed population), so no one can spare a
  `TRADE_UNIT` to give. It is **not** a `pick_swap` demand-satiation "absorbing
  state" (goods remain available in the biome and every agent's `want` is maxed at
  the freeze). So demand-side fixes (perishability, non-satiating `want`,
  demand-driven pricing) do **not** work — perishability was measured to freeze
  `biome-trade` at the same tick and *reduce* `geographic-trade` throughout.
  Redesign the **supply** side instead (candidates: preserve inventory on death,
  sustain harvest access at scale, temper churn dilution). *Depends:* none.
  *Done when:* `biome-trade`/`geographic-trade` sustain nonzero trade past the
  current freeze tick, gated behind an opt-in flag, with a determinism rehash.

**Phase-2 exit:** the flagship story is either emergent or honestly documented, and
new mechanics/experiments are chosen by scorecard evidence.

---

## Phase 3 — New mechanics & consolidation (October)

*Spend the earned evidence on one new subsystem, then harden and document.*

- **[E, L] One new emergence subsystem (pick from evidence).** Candidates, to be
  chosen from Phase-2 scorecard gaps: **knowledge accumulation** (Writing → durable
  per-culture tech memory that survives bottlenecks), **disease/epidemiology**
  (Medicine as a real counter-pressure), or **climate-refugia migration** (drifting
  climate driving directed range shifts). Ship as an opt-in scenario flag with its
  own codex detector(s). *Depends:* Phase-2 findings. *Done when:* the flag is
  off-by-default, has an integration test + golden hashes, and fires a new codex
  event type observed in a sweep.
- **[T, M] Determinism & save/load hardening.** Extend the golden gate: add a
  save→load→step round-trip test for every opt-in subsystem (the `#[serde(skip)]`
  accumulator footgun silently breaks replay). Audit detector state for skipped
  fields that feed hashed state. *Depends:* none. *Done when:* every subsystem has a
  round-trip test and the determinism suite covers all opt-in flags.
- **[E, S] Perf headroom for large sweeps.** Build on the opt-in codex-observer
  cadence (#92): profile the tick hot paths at 10k+ agents and shave the next
  bottleneck, benchmarked before/after (don't claim a win without the criterion
  delta). *Depends:* none. *Done when:* a documented ≥10% tick improvement at 10k
  agents, or a written "no cheap win found" with the profile.
- **[T, S] Docs & onboarding.** Fold the shipped subsystems (inventions,
  dimorphism, domestication, DIT, biomes) into the README status list and a short
  "reproduce a finding" guide; link scenarios → the phenomena they demonstrate.
  *Done when:* a newcomer can go from clone to a reproduced emergence run using only
  the docs.

**Phase-3 exit:** one evidence-backed new subsystem, a determinism gate that covers
every opt-in path, and docs that let someone else reproduce the science.

---

## Cross-cutting principles (hold all quarter)

- **Determinism is the contract.** Bit-identical per seed. Any intentional behavior
  change regenerates golden hashes (`UPDATE_HASHES=1 …`) in the same PR; a
  BiomeCell/World schema change rehashes all golden tests.
- **Opt-in by scenario flag.** New mechanics ship off-by-default (`*_enabled`) so
  existing scenarios and goldens stay stable.
- **Evidence before credit.** Sweeps, A/Bs, and benchmarks decide — especially for
  "this gene swept" and "this is faster" claims.
- **Green gates before merge.** `cargo fmt --check`, clippy, rustdoc `-D warnings`,
  and the Godot `gdformat --check` + `gdlint` blocking gate. Run the fast checks
  locally; let PR CI run the heavy determinism/emergence suite.

## Sequencing & dependencies (at a glance)

```
Phase 1 (Aug)  Host web player ─► Decks ─► Showcase-director ─► Capture pipeline
Phase 2 (Sep)  OoA climb problem ─┐
               Scorecard sweeps ──┼─► Gene-culture experiment
               Trade redesign ────┘
Phase 3 (Oct)  New subsystem (needs Phase-2 evidence)
               Determinism hardening ‖ Perf ‖ Docs   (parallel)
```

## Explicitly out of scope this quarter

- Neuroevolution / learned cognition (anabios is deliberately hand-engineered
  cognition + evolvable postfix programs).
- Real-time multiplayer or a playable game loop.
- Rewriting the viewer in another engine.

## Open questions to resolve as we go

1. Is emergent era-3 climb *achievable* at grand scale, or is seeded framing the
   real answer? (Phase-2 gates the showcase's honesty.)
2. Which new subsystem best fills the emergence coverage gap — knowledge, disease,
   or migration? (Let the scorecard corpus decide.)
3. Does the web player stay a curated replay viewer, or grow toward live in-browser
   simulation (WASM core)? (A Q4 fork, flagged now.)

---

_Living document — revise as phases close. Sizing is deliberately coarse; treat it
as sequencing intent, not commitment._
