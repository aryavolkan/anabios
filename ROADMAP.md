# anabios Roadmap — The Open-Ended Arc (Sep 2026 → mid 2027)

> Greek *ἀναβίωσις* — life arising. anabios is a **discovery engine**: deterministic
> ecosystems where speciation, culture, invention, and domestication *emerge* from
> local rules, and the codex catalogues the first time each phenomenon appears.

This is the long-horizon plan — three horizons across roughly three quarters, at
deliberately decreasing fidelity. Horizon 1 is item-level like the old quarterly
docs; Horizons 2–3 are sequencing intent that sharpens as earlier exits land. Items
carry a **track tag**, a **rough size** (S ≈ days, M ≈ 1–2 weeks, L ≈ 3+ weeks),
**dependencies**, and a **done-when** bar; research items also carry an **"if
wrong"** branch, because a falsified premise is a result, not a failure.

## Where we stand (Q3 2026 outcome)

The Q3 quarterly roadmap closed **completely, weeks early** — every phase exit met
by 2026-08-10. The full status record (per-item evidence, verdicts, data links)
lives in
[`docs/superpowers/specs/2026-08-07-detailed-roadmap.md`](docs/superpowers/specs/2026-08-07-detailed-roadmap.md);
highlights:

- **Showcase shipped.** Web replay player hosted (GitHub Pages), four curated
  decks, cinematic showcase-director, one-command asset regeneration
  (`scripts/emergence.sh showcase`).
- **Research gaps closed — honestly.** The Out-of-Africa climb is blocked
  *ecologically*, not cognitively (`2026-08-02-ooa-climb-findings.md`; seeding
  recorded as the honest framing). Gene-culture coupling: **negative**. O2b
  payoff-biased learning: shipped and measured **negative**. Trade freeze:
  diagnosed supply-side, fixed opt-in.
- **Hardened.** Save→load round-trip tests for all 21+ opt-in flags (caught a real
  serde-skip bug), emergence scorecard + corpus sweeps operational, perf verdict
  "no cheap win" documented, `docs/reproduce.md` verified cold-start.

Plus work *beyond* the written Q3 plan: real-Earth worldgen
(`out-of-africa-earth`), trade hubs + caravans, viewer world-scale polish, and the
anthropogenic arms-race subsystem. Current inventory: **60 codex event types**,
**~26 opt-in scenario flags**, 43 curated scenarios, the e1.3 emergence corpus.

**Bookkeeping convention for this document:** completed items are struck through
with their evidence link, never deleted; negative results are recorded in place as
*Resolved (negative)*. The live thread at writing time: the OoA-Earth stage-2
probes don't crack the exclusion wall (`2026-08-11-ooa-earth-emergence-probe-findings.md`).

## Tracks

- **R — Research & Science.** The O-track questions: why culture loses, and what
  makes it win. The reason the engine exists.
- **E — Simulation Engine & Mechanics.** The Rust core (`anabios-core`): new
  substrates, detectors, subsystems.
- **V — Viewer & Showcase.** The Godot client (`game/`) and web replay player
  (`showcase/`) — how emergence is *seen* and *shared*.
- **T — Tooling, Determinism & Infra.** Headless CLI, sweeps, soak harness, WASM,
  determinism gates, docs.

## North star for the arc

**Make sustained complexity pay for itself.** The E1–E10 arc proved we can *see*
emergence — 53→60 detectors across ecology, culture, economy, war, affect. The
O-track asks the harder question: can emergence **keep going** — climb, compound,
and cross levels of organization — on its own? Today it cannot: the grand run
stalls at era-1, novelty decays, no higher-level units form, one strategy
dominates. One root cause underlies all four: fast, simple, asocial strategies win
because the selective landscape never rewards cumulative investment. This arc
engineers that landscape — and proves it on the hardest concrete target first: an
**emergent, unseeded climb to era-3**.

The detail layer is the approved O1–O8 arc spec
([`docs/superpowers/specs/2026-08-02-open-ended-complexity-arc-design.md`](docs/superpowers/specs/2026-08-02-open-ended-complexity-arc-design.md));
O1 (exclusion autopsy) and O2 (lifetime learning) are closed. This roadmap
sequences what remains.

---

## Horizon 1 (Sep–Nov 2026) — Make culture pay

*The climb-attack chain O3 → O4, aimed at the flagship target. Everything here either moves
the invasion margin or gets honestly closed.*

- **[R, L] O3 — Cultural niche construction.** The flagship: make culture
  *construct* niches where it is rent-free, so cumulative investment beats fast
  asocial foraging. Builds directly on the O1 autopsy (asocial exclusion is the
  measured blocker) and the O2b negative (payoff-biased transmission alone is not
  the lever — the pivot prescribed by the detailed spec §6.3). *Depends:* O1
  findings (done), O2 closure (done). *Done when:* a flagged mechanism measurably
  moves the bidirectional invasion margin toward culture in the O1 apparatus, with
  a confound-guarded attribution — or the failure is written up with the fitness
  ledger showing why. *If wrong:* if no niche-construction mechanism moves the margin,
  pivot the same levers to O5's co-evolving environment (Horizon 2 pulls forward) and record
  the pivot.
- **[E, M] O4 — Transmission fidelity at demographic scale.** The collective-brain
  mechanism: fidelity/teaching scaling with effective population size so large
  populations hold more culture than small ones. *Depends:* O3 instrumentation
  (shares the fitness ledger). *Done when:* a flagged fidelity mechanism shows
  adoption-fidelity rising with population size in a sweep, behind its own flag,
  with goldens + round-trip coverage. *If wrong:* if demographic scaling is
  flat, document and fold the finding into the O5 redesign.
- **[R, M] OoA-Earth stage 2 — close-out.** The live thread: stage-2 placement on
  the real-Earth map "doesn't crack the exclusion wall." Re-attack with the O3
  levers, then close either way. *Depends:* O3 (or its interim findings).
  *Done when:* either an unseeded era-3 climb is observed on
  `out-of-africa-earth` (codex evidence + replay-verified), or
  `docs/showcase-plan.md` records the final honest framing with the measured
  margin. *If wrong:* the saga keeps `starting_inventions` and the showcase
  narrative says so — that outcome is already priced in.
- **[E, M] Disease/epidemiology subsystem.** The 3.1 runner-up from Q3 (knowledge ratchet won the
  pick; this is the leading remaining candidate — Medicine gains a real
  counter-pressure. Opt-in flag, off by default; new codex events appended per the
  `event.rs` append-only convention; climate-refugia is deferred to ride O5's drifting climate.
  *Depends:* scorecard-corpus gap analysis (which coverage hole it fills).
  *Done when:* flag off-by-default, integration test + goldens + round-trip, new event
  types observed firing in a corpus sweep. *If wrong:* if the scorecard shows
  disease doesn't close a real coverage gap, substitute the corpus's top gap.
- **[V, S] Scenario garden.** ~~A `scenarios/decks/` tier pinned to showcase assets, distinct from the test-pinned core set —
  as decks accumulate beyond the current four.~~ **Done 2026-09-01:** `scenarios/decks/README.md` documents the
  tier (pinned to a *recording*, not a phenomenon claim), the pin convention, and the current deck →
  scenario · seed · asset registry; `tests/deck_scenarios.rs` enforces the pin contract (curated deck
  `_comment`/`seed` → `scenario=<name>` pins resolve and run 200 ticks at the pinned seed); the saga deck now declares
  its pin in-JSON (was script-only). *Done when:* ~~the tier exists with its own smoke coverage, and
  `docs/scenarios.md` maps it.~~ ✔

**Horizon-1 exit:** the era-3 climb is either emergent on the Earth map or closed
with a named, measured blocker; O4's demographic claim is adjudicated; one new
evidence-backed subsystem fires in the corpus.

---

## Horizon 2 (Dec 2026–Feb 2027) — Keep pressure on, fill niches

*Same machinery, re-aimed at the next two north stars. Scope sharpens at the
Horizon-1 exit.*

- **[E, L] O5 — Co-evolving environment (POET-lite).** Slow drifting climate plus
  environmental challenge generation so selection pressures never stationarize —
  the attack on **non-decaying novelty**. Absorbs E10's drifting-climate item and
  carries the deferred climate-refugia subsystem. *Depends:* Horizon-1 pivot
  decision. *Done when:* a flagged drift/coevolution mechanism shows
  novelty-per-100k-ticks decaying *slower* than baseline in soak runs, with
  novelty curves committed to the plan.
- **[E, L] O7 — Adaptive radiation & niche depth.** Many niches persisting
  instead of one dominant strategy. *Depends:* O5 pressure (soft).
  *Done when:* a sweep shows sustained multi-strategy coexistence (measured on
  the O1 fitness ledger's strategy decomposition) materially above baseline.
- **[R, M] O6 — Major transitions spike (timeboxed).** The highest-risk item:
  can higher-level individuality form within hand-engineered cognition at all?
  Timeboxed probe, not a build commitment. *Depends:* none. *Done when:* a
  written verdict — a minimal mechanism that forms a higher-level unit, or a
  principled "not within this substrate" with the evidence. *If wrong:* if the
  spike shows neural cognition is genuinely required, that is the arc's one
  sanctioned exception to the won't-do list — decision recorded, not taken
  silently.
- **[T, M] WASM core spike → web-player fork decision.** The Q3 open question,
  now with data (the perf verdict: `culture_step` ≈73% of step at 12k agents).
  Compile `anabios-core` to WASM, measure tick rate in-browser, then decide:
  curated replay viewer vs live in-browser simulation. *Depends:* none.
  *Done when:* the decision is recorded with measured numbers either way.

**Horizon-2 exit:** novelty decay measurably slowed (or the pressure mechanism
honestly closed), the O6 feasibility verdict written, the web player's future
decided.

---

## Horizon 3 (Mar–May 2027) — Capstone

*Instrument everything the arc built. Lowest fidelity — defined in detail at the
Horizon-2 exit.*

- **[T/E, L] O8 — Open-ended soak & discovery meta-loop.** Million-tick soak runs
  with memory/perf telemetry (≥30 ticks/s at 10k agents end-to-end), novelty
  archive wired into the soak harness reporting novelty-per-100k-ticks decay
  curves across the full detector roster.
- **[E/V, M] Persistent cross-world codex** (absorbs E10's meta-game): SQLite
  codex DB, chapters with hidden entries, per-entry replay links, discovery
  progress — the "game the original design promised," scoped by the WASM
  decision.
- **[T, S] Consolidation & docs.** Determinism gate extended to every new flag,
  README event roster and status list refreshed, `docs/reproduce.md` re-verified
  verbatim.

**Horizon-3 exit:** a world that keeps generating catalogued novelty
indefinitely, and a codex that proves it.

---

## Cross-cutting principles (hold all arc)

- **Determinism is the contract.** Bit-identical per seed. Intentional behavior
  changes regenerate goldens (`UPDATE_HASHES=1 …`) in the same PR; schema changes
  bump `FORMAT_VERSION`. Per the O-track renegotiation, determinism is negotiable
  *per-mechanism* only where it demonstrably buys open-endedness — behind its own
  flag, never on a default path.
- **Opt-in by scenario flag.** New mechanics ship off-by-default (`*_enabled`) so
  existing scenarios and goldens stay stable. Codex events append to the end of
  the `event.rs` enum, never renumbered.
- **Evidence before credit.** Sweeps, A/Bs, and benchmarks decide. Every new
  detector ships the evidence trio: handcrafted positive test, handcrafted
  *negative* test, ≥1 long-run seed firing it. Perf claims quote criterion
  deltas; "no cheap win found" with a profile is a valid result.
- **Perf budget.** ≤10% tick-time regression at 10k agents per milestone;
  detectors stay inside the fused per-species aggregation pass.
- **Green gates before merge.** `cargo fmt --check`, clippy, rustdoc
  `-D warnings`, Godot `gdformat --check` + `gdlint`. Fast checks locally; PR CI
  runs the heavy determinism/emergence suite.

## Sequencing (at a glance)

```
H1 Sep–Nov   O3 niche construction ─► O4 fidelity ─┐
              OoA-Earth stage-2 close-out ◄────────┘
              disease subsystem ‖ scenario garden
H2 Dec–Feb   O5 co-evolving env ─► O7 radiation
              O6 transitions spike (timeboxed) ‖ WASM spike → decision
H3 Mar–May   O8 soak + meta-loop ─► persistent codex ─► consolidation
```

## Explicitly out of scope (guardrails)

- **Neuroevolution / deep learned cognition** — *with one sanctioned exception:*
  only if the O6 spike proves higher-level transitions require it, and only after
  that decision is recorded. Hand-engineered cognition + evolvable postfix
  programs is the design.
- Real-time multiplayer or a playable game loop.
- Rewriting the viewer in another engine.
- "Buildings" or "government"-level substrate — settlements stay field
  properties; institutions stay lineage ids (per the E-arc scope-creep
  guardrail).
- Resurrecting the gene-culture coupling experiment without a stable-population
  scenario (`2026-08-03-gene-culture-finding.md`).
- Demand-side trade fixes — the freeze is measured supply-side; that door is
  closed with data.

## Open questions (resolve as horizons close)

1. **The arc's central bet:** is an emergent, unseeded era-3 climb achievable at
   all within this substrate? (Horizon 1 answers it.)
2. Can major transitions (O6) form within hand-engineered cognition, or is that
   the wall? (Horizon 2's spike.)
3. Does the web player stay curated replay or grow a live WASM core? (Decided
   end of Horizon 2, with measured numbers.)
4. Which coverage gap does the disease subsystem actually close — let the
   scorecard corpus answer before building.

---

_Living document — horizons, not commitments. Reviewed at each horizon exit;
completed items are struck through with evidence, negative results recorded in
place. Per-milestone spec + plan pairs continue under `docs/superpowers/`; the
Q3 2026 quarterly plan's status record is
[`docs/superpowers/specs/2026-08-07-detailed-roadmap.md`](docs/superpowers/specs/2026-08-07-detailed-roadmap.md)._
