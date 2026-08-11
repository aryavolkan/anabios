# Reproduce a finding

From clone to a reproduced emergence run, using only the docs. Every command
runs verbatim on a clean checkout (macOS; Linux works the same — the viewer
needs Godot 4.6+, everything else is cargo-only).

## 0. Build

```sh
git clone https://github.com/aryavolkan/anabios.git && cd anabios
cargo build --release -p anabios-headless
```

`scripts/emergence.sh` wraps the release binary and rebuilds it if stale;
all commands below use it.

## 1. Watch an emergence tally (2 min)

```sh
scripts/emergence.sh run inventions
```

Runs `scenarios/inventions.toml` once and tallies the codex events —
`InventionDiscovered` (Stone Tools, Fire, …) fired by emergence, not
scripting. Every number below is reproducible exactly: bit-identical per
seed (`docs/determinism-contract.md`).

## 2. Reproduce the O1 culture-exclusion finding (the science)

The O-track's headline result: culture is competitively excluded because its
transmission is payoff-blind. Reproduce the corrected (founder-tag) read on
the same scenario+seed the finding used:

```sh
./target/release/anabios-headless autopsy \
  --scenario scenarios/experiments/o1-invasion-cultural-into-asocial.toml \
  --seed 1 --ticks 2500 --window 500 --tag founder --mutant cultural \
  --out /tmp/o1.csv
```

Expect `invasion_fitness_share mutant=cultural r≈-0.97 EXCLUDED` — the
cultural founder lineage is excluded, exactly as reported in
`docs/superpowers/specs/2026-08-07-o2a-corrected-decomposition.md`. Compare
`--tag module` on the same run to see the instrument drift that hid it.

## 3. Reproduce the trade-freeze fix (the mechanics)

`biome-trade` freezes permanently at ~t10k; the `unilateral-trade` variant
(surplus-gift exchange + goods conserved on death) keeps trading past the
freeze. Trade volume isn't a codex event (the `ResourceTraded` event latches
on the first swap), so read the `total_trades` column the sweep CSV exports:

```sh
./target/release/anabios-headless sweep --scenario scenarios/biome-trade.toml \
  --seeds 3 --ticks 20000 --out /tmp/freeze-base
./target/release/anabios-headless sweep --scenario scenarios/unilateral-trade.toml \
  --seeds 3 --ticks 20000 --out /tmp/freeze-fixed
# compare the total_trades column in each summary.csv
```

The dramatic pinned-seed version is pre-measured:
`docs/superpowers/data/trade-o26-unilateral-windows.csv` (baseline: 54 swaps
in the t10–12k window, 1 in t18–20k; the fix: 52,261 and 2,851). Diagnosis:
`docs/superpowers/specs/2026-08-02-trade-freeze-diagnosis.md`.

## 4. Mine for something new (the discovery loop)

```sh
ANABIOS_CORPUS=runs/corpus-e1.3 \
  scripts/emergence.sh sweep-archived predator-prey --seeds 16 --ticks 8000
```

Ranks 16 seeds by emergence score against the reference corpus and copies
corpus-unseen runs to `<out>/novel/`. The corpus is local — build it with
the recipe in `docs/emergence-corpus.md` §1 (or sweep without `--archive` to
use the shipped default weights). Triage the shortlist per
`docs/emergence-corpus.md` §3: `summary.csv` sorted by `emergence_score`,
then `soak <scenario> --seed <n>`, then `view <scenario> --seed <n>`.

## 5. Watch it in the viewer

```sh
scripts/emergence.sh view out-of-africa-saga --seed 318
```

The flagship: seeded era-3 tech (the honest framing — the emergent climb is
measured unreachable at grand scale, `docs/showcase-plan.md` §2), downstream
tech emergent. Or watch the same story in a browser, no install:
<https://aryavolkan.github.io/anabios/> (reproducible:
`scripts/emergence.sh record-web out-of-africa-saga --seed 318` regenerates
the hosted deck bit-for-bit).

## Which scenario shows what

`docs/scenarios.md` maps all 42 curated scenarios to their phenomena and
flags. Roadmap and per-item plans: [`ROADMAP.md`](../ROADMAP.md) +
[`docs/superpowers/plans/`](../docs/superpowers/plans/2026-08-01-roadmap-plans-index.md).
