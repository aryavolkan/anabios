# Q3 2026 Roadmap — Implementation & Testing Plans Index

This indexes the per-item implementation/testing plans for [`ROADMAP.md`](../../../ROADMAP.md). Each roadmap item maps to a plan file below. Code-bearing items use TDD task structure (failing test → minimal impl → verify → commit); research items use an experiment protocol with pre-registered criteria and a golden-tested regression. Two lighter items (perf, docs) have their concise plans inline at the bottom.

**Execution:** each plan is self-contained and self-testable. To execute one, use `superpowers:subagent-driven-development` (fresh subagent per task, review between) or `superpowers:executing-plans` (inline batches with checkpoints).

## Roadmap item → plan

| Phase | Track | Roadmap item | Plan |
|-------|-------|--------------|------|
| 1 | V/T | Publish & host web player · deck authoring · showcase-director cinematic · one-command capture | [web-showcase-and-capture](2026-08-01-web-showcase-and-capture.md) |
| 2 | R | Out-of-Africa era-3 climb problem | [out-of-africa-climb-experiment](2026-08-01-out-of-africa-climb-experiment.md) |
| 2 | T | Emergence-scorecard-driven sweeps | [emergence-scorecard-sweeps](2026-08-01-emergence-scorecard-sweeps.md) |
| 2 | R | Next gene-culture experiment (confound-controlled) | [gene-culture-experiment](2026-08-01-gene-culture-experiment.md) |
| 2 | E | Trade-economy redesign | [trade-economy-redesign](2026-08-01-trade-economy-redesign.md) |
| 3 | E | One new emergence subsystem (knowledge accumulation) | [new-emergence-subsystem-knowledge](2026-08-01-new-emergence-subsystem-knowledge.md) |
| 3 | T | Determinism & save/load hardening | [determinism-saveload-hardening](2026-08-01-determinism-saveload-hardening.md) |
| 3 | E | Perf headroom for large sweeps | inline below (§A) |
| 3 | T | Docs & onboarding | inline below (§B) |

The four Phase-1 items are combined into one plan because they share the showcase pipeline and are best sequenced together; the plan's five tasks map to them (hosting = Task 2, decks = Task 4/5, cinematic = Task 5, capture pipeline = Task 1/3).

## Cross-plan coordination notes

- **CSV column ordering:** both `emergence-scorecard-sweeps` (adds `novel_types`) and `trade-economy-redesign` (adds `total_trades`) touch `write_summary_csv`. Pick one canonical final-column order (suggest: `…coverage, total_trades, novel_types`) and update both plans' CSV tests together to avoid a merge collision.
- **New codex event + scorer sync:** `new-emergence-subsystem-knowledge` adds `KnowledgeRatchet` (event 53) and a `knowledge_ratchet` scorer name; if the scorecard corpus is regenerated (scorecard plan Task 3), do it *after* the new event lands so the corpus reflects it.
- **Round-trip coverage:** `new-emergence-subsystem-knowledge` and `trade-economy-redesign` each add a scenario that `determinism-saveload-hardening` should also cover — add their scenarios to that plan's round-trip table.
- **Determinism discipline (all engine plans):** any change to a default-on path regenerates goldens (`UPDATE_HASHES=1 cargo test -p anabios-core --test determinism`) in the same PR; prefer gating new behavior behind an opt-in scenario flag so goldens don't move until intended. Snapshot field additions bump `FORMAT_VERSION` (`snapshot.rs:102`) with a changelog line.

---

## §A — Perf headroom for large sweeps (concise plan)

**Goal:** Shave the next tick hot-path bottleneck at 10k+ agents, with a benchmarked before/after — or a documented "no cheap win."

**Background:** Tick is ~0.75 ms @1k, ~2.5 ms @10k agents (README); `sense`/`decide` run parallel over rayon; codex detectors share one fused per-species aggregation pass. Recent perf work: opt-in codex-observer cadence (PR #92, `World.codex_interval`). Criterion suite: `crates/anabios-core/benches/tick_bench.rs` (tick / stages / scavenge groups).

**Tasks:**
- [ ] **Profile first, don't guess.** Run `cargo bench -p anabios-core` to capture the current baseline, then profile a 10k-agent tick (e.g. `cargo flamegraph` or `perf` on a headless run of `sandbox-xlarge.toml`) to find the dominant stage. Record the baseline numbers.
- [ ] **Form one hypothesis** from the profile (candidates from the codebase: spatial-hash rebuild cost, the fused codex aggregation pass, allocation in per-tick scratch, or the `decide` register loop). Pick the single biggest contributor.
- [ ] **Add a benchmark that isolates it** (a new criterion group in `tick_bench.rs` if the existing groups don't already cover the stage), so the win is measurable at the stage level, not just whole-tick.
- [ ] **Implement the optimization** as a **byte-identical refactor** — it must not change `state_hash` (run `cargo test -p anabios-core --test determinism` after). Reuse scratch buffers rather than allocating; hoist invariants; avoid changing iteration order.
- [ ] **Measure.** Re-run the criterion group. Accept only a **≥10% whole-tick improvement at 10k agents** with goldens unchanged. If none is found, write a short `docs/perf-notes.md` recording the profile and the "no cheap win" conclusion so the next attempt starts informed.
- [ ] **Commit** the change (or the notes) with the criterion delta quoted in the message.

**Testing:** `cargo test -p anabios-core --test determinism` (goldens must not move — perf work is byte-identity-preserving); `cargo bench -p anabios-core` before/after; `parallel_matches_serial_across_thread_counts` (`determinism.rs:175`) still green if the change touches parallel stages.

**Done when:** a ≥10% tick speedup at 10k agents lands with unchanged goldens and a quoted criterion delta, OR `docs/perf-notes.md` documents the profile and why no cheap win exists.

---

## §B — Docs & onboarding (concise plan)

**Goal:** A newcomer can go from clone to a reproduced emergence run using only the docs; the shipped subsystems are all reflected.

**Background:** `README.md` has a "Shipped to date" status list (inventions, dimorphism, domestication, DIT, biomes) and run/sweep/demo recipes. `docs/` holds design specs and `showcase-plan.md`; no single "reproduce a finding" guide links scenarios → the phenomena they demonstrate.

**Tasks:**
- [ ] **Audit the status list.** Diff `README.md`'s "Shipped to date" against the actual opt-in flags (`world.rs` flag block) and codex events (`event.rs` enum). Add any missing subsystem (e.g. gene-requirements/helix from PR #90, and the new knowledge subsystem once it lands).
- [ ] **Write `docs/scenarios.md`** — a table mapping each `scenarios/*.toml` to the phenomenon/event it demonstrates and the flags it enables (derive from each scenario's header comment + its flags). This is the "which scenario shows X" index.
- [ ] **Write a "Reproduce a finding" guide** (a README section or `docs/reproduce.md`): clone → build → `scripts/emergence.sh demo inventions` (watch the invention race) → `scripts/emergence.sh sweep <scn>` (mine the CSV) → open the viewer (`emergence.sh view <scn>`). Include the emergence-corpus/triage loop (cross-link the scorecard-sweeps plan's runbook).
- [ ] **Link the plans.** Add a short "Roadmap & plans" pointer in `README.md` to `ROADMAP.md` and this plans index.
- [ ] **Verify by walkthrough.** Follow the guide on a clean checkout (or have a fresh subagent do it) and fix any step that doesn't work verbatim.
- [ ] **Commit.**

**Testing:** the acceptance test is the walkthrough — every command in the guide runs verbatim on a clean clone and produces the described output. No automated test; the "done" bar is a successful cold-start reproduction.

**Done when:** `README.md`'s status list matches the code, `docs/scenarios.md` maps scenarios→phenomena, and a documented cold-start path reproduces an emergence run end-to-end.

---

## Self-review (index-level)

- **Spec coverage:** every one of the roadmap's 12 items maps to a plan or an inline §. ✓
- **Grounding:** each code plan cites exact files/lines/signatures from the current `main` (verified via code exploration on 2026-08-01), not invented APIs.
- **Determinism thread:** every engine plan carries the golden/snapshot discipline explicitly.
- **Known coordination hazards** (shared `write_summary_csv`, codex event sync, round-trip table) are called out above so parallel execution doesn't collide.
