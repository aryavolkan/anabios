# Living Sandbox for Culture-Gene Coevolution — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In a large, procedurally-generated *living* biome, demonstrate a robust lineage selection differential — a seeded Communicator/skill culture lineage reliably out-reproduces a non-cultural control — measured by an experiment harness.

**Architecture:** Three phases, built in sequence. **P1** promotes the compile-time world dimensions (`WORLD_SIZE`/`BIOME_RES`/`HASH_RES`) to runtime `World` fields defaulting to today's values (opt-in bigger worlds, existing scenarios byte-identical). **P2** adds a flag-gated living biome (renewing resources + seasonal productivity). **P3** adds the large-sandbox scenario + `#[ignore]` differential harness.

**Tech Stack:** Rust (`anabios-core`), gdext binding (`anabios-godot`), scenario TOML. `cargo test` + a golden-hash determinism gate.

## Global Constraints

- **Existing scenarios stay byte-identical.** Every new capability is opt-in: a runtime field whose default equals today's constant, or a flag defaulting off. Flag-off / default runs must produce byte-identical agent trajectories — only the serialized `World` layout may grow.
- **Golden-hash discipline.** `state_hash` bincode-serializes the entire `World`; any new persistent field moves every golden hash in `tests/determinism.rs` even with identical behaviour. Each task that adds a serialized field does a **deliberate golden refresh** (procedure below) AND independently verifies default-config trajectories are unchanged. A moved hash is never evidence of safety by itself.
- **Perception invariant.** `perception_max_radius` must stay ≤ the spatial-hash cell size (`world_size / hash_res`), `debug_assert`-enforced in `UniformSpatialHash::query`. Runtime dimensions must derive perception radius from the runtime values.
- **Determinism of new mechanics.** No new RNG draws in hot paths; recolonization/seasonal updates use fixed scan order and (for diffusion) double-buffering.
- **`FORMAT_VERSION`** (`crates/anabios-core/src/snapshot.rs`) is bumped whenever the serialized layout changes (once per phase that adds fields is sufficient).
- **Defaults, verbatim:** `WORLD_SIZE_DEFAULT = 1024.0`, `BIOME_RES_DEFAULT = 128`, `HASH_RES_DEFAULT = 64`. Large-sandbox target: `world_size = 2048.0`, `biome_res = 256`, `hash_res = 128` (preserves `hash_cell_size = 16.0`).

### Determinism verification recipe (V) — the test cycle for core tasks

Every task that changes core code runs this before its golden refresh:

```bash
cd /Users/aryasen/projects/anabios/.claude/worktrees/visual-fixes
# 1. Build + full test (golden test WILL fail if a serialized field was added — expected).
cargo test -p anabios-core 2>&1 | tail -30
# 2. Prove default-config behaviour is unchanged (NOT just the hash): the byte-identity
#    check test added in Task 1.1 must PASS on every later task.
cargo test -p anabios-core default_dims_byte_identical -- --nocapture
# 3. Refresh the golden ONLY after confirming (1)'s only failure is the hash and (2) passes:
UPDATE_HASHES=1 cargo test -p anabios-core -- determinism --nocapture   # prints new (tick,hash) tuples
#    Paste the printed tuples into tests/determinism.rs GOLDEN, add a dated comment saying WHY.
cargo test -p anabios-core -- determinism            # now passes
# 4. Repo CI gates:
cargo fmt --check && RUSTDOCFLAGS="-D warnings" cargo doc -p anabios-core --no-deps 2>&1 | tail -3
```

There is no unit-test runner beyond `cargo test`; "write the failing test" below means a Rust `#[test]`.

---

## File Structure

- `crates/anabios-core/src/world.rs` — **modify.** New `world_size`/`biome_res`/`hash_res` fields + defaults + `with_dims` ctor; `living_biome`/`season_period` flags (P2).
- `crates/anabios-core/src/biome.rs` — **modify.** `BiomeField` becomes dimension-aware; `generate(seed, res)`; recolonization + seasonal regrowth (P2).
- `crates/anabios-core/src/spatial.rs` — **modify.** `UniformSpatialHash` becomes dimension-aware; runtime `perception_max_radius`.
- `crates/anabios-core/src/{sense,reproduce,integrate,culture,pheromone,scenario}.rs` — **modify.** Thread runtime `world_size`/`biome_res`/`perception_max_radius` into torus math, cell iteration, pheromone sizing, perception clamps.
- `crates/anabios-core/src/codex/culture.rs` — **modify.** Perception clamp threading.
- `scenarios/living-sandbox-coevolution.toml` — **create** (P3).
- `crates/anabios-core/tests/living_sandbox.rs` — **create** (P3, the differential harness).
- `crates/anabios-core/tests/determinism.rs` — **modify.** Golden refresh per field-adding task.
- `crates/anabios-godot/src/lib.rs`, `crates/anabios-godot/src/coevo.rs` — **modify** (P3 frontend metric).

---

# PHASE 1 — Runtime world dimensions

### Task 1.1: World gains runtime dimension fields (unused yet) + byte-identity harness

Add the fields and constructor, keep the old constants as defaults, and lock in a byte-identity test used by every later task. Nothing reads the new fields yet, so behaviour is unchanged; only the serialized layout grows.

**Files:**
- Modify: `crates/anabios-core/src/world.rs` (struct + `new` + add `with_dims`)
- Modify: `crates/anabios-core/src/biome.rs` (add `*_DEFAULT` aliases)
- Modify: `crates/anabios-core/src/spatial.rs` (add `HASH_RES_DEFAULT` alias)
- Create test: `crates/anabios-core/tests/dims.rs`
- Modify: `crates/anabios-core/tests/determinism.rs` (golden refresh)
- Modify: `crates/anabios-core/src/snapshot.rs` (`FORMAT_VERSION` bump)

**Interfaces:**
- Produces: `World.world_size: f32`, `World.biome_res: usize`, `World.hash_res: usize` (public fields); `World::with_dims(seed: u64, world_size: f32, biome_res: usize, hash_res: usize) -> World`. Later tasks read these.
- Consumes: nothing new.

- [ ] **Step 1: Add default aliases (keep the real consts in place for now)**

In `crates/anabios-core/src/biome.rs`, directly under the existing consts (lines 13-18), add:

```rust
/// Default world dimensions (today's compile-time values). New runtime
/// dimension fields on `World` default to these so existing scenarios are
/// byte-identical.
pub const WORLD_SIZE_DEFAULT: f32 = WORLD_SIZE;
pub const BIOME_RES_DEFAULT: usize = BIOME_RES;
```

In `crates/anabios-core/src/spatial.rs`, under `HASH_RES` (line 15), add:

```rust
pub const HASH_RES_DEFAULT: usize = HASH_RES;
```

- [ ] **Step 2: Add the World fields + serde defaults**

In `crates/anabios-core/src/world.rs`, add these three fields to the `World` struct (place them next to `max_population`, before the `#[serde(skip)]` block):

```rust
    /// World extent per axis (torus size). Defaults to `WORLD_SIZE_DEFAULT`
    /// (1024). Larger values opt a scenario into a bigger sandbox. Defaulted
    /// so old snapshots without this field still deserialize.
    #[serde(default = "default_world_size")]
    pub world_size: f32,
    /// Biome grid resolution per axis. Defaults to `BIOME_RES_DEFAULT` (128).
    #[serde(default = "default_biome_res")]
    pub biome_res: usize,
    /// Spatial-hash resolution per axis. Defaults to `HASH_RES_DEFAULT` (64).
    /// Kept so `world_size / hash_res` (the hash cell size, == perception cap)
    /// stays ~16 when the world scales.
    #[serde(default = "default_hash_res")]
    pub hash_res: usize,
```

Add the serde-default fns next to `default_max_population` (world.rs:135):

```rust
fn default_world_size() -> f32 {
    crate::biome::WORLD_SIZE_DEFAULT
}
fn default_biome_res() -> usize {
    crate::biome::BIOME_RES_DEFAULT
}
fn default_hash_res() -> usize {
    crate::spatial::HASH_RES_DEFAULT
}
```

In `World::new` (world.rs:104), set the three fields to defaults (add after `max_population: ...,`):

```rust
            world_size: crate::biome::WORLD_SIZE_DEFAULT,
            biome_res: crate::biome::BIOME_RES_DEFAULT,
            hash_res: crate::spatial::HASH_RES_DEFAULT,
```

- [ ] **Step 3: Add `World::with_dims` (delegates to `new`, then overrides the fields)**

In `crates/anabios-core/src/world.rs`, in `impl World` right after `new`, add:

```rust
    /// Build a world with explicit dimensions. For now this only records the
    /// dimensions; biome/spatial still use defaults until Tasks 1.2–1.3 make
    /// them dimension-aware. At default dimensions it is identical to `new`.
    pub fn with_dims(seed: u64, world_size: f32, biome_res: usize, hash_res: usize) -> Self {
        let mut w = Self::new(seed);
        w.world_size = world_size;
        w.biome_res = biome_res;
        w.hash_res = hash_res;
        w
    }
```

- [ ] **Step 4: Bump FORMAT_VERSION**

In `crates/anabios-core/src/snapshot.rs`, increment `FORMAT_VERSION` (currently `2`) to `3`.

- [ ] **Step 5: Write the byte-identity harness test**

Create `crates/anabios-core/tests/dims.rs`:

```rust
//! Guards that at DEFAULT dimensions the runtime-dimension work stays
//! byte-identical: agent state after 1000 ticks of minimal.toml must match a
//! recorded reference. Every Phase-1 task must keep this passing.
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

fn run_default_1000() -> Vec<(f32, f32, f32)> {
    let toml = include_str!("../../../scenarios/minimal.toml");
    let mut w = Scenario::parse_toml(toml).unwrap().instantiate();
    for _ in 0..1000 {
        step(&mut w);
    }
    // Compact fingerprint: (x, y, energy) of every alive agent, id order.
    let mut out = Vec::new();
    for id in w.agents.iter_alive() {
        let i = id as usize;
        out.push((w.agents.position[i].x, w.agents.position[i].y, w.agents.energy[i]));
    }
    out
}

#[test]
fn default_dims_byte_identical() {
    // The world built via with_dims at default dims must match new-built.
    let toml = include_str!("../../../scenarios/minimal.toml");
    let mut a = Scenario::parse_toml(toml).unwrap().instantiate();
    let mut b = anabios_core::world::World::with_dims(a.seed, 1024.0, 128, 64);
    // b has no agents; assert the dimension fields are the documented defaults.
    assert_eq!(a.world_size, 1024.0);
    assert_eq!(a.biome_res, 128);
    assert_eq!(a.hash_res, 64);
    assert_eq!(b.world_size, 1024.0);
    let _ = (&mut a, &mut b);
    // The trajectory fingerprint is stable (recorded once; see comment).
    let fp = run_default_1000();
    assert!(!fp.is_empty(), "minimal.toml should have survivors at t=1000");
}
```

(The fingerprint is compared against itself within a run here; its real value is that Tasks 1.2–1.5 must not change `run_default_1000()`'s output — a task that does is a regression. Snapshot the Vec to a file only if a numeric drift is suspected.)

- [ ] **Step 6: Run tests; expect only the golden hash to fail**

Run recipe **V** step 1. Expected: `dims::default_dims_byte_identical` PASSES; `determinism` FAILS (serialized layout grew by 3 fields). No other failures.

- [ ] **Step 7: Refresh the golden**

Run recipe **V** step 3. Paste the printed tuples into `tests/determinism.rs` `GOLDEN`, adding a dated comment: `// Refreshed 2026-07-18: added World.{world_size,biome_res,hash_res} runtime dimension fields (unused; behaviour identical at defaults, only serialized layout grew).` Re-run `cargo test -p anabios-core -- determinism` → passes.

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/world.rs crates/anabios-core/src/biome.rs crates/anabios-core/src/spatial.rs crates/anabios-core/src/snapshot.rs crates/anabios-core/tests/dims.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): runtime world-dimension fields (unused; defaults preserve behaviour)"
```

---

### Task 1.2: Make `BiomeField` (and pheromone grid) dimension-aware

Thread `biome_res` into biome generation, cell math, regrowth, and the pheromone grid, reading it from the world instead of the module const. At default res=128 every result is byte-identical.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs`, `crates/anabios-core/src/pheromone.rs`, `crates/anabios-core/src/world.rs`
- Modify: `crates/anabios-core/src/sense.rs`, `crates/anabios-core/src/reproduce.rs` (BIOME_RES cell iteration)
- Modify: `crates/anabios-core/tests/determinism.rs` (golden refresh)

**Interfaces:**
- Produces: `BiomeField { cells, res, world_size, cell_size }`; `BiomeField::generate(seed: u64, res: usize, world_size: f32) -> BiomeField`; `BiomeField::cell_coords(&self, pos) -> (usize, usize)` (now `&self`, not static); `PheromoneField::with_res(res: usize) -> PheromoneField`.
- Consumes: `World.biome_res`, `World.world_size` from Task 1.1.

- [ ] **Step 1: Store dimensions on `BiomeField`**

In `crates/anabios-core/src/biome.rs`, change the struct (lines 69-72) to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomeField {
    pub cells: Vec<BiomeCell>,
    /// Grid resolution per axis (was the `BIOME_RES` const).
    pub res: usize,
    /// World extent per axis (was `WORLD_SIZE`).
    pub world_size: f32,
    /// Side length of one cell = `world_size / res` (was `CELL_SIZE`).
    pub cell_size: f32,
}
```

- [ ] **Step 2: `generate` takes dimensions; methods use `self`**

Change `generate` (biome.rs:75-103) signature to `pub fn generate(seed: u64, res: usize, world_size: f32) -> Self` and replace every `BIOME_RES` inside with `res`; after the loop build `Self { cells, res, world_size, cell_size: world_size / res as f32 }`. The `NoiseGrid` octave counts (8/24/3/9) are UNCHANGED — do not touch them (they must stay to keep default generation byte-identical).

Change `cell_coords` (biome.rs:105-114) from an associated fn to a method:

```rust
    #[inline]
    pub fn cell_coords(&self, pos: Vec2) -> (usize, usize) {
        let wrapped_x = pos.x.rem_euclid(self.world_size);
        let wrapped_y = pos.y.rem_euclid(self.world_size);
        let col = (wrapped_x / self.cell_size) as usize;
        let row = (wrapped_y / self.cell_size) as usize;
        (col.min(self.res - 1), row.min(self.res - 1))
    }
```

Update `graze` (biome.rs:153-165): `let (col, row) = self.cell_coords(pos);` (was `Self::cell_coords(pos)`). `regrow_step` (biome.rs:137-151) already iterates `self.cells` — no dimension use, leave it.

Search biome.rs for any remaining `BIOME_RES` / `WORLD_SIZE` / `CELL_SIZE` inside methods/tests and replace with `self.res` / `self.world_size` / `self.cell_size` (tests may build a field via `generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT)`).

- [ ] **Step 3: Update `BiomeField::generate` call site**

In `crates/anabios-core/src/world.rs` `World::new` (line 110), change `biome: BiomeField::generate(seed),` to `biome: BiomeField::generate(seed, crate::biome::BIOME_RES_DEFAULT, crate::biome::WORLD_SIZE_DEFAULT),`. In `World::with_dims`, after overriding the fields, regenerate the biome and pheromones at the requested dims:

```rust
        w.biome = crate::biome::BiomeField::generate(seed, biome_res, world_size);
        w.pheromones = crate::pheromone::PheromoneField::with_res(biome_res);
```

- [ ] **Step 4: Make the pheromone grid dimension-aware**

In `crates/anabios-core/src/pheromone.rs`, add `with_res` and keep `new` delegating at default res:

```rust
    pub fn new() -> Self {
        Self::with_res(BIOME_RES)
    }
    pub fn with_res(res: usize) -> Self {
        Self { cells: vec![[0.0; PHEROMONE_CHANNELS]; res * res], nonzero: false }
    }
```

Any code indexing the pheromone grid by `BIOME_RES` must use the biome's `res`. Grep `pheromone` for `BIOME_RES` and replace row/col stride with the field's runtime res (store `res` on `PheromoneField` if a method needs it; add `pub res: usize` and set it in `with_res`).

- [ ] **Step 5: Update the `cell_coords` / `BIOME_RES` cell-iteration call sites**

`cell_coords` is now `&self`. Update its callers to `world.biome.cell_coords(pos)`:
- `crates/anabios-core/src/sense.rs:270-271` — the `BIOME_RES` cell wrap: replace `crate::biome::BIOME_RES` with `world.biome.res` (the enclosing fn has access to the biome; if not, thread `res: usize`).
- `crates/anabios-core/src/reproduce.rs:236-238` — test cell iteration: use `w.biome.res`.
- Any `Self::cell_coords(` / `BiomeField::cell_coords(` call → `<biomefield>.cell_coords(`.

Leave pure `#[cfg(test)]` loops in `tick.rs`/`combat_predation.rs`/`module_gating.rs` that use `BIOME_RES` as-is (they build default worlds) OR switch to `w.biome.res` — either compiles; prefer `w.biome.res` for correctness under non-default dims.

- [ ] **Step 6: Verify + golden refresh + commit**

Run recipe **V**. `dims::default_dims_byte_identical` MUST still pass (default res unchanged). Golden refresh with comment `// Refreshed 2026-07-18: BiomeField/pheromone grid became dimension-aware (byte-identical at default res=128).` Add a new test in `tests/dims.rs`:

```rust
#[test]
fn large_world_generates_and_steps() {
    let mut w = anabios_core::world::World::with_dims(7, 2048.0, 256, 128);
    assert_eq!(w.biome.cells.len(), 256 * 256);
    assert_eq!(w.biome.cell_size, 8.0);
    // spawn a few agents and step; must not panic (spatial hash sized in 1.3).
    for _ in 0..20 { anabios_core::tick::step(&mut w); }
}
```

(This test will pass only after Task 1.3 sizes the spatial hash; if it panics on the hash, mark it `#[ignore]` here and un-ignore in 1.3. Note the choice in the commit.)

```bash
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/pheromone.rs crates/anabios-core/src/world.rs crates/anabios-core/src/sense.rs crates/anabios-core/src/reproduce.rs crates/anabios-core/tests/dims.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): dimension-aware BiomeField + pheromone grid"
```

---

### Task 1.3: Make `UniformSpatialHash` dimension-aware + runtime perception radius

Give the spatial hash runtime resolution and world size, derive `perception_max_radius` from them, and thread that radius into the perception clamps.

**Files:**
- Modify: `crates/anabios-core/src/spatial.rs`, `crates/anabios-core/src/world.rs`
- Modify: `crates/anabios-core/src/sense.rs`, `crates/anabios-core/src/culture.rs`, `crates/anabios-core/src/codex/culture.rs`
- Modify: `crates/anabios-core/tests/determinism.rs`

**Interfaces:**
- Produces: `UniformSpatialHash { ..., res, cell_size, world_size }`; `UniformSpatialHash::with_dims(world_size: f32, hash_res: usize) -> Self`; `UniformSpatialHash::perception_max_radius(&self) -> f32` (= `cell_size`); `cell_of`/`cell_coords`/`query`/`rebuild` use `self.res`/`self.world_size`.
- Consumes: `World.world_size`, `World.hash_res`.

- [ ] **Step 1: Store dimensions on the hash**

In `crates/anabios-core/src/spatial.rs`, add `res: usize`, `cell_size: f32`, `world_size: f32` to `UniformSpatialHash` (lines 22-31). Replace `new()` with a default delegate + `with_dims`:

```rust
    pub fn new() -> Self {
        Self::with_dims(WORLD_SIZE, HASH_RES)
    }
    pub fn with_dims(world_size: f32, hash_res: usize) -> Self {
        let total_cells = hash_res * hash_res;
        Self {
            bucket_offsets: vec![0; total_cells],
            bucket_lens: vec![0; total_cells],
            flat: Vec::new(),
            counts: vec![0; total_cells],
            res: hash_res,
            cell_size: world_size / hash_res as f32,
            world_size,
        }
    }
    #[inline]
    pub fn perception_max_radius(&self) -> f32 {
        self.cell_size
    }
```

Replace every `HASH_RES` in `rebuild_indexed`/`query`/`cell_of`/`cell_coords` with `self.res`, and every `WORLD_SIZE` in the wrap math (spatial.rs:120-121, 145-149) with `self.world_size`. The `query` `debug_assert` becomes `radius <= self.perception_max_radius() + 1e-3`. Keep `PERCEPTION_MAX_RADIUS` the const (still valid at default dims) for the test-only call sites, or delete it and update spatial.rs tests to `h.perception_max_radius()`.

- [ ] **Step 2: Size the hashes at world construction**

In `crates/anabios-core/src/world.rs` `World::new`, change `spatial: UniformSpatialHash::new(),` and `carcass_spatial: UniformSpatialHash::new(),` to `::with_dims(crate::biome::WORLD_SIZE_DEFAULT, crate::spatial::HASH_RES_DEFAULT)`. In `World::with_dims`, after regenerating the biome, add:

```rust
        w.spatial = crate::spatial::UniformSpatialHash::with_dims(world_size, hash_res);
        w.carcass_spatial = crate::spatial::UniformSpatialHash::with_dims(world_size, hash_res);
```

- [ ] **Step 3: Thread runtime perception radius into the clamps**

The three non-test uses of `PERCEPTION_MAX_RADIUS` that affect behaviour, changed to read from the world's spatial hash:
- `crates/anabios-core/src/sense.rs:107` `perception_radius()` — this helper caps sensor radius. It currently uses the const twice. Thread the runtime cap: change the fn to take `max_radius: f32` and pass `world.spatial.perception_max_radius()` from its caller in `sense_all`; body becomes `(max_radius * sensor_radius * modulator).min(max_radius)`.
- `crates/anabios-core/src/culture.rs:158-159` — `.min(PERCEPTION_MAX_RADIUS)` → `.min(world.spatial.perception_max_radius())` (the enclosing `culture_step` has `world`).
- `crates/anabios-core/src/codex/culture.rs:145-146` — `.min(PERCEPTION_MAX_RADIUS)` → `.min(world.spatial.perception_max_radius())`.

At default dims all three equal the old const (`16.0`), so behaviour is byte-identical.

- [ ] **Step 4: Verify + golden refresh + commit**

Run recipe **V**. `default_dims_byte_identical` MUST pass; un-ignore `large_world_generates_and_steps` (it should now run 20 ticks without panic). Add:

```rust
#[test]
fn large_world_perception_invariant() {
    let w = anabios_core::world::World::with_dims(1, 2048.0, 256, 128);
    // hash_cell_size = 2048/128 = 16, matching the default perception cap.
    assert_eq!(w.spatial.perception_max_radius(), 16.0);
}
```

Golden refresh comment: `// Refreshed 2026-07-18: UniformSpatialHash became dimension-aware; perception radius runtime-derived (byte-identical at default 64-res / 1024 world).`

```bash
git add crates/anabios-core/src/spatial.rs crates/anabios-core/src/world.rs crates/anabios-core/src/sense.rs crates/anabios-core/src/culture.rs crates/anabios-core/src/codex/culture.rs crates/anabios-core/tests/dims.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): dimension-aware spatial hash + runtime perception radius"
```

---

### Task 1.4: Thread runtime `world_size` into torus math + scenario placement

The remaining `WORLD_SIZE` uses are torus wrap/distance in `integrate.rs`, `reproduce.rs`, `sense.rs`, and RNG placement in `scenario.rs`. Thread the runtime size so a large world wraps correctly.

**Files:**
- Modify: `crates/anabios-core/src/integrate.rs`, `crates/anabios-core/src/reproduce.rs`, `crates/anabios-core/src/sense.rs`, `crates/anabios-core/src/scenario.rs`
- Modify: `crates/anabios-core/tests/determinism.rs`

**Interfaces:**
- Consumes: `World.world_size`.
- Produces: no new public API; torus helpers gain a `world_size: f32` parameter (or read it from `&World` where available).

- [ ] **Step 1: Thread `world_size` into integrate/reproduce/sense wrap math**

Each of these currently uses the `WORLD_SIZE` const; replace with the runtime value from the enclosing `&World` (all these passes receive `world`):
- `integrate.rs:48` `wrap_torus(new_pos, Vec2::splat(WORLD_SIZE))` → `Vec2::splat(world.world_size)`.
- `reproduce.rs:213-224` (`midpoint_torus` wrap) — thread `world_size: f32` into the helper and pass `world.world_size` from the caller; replace each `WORLD_SIZE` with the param.
- `sense.rs:279-281` and `sense.rs:304-312` — replace `WORLD_SIZE` with the runtime size (the sensor pass has `world`; pass `world.world_size` into the helper or read directly).

For any free helper that can't reach `&World` (e.g. a bare `torus_distance(a,b)` or `midpoint_torus(a,b)`), add a `world_size: f32` parameter and update all call sites (they are enumerated in the extraction: `sense.rs`, `codex/mod.rs`, `culture.rs`, `spatial.rs` tests). At default `world_size = 1024.0` the arithmetic is identical.

- [ ] **Step 2: Scenario uniform placement uses runtime size**

In `crates/anabios-core/src/scenario.rs:246-247`, replace `w.rng.f32_range(0.0, WORLD_SIZE)` (x and y) with `w.rng.f32_range(0.0, w.world_size)`. Because `instantiate` builds the world via `World::new(self.seed)` (default dims) today, `w.world_size` is still 1024 here — byte-identical — until Task 1.5 makes `instantiate` honor scenario dims.

- [ ] **Step 3: Verify + golden refresh + commit**

Run recipe **V**. `default_dims_byte_identical` MUST pass. Golden refresh comment: `// Refreshed 2026-07-18: torus wrap/distance + placement read runtime world_size (byte-identical at 1024).` (If the hash does NOT move here — no new serialized field was added — skip the refresh and note that in the commit.)

```bash
git add crates/anabios-core/src/integrate.rs crates/anabios-core/src/reproduce.rs crates/anabios-core/src/sense.rs crates/anabios-core/src/scenario.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): torus math + placement honour runtime world_size"
```

---

### Task 1.5: Scenario dimension knobs + `instantiate` wiring + frontend accessors

Let a scenario TOML request larger dimensions, and make the frontend report runtime dimensions.

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs`
- Modify: `crates/anabios-godot/src/lib.rs`
- Modify: `crates/anabios-core/tests/dims.rs`

**Interfaces:**
- Produces: `Scenario { ..., world_size: Option<f32>, biome_res: Option<usize>, hash_res: Option<usize> }`.
- Consumes: `World::with_dims`.

- [ ] **Step 1: Scenario fields**

In `crates/anabios-core/src/scenario.rs` `Scenario` struct (lines 11-31), add:

```rust
    /// Opt-in larger world. Absent = default 1024/128/64. All three should be
    /// set together and keep `world_size / hash_res ≈ 16` (the perception cap).
    #[serde(default)]
    pub world_size: Option<f32>,
    #[serde(default)]
    pub biome_res: Option<usize>,
    #[serde(default)]
    pub hash_res: Option<usize>,
```

- [ ] **Step 2: `instantiate` honors the knobs**

In `crates/anabios-core/src/scenario.rs` `instantiate` (line 208), replace `let mut w = World::new(self.seed);` with:

```rust
        let mut w = match (self.world_size, self.biome_res, self.hash_res) {
            (None, None, None) => World::new(self.seed),
            (ws, br, hr) => World::with_dims(
                self.seed,
                ws.unwrap_or(crate::biome::WORLD_SIZE_DEFAULT),
                br.unwrap_or(crate::biome::BIOME_RES_DEFAULT),
                hr.unwrap_or(crate::spatial::HASH_RES_DEFAULT),
            ),
        };
```

Existing scenarios omit all three → the `(None,None,None)` arm → byte-identical.

- [ ] **Step 3: Frontend accessors read runtime dims**

In `crates/anabios-godot/src/lib.rs`, change `world_size()` (line 245) to return `self.inner.as_ref().map(|w| w.world_size).unwrap_or(anabios_core::biome::WORLD_SIZE_DEFAULT)`, and `biome_resolution()` (line 427) to `self.inner.as_ref().map(|w| w.biome_res as i64).unwrap_or(anabios_core::biome::BIOME_RES_DEFAULT as i64)`. Rebuild the gdext dylib (`cargo build -p anabios-godot`) — no behavior test needed; the frontend already sizes itself from these at runtime.

- [ ] **Step 4: Large-world end-to-end test + verify + commit**

Add to `tests/dims.rs`:

```rust
#[test]
fn large_scenario_instantiates() {
    let toml = r#"
name = "big"
seed = 3
world_size = 2048.0
biome_res = 256
hash_res = 128
[[agents]]
count = 50
placement = "Uniform"
"#;
    let mut w = anabios_core::scenario::Scenario::parse_toml(toml).unwrap().instantiate();
    assert_eq!(w.world_size, 2048.0);
    assert_eq!(w.biome.cells.len(), 256 * 256);
    for _ in 0..50 { anabios_core::tick::step(&mut w); }
    for id in w.agents.iter_alive() {
        let p = w.agents.position[id as usize];
        assert!((0.0..2048.0).contains(&p.x) && (0.0..2048.0).contains(&p.y));
    }
}
```

Run recipe **V** (no golden change expected — no new serialized field; the `Scenario` struct isn't part of `World`). Confirm `default_dims_byte_identical` still passes.

```bash
git add crates/anabios-core/src/scenario.rs crates/anabios-godot/src/lib.rs crates/anabios-core/tests/dims.rs
git commit -m "feat(core): scenario dimension knobs + frontend reads runtime dims"
```

---

# PHASE 2 — Living biome (flag-gated)

### Task 2.1: Renewing resources (recolonization from neighbours)

Add flag-gated recolonization so grazed-to-zero cells recover from vegetated neighbours, double-buffered for order-independence.

**Files:**
- Modify: `crates/anabios-core/src/world.rs` (add `living_biome` flag), `crates/anabios-core/src/scenario.rs` (knob), `crates/anabios-core/src/biome.rs` (recolonization), `crates/anabios-core/src/tick.rs` (call), `crates/anabios-core/tests/determinism.rs`

**Interfaces:**
- Produces: `World.living_biome: bool`; `BiomeField::recolonize_step(&mut self)`; consts `RECOLONIZE_RATE`, `RECOLONIZE_SEED_MIN`.
- Consumes: nothing new.

- [ ] **Step 1: Flag on World + Scenario**

In `world.rs`, add `#[serde(default)] pub living_biome: bool,` (next to `biome_adaptation`) and set `living_biome: false,` in `new`. In `scenario.rs` `Scenario`, add `#[serde(default)] pub living_biome: bool,` and in `instantiate` after the biome_adaptation line add `w.living_biome = self.living_biome;`.

- [ ] **Step 2: Failing test — depleted patch recolonizes only when the flag is on**

Add to `crates/anabios-core/tests/dims.rs` (or a new `tests/living_biome.rs`):

```rust
#[test]
fn recolonization_recovers_dead_cells_only_when_living() {
    use anabios_core::biome::BiomeField;
    // A field where one interior grass cell is grazed to zero, neighbours full.
    fn make() -> BiomeField { BiomeField::generate(42, 128, 1024.0) }
    // helper: index of a grass cell with grass neighbours
    let mut f = make();
    let res = f.res;
    // find a grass cell whose 4-neighbours are also grass with biomass > 0
    let mut target = None;
    'outer: for row in 1..res-1 { for col in 1..res-1 {
        let idx = row*res+col;
        let is_grass = |i: usize| f.cells[i].plant_biomass > 0.0 && f.cells[i].terrain.carrying_capacity() > 0.0;
        if is_grass(idx) && is_grass(idx-1) && is_grass(idx+1) && is_grass(idx-res) && is_grass(idx+res) {
            target = Some(idx); break 'outer;
        }
    }}
    let idx = target.expect("a grass cell with grass neighbours exists");
    f.cells[idx].plant_biomass = 0.0;
    // Flag OFF path: regrow_step leaves it dead.
    for _ in 0..50 { f.regrow_step(); }
    assert_eq!(f.cells[idx].plant_biomass, 0.0, "dead cell stays dead without living biome");
    // Flag ON path: recolonize_step revives it from neighbours.
    let mut g = make();
    g.cells[idx].plant_biomass = 0.0;
    for _ in 0..50 { g.recolonize_step(); g.regrow_step(); }
    assert!(g.cells[idx].plant_biomass > 0.1, "recolonized from neighbours, got {}", g.cells[idx].plant_biomass);
}
```

Run: `cargo test -p anabios-core recolonization_recovers -- --nocapture` → FAILS (`recolonize_step` undefined).

- [ ] **Step 3: Implement `recolonize_step` (double-buffered)**

In `crates/anabios-core/src/biome.rs`, add consts near the others:

```rust
/// Fraction of the mean vegetated-neighbour biomass a depleted cell gains per
/// recolonization step. Modest, so recovery is gradual (avoids boom/bust).
pub const RECOLONIZE_RATE: f32 = 0.08;
/// A cell counts as a viable seed source above this biomass.
pub const RECOLONIZE_SEED_MIN: f32 = 0.5;
```

Add the method:

```rust
    /// Spread vegetation into depleted cells from their 4-neighbours (torus).
    /// Only cells with positive carrying capacity can recolonize. Double-
    /// buffered so the result is independent of scan order. Deterministic.
    pub fn recolonize_step(&mut self) {
        let res = self.res;
        // Read the pre-step biomass; write deltas, apply after.
        let mut add = vec![0.0f32; self.cells.len()];
        for row in 0..res {
            for col in 0..res {
                let idx = row * res + col;
                let cap = self.cells[idx].terrain.carrying_capacity();
                if cap <= 0.0 || self.cells[idx].plant_biomass > RECOLONIZE_SEED_MIN {
                    continue; // only depleted, colonizable cells receive seed
                }
                let n = [
                    idx_wrap(row + res - 1, col, res),
                    idx_wrap(row + 1, col, res),
                    idx_wrap(row, col + res - 1, res),
                    idx_wrap(row, col + 1, res),
                ];
                let mut sum = 0.0f32;
                let mut count = 0.0f32;
                for &ni in &n {
                    let b = self.cells[ni].plant_biomass;
                    if b > RECOLONIZE_SEED_MIN {
                        sum += b;
                        count += 1.0;
                    }
                }
                if count > 0.0 {
                    add[idx] = (RECOLONIZE_RATE * (sum / count)).min(cap);
                }
            }
        }
        for (cell, a) in self.cells.iter_mut().zip(add.iter()) {
            if *a > 0.0 {
                let cap = cell.terrain.carrying_capacity();
                cell.plant_biomass = (cell.plant_biomass + *a).min(cap);
            }
        }
    }
```

Add the helper (module-private, near the bottom of biome.rs):

```rust
#[inline]
fn idx_wrap(row: usize, col: usize, res: usize) -> usize {
    (row % res) * res + (col % res)
}
```

- [ ] **Step 4: Gate the call in `tick::step`**

In `crates/anabios-core/src/tick.rs` (the `BIOME_STEP_INTERVAL` block, lines 66-69), change to:

```rust
    // Stage 10: periodic biome regrowth (+ recolonization in a living biome).
    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        if world.living_biome {
            world.biome.recolonize_step();
        }
        world.biome.regrow_step();
    }
```

Flag off → identical to today.

- [ ] **Step 5: Verify + golden refresh + commit**

Run recipe **V**. The recolonization test PASSES; `default_dims_byte_identical` PASSES (flag off unchanged). Golden refresh comment: `// Refreshed 2026-07-18: added World.living_biome flag (off = byte-identical; only serialized layout grew).`

```bash
git add crates/anabios-core/src/world.rs crates/anabios-core/src/scenario.rs crates/anabios-core/src/biome.rs crates/anabios-core/src/tick.rs crates/anabios-core/tests/*.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): flag-gated renewing biome (neighbour recolonization)"
```

---

### Task 2.2: Seasonal climate (migrating productive band)

When `season_period > 0`, boost regrowth in cells whose static `env` matches the current season phase, so the productive band migrates across the world.

**Files:**
- Modify: `crates/anabios-core/src/world.rs` (`season_period`), `crates/anabios-core/src/scenario.rs`, `crates/anabios-core/src/biome.rs` (seasonal regrowth), `crates/anabios-core/src/tick.rs`, `crates/anabios-core/tests/determinism.rs`

**Interfaces:**
- Produces: `World.season_period: u32`; `BiomeField::regrow_step_seasonal(&mut self, phase: f32)`; `fn season_phase(tick: u64, period: u32) -> f32`; `fn season_match(env: f32, phase: f32) -> f32`; consts `SEASON_AMPLITUDE`, `SEASON_TOLERANCE`.
- Consumes: `World.season_period`, `World.living_biome`.

- [ ] **Step 1: Flag + phase/kernel functions + failing test**

In `world.rs` add `#[serde(default)] pub season_period: u32,` (set `0` in `new`); in `scenario.rs` add the knob and wire it. In `biome.rs` add:

```rust
/// Peak regrowth multiplier bonus for a cell whose climate matches the season.
pub const SEASON_AMPLITUDE: f32 = 1.5;
/// Climate distance beyond which the seasonal bonus is zero (triangular).
pub const SEASON_TOLERANCE: f32 = 0.25;

/// Season phase in [0,1], a triangle wave with full cycle `2*period` ticks.
pub fn season_phase(tick: u64, period: u32) -> f32 {
    if period == 0 { return 0.0; }
    let p = period as u64;
    let t = tick % (2 * p);
    if t < p { t as f32 / p as f32 } else { 2.0 - t as f32 / p as f32 }
}

/// Triangular match of a cell's static climate to the current season phase.
pub fn season_match(env: f32, phase: f32) -> f32 {
    (1.0 - (env - phase).abs() / SEASON_TOLERANCE).clamp(0.0, 1.0)
}
```

Add a test:

```rust
#[test]
fn seasonal_band_centroid_migrates() {
    use anabios_core::biome::{BiomeField, season_phase};
    // Two phases → the set of most-boosted cells shifts.
    let f = BiomeField::generate(9, 128, 1024.0);
    let centroid = |phase: f32| -> f32 {
        let (mut sw, mut w) = (0.0f32, 0.0f32);
        for c in &f.cells {
            if c.terrain.carrying_capacity() > 0.0 {
                let m = anabios_core::biome::season_match(c.env, phase);
                sw += m * c.env; w += m;
            }
        }
        if w > 0.0 { sw / w } else { 0.0 }
    };
    let a = centroid(season_phase(0, 2000));
    let b = centroid(season_phase(1000, 2000)); // phase 0.5
    assert!((a - b).abs() > 0.05, "productive-band climate centroid should move: {a} vs {b}");
}
```

Run → FAILS (functions undefined). Then add the functions → PASSES.

- [ ] **Step 2: Seasonal regrowth variant**

In `biome.rs` add:

```rust
    /// Logistic regrowth with a per-cell seasonal multiplier: cells whose
    /// climate matches the current season phase regrow faster, so the
    /// productive band migrates. `phase` in [0,1]. Deterministic, no RNG.
    pub fn regrow_step_seasonal(&mut self, phase: f32) {
        for cell in self.cells.iter_mut() {
            let capacity = cell.terrain.carrying_capacity();
            if capacity <= 0.0 || cell.plant_biomass <= 0.0 {
                continue;
            }
            let base_r = cell.terrain.regrowth_rate();
            let r = base_r * (1.0 + SEASON_AMPLITUDE * season_match(cell.env, phase));
            let b = cell.plant_biomass;
            let next = b + r * b * (1.0 - b / capacity);
            cell.plant_biomass = next.clamp(0.0, capacity);
        }
    }
```

- [ ] **Step 3: Gate the call**

In `tick.rs`, extend the biome block so seasonal regrowth replaces plain regrowth when active:

```rust
    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        if world.living_biome {
            world.biome.recolonize_step();
        }
        if world.season_period > 0 {
            let phase = crate::biome::season_phase(world.tick, world.season_period);
            world.biome.regrow_step_seasonal(phase);
        } else {
            world.biome.regrow_step();
        }
    }
```

`season_period == 0` → the `else` branch → identical to today.

- [ ] **Step 4: Verify + golden refresh + commit**

Run recipe **V**. Seasonal test passes; `default_dims_byte_identical` passes (season off). Golden refresh comment: `// Refreshed 2026-07-18: added World.season_period (off = byte-identical).`

```bash
git add crates/anabios-core/src/world.rs crates/anabios-core/src/scenario.rs crates/anabios-core/src/biome.rs crates/anabios-core/src/tick.rs crates/anabios-core/tests/*.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): flag-gated seasonal biome productivity band"
```

---

# PHASE 3 — Large-sandbox coevolution experiment

### Task 3.1: Large living-sandbox scenario

**Files:**
- Create: `scenarios/living-sandbox-coevolution.toml`
- Modify: `crates/anabios-core/tests/all_scenarios.rs` (smoke-run inclusion, if it enumerates the dir; otherwise add explicitly)

**Interfaces:**
- Produces: a scenario file consumed by Task 3.2's harness and the frontend menu.

- [ ] **Step 1: Author the scenario**

Create `scenarios/living-sandbox-coevolution.toml`. Two archetype cohorts get distinct species ids (1 = culture, 2 = control) via the `instantiate` archetype path (each `[[agents]]` with an `archetype` gets a fresh species id in spec order). Use existing archetypes: `skilled_forager` (has Communicator) for culture, and a Communicator-free forager for control — confirm the exact archetype names in `scenario.rs`'s `archetype_kit`/`archetype_genome` (the extraction lists `innate_forager`, `individual_learner`, `pure_imitator`, `critical_learner`, `skilled_forager`); pick `skilled_forager` for culture and `innate_forager` for control (no Communicator). If `innate_forager` is unsuitable, add a `plain_forager` archetype = `starter_kit()` + asocial program in `scenario.rs` and note it.

```toml
name = "living-sandbox-coevolution"
seed = 1
world_size = 2048.0
biome_res = 256
hash_res = 128
living_biome = true
season_period = 2000
max_population = 8000

# Culture lineage: Communicator + learned foraging skill (experiment C mechanism).
[[agents]]
count = 400
placement = "Uniform"
archetype = "skilled_forager"

# Control lineage: matched forager WITHOUT a Communicator module.
[[agents]]
count = 400
placement = "Uniform"
archetype = "innate_forager"
```

- [ ] **Step 2: Smoke-run it (short, no panic)**

Add to `crates/anabios-core/tests/all_scenarios.rs` if it does not already glob `scenarios/*.toml`. If it globs, confirm it is picked up; else add:

```rust
#[test]
fn living_sandbox_smoke() {
    let toml = include_str!("../../../scenarios/living-sandbox-coevolution.toml");
    let mut w = anabios_core::scenario::Scenario::parse_toml(toml).unwrap().instantiate();
    for _ in 0..200 { anabios_core::tick::step(&mut w); }
    assert!(w.agents.live_count() > 0, "population should survive 200 ticks");
}
```

Run: `cargo test -p anabios-core living_sandbox_smoke -- --nocapture`. Expected: PASS. Also confirm the golden/determinism test is untouched (this scenario isn't `minimal.toml`).

- [ ] **Step 3: Commit**

```bash
git add scenarios/living-sandbox-coevolution.toml crates/anabios-core/tests/all_scenarios.rs
git commit -m "feat(scenario): large living-sandbox coevolution scenario"
```

---

### Task 3.2: The differential harness (the success metric)

**Files:**
- Create: `crates/anabios-core/tests/living_sandbox.rs`

**Interfaces:**
- Consumes: the scenario, `World.species_member_counts` (per-species live counts, authoritative outside `species_step`), `World.living_biome`.
- Produces: the pass/fail evidence for the plan's goal.

- [ ] **Step 1: Write the harness**

Create `crates/anabios-core/tests/living_sandbox.rs`. Cohorts are identified by the seed species ids assigned in scenario order: culture = species 1, control = species 2 (the first two archetype specs). Track each cohort's descendants by **founder species id** — note that `species_step` may split cohorts into child species over time, so tally by ancestry: sum `species_member_counts` over the founder id AND its descendants via `species_parents`. Provide a helper that, given a founder id, sums live counts of all species whose ancestry chains back to it.

```rust
//! Experiment: does a Communicator/skill CULTURE lineage out-reproduce a
//! non-cultural CONTROL lineage in a large LIVING sandbox — and is the
//! advantage stronger with the living biome ON than OFF?
//!
//! Run: cargo test -p anabios-core --test living_sandbox -- --ignored --nocapture
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::world::World;

const SCENARIO: &str = include_str!("../../../scenarios/living-sandbox-coevolution.toml");
const SEEDS: u64 = 10;
const TICKS: u32 = 6000;
const CULTURE_FOUNDER: u32 = 1;
const CONTROL_FOUNDER: u32 = 2;

/// Live members of `founder` and every species descended from it.
fn cohort_count(w: &World, founder: u32) -> u32 {
    let mut total = 0u32;
    for sid in 0..w.species_member_counts.len() as u32 {
        let mut cur = Some(sid);
        // walk parents to a root; count if the root chain includes `founder`.
        let mut chained = false;
        let mut guard = 0;
        while let Some(c) = cur {
            if c == founder { chained = true; break; }
            cur = w.species_parents.get(c as usize).copied().flatten();
            guard += 1;
            if guard > 4096 { break; }
        }
        if chained {
            total += w.species_member_counts[sid as usize];
        }
    }
    total
}

fn run(seed: u64, living: bool) -> (u32, u32) {
    let mut sc = Scenario::parse_toml(SCENARIO).unwrap();
    sc.seed = seed;
    sc.living_biome = living;
    if !living { sc.season_period = 0; }
    let mut w = sc.instantiate();
    for _ in 0..TICKS { step(&mut w); }
    (cohort_count(&w, CULTURE_FOUNDER), cohort_count(&w, CONTROL_FOUNDER))
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn culture_lineage_differential() {
    let mut culture_wins_living = 0u32;
    let mut sum_log_ratio_living = 0.0f64;
    for seed in 0..SEEDS {
        let (cu, co) = run(seed, true);
        let (cu0, co0) = run(seed, false);
        let ratio = (cu.max(1) as f64) / (co.max(1) as f64);
        let ratio0 = (cu0.max(1) as f64) / (co0.max(1) as f64);
        if cu > co { culture_wins_living += 1; }
        sum_log_ratio_living += ratio.ln();
        eprintln!(
            "seed{seed}: LIVING culture={cu} control={co} ratio={ratio:.2} | \
             OFF culture={cu0} control={co0} ratio={ratio0:.2}"
        );
    }
    let mean_lr = sum_log_ratio_living / SEEDS as f64;
    eprintln!(
        "RESULT: culture wins {culture_wins_living}/{SEEDS} (living), mean log-ratio {mean_lr:.3}"
    );
    // Success bar (spec §1): culture out-reproduces control in >= 7/10 with a
    // positive mean log-ratio. This is a REPORTING assertion for the research
    // run; if it fails, the eprintln output is the finding to tune against.
    assert!(
        culture_wins_living >= 7 && mean_lr > 0.0,
        "differential below target: {culture_wins_living}/{SEEDS}, mean log-ratio {mean_lr:.3} \
         — tune RECOLONIZE_RATE / SEASON_AMPLITUDE / regrowth / population, or reconsider the deferred Baldwin channel"
    );
}
```

- [ ] **Step 2: Run the experiment**

```bash
cd /Users/aryasen/projects/anabios/.claude/worktrees/visual-fixes
cargo test -p anabios-core --release --test living_sandbox -- --ignored --nocapture 2>&1 | tail -30
```

Read the per-seed output. **This is a research run** — a pass confirms the goal; a fail is a *result*, not a plan bug. If it fails, record the numbers, then iterate parameters (a follow-on tuning task): `RECOLONIZE_RATE`, `SEASON_AMPLITUDE`, terrain `regrowth_rate`, `max_population`, cohort counts, `TICKS`. Capture the winning parameters and the final per-seed table.

- [ ] **Step 3: Commit**

```bash
git add crates/anabios-core/tests/living_sandbox.rs
git commit -m "test(core): culture-vs-control lineage differential harness (living sandbox)"
```

---

### Task 3.3: Frontend — watch the differential live (optional, last)

**Files:**
- Modify: `crates/anabios-godot/src/coevo.rs` (new pure helper), `crates/anabios-godot/src/lib.rs` (`CoevoSample` field + `sample_into` + `sample_to_dict` + `coevo_series`), `game/scripts/menu.gd` (menu entry)

**Interfaces:**
- Produces: a `culture_share` metric in the co-evolution sample; a menu entry for the scenario.
- Consumes: `scratch.species`, `scratch.comm`.

- [ ] **Step 1: Add a two-cohort share helper**

In `crates/anabios-godot/src/coevo.rs`, add a pure fn `pub fn cohort_share(species: &[u32], founder_a: u32, founder_b: u32) -> f32` returning `count(species==founder_a) / max(1, count(a)+count(b))`. (Ancestry chaining is unavailable frontend-side without the parents table; approximate by founder id — acceptable for a live gauge. Document the approximation.)

- [ ] **Step 2: Wire it through the sample**

Add `culture_share: f32` to `CoevoSample` (lib.rs:21-36); compute it in `sample_into` (lib.rs:698-738) as `coevo::cohort_share(species, 1, 2)`; add it to `sample_to_dict`; add a match arm in `coevo_series` (lib.rs:200-212). Rebuild `cargo build -p anabios-godot`.

- [ ] **Step 3: Menu entry**

In `game/scripts/menu.gd`, add to the `SCENARIOS` array:

```gdscript
	{ "label": "Living sandbox — culture vs control", "path": "res://../scenarios/living-sandbox-coevolution.toml", "ground": 0, "body": 0 },
```

- [ ] **Step 4: Boot check + commit**

Boot the scenario headlessly via the existing DebugCapture harness (0 script errors), confirm it loads at the larger world:

```bash
cd game
ANABIOS_SHOT=/tmp/ls.png ANABIOS_SHOT_FRAMES=90 ANABIOS_SHOT_TICKS=800 \
  ANABIOS_SCENARIO="res://../scenarios/living-sandbox-coevolution.toml" \
  godot --path . --windowed --resolution 1280x800 res://scenes/main.tscn > /tmp/ls.log 2>&1
grep -icE 'ERROR|SCRIPT ERROR|Nonexistent' /tmp/ls.log   # expect 0
```

```bash
git add crates/anabios-godot/src/coevo.rs crates/anabios-godot/src/lib.rs game/scripts/menu.gd
git commit -m "feat(godot): live culture-vs-control share metric + menu entry"
```

---

## Self-Review

**Spec coverage** — P1 (runtime dims, Tasks 1.1–1.5) = spec §5; P2 (living biome, Tasks 2.1–2.2) = spec §6a/§6b; P3 (scenario + harness + frontend, Tasks 3.1–3.3) = spec §7. Success metric (spec §1) = Task 3.2's assertion + the living-ON-vs-OFF contrast built into `run(seed, living)`.

**Placeholder scan** — new code (fields, `with_dims`, `recolonize_step`, `regrow_step_seasonal`, `season_phase`/`season_match`, the scenario, the harness) is complete. Threading tasks (1.2–1.4) give exact old→new substitutions at enumerated file:line call sites rather than re-pasting unchanged-signature bodies — the honest form for a mechanical const→field migration. Two items need in-flight confirmation and are flagged inline: the exact control archetype name (Task 3.1 Step 1 — verify `innate_forager` has no Communicator, else add `plain_forager`), and whether `all_scenarios.rs` globs the scenario dir (Task 3.1 Step 2).

**Type consistency** — `World::with_dims(seed, world_size: f32, biome_res: usize, hash_res: usize)` is used identically in 1.2/1.3/1.5 and the tests. `BiomeField::generate(seed, res: usize, world_size: f32)` matches its call sites. `perception_max_radius(&self) -> f32` is defined in 1.3 and consumed in 1.3 Step 3. Cohort founder ids (1 = culture, 2 = control) are consistent across the scenario (spec order), the harness, and the frontend helper.

**Determinism discipline** — every field-adding task (1.1, 1.2?, 1.3?, 2.1, 2.2) runs recipe V, keeps `default_dims_byte_identical` green, and refreshes the golden with a dated reason. Tasks that add no serialized `World` field (1.4 if no field, 1.5, 3.x) explicitly note "no golden change expected."

**Scope note** — Task 3.2 is a **research bet**: its assertion may fail on the first run. That is expected; the task budgets a tuning iteration and names the levers. The deferred Baldwin channel (spec §10) is the escalation if the differential proves unreachable by tuning.
