# E1 — Emergence Scorecard & Novelty Archive — Design Spec

**Date:** 2026-07-22
**Status:** Approved
**Milestone:** E1 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crate:** `anabios-headless` only — zero sim impact, no determinism/perf risk.

## 1. Goal & success criteria

Compute what the sweep CSV currently only hints at: how *much* and how *rarely* a world emerges. Every sweep run gets a scalar **emergence score**, a **coverage** fraction, and a **novel-events** count, so overnight sweeps can be ranked by novelty instead of eyeballed.

Success criteria:

1. `summary.csv` gains `emergence_score`, `novel_events`, `coverage` columns (appended last; existing column order unchanged).
2. `anabios-headless sweep --archive <dir>` scores each run against a corpus of prior runs; runs firing corpus-novel event types get their event streams copied to `<out>/novel/`.
3. Without `--archive`, scoring uses a shipped, versioned default weight table derived from a recorded reference corpus.
4. A type that fires in *every* corpus run contributes zero to the score (common = boring); a type never seen in the corpus contributes the maximum **novelty bonus**.
5. Scoring is pure post-processing over drained event counts — the sim binary path is untouched and golden hashes cannot move.

## 2. Definitions

Let a run fire a set of distinct event types `T_run` (counts > 0). Let the corpus be `N` prior runs, with `n_t` = number of corpus runs in which type `t` fired at least once.

- **Weight (IDF):** `w(t) = ln(N / n_t)` for `n_t > 0`; `w(t) = NOVELTY_BONUS = ln(N) + 1` for `n_t = 0`. Types firing in all `N` runs get `ln(1) = 0`.
- **Emergence score:** `score = Σ_{t ∈ T_run} w(t)` — distinct types only; repetition within a run adds nothing (detector chatter is not emergence).
- **Coverage:** `|T_run| / EVENT_TYPE_COUNT`.
- **Novel events:** `|{t ∈ T_run : n_t = 0}|` — count of distinct fired types absent from the corpus. Without `--archive`, the corpus is the reference corpus backing the default table, so "novel" means "never observed in the reference corpus".

With `--archive`, weights are recomputed from the archive corpus (empirical IDF); without it, the default table is used. In both modes the same three formulas apply.

## 3. Reference corpus & default table

The default weight table is derived from a committed-record corpus: 16 seeds × 4 scenarios (`divergent`, `inventions`, `predator-prey`, `cooperation`) @ 5000 ticks = 64 runs, swept once on 2026-07-22 during E1 implementation. The table lives as a const in `score.rs` with `WEIGHTS_VERSION`, the corpus recipe in a doc comment, and regeneration instructions (re-sweep the four scenarios into one dir, run the weight-derivation math, paste values). The corpus event streams themselves are not committed (too large); the recipe reproduces them deterministically.

## 4. CLI surface

```
anabios-headless sweep --scenario S --seeds N --ticks T --out DIR [--threads K] [--archive CORPUS_DIR]
```

- `--archive` points at a directory tree of prior sweep outputs; every `*.events.jsonl` file found recursively counts as one corpus run.
- End-of-sweep stdout gains: top-5 runs by score, and the list of novel runs (if any) with their novel type names.

## 5. Non-goals

- No W&B wiring beyond documenting `emergence_score` as the sweep metric (README line).
- No per-event-`value` granularity (e.g., which invention id) — scoring is per event *type* only.
- No incremental/within-sweep archive (runs in the same sweep do not score against each other).
- `run`/`demo` subcommands unchanged.

## 6. Testing

- Unit: IDF math on a tiny synthetic corpus (unseen type → bonus, ubiquitous type → 0); score sums distinct types only; coverage denominator is `EVENT_TYPE_COUNT`; novel-set difference.
- Integration: corpus loader over a tempdir fixture of hand-written `*.events.jsonl` files (nested subdir included), verifying per-run type sets and handling of malformed lines (skip with warning, don't fail the sweep).
- Evidence (recorded in the plan): build the 64-run reference corpus, print the derived table, run a scored sweep with `--archive` against it and show runs flagged novel (`biome-trade`, whose `resource_traded`/`dowry_birth` never fire in the corpus scenarios, is novel by construction).
