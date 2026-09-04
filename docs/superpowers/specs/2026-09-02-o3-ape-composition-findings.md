# O3 findings II: ape composition, the material gate, and the first era-2 event

**Date:** 2026-09-02. **Milestone:** O3 continuation (branch
`claude/o3-ape-composition`, stacked on PR #145). **Prior:**
[`2026-09-02-o3-repro-bias-findings.md`](2026-09-02-o3-repro-bias-findings.md)
ended with the blocker handoff "the winning culture is non-ape; era
progression is not testable until an ape lineage is among the winners."
**Data:** `docs/superpowers/data/o3/raw-ape/` (40 files); probe log
`../data/o3/probe-log.md` (findings 8+). All runs: OoA grand run with
`repro_biased_learning = true` (rb), 12–20k ticks, module tag, env-gated
throwaway probes marked THROWAWAY in source.

## 1. Ape-survival probes (P1 tier / P2 diet)

- **P1** `ANABIOS_O3_APE_TIER=1.0` (is_ape-only bite ×2 — the arc spec's
  "vertical resource tier", aimed at the population the tech tree is
  actually gated to) and **P2** `ANABIOS_O3_APE_DIET=0.34` (ape founders at
  the herbivorous band edge) both work on their own terms: apes survive to
  t20k (P1: 16–49 apes in 4/4 seeds; previously `comm_ape` → 0), and each
  lever flips previously-collapsed worlds (seed 318 → 99.8% / 93.5%
  cultural).
- Per-seed outcomes churn violently across conditions (seed 3: 94% → 2%
  under P1). The regime is **bistable**; only distributional reads count.

## 2. The era gate is materials (gate decomposition)

Extended `[o3diag]` on the stable 99.8%-cultural P1 world (seed 318):
**95% of apes clear the era-1 IQ gate; 0–1 of ~50 apes hold Stone Tools'
2-obsidian basket; invention holding is zero at equilibrium.** Invention
channels show transmission progress that can never cross the held
threshold — `materials_permit` gates progress, and goods do not circulate.
This is the measured **trade-freeze result operating on the invention
economy**: `unilateral_trade` (the shipped freeze fix, PR #129) and
`conserve_goods_on_death` are both OFF in `out-of-africa.toml`.

## 3. Trade-fix sweep — best flip rate yet, and the first era-2

`o3-ooa-rb-trade.toml` (= rb + `unilateral_trade` +
`conserve_goods_on_death`) + P1 tier, seeds 1–10 + 318, 12k ticks:

- **7/11 seeds end culture-dominant** (>10% share; five above 78%) — the
  strongest condition measured (rb alone: 3/6).
- Materials flow: ape material coverage 4% → 20–26%; `held_any` 41 → 173;
  cultural `mean_era` reaches 0.26–0.44 in several seeds (vs 0.000 in
  every pre-trade run).
- **Seed 4 reaches `max_era` 2 — the first era-2 event in the project's
  history** (emergent, unseeded; no other run in ~140 this campaign ever
  produced an era-2 row).

## 4. The remaining gap, named: inventions don't pay demographically

Across the sweep, cultural dominance and invention activity
**anti-correlate**: the culture-dominant worlds are invention-dead (seeds
1, 7: peak `mean_era` 0.000) while the invention-active worlds — including
the era-2 seed — are demographically collapsing (seed 4: peak `mean_era`
0.435, ends n=10). Ape-heavy cultural remnants invent; non-ape cultural
masses win. Stone Tools' +25% bite and Fire's energy multiplier do not
make the ape-invention package beat the plain herbivore-communicator
package in the intra-cultural contest.

**Handoff:** the O3 thesis "make culture pay" now localizes one level
down — **make inventions pay**: the invention-buff economics (magnitudes,
`gene_tech_coupling`, or an invention-gated tier that replaces the probe's
flat ape tier) must give invention-holding lineages a demographic edge.
That is the next pre-registerable mechanism cycle; its success metric is
already defined (culture-dominant AND era-active in the same world,
distributionally).

## What this branch carries

The extended `[o3diag]` gate-decomposition instrument;
`scenarios/experiments/o3-ooa-rb-trade.toml` (committed flags only); the raw
ledgers.

The `ANABIOS_O3_APE_TIER` / `ANABIOS_O3_APE_DIET` probe knobs were **removed
after measurement** (repo convention: measure, commit findings, tear out the
scaffolding). Review found they crossed lines the earlier probes hadn't:
APE_TIER changed hot-loop behavior without being recorded in snapshots or
events (a replay/golden-regen poisoning hazard from ambient shell state),
values were unvalidated (an out-of-band diet or a locale-comma typo silently
ran a different experiment), and the OnceLock latched the first value
process-wide. The diet/tier arms in the tables above therefore do **not**
reproduce from this tree — their raw logs are committed beside this doc, and
the `o3-ooa-rb-trade.toml` arm (the headline: 7/11 culture-dominant, first
era-2 event) reproduces from committed flags alone. If a future cycle needs
the tier/diet levers, they graduate to `#[serde(default)]` scenario flags
(self-recording, validated at parse), not env vars.

Post-measurement `[o3diag]` fixes (same review): all four gate counters now
share the communicator-ape denominator (`held_any` → `ape_held`; it
previously counted ALL alive agents against the "among apes" label),
`ape_iq1` goes through the real `iq_permits` (a cognition-off world reads
open, not blocked), and `ape_mat` documents that an agent that already PAID
the Stone Tools basket reads false — 'never afforded' and 'already paid'
are indistinguishable in that column. Old committed logs use the previous
keys/semantics; compare by key, not position.
