# The determinism contract

anabios is bit-identical per seed. Two mechanisms uphold that:

- **`state_hash`** (`crates/anabios-core/src/snapshot.rs`) — FNV-1a over the
  bincode-serialized `World`. Golden tests pin trajectories
  (`tests/determinism.rs` + per-subsystem pins); any intentional behavior
  change regenerates them in the same PR (`UPDATE_HASHES=1 …`).
- **Save/load round-trip** (`snapshot::{save_to_bytes, load_from_bytes}`) — a
  snapshot must restore-and-continue bit-identically. Guarded per opt-in
  subsystem by `tests/save_load_roundtrip.rs`.

The sharp edge: `state_hash` hashes exactly the *serialized* fields. A
`#[serde(skip)]` field is invisible to the hash — but if it feeds future
ticks, a save→load→step silently diverges (the v13 `still_ticks` footgun;
the custom-dims spatial-hash bug fixed in the same hardening pass that wrote
this doc).

## The three-category skip rule

Every `#[serde(skip)]` field must be justifiable in exactly one of:

- **(a) Pure per-tick scratch** — cleared/rebuilt before every read within a
  tick (e.g. `sensors`, `desired_direction`, `actions`, `combat_damaged`,
  `trade_routes`, `reproduced_this_tick`). Safe because no read can observe
  the reset-to-default state.
- **(b) Viewer/observability only, hash-excluded by design** — never read by
  the simulation at all (e.g. `combat_streaks`, `trade_routes`,
  `total_trades`, `codex_interval`).
- **(c) A cache derived from serialized state** — re-derived in
  `load_from_bytes` so the reloaded world matches the live one. **This is
  the dangerous category**: forgetting the re-derivation compiles fine, the
  load-identity hash matches, and the world diverges one step later.

## Current skip inventory

| Field | Category | Notes |
|---|---|---|
| `codex_interval` | (b) runtime cadence knob; resets to every-tick on load |
| `spatial`, `carcass_spatial`, `resource_spatial` | (a) rebuilt each tick **+ (c)** dims re-derived in `load_from_bytes` |
| `sensors`, `desired_direction`, `actions` | (a) rewritten by sense/decide each tick |
| `codex_agg` | (a) rebuilt at the top of every `observe_all` |
| `reproduced_this_tick` | (a) cleared at the start of `reproduce_all` |
| `combat_damaged`, `combat_attacker` | (a) reset each tick in `interact_all` |
| `combat_streaks`, `trade_routes` | (b) viewer-only per-tick buffers |
| `total_trades` | (b) HUD tally; resets to zero on load |
| `agents.scratch_ids` | (a) take/restore scratch in `culture_step` |
| `agents.track_livestock` | (c) re-derived from `domestication_enabled` at load |
| `sense::SensorRegister` | (a) rewritten by `sense_all` |
| `pheromones.nonzero` | (c) re-derived by `refresh_nonzero()` at load |
| `codex::SpeciesAggTable` | (a) see `codex_agg` |

`CodexState` has **zero** skips — detector state is always serialized.
`still_ticks` and `prev_desired_direction` are path-dependent accumulators
that feed serialized codex state, so they are **serialized** (not skipped) —
the v13 lesson.

## Load-time re-derivations (load-bearing)

`load_from_bytes` currently re-derives, in order:

1. `world.pheromones.refresh_nonzero()` — the decay fast-path cache.
2. `world.agents.track_livestock = world.domestication_enabled` — the orphan
   cleanup gate.
3. `world.{spatial, carcass_spatial, resource_spatial}` —
   `UniformSpatialHash::with_dims(world.world_size, world.hash_res)`,
   mirroring `World::with_dims`. Without this, a custom-dims world reloads
   with the default 1024/64 grid: wrong `cell_size` (clamps every perception
   radius wrong) and wrong torus extent. Found by
   `tests/save_load_roundtrip.rs` (`season_period_roundtrip`,
   `living_biome_roundtrip`); pinned by
   `tests/serde_skip_audit.rs::load_rederives_spatial_hash_dims`.

## Checklist: adding a subsystem

1. New persistent state → serialize it (bincode layout grows → bump
   `FORMAT_VERSION` with a changelog line, refresh layout goldens).
2. New `#[serde(skip)]` field → justify it in category (a), (b), or (c)
   above; add it to the inventory table.
3. Category (c) → add the re-derivation to `load_from_bytes` **and** a guard
   in `tests/serde_skip_audit.rs`.
4. New opt-in flag → add a round-trip test to
   `tests/save_load_roundtrip.rs` with a scenario that enables it, warmed
   enough that the subsystem's state is non-trivial (cross-check the
   subsystem's own integration test tick range — a too-short warm-up is
   false-green).
5. Detector state lives in `CodexState` — keep it skip-free.
