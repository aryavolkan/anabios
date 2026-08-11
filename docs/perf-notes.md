# Perf notes (roadmap 3.3, 2026-08-10)

**Verdict: no cheap win found.** Profiled the 10k+ tick hot paths, isolated
the dominant stage with a new microbench, attempted the one byte-identical
trim the code suggested, measured no delta. Recorded so the next attempt
starts informed. Machine: Apple Silicon (10 cores), `--release`.

## Baselines (criterion, `crates/anabios-core/benches/tick_bench.rs`)

| bench | time |
|---|---|
| `tick/1000` | 1.07 ms |
| `tick/10000` | 4.44 ms |
| `tick/step_with_codex/10000` | 2.99 ms |
| `tick/step_skip_codex/10000` | 2.36 ms |
| `stages/sense/10000` | ~1.0 ms |
| `stages/codex/10000` | 2.2 ms (amortized via `codex_interval` in practice) |
| `stages/spatial_rebuild/10000` | 59 µs |
| `culture/culture_step/10000_dense` (new) | 40.4 ms |

## Where the time actually goes (sampling profile)

`sample` on a release `run` of `experiments/sandbox-xlarge.toml` (12k alive,
dense clusters, cognition/inventions on), 8095 samples:

- **`culture::culture_step` ≈ 73% of `step`** (5905/8095). This is the 10k+
  bottleneck — *not* `sense` (the criterion default world has no
  communicators, which is why `tick/10000` hides it).
- `codex::observe_all` ≈ 4% (SpeciesAggTable::build inside it ≈ 3%).
- Everything else is noise.

The new `culture/culture_step/10000_dense` bench (10k communicators in a
radius-100 disc, inventions+cognition on) reproduces the hot case in
isolation: ~7.3M neighbour-candidate visits per call, ≈5.5 ns each.

## What was tried

**Generic-channel-only broadcast accumulation** (`scan_neighbor_memes`
accumulated `sum`/`count` over all 20 meme channels per neighbour; only the
6 generic channels are ever read, and every channel's count is identical, so
trimming to 6 sums + 1 scalar is provably bit-identical). Implemented,
goldens unchanged, **measured delta: none** (40.4 → 40.9 ms, within noise).
Reverted. The per-candidate cost is not the channel loop.

## Why no cheap win exists on this path

- The post-trim per-candidate body is ~19 cheap ops (module check, skill
  compare, 10 invention compares, 2 practice compares, 6 broadcast adds) and
  the sampling profile shows it **fully inlined into `culture_step` with no
  sub-frame hotspot** — the cost is the visit count itself
  (~7.3M/call in dense clusters), not any operation inside the visit.
- **Parallelization is blocked by byte-identity**: receivers apply updates
  in-place in ascending-id order and later receivers' scans observe earlier
  receivers' *updated* meme vectors. A two-phase scan-then-apply (which
  would parallelize cleanly) changes results — out of the byte-identical
  budget this milestone allowed.
- The spatial `query` ring (3×3 cells, no exact-distance filter) *is* the
  neighbourhood semantics for culture — adding an exact filter changes who
  counts as a neighbour → behavior change, same budget problem.

## For the next attempt (all behavior-altering; need a golden rehash buy-in)

1. **Two-phase culture step** (scan all receivers from tick-start meme
   state, then apply): unlocks rayon over receivers, the same structure as
   `sense_all`. Expect the dense case to go ~parallel-limited (≈5–8× at 10
   cores). Changes results vs in-place serial order → golden refresh.
2. **Exact-distance filter in culture's ring scan** (or accept the 3×3
   square): ~2.9× fewer candidates in dense clusters (circle vs square).
   Changes neighbourhood membership → golden refresh.
3. **Skip zero-meme neighbours**: a per-agent "has anything worth copying"
   summary (recomputed on meme writes) could early-out most candidates — but
   the broadcast-mean *denominator* counts every Communicator neighbour, so
   only the max-tracking loops can be skipped, and only when the neighbour
   holds nothing above the receiver's levels. Modest, fiddly.
4. **Codex cadence** (#92) already amortizes `observe_all`; at
   `codex_interval > 1` its ~4% shrinks proportionally.

Note the benchmark caveat: the default `tick/10000` world is
communicator-free, so whole-tick numbers can look fine while
culture-dominated scenarios (sandbox-xlarge, grand-theater, OoA) pay 73% in
`culture_step`. Any future claim of a whole-tick win should quote a
communicator-dense bench alongside `tick/10000`.
