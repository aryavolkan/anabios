# E11 — Climate maladaptation (`MaladaptationLag`)

## Goal

E10 made the environmental optimum a *moving target*: `culture::env_optimum_at`
sweeps seasonally (`env_period`) and, with `climate_drift_rate > 0`, wanders its
baseline on a slow secular drift that never stationarizes. E11 detects the
emergent stress that only the moving target can produce: a species whose learned
foraging skill stays **chronically far** from the current optimum for a long
stretch is *maladapted* — it cannot keep up with the drift. This is a pure
emergent observation over existing state (no new sim mechanic).

## Detector

`crates/anabios-core/src/codex/climate.rs::detect_maladaptation`, registered last
in `codex/mod.rs::observe_all` and amortized on `CYCLE_CHECK_INTERVAL` (every 10
ticks), mirroring the other streak detectors (`traditions::detect_ratchet`).

Per amortized check:

1. **Short-circuit** `return` when `world.env_period == 0` — with no seasonal
   optimum there is nothing to lag behind, so non-DIT scenarios never fire.
2. `optimum = culture::env_optimum_at(tick, env_period, climate_drift_rate)`.
3. For each active species, reuse the fused `SpeciesAgg.meme_sums` to get the
   mean skill-channel value: `mean_skill = meme_sums[SKILL_CHANNEL] / count`.
4. `lag = (mean_skill − optimum).abs()`.
5. Advance a per-species streak by `CYCLE_CHECK_INTERVAL` while
   `lag >= MALADAPT_LAG_MIN`; reset it to `0` the moment the species catches up.
6. Fire (`value = lag`) through the shared `edge_trigger_species` latch when the
   streak reaches `MALADAPT_WINDOW`. The latch means one event per maladapted
   stretch; it re-arms automatically when the streak resets (catch-up).

At the end of the pass **both** per-species maps are pruned to the active set
(`retain(|sid, _| active.contains(sid))`) so extinct species cannot grow the
scratch unbounded — the recurring per-species-map leak this codebase guards
against.

## Constants

| Constant | Value | Meaning |
|----------|-------|---------|
| `MALADAPT_LAG_MIN` | `0.25` | Min `|mean skill − optimum|` counting toward the streak. The seasonal band is 0.5 wide, so a quarter-band gap is genuinely off-target. |
| `MALADAPT_WINDOW` | `500` ticks | Sustained lag before firing (50 amortized checks). A persistent stress, not a transient dip. |

## State (persistent, hashed — NOT `#[serde(skip)]`)

- `CodexState.maladapt_streak: BTreeMap<u32, u32>` — per-species consecutive-lag ticks.
- `CodexState.maladapt_active: BTreeSet<u32>` — species currently latched maladapted.

## Wiring (parallel arrays kept in lockstep, all = 49)

- `codex/mod.rs`: `EventType::MaladaptationLag = 48`; `EVENT_TYPE_COUNT` now
  derives from `MaladaptationLag` (= 49).
- `anabios-headless/src/score.rs`: `ALL_EVENT_NAMES` (49), `DEFAULT_CORPUS_NT`
  `("maladaptation_lag", 0)` (post-corpus), and an `event_name` match arm.
- `anabios-headless/src/sweep.rs::write_summary_csv`: header column +
  `g("maladaptation_lag")` arg (header/format/args balanced).
- `game/scripts/codex_panel.gd`: `CHAPTER_NAMES` + `CHAPTER_COLORS` entry (49
  each; the boot assert checks both equal `event_type_count()`).

## Determinism

`FORMAT_VERSION` bumped 17 → 18 (v18 changelog line in `snapshot.rs`). Golden
state-hash tables in `tests/{determinism,inventions,cognition}.rs` regenerated
via `UPDATE_HASHES=1`. All three golden scenarios have `env_period == 0`, so the
detector short-circuits and never fires — the simulation trajectory is
byte-identical; the hashes moved only because the serialized layout grew by the
two new `CodexState` maps and the wider event-buffer variant space.

## Evidence

- Unit tests (`codex/climate.rs`): a species pinned far from the optimum fires
  exactly once (latched); a species tracking the optimum never fires;
  `env_period == 0` never fires.
- Scenario: `scenarios/maladaptation.toml` (`env_period = 400`,
  `climate_drift_rate = 0.00008`, a slow-learning asocial `innate_forager`
  stock that never advances its skill). A 3000-tick headless run fires
  `MaladaptationLag` — the chronically-lagging stock is flagged maladapted.
