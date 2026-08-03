# Emergence corpus, weekly sweeps, and triage

The operational loop for scorecard-driven emergence discovery: maintain a
reference corpus, run weekly archive-weighted sweeps per scenario, triage the
results for novel behavior, and drill into promising seeds. Everything below
is a thin wrapper over `anabios-headless` via `scripts/emergence.sh` — no
sim-behavior changes, so it never touches golden hashes.

## 1. Reference corpus

The corpus is a directory of `*.events.jsonl` files (one per swept seed,
`seed_XXXXXXXX.events.jsonl`), collected recursively. `sweep --archive <dir>`
loads it, computes an empirical IDF weight per event type — `ln(N / n_t)`
where `N` is the number of runs and `n_t` is how many of them fired that
type — and scores every seed in the current sweep against those weights.
Rarer-in-the-corpus event types score higher; types never seen in the corpus
score at the fixed `NOVELTY_BONUS` (`ln(N) + 1`).

`load_corpus` (`crates/anabios-headless/src/score.rs`) walks the directory
tree and skips any subdirectory literally named `novel/`
(`score.rs:328-332`). Sweep output routes its own corpus-unseen runs into
`<out>/novel/` (see below), so it is safe to point `--archive` directly at an
**accumulating** output tree — re-running sweeps into the same corpus root
never double-counts the novel-run copies it wrote on a previous pass.

### Where it lives

`runs/` is git-ignored wholesale (`.gitignore`), so any corpus directory
under it — conventionally `runs/corpus-eN.M/` (e.g. `runs/corpus-e1.1/`,
`runs/corpus-e1.2/`) — is local-only and never committed. Point
`ANABIOS_CORPUS` at whichever one you're using; `scripts/emergence.sh
sweep-archived` defaults to `runs/corpus` if it's unset, so either set the
env var or symlink `runs/corpus` at the vintage you want active.

### Which weight table it corresponds to — read this before trusting scores

The **shipped default table** (`ScoreTable::default_table()`, used whenever
you sweep *without* `--archive`) is derived from `DEFAULT_CORPUS_NT` in
`score.rs`, a hand-transcribed table of per-type counts from a **64-run
reference sweep taken 2026-07-22** (16 seeds each of `divergent`,
`inventions`, `predator-prey`, `cooperation` at 5000 ticks). That table's
version is `score::WEIGHTS_VERSION = "e1.1"`.

**As of this writing, e1.1 is still current — it has not been regenerated
since.** See the caveat in section 4 before reading anything into an
absolute `emergence_score` for a type introduced after E3.

### Regen recipe (produces the next vintage, e.g. e1.2 — NOT currently done)

```bash
cargo build --release -p anabios-headless
for s in divergent inventions predator-prey cooperation; do
  ./target/release/anabios-headless sweep --scenario scenarios/$s.toml \
    --seeds 16 --ticks 5000 --out runs/corpus-e1.2/$s
done
# then count per-type file-presence across the 64 *.events.jsonl (map the
# CamelCase event_type -> snake_case via score::event_name) into
# DEFAULT_CORPUS_NT, bump WEIGHTS_VERSION to "e1.2", and update the pinned
# regression test `default_weights_match_documented_values` in score.rs.
```

Regenerating the default table shifts every scenario's absolute
`emergence_score` (it's a rescale, not just an addition), so land it as its
own commit/PR — keep a clean `WEIGHTS_VERSION` boundary in score-trend
history rather than folding it into an unrelated change.

Note this recipe builds the *default reference table*, not an `--archive`
corpus per se — but the same sweep output (a directory of
`seed_XXXXXXXX.events.jsonl` files) is exactly what `--archive` consumes, so
`runs/corpus-e1.2/` produced this way can also be pointed at directly with
`--archive` / `ANABIOS_CORPUS` if you want empirical (not hand-baked)
scoring at that vintage.

## 2. Weekly sweep

For each scenario under active investigation:

```bash
ANABIOS_CORPUS=runs/corpus-e1.1 scripts/emergence.sh sweep-archived <scenario> --seeds 32 --ticks 8000
```

(Omit `ANABIOS_CORPUS` to use the default `runs/corpus`.) This wraps `sweep
--scenario <scenario> --archive $ANABIOS_CORPUS --out <out>`, building the
headless binary first if needed. It prints to stdout as it runs:

- progress lines (`[sweep] N/seeds done (seed=S)`)
- **top-5-by-`emergence_score`**, each with `seed`, `score`, `coverage`,
  `novel` count
- a **novel-run list** — every seed that fired at least one corpus-unseen
  event type, with its `novel_types` inline — *if* any fired
- a final `summary: <out>/summary.csv   novel: <out>/novel/` line

Capture that stdout (or just keep the terminal scrollback) — the top-5 and
novel lists are the fastest way to see whether the sweep produced anything
interesting before opening the CSV.

## 3. Triage

1. Open `<out>/summary.csv`. Columns: `seed, ticks, final_alive,
   final_biomass, state_hash`, then one count column per event type (53
   columns, in `score::ALL_EVENT_NAMES` order), then `emergence_score,
   novel_events, coverage, novel_types` — 62 columns total, `novel_types`
   last.
2. Sort by `emergence_score` descending.
3. For any row with `novel_events > 0`, read its `novel_types` column
   (semicolon-joined event-type names, persisted directly in the CSV — no
   need to scrape stdout or re-grep the JSONL for this).
4. Open `<out>/novel/seed_XXXXXXXX.events.jsonl` for that seed — sweep
   auto-copies every novel run's full event stream there — and inspect the
   events around the novel type's first occurrence.
5. For a seed worth digging into further, run its long-horizon
   novelty-decay curve:
   ```bash
   scripts/emergence.sh soak <scenario> --seed <n>
   ```
6. Then load that same seed in the Godot viewer to watch it directly:
   ```bash
   scripts/emergence.sh view <scenario> --seed <n>
   ```

## 4. Caveat: e1.1 scores post-E3 detectors as permanently novel

**Task 3 (regenerating the weight table to e1.2) was not done.**
`score::WEIGHTS_VERSION` is still `"e1.1"`, built from the 2026-07-22 sweep —
which predates roughly 30 detectors added since E3 (e.g. `pop_cycle`, `war`,
`settlement`, `specialization_split`, and others; see the full list of
zero-count entries in `DEFAULT_CORPUS_NT` in `score.rs`).

Under e1.1, every one of those ~30 newer detectors has `n_t = 0` in the
reference table, which the scoring math treats identically to "truly never
seen in any corpus run": they score at the fixed `NOVELTY_BONUS` (`ln(64) +
1 ≈ 5.16`) on *every* firing, forever — not just the first time. This
inflates `emergence_score` for any run that fires a modern detector,
independent of how common that behavior actually turns out to be once you
look. It also means `novel_types`/`novel_events` will flag those detectors
as "corpus-unseen" on every sweep, even after you've seen them fire hundreds
of times across your own weekly runs.

Practical effect on triage: don't read a high `emergence_score` alone as
"rare and interesting" while e1.1 is active — cross-check against
`novel_types` and use judgment about which of the flagged types are
genuinely new to you versus just structurally excluded from the reference
corpus. Regenerating to e1.2 with a corpus that includes post-E3 scenarios
(or points `--archive` at an accumulated `runs/corpus-e1.1/` tree from real
sweeps) will fix the discrimination — see the regen recipe in section 1 when
that's wanted.
