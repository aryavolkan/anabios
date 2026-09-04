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
`score.rs`, a hand-transcribed table of per-type counts from a **168-run
reference sweep taken 2026-08-09** (the e1.3 vintage; recipe below). That
table's version is `score::WEIGHTS_VERSION = "e1.3"`. The current corpus
lives at `runs/corpus-e1.3/`.

Vintage history: e1.1 (2026-07-22, 64 runs × 4 scenarios) predated ~30
post-E3 detectors; e1.2 (2026-08-03, same 64 runs) measured the post-E3
detectors but still left 24/59 types at `n_t = 0` (no economy/culture/
domestication/affect scenario in the corpus); **e1.3** broadened the corpus
to 13 scenarios, cutting permanently-novel types to **2/59** (`evolved_tool`,
`territorial_rage` — both genuinely rare).

### Regen recipe (the e1.3 vintage)

```bash
cargo build --release -p anabios-headless
for s in divergent inventions predator-prey cooperation \
         biome-trade traditions trophic-cascade settlement; do
  ./target/release/anabios-headless sweep --scenario scenarios/$s.toml \
    --seeds 16 --ticks 5000 --out runs/corpus-e1.3/$s
done
for s in dimorphism domestication knowledge-ratchet affect-showcase; do
  ./target/release/anabios-headless sweep --scenario scenarios/$s.toml \
    --seeds 8 --ticks 5000 --out runs/corpus-e1.3/$s
done
./target/release/anabios-headless sweep \
  --scenario scenarios/experiments/o1-invasion-cultural-into-asocial.toml \
  --seeds 8 --ticks 5000 --out runs/corpus-e1.3/o1-invasion
# then count per-type file-presence across the 168 *.events.jsonl (map the
# CamelCase event_type -> snake_case via score::event_name) into
# DEFAULT_CORPUS_NT, bump WEIGHTS_VERSION, and update the pinned regression
# test `default_weights_match_documented_values` in score.rs.
```

Regenerate the corpus (and table) whenever a new codex event type lands —
new detectors sit at `n_t = 0` until the next vintage.

Regenerating the default table shifts every scenario's absolute
`emergence_score` (it's a rescale, not just an addition), so land it as its
own commit/PR — keep a clean `WEIGHTS_VERSION` boundary in score-trend
history rather than folding it into an unrelated change.

Note this recipe builds the *default reference table*, not an `--archive`
corpus per se — but the same sweep output (a directory of
`seed_XXXXXXXX.events.jsonl` files) is exactly what `--archive` consumes, so
`runs/corpus-e1.3/` produced this way can also be pointed at directly with
`--archive` / `ANABIOS_CORPUS` if you want empirical (not hand-baked)
scoring at that vintage.

## 2. Weekly sweep

For each scenario under active investigation:

```bash
ANABIOS_CORPUS=runs/corpus-e1.3 scripts/emergence.sh sweep-archived <scenario> --seeds 32 --ticks 8000
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
   final_biomass, state_hash`, then one count column per event type
   (`EVENT_TYPE_COUNT` columns — 63 as of the disease events — in
   `score::ALL_EVENT_NAMES` order), then `emergence_score, novel_events,
   coverage, total_trades, novel_types` — `5 + EVENT_TYPE_COUNT + 5`
   columns total, `novel_types` last. (Stated by reference because this
   line went stale twice when literals were used.)
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

## 4. Validated: the e1.3 loop discriminates (2026-08-09)

First weekly-style archived sweeps against `runs/corpus-e1.3` (16 seeds ×
8000 ticks each): `biome-trade`, `predator-prey`, and `weapons-arms-race`
(8 seeds) all produce **novel=0** shortlists whose `emergence_score` spread
reflects genuine rarity differences (e.g. biome-trade seeds 13.8→10.8;
arms-race seed 5 at 15.6 vs seed 3 at 10.0) — versus the pre-e1.3 behavior
where *every* run flagged 10-14 permanently-novel types and the shortlist
was noise. Under e1.3 a `novel_events > 0` row means the run fired
`evolved_tool` or `territorial_rage` (the only two corpus-unseen types) or a
detector added after 2026-08-09 — a triageable signal.

(The old section-4 caveat — pre-e1.2 tables scored ~30 post-E3 detectors as
permanently novel — is resolved by the e1.2/e1.3 regenerations; it survives
here only as vintage history in section 1.)
