# E1 — Emergence Scorecard & Novelty Archive — Implementation Plan

**Goal:** Add rarity-weighted emergence scoring, coverage, and novelty detection to `anabios-headless sweep`, per the spec `docs/superpowers/specs/2026-07-22-e1-emergence-scorecard-design.md`.

**Approach:** New `score.rs` module in `anabios-headless` (weights, scoring, corpus loading — all pure post-processing). `sweep.rs` extends `RunSummary` + CSV. `main.rs` adds `--archive`. No changes to `anabios-core` → no determinism or perf risk.

**Determinism:** untouched — scoring runs on drained event counts after the sim. Golden suites are not regenerated in this milestone.

---

## Task S1: `score.rs` — weights, scoring, corpus loader
**Files:** `crates/anabios-headless/src/score.rs` (new), `main.rs` (`mod score;`).

- `pub const ALL_EVENT_NAMES: [&str; 23]` — snake_case names in the existing CSV order (single source for coverage denominator cross-check against `EVENT_TYPE_COUNT`).
- `pub const WEIGHTS_VERSION: &str = "e1.1"`, `pub const CORPUS_RUNS: u64 = 64`, `pub const NOVELTY_BONUS: f64 = ln(64)+1 ≈ 5.159`.
- `pub const DEFAULT_WEIGHTS: [(&str, f64); 23]` — filled in Task S4 from the real corpus; placeholder `NOVELTY_BONUS` for all until then (compiles and tests pass either way).
- `pub struct ScoreTable { pub weights: BTreeMap<String, f64>, pub known: BTreeSet<String> }` — `known` = types with `n_t > 0` in the backing corpus.
- `ScoreTable::default_table()` — from `DEFAULT_WEIGHTS`; `known` = entries with weight `< NOVELTY_BONUS`.
- `ScoreTable::from_corpus(runs: &[BTreeSet<String>])` — per-type run counts → IDF weights; empty corpus → all `NOVELTY_BONUS`, empty `known`.
- `score(counts: &BTreeMap<&str, u64>, table) -> f64` — sum weights over types with count > 0 (unknown names ignored defensively).
- `coverage(counts) -> f64` — distinct fired / `EVENT_TYPE_COUNT`.
- `novel_types(counts, table) -> Vec<&str>` — fired types not in `table.known`, sorted.
- `load_corpus(dir: &Path) -> Result<Vec<BTreeSet<String>>>` — walk `dir` recursively (manual `read_dir` recursion, no new deps), parse each `*.events.jsonl` line as `{"event_type": EventType, …}` (serde ignores other fields), map `EventType → name` via the existing `sweep::event_name` (move it to `score.rs` or make it `pub(crate)`); malformed lines → `eprintln!` warning + skip. Return one type-set per file.
- Tests: IDF edge cases (ubiquitous → 0, unseen → bonus, empty corpus); score distinct-only; coverage; novel difference; `load_corpus` over a `std::env::temp_dir()` fixture incl. nested subdir + one malformed line.

## Task S2: sweep wiring + CSV columns
**Files:** `sweep.rs`, `main.rs`.

- `Sweep` gains `--archive: Option<PathBuf>`; `sweep::run` signature gains `archive: Option<PathBuf>`.
- `run()`: build `ScoreTable` once before the parallel loop (`--archive` → `load_corpus` + `from_corpus`, else `default_table()`); share `&ScoreTable` into `run_one`.
- `RunSummary` gains `emergence_score: f64`, `novel_events: u64`, `coverage: f64`, plus `novel: Vec<String>` (non-serialized helper for stdout/copy step — or recompute; simplest: keep field, skip serde if RunSummary stays CSV-only).
- CSV: append `emergence_score,novel_events,coverage` to header and rows (`{:.3}` for score/coverage). Existing columns untouched.
- Post-loop: runs with `novel_events > 0` → `std::fs::create_dir_all(out/novel)` + copy their `seed_*.events.jsonl`; stdout prints top-5 by score and each novel run with its novel type names.
- Tests: keep existing `event_name` tests compiling (adjust imports if `event_name` moves).

## Task S3: reference corpus sweep (evidence, no code)
- `cargo build --release --bin anabios-headless`
- 16 seeds × 5000 ticks into `runs/corpus-e1/` for `divergent`, `inventions`, `predator-prey`, `cooperation` (64 runs total; ~minutes on 10 cores).

## Task S4: derive default table + fill consts
- Derive IDF weights from `runs/corpus-e1/` (via `--archive` dry-run output or a scratch script reading the JSONL); paste into `DEFAULT_WEIGHTS` with the corpus recipe in the doc comment.
- Verify: a scored sweep of `weapons-arms-race` (not in corpus) flags ≥1 novel run; a `divergent` re-sweep scores within a sane band (no novelty-bonus-only inflation).

## Task S5: README + gate
- README sweep section: document the three new columns and `--archive`; one line naming `emergence_score` as the sweep metric (design §10).
- Gate: `cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`. No golden-hash regeneration (prove it: determinism suite passes untouched).

---

## Completion notes (2026-07-22)

All tasks complete. Evidence:

- **Corpus:** 64 runs in `runs/corpus-e1/` (16 seeds × 5000 ticks × divergent/inventions/predator-prey/cooperation; missing seeds from interrupted sweeps backfilled via single-seed `run --events-jsonl`, identical format).
- **Derived table** (`WEIGHTS_VERSION = "e1.1"`): ubiquitous `extinction` (n_t=61) → 0.048; `arms_race` (n_t=10) → 1.86; `pack_hunting` (n_t=3) → 3.06; `alarm_call`, `practice_*`, `resource_traded`, `dowry_birth` unseen → `NOVELTY_BONUS` 5.159.
- **Archive verification:** `biome-trade` 4 seeds × 3000 ticks vs corpus → all 4 runs flagged novel (`resource_traded`), event streams copied to `runs/e1-verify-biome-trade/novel/`.
- **Default-table verification:** `divergent` 4 seeds × 2000 ticks, no archive → scores 1.63–3.98, coverage 0.43–0.52, novel=0 across the board (no bonus-only inflation).
- **Determinism:** no `anabios-core` changes; golden suites untouched and passing.
