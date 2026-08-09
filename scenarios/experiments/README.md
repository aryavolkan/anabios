# Archived experiment scenarios

One-variable-apart ablation suites and flavor variants from completed
experiments. Kept runnable (and smoke-tested by `tests/all_scenarios.rs`,
which walks `scenarios/` recursively) so the published findings stay
reproducible, but out of the top-level directory and the Godot viewer menu.

Run any of them directly, e.g.:

```bash
./target/release/anabios-headless run --scenario scenarios/experiments/dit-rogers.toml --ticks 5000
```

| Suite | Files | Findings |
|---|---|---|
| O1 exclusion ablations | `o1-invasion-*.toml`, `o1-lever-*.toml` (7) | [`docs/superpowers/specs/2026-08-03-o1-exclusion-findings.md`](../../docs/superpowers/specs/2026-08-03-o1-exclusion-findings.md); data in `docs/superpowers/data/o1/` |
| DIT boundary suite | `dit-env-slow.toml`, `dit-env-fast.toml`, `dit-env-static.toml`, `dit-rogers.toml` | [`docs/superpowers/specs/2026-07-12-dit-boundary-suite-design.md`](../../docs/superpowers/specs/2026-07-12-dit-boundary-suite-design.md) |
| Biome / climate variants | `arid-world.toml`, `lush-world.toml`, `ice-age.toml`, `desert-tropical.toml`, `archipelago.toml`, `drifting-climate.toml`, `maladaptation.toml` | [`docs/superpowers/specs/2026-07-24-e11-maladaptation-design.md`](../../docs/superpowers/specs/2026-07-24-e11-maladaptation-design.md) |
| Misc | `sandbox-xlarge.toml` (superseded by `sandbox-large.toml`), `gene-requirements.toml` (all invention gates on; see `tech-gene-coupling.toml` for the curated demo) | — |
