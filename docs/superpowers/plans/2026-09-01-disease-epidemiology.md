# Plan: Disease & Epidemiology subsystem (2026-09-01)

**Goal:** implement `docs/superpowers/specs/2026-09-01-disease-epidemiology-design.md`
— flag `disease_enabled` (off by default), `EpidemicOutbreak`/`MedicineContainment`
codex events, `scenarios/disease.toml`, full determinism round-trip coverage.

**Architecture:** spec §Components A–D. **Tech stack:** anabios-core (Rust), Godot
codex panel, anabios-headless scorer.

TDD task list (each task: failing test → minimal impl → verify → move on). Repo
conventions that apply to every task: deterministic iteration (ascending id,
BTreeMap/BTreeSet); `world.rng` draws only under the flag; no behavior change
with the flag off except the one-time schema rehash.

- [ ] **T1 flag + column plumbing.** `agent.rs` (`infection` in
  `spawn`/`grow_one`/`kill`), `world.rs` (flag field + `World::new`), `scenario.rs`
  (TOML field + instantiate copy). Compile gate.
- [ ] **T2 tick stage.** `src/disease.rs` (`disease_step` + params), `lib.rs`,
  `tick.rs` stage 6g (after knowledge 6f, before age+starve 7).
- [ ] **T3 codex events + detector.** `event.rs` (60/61 + pinning test), `agg.rs`
  (`infection_sum` + Default/build/reset — the reset test must stay green),
  `codex/mod.rs` (`mod disease`, `CodexState.epidemic_latched`, gated call),
  `codex/disease.rs` (both detectors).
- [ ] **T4 persistence.** `snapshot.rs` FORMAT_VERSION 34→35 + changelog;
  `save_load_roundtrip.rs` disease row; `UPDATE_HASHES=1` golden rehash
  (determinism + flag-on goldens in affect/cognition/inventions etc.).
- [ ] **T5 scenario + integration tests.** `scenarios/disease.toml`;
  `tests/disease.rs` (outbreak fires in crowded world; flag-off no-op; medicine
  A/B; sparse-world negative).
- [ ] **T6 headless + viewer.** `score.rs` (names/corpus/name arms),
  `tests/sweep_csv.rs` column count 70→72; `codex_panel.gd` CHAPTER_NAMES/COLORS.
- [ ] **T7 docs + gates.** `docs/scenarios.md` row, README status line; `cargo fmt
  --check`, clippy `-D warnings`, rustdoc, full workspace test run; sweep
  observation of `epidemic_outbreak` recorded below.

**Done when:** flag off-by-default with byte-identical flag-off behavior (one-time
schema rehash), integration + round-trip + golden coverage green, and
`epidemic_outbreak` observed firing in a sweep of `scenarios/disease.toml`.

## Execution log

- **T1–T3** (2026-09-01): flag + `infection` column + `disease_step` stage 6g +
  detectors landed; codex unit tests green on first run.
- **T4** (2026-09-01): FORMAT_VERSION 34→35; golden rehash across determinism +
  affect/affect_play/affect_social/cognition/inventions (layout growth only —
  flag-off trajectories byte-identical, verified by the flag-off no-op test).
- **T5** (2026-09-01): `scenarios/disease.toml` (two-band design) +
  `tests/disease.rs`. Two measured corrections vs. the spec draft: (a) the
  detector needed an `infected_count` agg field — mean intensity
  (`infection_sum/count ≈ 0.06`) never crosses threshold during a real wave;
  (b) seeding Medicine on the whole population prevents outbreaks outright
  (0.25× susceptibility + 3× recovery → R0 < 1), so the scenario splits into
  a susceptible grazer herd + a medicine-bearing innovator band.
- **Emergence evidence** (2026-09-01): release run `epidemic_outbreak_emerges_across_seeds`
  — `EpidemicOutbreak` fired in **4/5 seeds** (first ticks 25/30/37/71) of
  `scenarios/disease.toml` at 2000 ticks. A manual single-agent seed in the
  grazer herd swept to 109 infected (~30% of the species) before resolving.
- **T6–T7** (2026-09-01): headless scorer (62 columns), viewer chapter
  names/colors, README + `docs/scenarios.md`; `cargo fmt --check`, clippy,
  rustdoc `-D warnings`, full workspace suite (61 suites), gdformat + gdlint
  all green.
