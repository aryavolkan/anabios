# Emergence-Scorecard-Driven Sweeps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Operationalize the E1 emergence scorecard so weekly sweeps rank runs by novelty against a *current* reference corpus and surface a triageable shortlist of corpus-unseen runs — with the per-seed novel-type list persisted, not lost to stdout.

**Architecture:** Pure `anabios-headless` tooling + operational workflow; zero simulation impact (matches the E1 non-goals). Three code changes (persist `novel_types` to CSV; regenerate the reference weight table to cover the 30 post-E3 detectors; a corpus-management wrapper), then a documented corpus + weekly-sweep + triage protocol.

**Tech Stack:** Rust (`anabios-headless`: `sweep.rs`, `score.rs`, `main.rs`), `clap`, `rayon`, `bash` (`scripts/emergence.sh`).

## Global Constraints

- Zero simulation-crate impact: changes live in `crates/anabios-headless/` and `scripts/`. Do not touch `anabios-core` except the codex event enumeration if (and only if) regenerating weights (Task 3).
- The scorecard is **type-level** (distinct fired event types), never per-`value` — do not add per-payload granularity.
- `ALL_EVENT_NAMES` (`score.rs`), `EventType`/`EVENT_TYPE_COUNT` (`anabios-core/src/codex/event.rs`), and `DEFAULT_CORPUS_NT` (`score.rs`) must stay length-synced; existing asserts at `score.rs:359-368` enforce this — keep them passing.
- CSV header and rows both iterate `ALL_EVENT_NAMES`; any column change must update both emit sites so they cannot drift.
- Bump `WEIGHTS_VERSION` (`score.rs:22`) whenever `DEFAULT_CORPUS_NT` changes; update the pinned reference-weight regression test (`score.rs:371-410`).

---

## File Structure

- `crates/anabios-headless/src/sweep.rs` — `RunSummary`, `run_one`, `write_summary_csv`, `report_novelty`. **Modify** to persist `novel_types` into the CSV.
- `crates/anabios-headless/src/score.rs` — scorer, `ScoreTable`, `DEFAULT_CORPUS_NT`, `WEIGHTS_VERSION`, regression test. **Modify** only if regenerating the corpus (Task 3).
- `crates/anabios-headless/tests/sweep_csv.rs` — **Create**: integration test over a tempdir sweep output asserting the CSV schema (incl. the new column) and novel-run routing.
- `scripts/emergence.sh` — **Modify**: add a `corpus`/`sweep-archived` convenience that points `--archive` at a maintained corpus dir.
- `docs/emergence-corpus.md` — **Create**: the corpus + weekly-sweep + triage runbook.

---

## Task 1: Persist `novel_types` into `summary.csv`

Today `report_novelty` (`sweep.rs:138-170`) prints per-seed `novel_types` to stdout only; `summary.csv` has the count (`novel_events`) but not the names, so triage requires scraping stdout. Add a trailing `novel_types` column (semicolon-joined names) so the CSV is self-sufficient.

**Files:**
- Modify: `crates/anabios-headless/src/sweep.rs` (`write_summary_csv`, ~`sweep.rs:172-195`; `RunSummary` already carries `novel_types: Vec<&'static str>` set at `sweep.rs:123`)
- Test: `crates/anabios-headless/tests/sweep_csv.rs`

**Interfaces:**
- Consumes: `RunSummary { seed, ticks, final_alive, final_biomass, state_hash, counts, emergence_score, novel_events, coverage, novel_types }` (existing).
- Produces: `summary.csv` with a final column `novel_types` (semicolon-joined, empty string when none). Column order becomes: 5 fixed prefix + 53 event counts + `emergence_score,novel_events,coverage,novel_types` (suffix now 4). Total columns = 62.

- [ ] **Step 1: Write the failing test**

Create `crates/anabios-headless/tests/sweep_csv.rs`:

```rust
use std::process::Command;

// Runs a tiny sweep and asserts the CSV header/rows carry the novel_types column.
#[test]
fn summary_csv_has_novel_types_column() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("swp");
    let status = Command::new(env!("CARGO_BIN_EXE_anabios-headless"))
        .args([
            "sweep",
            "--scenario",
            "scenarios/minimal.toml",
            "--seeds",
            "2",
            "--ticks",
            "50",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let csv = std::fs::read_to_string(out.join("summary.csv")).unwrap();
    let mut lines = csv.lines();
    let header = lines.next().unwrap();
    assert!(
        header.ends_with(",emergence_score,novel_events,coverage,novel_types"),
        "header was: {header}"
    );
    // every data row must have exactly 62 fields
    for row in lines {
        assert_eq!(row.split(',').count(), 62, "row: {row}");
    }
}
```

Add `tempfile` to `[dev-dependencies]` in `crates/anabios-headless/Cargo.toml` if absent.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-headless --test sweep_csv`
Expected: FAIL — header ends with `,coverage` (no `novel_types`), row field count is 61.

- [ ] **Step 3: Implement the column**

In `write_summary_csv` (`sweep.rs`), extend the header and each row. Header: after the `coverage` field append `,novel_types`. Row: after the `coverage` value append the joined names:

```rust
// header (append after "coverage")
header.push_str(",novel_types");

// per-row (after writing coverage)
let novel = r.novel_types.join(";");
line.push(',');
line.push_str(&novel);
```

Match the actual local variable names in `write_summary_csv`; the join must never contain a comma (event names are snake_case, safe).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-headless --test sweep_csv`
Expected: PASS.

- [ ] **Step 5: Fix any column-count assertion elsewhere**

Run: `cargo test -p anabios-headless` and `grep -rn '61\|columns' crates/anabios-headless/`. If any existing test pins 61 columns / the old suffix, update it to 62 / the new suffix.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-headless/src/sweep.rs crates/anabios-headless/tests/sweep_csv.rs crates/anabios-headless/Cargo.toml
git commit -m "feat(headless): persist per-seed novel_types in sweep summary.csv"
```

---

## Task 2: Integration test for novel-run routing

Lock in the `<out>/novel/` behavior so triage stays reliable: a scenario that fires a corpus-unseen type must land a copy under `novel/`. Per the E1 evidence, `biome-trade` fires `resource_traded`/`material_learning`, which the default table treats as rare/novel.

**Files:**
- Test: `crates/anabios-headless/tests/sweep_csv.rs` (add a second test)

**Interfaces:**
- Consumes: the `sweep` binary; the default table (no `--archive`).
- Produces: assertion that `<out>/novel/seed_XXXXXXXX.events.jsonl` exists when `novel_events > 0`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn novel_runs_are_copied_to_novel_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("swp");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_anabios-headless"))
        .args([
            "sweep", "--scenario", "scenarios/biome-trade.toml",
            "--seeds", "4", "--ticks", "1500",
            "--out", out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let csv = std::fs::read_to_string(out.join("summary.csv")).unwrap();
    let any_novel = csv.lines().skip(1).any(|r| {
        // novel_events is the 60th field (index 59): after 5 prefix + 53 counts + score
        r.split(',').nth(59).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) > 0
    });
    if any_novel {
        let novel = out.join("novel");
        assert!(novel.is_dir(), "novel/ dir missing though a run had novel_events>0");
        assert!(std::fs::read_dir(&novel).unwrap().count() > 0);
    }
}
```

- [ ] **Step 2: Run and confirm it passes (behavior already exists)**

Run: `cargo test -p anabios-headless --test sweep_csv::novel_runs_are_copied_to_novel_dir`
Expected: PASS. If it fails because no run fired a novel type, raise `--ticks` to `3000`. This test guards a regression, so passing-first is acceptable here — it is a characterization test, not TDD scaffolding.

- [ ] **Step 3: Commit**

```bash
git add crates/anabios-headless/tests/sweep_csv.rs
git commit -m "test(headless): guard novel-run routing to <out>/novel/"
```

---

## Task 3: Regenerate the reference weight table to cover post-E3 detectors (optional, gated)

The shipped `DEFAULT_CORPUS_NT` (`score.rs:105-159`, `WEIGHTS_VERSION = "e1.1"`) was built from a 2026-07-22 sweep predating E3–E13, so the 30 newer event types sit at `n_t = 0` — permanently "novel" until regenerated. That inflates novelty scores for anything firing a modern detector. Regenerate **only if** you want scores that discriminate among current detectors. If you prefer the historical baseline, skip this task and note the caveat in the runbook (Task 5).

**Files:**
- Modify: `crates/anabios-headless/src/score.rs` (`DEFAULT_CORPUS_NT`, `WEIGHTS_VERSION`, the pinned regression test at `score.rs:371-410`)

**Interfaces:**
- Consumes: a freshly built 64-run corpus (recipe below).
- Produces: updated `DEFAULT_CORPUS_NT` (53 entries), bumped `WEIGHTS_VERSION`, updated regression expectations.

- [ ] **Step 1: Build the reference corpus (deterministic)**

Follow the recipe in `score.rs:6-13`. Sweep the four reference scenarios, 16 seeds × 5000 ticks each, into one directory:

```bash
cargo build --release -p anabios-headless
for s in divergent inventions predator-prey cooperation; do
  ./target/release/anabios-headless sweep \
    --scenario scenarios/$s.toml --seeds 16 --ticks 5000 \
    --out runs/corpus-e1.2/$s
done
```

- [ ] **Step 2: Compute per-type run counts**

Count, for each of the 53 event names, how many of the 64 `*.events.jsonl` runs fired it at least once (a run = a file; presence, not frequency). A throwaway `--bin` or a `jq` one-liner over the JSONL is fine; the numbers are `n_t` for `DEFAULT_CORPUS_NT`. Record all 53 counts in `ALL_EVENT_NAMES` order.

- [ ] **Step 3: Update the table and version**

Replace `DEFAULT_CORPUS_NT` entries with the measured counts (keep name order identical to `ALL_EVENT_NAMES`), set `pub const WEIGHTS_VERSION: &str = "e1.2";`, and set `CORPUS_RUNS = 64` (unchanged if still 4×16).

- [ ] **Step 4: Update the pinned regression test**

`score.rs:371-410` asserts specific reference weights. Recompute the expected `idf_weight(n_t)` values for the checked names and update them. Keep the sync asserts (`score.rs:359-368`) intact.

- [ ] **Step 5: Run the scorer tests**

Run: `cargo test -p anabios-headless --lib score`
Expected: PASS (sync asserts + updated weight expectations).

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-headless/src/score.rs
git commit -m "chore(score): regenerate E1 weight table (e1.2) over current 53 detectors"
```

---

## Task 4: `emergence.sh` archived-sweep convenience

Make the archive-weighted weekly sweep a one-liner against a maintained corpus dir, instead of hand-passing `--archive`.

**Files:**
- Modify: `scripts/emergence.sh` (add a `sweep-archived` case near the existing `sweep` case, `~lines 193-200`)

**Interfaces:**
- Consumes: `$ANABIOS_CORPUS` env (default `runs/corpus`), the release binary.
- Produces: a sweep under `$OUT_DIR/sweep-<scn>-archived/` scored against the corpus.

- [ ] **Step 1: Add the case**

```bash
sweep-archived)
  scn="$(resolve "${1:-}")"; shift || true; build
  corpus="${ANABIOS_CORPUS:-runs/corpus}"
  out="$OUT_DIR/sweep-$(basename "$scn" .toml)-archived"
  "$BIN" sweep --scenario "$scn" --archive "$corpus" --out "$out" "$@"
  echo "summary: $out/summary.csv   novel: $out/novel/"
  ;;
```

Add `sweep-archived` to the usage/help block in the script header.

- [ ] **Step 2: Smoke it**

Run: `ANABIOS_CORPUS=runs/corpus-e1.2 scripts/emergence.sh sweep-archived biome-trade --seeds 4 --ticks 1500`
Expected: prints a `summary:` path and a `novel:` path; `novel/` populated if any run fired a corpus-unseen type.

- [ ] **Step 3: Commit**

```bash
git add scripts/emergence.sh
git commit -m "feat(scripts): emergence.sh sweep-archived against a maintained corpus"
```

---

## Task 5: The corpus + weekly-sweep + triage runbook

Codify the operational loop so it survives beyond this session.

**Files:**
- Create: `docs/emergence-corpus.md`

- [ ] **Step 1: Write the runbook**

Include, concretely:
- **Reference corpus:** what it is (a dir of `*.events.jsonl`), how it's built (Task 3 recipe), where it lives (`runs/corpus-eN.M/`), and the `WEIGHTS_VERSION` it corresponds to. Note the `novel/` subdir is auto-excluded from corpus counting (`score.rs:328-332`), so an accumulating tree is safe to point `--archive` at.
- **Weekly sweep:** `scripts/emergence.sh sweep-archived <scenario> --seeds 32 --ticks 8000` for each scenario under investigation; capture stdout (the top-5-by-score and novel-run lists print there).
- **Triage:** open `<out>/summary.csv`, sort by `emergence_score` desc; for rows with `novel_events > 0`, read the `novel_types` column (now persisted, Task 1) and inspect `<out>/novel/seed_XXXXXXXX.events.jsonl`. For a promising seed, run `scripts/emergence.sh soak <scenario> --seed <n>` for its long-horizon novelty-decay curve, then load that seed in the Godot viewer to watch it.
- **Caveat:** if Task 3 was skipped, state that post-E3 detectors score as permanently novel under `e1.1`.

- [ ] **Step 2: Commit**

```bash
git add docs/emergence-corpus.md
git commit -m "docs: emergence corpus + weekly-sweep + triage runbook"
```

---

## Testing Plan (summary)

| Level | What | Where |
|---|---|---|
| Unit | IDF math, coverage, novel_types selection (already exist) | `score.rs` tests |
| Unit | Reference weights pinned (update on regen) | `score.rs:371-410` |
| Unit | Length-sync of names/enum/corpus (already exist) | `score.rs:359-368` |
| Integration | CSV carries `novel_types`, 62 columns | `tests/sweep_csv.rs` (Task 1) |
| Integration | Novel-run routing to `novel/` | `tests/sweep_csv.rs` (Task 2) |
| Smoke | `sweep-archived` end-to-end | manual (Task 4) |
| Determinism | Sweep is zero-sim-impact; `state_hash` per run unchanged | existing `determinism.rs` (no new hashes) |

**Done when:** a maintained corpus dir exists; `scripts/emergence.sh sweep-archived <scn>` produces a ranked `summary.csv` whose `novel_types` column and `novel/` copies let you triage without reading stdout; and `docs/emergence-corpus.md` documents the weekly loop.

## Self-Review notes

- Spec coverage: corpus (Task 3/5), archive-weighted sweep (Task 4), novel triage (Task 1/2/5) — all covered.
- The `novel_events` field index in Task 2's test (nth(59)) assumes 5 prefix + 53 counts + `emergence_score` = index 59 for `novel_events`; verify against the actual `write_summary_csv` order before relying on it.
- Risk: regenerating weights (Task 3) shifts every scenario's absolute `emergence_score`. Keep it a separate commit/PR so score-trend history has a clean version boundary (`WEIGHTS_VERSION`).
