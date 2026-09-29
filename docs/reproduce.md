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
scripts/emergence.sh run tribes
```

Runs `scenarios/tribes.toml` for 1000 ticks and tallies the codex events.
Stone Tools is seeded on the innovators and the hunter band, so its
`InventionDiscovered` fires on the seeding at tick 0; the one for Fire,
at tick 947, is fired by emergence, not scripting. Every number below is
reproducible exactly: bit-identical per seed
(`docs/determinism-contract.md`).

## 2. Reproduce the O1 culture-exclusion finding (the science)

The O-track's headline result: culture is competitively excluded because its
transmission is payoff-blind. Reproduce the corrected (founder-tag) read on
the same scenario+seed the finding used. That scenario,
`scenarios/experiments/o1-invasion-cultural-into-asocial.toml`, is retired
(the twelve-world consolidation removed `scenarios/experiments/`); restore
it from commit `16d9731`, the last commit that carried it. The scenario
schema has since started defaulting every feature knob on, so the knobs the
file never mentioned (off by default, or not yet existing, at that commit)
are written off ahead of it; with them off the engine's trajectory is
unchanged since `16d9731`, so this replays the file's run at that commit
exactly:

```sh
{ printf '%s = false\n' gene_requirements affect_enabled conserve_goods_on_death \
    knowledge_enabled basic_needs_enabled mate_seeking_enabled territory_enabled \
    repro_biased_learning anthro_race_enabled disease_enabled gait_enabled growth_enabled \
    turning_enabled gestation_enabled chase_enabled
  git show 16d9731:scenarios/experiments/o1-invasion-cultural-into-asocial.toml
} > /tmp/o1.toml
./target/release/anabios-headless autopsy \
  --scenario /tmp/o1.toml \
  --seed 1 --ticks 2500 --window 500 --tag founder --mutant cultural \
  --out /tmp/o1.csv
```

Expect `invasion_fitness_share mutant=cultural r=-0.29185 EXCLUDED` — the
cultural founder lineage is excluded, the verdict reported in
`docs/superpowers/specs/2026-08-07-o2a-corrected-decomposition.md`. (This
page used to quote `r≈-0.97`, read on the engine of that date; later engine
changes moved the magnitude, not the verdict. The value above is what the
command prints on the current engine, unchanged since `16d9731`.) Compare
`--tag module` on the same run — it prints `r=none (no rare-phase data)`:
read by Communicator module, the cultural strategy never shows a rare
phase at all, the instrument drift that hid the exclusion.

## 3. Reproduce the trade-freeze fix (the mechanics)

The freeze was diagnosed on `biome-trade`, whose bilateral barter froze
permanently at ~t10k, while the `unilateral-trade` variant (surplus-gift
exchange + goods conserved on death) kept trading past it. Both files are
retired: `markets` absorbed their founders, and the `unilateral_trade` lever
stays off in every curated world — the retired file's configuration lives on
as an inline fixture in `crates/anabios-core/tests/trade.rs`. The pinned-seed
evidence is pre-measured:
`docs/superpowers/data/trade-o26-unilateral-windows.csv` (baseline: 54 swaps
in the t10–12k window, 1 in t18–20k; the fix: 52,261 and 2,851). Diagnosis:
`docs/superpowers/specs/2026-08-02-trade-freeze-diagnosis.md`.

To try the lever on today's world, sweep `markets` with and without it.
Trade volume isn't a codex event (the `ResourceTraded` event latches on the
first swap), so read the `total_trades` column the sweep CSV exports:

```sh
./target/release/anabios-headless sweep --scenario scenarios/markets.toml \
  --seeds 3 --ticks 20000 --out /tmp/markets-bilateral
{ echo 'unilateral_trade = true'; cat scenarios/markets.toml; } > /tmp/markets-unilateral.toml
./target/release/anabios-headless sweep --scenario /tmp/markets-unilateral.toml \
  --seeds 3 --ticks 20000 --out /tmp/markets-unilateral
# compare the total_trades column in each summary.csv
```

On the full-stack `markets` the lever raises total swaps over 20k ticks on
every seed (seeds 0/1/2: 10,369 / 4,430 / 4,450 bilateral against 41,199 /
5,961 / 9,859 with the lever); the pinned-window freeze itself was measured
on the retired world.

## 4. Mine for something new (the discovery loop)

```sh
ANABIOS_CORPUS=runs/corpus-e1.3 \
  scripts/emergence.sh sweep-archived predator-prey --seeds 16 --ticks 8000
```

Ranks 16 seeds by emergence score against the reference corpus and copies
corpus-unseen runs to `<out>/novel/`. The corpus is local — build it with
the recipe in `docs/emergence-corpus.md` §1 (or sweep without `--archive` to
use the shipped default weights). Both the e1.3 corpus and the shipped
weights were built on the pre-consolidation scenario files, most of which
ran with the subsystems off; today's full-stack worlds fire many of those
event types far more often, so scores against them run compressed until a
new vintage is swept on the twelve worlds. Triage the shortlist per
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
the hosted deck bit-for-bit). The deck's `/atlas/` runs the same core *live*
in the browser (WebAssembly + three.js) — `scripts/web.sh build && scripts/web.sh serve`
locally, see [`web/README.md`](../web/README.md).

## Which scenario shows what

`docs/scenarios.md` maps the twelve worlds to their phenomena, opt-outs and
validation results. Roadmap and per-item plans: [`ROADMAP.md`](../ROADMAP.md) +
[`docs/superpowers/plans/`](../docs/superpowers/plans/2026-08-01-roadmap-plans-index.md).
