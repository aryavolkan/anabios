# Territory, Habitat & Collision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Behind a new opt-in `territory_enabled` flag, give every agent a heritable Land/Water/Air locomotion class that limits where it can move and graze, give each species a territory it roams inside with a soft-edge pull back, and stop agents from overlapping through separation steering plus a hard minimum-gap resolve.

**Architecture:** Three new pure-ish modules in `crates/anabios-core/src/`:
- `habitat.rs`: the locomotion class, terrain rules, movement gate and nearest-valid relocation.
- `territory.rs`: the per-species `Territory` record, `territory_step` and the pull.
- `collision.rs`: body radius, a fine 4-unit collision hash, the steering vector and the Jacobi resolve stage.

They plug into existing stages: sense (class-aware plant sensing), decide (pulls), integrate (gate), a new stage 4', feed_pass (grazing gate), the species step, the biome step and scenario instantiate. The flag off means zero RNG draws and byte-identical trajectories. A new serialized `World` field only grows the snapshot layout, so the goldens get regenerated once and a layout-independent trajectory guard proves nothing else moved.

**Tech Stack:** Rust (`anabios-core`, rayon, bincode, glam `Vec2`), `anabios-godot` gdext bridge, GDScript (Godot 4.7) viewer, gdtoolkit.

**Spec:** `docs/superpowers/specs/2026-09-25-territory-habitat-collision-design.md` (already amended with the "Spec amendments" below).

## Global Constraints

- **Byte-identical when off.** With the flag off, no stage may draw RNG or change arithmetic. Every new branch is gated on `territory_enabled` (or on an `Option` that is `None` when it is off).
- **Two trajectory guards must stay green in every task after Task 1.** These are `minimal_trajectory_unchanged_by_territory_substrate` and `grand_theater_trajectory_unchanged_by_territory_substrate`.
- **Snapshot format.** `FORMAT_VERSION` goes from 43 to 44, with a history note in `crates/anabios-core/src/snapshot.rs`.
- **Serialization rules.** Path-dependent state must be serialized. Only per-tick scratch may be `#[serde(skip)]`; this is the still-ticks v13 footgun.
- **Parallel stages.** They must write index-disjoint slots and read only snapshots of other agents' state, so the result is identical at 1 thread and at N threads.
- **Staging.** Never `git add -A` or `git add .`; stage explicit paths.
- **Local gate before pushing.**
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items`
  - `gdformat --check game/scripts/`
  - `gdlint game/scripts/`
- **Heavy suites run on PR CI.** Locally, run only the tests each task names. The full golden and determinism suite runs on PR CI.
- **Viewer line cap.** `game/scripts/*.gd` files must stay ≤ 1000 lines (gdlint max-file-lines).
- **Worktree safety.** Work only inside the worktree or branch checkout `claude/territory-habitat-collision`, and never `cd` into another checkout of the repo.

## Spec amendments (discovered while reading the code; already applied to the spec)

1. **Locomotion bands.** They are Water `< 0.25`, Land `[0.25, 0.75)`, Air `>= 0.75`. `Genome::neutral()` (all 0.5) is therefore Land, so the spec's "write 0.2 at spawn" override is not needed. Pins are `locomotion = 0.1` (water), `0.5` (land) and `0.9` (air), set through the existing `[agents.traits]` override table rather than a new string key.
2. **No cached `locomotion` column.** The class is `Locomotion::of(&genome)`, a single slot read, so nothing new is stored per agent and `AgentBuffers` keeps its layout.
3. **Territory centre tracks member positions**, averaged torus-safely around a reference point, instead of E8 anchors. There is no coupling to `settlement_enabled`, and anchor learning is unchanged.
4. **Collision uses its own fine hash** (`COLLISION_CELL = 4.0`), rebuilt at stage 1 and again inside the resolve, instead of accumulating in the sense loop. This leaves the sense hot loop (≈32% of the tick) untouched. The spec's "stale by ≤ 4" reasoning becomes "rebuilt fresh".
5. **Body radius** is `0.4 + 0.35 · Size`. `Size` is a genome value in `[0, 1]`, so the pair gap is at most 1.5 and stays below the 2-unit contact ranges.
6. **No valid cell:** the agent stays where it spawned and the gate keeps it stationary. No instantiate warning (YAGNI).
7. **Addition:** plant sensing (`best_plant_direction`, `local_plant_biomass`) becomes class-aware under the flag, so land agents don't chase aquatic biomass into the shoreline.
8. **Aquatic regrowth** is a dedicated `BiomeField::aquatic_regrow_step`. `TerrainType::carrying_capacity` is a `const fn` that can't see the flag, so it stays unchanged.

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `crates/anabios-core/src/habitat.rs` | create | `Locomotion` class, terrain rules, `gate_move`, `nearest_valid`, `pull_allowed`, `damp_locomotion_mutation` |
| `crates/anabios-core/src/territory.rs` | create | `Territory` record, `radius_for`, `territory_pull`, `territory_step` |
| `crates/anabios-core/src/collision.rs` | create | `body_radius`, collision hash rebuild, `separation_steer`, `resolve_overlaps` |
| `crates/anabios-core/src/lib.rs` | modify | `pub mod` the three new modules |
| `crates/anabios-core/src/genome.rs` | modify | rename slot 7 `_Reserved7` → `Locomotion` |
| `crates/anabios-core/src/world.rs` | modify | `territory_enabled`, `species_territories`, `collision_spatial`, `collision_scratch` |
| `crates/anabios-core/src/scenario.rs` | modify | `territory_enabled` field, `locomotion` trait override, aquatic seeding, founder relocation |
| `crates/anabios-core/src/snapshot.rs` | modify | `FORMAT_VERSION` 44 + history |
| `crates/anabios-core/src/spatial.rs` | modify | `UniformSpatialHash::res()` accessor |
| `crates/anabios-core/src/integrate.rs` | modify | habitat gate param |
| `crates/anabios-core/src/reproduce.rs` | modify | locomotion mutation damping, newborn relocation |
| `crates/anabios-core/src/biome.rs` | modify | aquatic constants, `seed_aquatic`, `aquatic_regrow_step` |
| `crates/anabios-core/src/sense.rs` | modify | class-aware plant sensing (`territory_enabled` param) |
| `crates/anabios-core/src/interact.rs` | modify | class-gated grazing |
| `crates/anabios-core/src/tick.rs` | modify | stage wiring, decide pulls |
| `scenarios/habitat-territories.toml` | create | flagship flag-on scenario (Land/Water/Air grazers) |
| `crates/anabios-core/tests/determinism.rs` | modify | trajectory guards, new golden, thread-count list |
| `crates/anabios-core/tests/invariants.rs` | modify | habitat property test + `#[ignore]` measurement probe |
| `crates/anabios-core/tests/save_load_roundtrip.rs` | modify | roundtrip entry |
| `crates/anabios-core/tests/{affect,affect_play,affect_social,cognition,inventions}.rs` | modify | golden regen (layout only) |
| `crates/anabios-core/benches/tick_bench.rs` | modify | 10k-agent territory-on tick bench |
| `crates/anabios-godot/src/lib.rs` | modify | `territory_active`, `alive_locomotion`, `species_territories` |
| `game/scripts/territory_layer.gd` | create | territory ring overlay |
| `game/scripts/test_territory_layer.gd` | create | headless test for the pure helper |
| `game/scripts/overlay_manager.gd`, `legend_panel.gd`, `agent_layer.gd`, `test_agent_layer.gd`, `main.gd` | modify | ground mode, airborne lift, wiring |
| `.github/workflows/ci.yml` | modify | add `test_territory_layer` to the smoke list |
| `docs/determinism-contract.md`, `docs/scenarios.md` | modify | new scratch fields, new scenario |

---

### Task 1: Trajectory guards (pin on unmodified code)

**Files:**
- Modify: `crates/anabios-core/tests/determinism.rs`

**Interfaces:**
- Produces: `trajectory_hash(&World) -> u64` (test-local) plus two pinned constants. Every later task must keep both tests green.

- [ ] **Step 1: Add the guard tests with placeholder zero pins**

Append to `crates/anabios-core/tests/determinism.rs`:

```rust
/// FNV-1a over `bincode(agents) ++ bincode(biome)`: the simulation trajectory
/// WITHOUT the `World` envelope. Adding a serialized `World` field (a layout
/// change) moves every `state_hash` golden but leaves this untouched, so it
/// separates "only the snapshot layout grew" from "behaviour changed".
/// Pinned before the territory/habitat/collision layer landed (flag off in
/// both scenarios); it must never move while that layer is off.
fn trajectory_hash(w: &anabios_core::world::World) -> u64 {
    let mut bytes = bincode::serialize(&w.agents).expect("agents serialize");
    bytes.extend(bincode::serialize(&w.biome).expect("biome serialize"));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

const MINIMAL_TRAJECTORY_AT_1000: u64 = 0x0;
const GRAND_THEATER_TRAJECTORY_AT_200: u64 = 0x0;

fn assert_trajectory(label: &str, src: &str, ticks: u64, pinned: u64) {
    let mut w = common::world(src);
    common::run(&mut w, ticks);
    let h = trajectory_hash(&w);
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// {label} trajectory at {ticks}: 0x{h:016x}");
        return;
    }
    assert_eq!(h, pinned, "{label}: trajectory moved with territory_enabled OFF");
}

#[test]
fn minimal_trajectory_unchanged_by_territory_substrate() {
    assert_trajectory("minimal", SCENARIO, 1000, MINIMAL_TRAJECTORY_AT_1000);
}

#[test]
fn grand_theater_trajectory_unchanged_by_territory_substrate() {
    assert_trajectory(
        "grand-theater",
        include_str!("../../../scenarios/grand-theater.toml"),
        200,
        GRAND_THEATER_TRAJECTORY_AT_200,
    );
}
```

- [ ] **Step 2: Capture the pins on the unmodified simulation**

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --release --test determinism trajectory_unchanged -- --nocapture`

Expected: two lines like `// minimal trajectory at 1000: 0x…`. Paste each value into its constant.

- [ ] **Step 3: Verify the guards pass**

Run: `cargo test -p anabios-core --release --test determinism trajectory_unchanged`

Expected: `2 passed`.

- [ ] **Step 4: Commit**

```bash
git add crates/anabios-core/tests/determinism.rs
git commit -m "test(determinism): layout-independent trajectory guards before territory layer

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `Locomotion` gene + habitat module (pure functions)

**Files:**
- Create: `crates/anabios-core/src/habitat.rs`
- Modify: `crates/anabios-core/src/lib.rs` (add `pub mod habitat;` in alphabetical position after `pub mod genome;`)
- Modify: `crates/anabios-core/src/genome.rs:42-43` (slot 7), `:188` (`SLOT_NAMES`), `slot_names_align_with_the_enum` test
- Modify: `crates/anabios-core/src/scenario.rs` `trait_overrides!` list

**Interfaces:**
- Produces:
  - `habitat::Locomotion { Land = 0, Water = 1, Air = 2 }` (`Copy`, `Serialize`, `Default = Land`)
  - `Locomotion::from_gene(f32) -> Locomotion`
  - `Locomotion::of(&Genome) -> Locomotion`
  - `.can_occupy(TerrainType) -> bool`, `.can_graze(TerrainType) -> bool`, `.collides_with(Locomotion) -> bool`, `.index() -> usize`
  - `habitat::gate_move(&BiomeField, Locomotion, pos: Vec2, v: Vec2) -> Vec2`
  - `habitat::nearest_valid(&BiomeField, pos: Vec2, Locomotion) -> Option<Vec2>`
  - `habitat::pull_allowed(&BiomeField, pos: Vec2, pull: Vec2, Option<Locomotion>) -> bool`
  - `habitat::damp_locomotion_mutation(before: f32, &mut Genome)`
  - Constants: `WATER_MAX = 0.25`, `AIR_MIN = 0.75`, `LOCOMOTION_MUTATION_SCALE = 0.1`, `RELOCATE_MAX_RING = 64`
  - `GenomeSlot::Locomotion = 7`
  - scenario trait `locomotion`

- [ ] **Step 1: Write the failing tests** (bottom of the new `habitat.rs`; create the file with only a `#[cfg(test)] mod tests` first so it fails to compile against missing items)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;
    use crate::world::World;

    /// 256-unit world, 32x32 cells of 8 units: all Grass, with columns
    /// `col >= 16` turned to Water (a straight north-south coastline at x=128).
    fn coast_world() -> World {
        let mut w = World::with_dims(1, 256.0, 32, 16);
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                w.biome.at_mut(col, row).terrain =
                    if col >= 16 { TerrainType::Water } else { TerrainType::Grass };
            }
        }
        w
    }

    #[test]
    fn bands_split_at_quarter_and_three_quarters() {
        assert_eq!(Locomotion::from_gene(0.0), Locomotion::Water);
        assert_eq!(Locomotion::from_gene(0.249), Locomotion::Water);
        assert_eq!(Locomotion::from_gene(0.25), Locomotion::Land);
        assert_eq!(Locomotion::from_gene(0.5), Locomotion::Land);
        assert_eq!(Locomotion::from_gene(0.749), Locomotion::Land);
        assert_eq!(Locomotion::from_gene(0.75), Locomotion::Air);
        assert_eq!(Locomotion::from_gene(1.0), Locomotion::Air);
        assert_eq!(Locomotion::of(&Genome::neutral()), Locomotion::Land, "neutral genome is Land");
    }

    #[test]
    fn terrain_rules() {
        use Locomotion::*;
        assert!(Land.can_occupy(TerrainType::Grass) && !Land.can_occupy(TerrainType::Water));
        assert!(Water.can_occupy(TerrainType::Water) && !Water.can_occupy(TerrainType::Grass));
        assert!(Air.can_occupy(TerrainType::Water) && Air.can_occupy(TerrainType::Rock));
        assert!(Water.can_graze(TerrainType::Water) && !Water.can_graze(TerrainType::Grass));
        assert!(Land.can_graze(TerrainType::Grass) && !Land.can_graze(TerrainType::Water));
        assert!(Air.can_graze(TerrainType::Grass) && !Air.can_graze(TerrainType::Water));
        assert!(Land.collides_with(Water) && Land.collides_with(Land));
        assert!(Air.collides_with(Air));
        assert!(!Air.collides_with(Land) && !Water.collides_with(Air));
    }

    #[test]
    fn gate_blocks_land_into_water_and_slides_along_the_coast() {
        let w = coast_world();
        let pos = Vec2::new(126.0, 100.0); // col 15 (grass), 2 units from the coast
        // Straight into the water: blocked entirely.
        assert_eq!(gate_move(&w.biome, Locomotion::Land, pos, Vec2::new(4.0, 0.0)), Vec2::ZERO);
        // Diagonal: the x part would enter water, the y part is fine -> slide.
        let v = gate_move(&w.biome, Locomotion::Land, pos, Vec2::new(2.8, 2.8));
        assert_eq!(v, Vec2::new(0.0, 2.8));
        // Air ignores terrain.
        let a = gate_move(&w.biome, Locomotion::Air, pos, Vec2::new(4.0, 0.0));
        assert_eq!(a, Vec2::new(4.0, 0.0));
        // Water can't climb out onto land.
        let wpos = Vec2::new(130.0, 100.0); // col 16 (water)
        assert_eq!(gate_move(&w.biome, Locomotion::Water, wpos, Vec2::new(-4.0, 0.0)), Vec2::ZERO);
    }

    #[test]
    fn nearest_valid_is_identity_on_valid_ground_and_finds_the_coast() {
        let w = coast_world();
        let land = Vec2::new(40.0, 40.0);
        assert_eq!(nearest_valid(&w.biome, land, Locomotion::Land), Some(land));
        // A water agent on land moves to the nearest water cell centre: col 16.
        let got = nearest_valid(&w.biome, Vec2::new(120.0, 100.0), Locomotion::Water).unwrap();
        assert_eq!(w.biome.sample(got).terrain, TerrainType::Water);
        assert!((got.x - 132.0).abs() < 1e-3, "col 16 centre is x=132, got {}", got.x);
        // Deterministic.
        assert_eq!(nearest_valid(&w.biome, Vec2::new(120.0, 100.0), Locomotion::Water), Some(got));
    }

    #[test]
    fn nearest_valid_is_none_when_the_class_has_no_habitat() {
        let mut w = World::with_dims(1, 256.0, 32, 16);
        for c in w.biome.cells.iter_mut() {
            c.terrain = TerrainType::Grass;
        }
        assert_eq!(nearest_valid(&w.biome, Vec2::new(10.0, 10.0), Locomotion::Water), None);
    }

    #[test]
    fn pull_allowed_masks_only_under_the_flag() {
        let w = coast_world();
        let pos = Vec2::new(126.0, 100.0);
        let seaward = Vec2::new(1.0, 0.0);
        assert!(pull_allowed(&w.biome, pos, seaward, None), "flag off: never masked");
        assert!(!pull_allowed(&w.biome, pos, seaward, Some(Locomotion::Land)));
        assert!(pull_allowed(&w.biome, pos, Vec2::new(-1.0, 0.0), Some(Locomotion::Land)));
        assert!(pull_allowed(&w.biome, pos, Vec2::ZERO, Some(Locomotion::Land)));
    }

    #[test]
    fn locomotion_mutation_is_damped_tenfold() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Locomotion, 0.6);
        damp_locomotion_mutation(0.5, &mut g);
        assert!((g.get(GenomeSlot::Locomotion) - 0.51).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p anabios-core --lib habitat`

Expected: compile error (`Locomotion`, `gate_move`, … not found; `GenomeSlot::Locomotion` not found).

- [ ] **Step 3: Rename the genome slot**

In `crates/anabios-core/src/genome.rs` replace:

```rust
    /// Reserved; formerly `ImmuneStrength`. No live behavior reads this slot.
    _Reserved7 = 7,
```

with:

```rust
    /// Heritable locomotion class (territory/habitat/collision layer), read as
    /// Water `< 0.25`, Land `[0.25, 0.75)`, Air `>= 0.75` by
    /// `habitat::Locomotion::from_gene`; neutral 0.5 is Land. Renamed in place
    /// from `_Reserved7` (formerly `ImmuneStrength`). Read only when
    /// `World::territory_enabled`; inert otherwise. Counts toward speciation
    /// distance, so lineages that change class split into their own species.
    Locomotion = 7,
```

In `SLOT_NAMES` replace `"reserved_7",` with `"Locomotion",`. In `slot_names_align_with_the_enum` add:

```rust
        assert_eq!(SLOT_NAMES[GenomeSlot::Locomotion.idx()], "Locomotion");
```

- [ ] **Step 4: Implement `habitat.rs`** (above the test module)

```rust
//! Habitat locomotion (territory/habitat/collision layer). Each agent's
//! heritable `GenomeSlot::Locomotion` gene reads as Land / Water / Air, which
//! decides the terrain it may occupy and graze and which bodies it collides
//! with. Pure functions only — every caller gates on `World::territory_enabled`,
//! so a flag-off world never reaches this module.

use serde::{Deserialize, Serialize};

use crate::biome::{BiomeField, TerrainType};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;

/// Genes below this read as Water.
pub const WATER_MAX: f32 = 0.25;
/// Genes at or above this read as Air.
pub const AIR_MIN: f32 = 0.75;
/// Scale applied to the Locomotion slot's per-birth mutation delta, so class
/// flips are rare (same RNG draw count — only the magnitude shrinks).
pub const LOCOMOTION_MUTATION_SCALE: f32 = 0.1;
/// Farthest ring (in biome cells) `nearest_valid` searches.
pub const RELOCATE_MAX_RING: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum Locomotion {
    #[default]
    Land = 0,
    Water = 1,
    Air = 2,
}

impl Locomotion {
    #[inline]
    pub fn from_gene(v: f32) -> Self {
        if v < WATER_MAX {
            Locomotion::Water
        } else if v >= AIR_MIN {
            Locomotion::Air
        } else {
            Locomotion::Land
        }
    }

    #[inline]
    pub fn of(g: &Genome) -> Self {
        Self::from_gene(g.get(GenomeSlot::Locomotion))
    }

    /// Whether this class may stand in a cell of terrain `t`.
    #[inline]
    pub fn can_occupy(self, t: TerrainType) -> bool {
        match self {
            Locomotion::Land => t != TerrainType::Water,
            Locomotion::Water => t == TerrainType::Water,
            Locomotion::Air => true,
        }
    }

    /// Whether this class may graze biomass in a cell of terrain `t`
    /// (flyers feed on land only).
    #[inline]
    pub fn can_graze(self, t: TerrainType) -> bool {
        match self {
            Locomotion::Water => t == TerrainType::Water,
            Locomotion::Land | Locomotion::Air => t != TerrainType::Water,
        }
    }

    /// Air bodies only collide with Air; Land and Water collide with each other.
    #[inline]
    pub fn collides_with(self, other: Locomotion) -> bool {
        (self == Locomotion::Air) == (other == Locomotion::Air)
    }

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Apply a proposed displacement `v` from `pos` under the habitat rule:
/// the full move if its destination is valid, else the x-only move, else the
/// y-only move (a slide along the coastline), else no move. Air is never
/// gated. Returns the displacement actually applied.
pub fn gate_move(biome: &BiomeField, class: Locomotion, pos: Vec2, v: Vec2) -> Vec2 {
    if class == Locomotion::Air {
        return v;
    }
    let ok = |d: Vec2| class.can_occupy(biome.sample(pos + d).terrain);
    if ok(v) {
        v
    } else if ok(Vec2::new(v.x, 0.0)) {
        Vec2::new(v.x, 0.0)
    } else if ok(Vec2::new(0.0, v.y)) {
        Vec2::new(0.0, v.y)
    } else {
        Vec2::ZERO
    }
}

/// `pos` itself when its cell is valid for `class`; otherwise the centre of
/// the closest valid cell on the first ring (Chebyshev radius 1, 2, …, up to
/// `RELOCATE_MAX_RING`) that has one. Within a ring the Euclidean-closest
/// cell wins, first-seen on ties (fixed scan order ⇒ deterministic, no RNG).
/// `None` if no valid cell is within reach.
pub fn nearest_valid(biome: &BiomeField, pos: Vec2, class: Locomotion) -> Option<Vec2> {
    if class.can_occupy(biome.sample(pos).terrain) {
        return Some(pos);
    }
    let (cx, cy) = biome.cell_coords(pos);
    let res = biome.res as i32;
    let max_ring = RELOCATE_MAX_RING.min(biome.res / 2) as i32;
    for k in 1..=max_ring {
        let mut best: Option<(f32, Vec2)> = None;
        for dy in -k..=k {
            for dx in -k..=k {
                if dx.abs() != k && dy.abs() != k {
                    continue;
                }
                let col = (cx as i32 + dx).rem_euclid(res) as usize;
                let row = (cy as i32 + dy).rem_euclid(res) as usize;
                if !class.can_occupy(biome.at(col, row).terrain) {
                    continue;
                }
                let off = biome.cell_offset_from(col, row, pos);
                let d2 = off.length_squared();
                if best.is_none_or(|(bd, _)| d2 < bd) {
                    best = Some((d2, off));
                }
            }
        }
        if let Some((_, off)) = best {
            let ws = biome.world_size;
            let p = pos + off;
            return Some(Vec2::new(p.x.rem_euclid(ws), p.y.rem_euclid(ws)));
        }
    }
    None
}

/// Whether a habitat-selection pull (`pull`, any length) may apply: the cell
/// one cell-width along it must be valid for `class`. `None` (flag off)
/// always allows, so flag-off arithmetic is untouched.
pub fn pull_allowed(biome: &BiomeField, pos: Vec2, pull: Vec2, class: Option<Locomotion>) -> bool {
    let Some(class) = class else {
        return true;
    };
    let probe = pos + pull.normalize_or_zero() * biome.cell_size;
    class.can_occupy(biome.sample(probe).terrain)
}

/// Shrink this birth's mutation of the Locomotion slot to
/// `LOCOMOTION_MUTATION_SCALE` of its drawn size: `before` is the slot value
/// after crossover and before mutation.
pub fn damp_locomotion_mutation(before: f32, g: &mut Genome) {
    let after = g.get(GenomeSlot::Locomotion);
    g.set(GenomeSlot::Locomotion, before + (after - before) * LOCOMOTION_MUTATION_SCALE);
}
```

Add `pub mod habitat;` to `crates/anabios-core/src/lib.rs` after `pub mod genome;`.

- [ ] **Step 5: Add the scenario trait override**

In `crates/anabios-core/src/scenario.rs`, inside `trait_overrides! { … }` after `thirst_tolerance => ThirstTolerance,`:

```rust
    /// Locomotion class gene (`GenomeSlot::Locomotion`; read only with
    /// `territory_enabled`): `0.1` water, `0.5` land, `0.9` air.
    locomotion => Locomotion,
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p anabios-core --lib habitat genome`

Expected: all pass.

Run: `cargo test -p anabios-core --release --test determinism trajectory_unchanged`

Expected: 2 passed. Nothing reads the slot yet.

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/habitat.rs crates/anabios-core/src/lib.rs crates/anabios-core/src/genome.rs crates/anabios-core/src/scenario.rs
git commit -m "feat(habitat): Locomotion gene (slot 7) + pure habitat rules

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `territory_enabled` flag, `Territory` storage, flagship scenario, FORMAT_VERSION 44, golden regen

**Files:**
- Create: `crates/anabios-core/src/territory.rs` (struct only in this task)
- Create: `scenarios/habitat-territories.toml`
- Modify: `crates/anabios-core/src/lib.rs`, `world.rs`, `scenario.rs`, `snapshot.rs`
- Modify (golden regen): `crates/anabios-core/tests/{determinism,affect,affect_play,affect_social,cognition,inventions}.rs`

**Interfaces:**
- Consumes: `habitat::Locomotion`
- Produces:
  - `territory::Territory { cx: f32, cy: f32, r: f32, class: Locomotion }` (`Copy`, `Default`, `Serialize`)
  - `Territory::is_set() -> bool` (`r > 0`)
  - `Territory::centre() -> Vec2`
  - `World::territory_enabled: bool`
  - `World::species_territories: Vec<Territory>`
  - `Scenario::territory_enabled`

- [ ] **Step 1: Write the failing tests**

In `crates/anabios-core/src/world.rs` tests module add:

```rust
    #[test]
    fn territory_layer_defaults_off_and_empty() {
        let w = World::new(1);
        assert!(!w.territory_enabled, "territory layer is opt-in; off by default");
        assert!(w.species_territories.is_empty());
    }
```

In `crates/anabios-core/src/scenario.rs` tests module add:

```rust
    #[test]
    fn territory_flag_parses_and_instantiates() {
        let s = Scenario::parse_toml(
            "name = \"t\"\nseed = 1\nterritory_enabled = true\n[[agents]]\ncount = 2\n\
             [agents.traits]\nlocomotion = 0.9\n",
        )
        .expect("parse");
        let w = s.instantiate();
        assert!(w.territory_enabled);
        for id in w.agents.iter_alive() {
            assert_eq!(
                crate::habitat::Locomotion::of(&w.agents.genome[id as usize]),
                crate::habitat::Locomotion::Air
            );
        }
    }
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p anabios-core --lib territory_`

Expected: compile error (no field `territory_enabled`).

- [ ] **Step 3: Create `territory.rs` with the record**

```rust
//! Species territories (territory/habitat/collision layer): a persistent
//! per-species range — centre, radius, locomotion class — maintained every
//! species step, and the soft-edge pull that keeps members roaming inside it.
//! Everything is gated on `World::territory_enabled`.

use serde::{Deserialize, Serialize};

use crate::habitat::Locomotion;
use crate::prelude::Vec2;

/// One species' territory. `r == 0.0` marks a row not yet initialized (no
/// member seen by `territory_step`), which applies no pull.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Territory {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
    pub class: Locomotion,
}

impl Territory {
    #[inline]
    pub fn is_set(&self) -> bool {
        self.r > 0.0
    }

    #[inline]
    pub fn centre(&self) -> Vec2 {
        Vec2::new(self.cx, self.cy)
    }
}
```

Add `pub mod territory;` to `lib.rs` after `pub mod tick;`.

- [ ] **Step 4: Add the `World` fields**

In `crates/anabios-core/src/world.rs`, after the `disease_enabled` field, add:

```rust
    /// When true, the territory/habitat/collision layer is active: the
    /// `Locomotion` gene gates movement/grazing by terrain (Land/Water/Air),
    /// Water cells carry aquatic biomass, each species keeps a territory with a
    /// soft-edge homing pull, and bodies are kept apart (separation steering +
    /// the stage-4' min-gap resolve). Off by default — zero RNG draws and
    /// byte-identical trajectories with the flag off.
    #[serde(default)]
    pub territory_enabled: bool,
    /// Per-species territory, indexed by species id. Grown lazily by
    /// `territory::territory_step`, ONLY when `territory_enabled` — empty (and
    /// unread) otherwise. Serialized: the centre is a path-dependent EMA, so
    /// dropping it on load would diverge restore-and-continue (still-ticks v13
    /// footgun).
    #[serde(default)]
    pub species_territories: Vec<crate::territory::Territory>,
```

In `World::new`, after `disease_enabled: false,` add:

```rust
            territory_enabled: false,
            species_territories: Vec::new(),
```

- [ ] **Step 5: Add the `Scenario` field and copy it**

In `crates/anabios-core/src/scenario.rs` `pub struct Scenario`, next to `mate_seeking_enabled`, add:

```rust
    /// Opt-in territory/habitat/collision layer: heritable Land/Water/Air
    /// locomotion (terrain-gated movement and grazing), aquatic biomass on
    /// Water cells, per-species territories with a soft-edge homing pull, and
    /// body collision (steering + hard min-gap resolve). Pin a spec's class
    /// with `[agents.traits] locomotion = 0.1 | 0.5 | 0.9`. `false` (default)
    /// keeps the world byte-identical.
    #[serde(default)]
    pub territory_enabled: bool,
```

In `instantiate`, after `w.disease_enabled = self.disease_enabled;` add:

```rust
        w.territory_enabled = self.territory_enabled;
```

- [ ] **Step 6: Create the flagship scenario `scenarios/habitat-territories.toml`**

```toml
# Territory / habitat / collision showcase (docs/superpowers/specs/
# 2026-09-25-territory-habitat-collision-design.md): three grazer species
# that differ only in their Locomotion gene — land walkers, water swimmers
# (graze aquatic biomass), and flyers (roam over land AND sea, feed on land).
# Each keeps a species territory and bodies never overlap.
#
# sea_level 0.45 + continentality carve real oceans at the default 1024 world,
# so all three classes have habitat. Uniform placement is relocated to the
# nearest valid cell for each founder's class at instantiate.
name = "habitat-territories"
seed = 11
max_population = 1500
territory_enabled = true

[climate]
continentality = 0.8
sea_level = 0.45

[[agents]]
count = 150
archetype = "grazer"
placement = { kind = "uniform" }
[agents.traits]
locomotion = 0.5
size = 0.5
lifespan_bias = 0.6
reproduction_threshold = 0.5

[[agents]]
count = 100
archetype = "grazer"
placement = { kind = "uniform" }
[agents.traits]
locomotion = 0.1
size = 0.4
lifespan_bias = 0.6
reproduction_threshold = 0.5

[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "uniform" }
[agents.traits]
locomotion = 0.9
size = 0.3
lifespan_bias = 0.6
reproduction_threshold = 0.5
```

(If `parse_toml` rejects a `[climate]` key, compare against `ScenarioClimate` in `scenario.rs:260` and use only the listed names. `continentality` and `sea_level` are both listed there.)

- [ ] **Step 7: Bump `FORMAT_VERSION`**

In `crates/anabios-core/src/snapshot.rs` append to the history doc comment, above `pub const FORMAT_VERSION`:

```rust
/// 44: territory/habitat/collision layer — `World.territory_enabled` (bool)
///     and `World.species_territories` (`Vec<Territory>`, serialized EMA
///     state; empty unless the flag is on). Genome slot 7 renamed in place to
///     `Locomotion` (layout unchanged). Flag absent/off in every pre-existing
///     scenario ⇒ trajectories byte-identical (pinned by the
///     `*_trajectory_unchanged_by_territory_substrate` guards in
///     `tests/determinism.rs`); only the serialized layout grew.
```

and change the constant to `pub const FORMAT_VERSION: u32 = 44;`.

- [ ] **Step 8: Run the unit tests**

Run: `cargo test -p anabios-core --lib territory_ world scenario`

Expected: pass.

- [ ] **Step 9: Prove the change is layout-only, then regenerate the goldens**

Run: `cargo test -p anabios-core --release --test determinism trajectory_unchanged`

Expected: 2 passed. This must pass BEFORE any regen; if it fails, stop, because behaviour moved.

Regenerate each `state_hash` golden, one test binary at a time, pasting the printed table into the named constant:

```bash
UPDATE_HASHES=1 cargo test -p anabios-core --release --test determinism minimal_scenario_matches_golden_hashes -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --release --test affect -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --release --test affect_play -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --release --test affect_social -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --release --test cognition -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --release --test inventions -- --nocapture
```

The constants to paste into:

| Test binary | Constant(s) |
|---|---|
| `determinism` | `GOLDEN` |
| `affect` | `AFFECT_GOLDEN`, `THREAT_GOLDEN` |
| `affect_play` | `PLAY_GOLDEN` |
| `affect_social` | `AFFECT_GOLDEN` |
| `cognition` | `COGNITIVE_GOLDEN` |
| `inventions` | `INVENTIONS_GOLDEN` |

Above each updated constant, add a refresh note in the existing style:

```rust
    // Refreshed 2026-09-25 (territory/habitat/collision layer, FORMAT_VERSION
    // 43→44): added World.territory_enabled + World.species_territories
    // (empty with the flag off). Layout growth only — trajectory proven
    // unchanged by tests/determinism.rs::*_trajectory_unchanged_by_territory_substrate.
```

- [ ] **Step 10: Verify the regenerated goldens**

Run: `cargo test -p anabios-core --release --test determinism --test affect --test affect_play --test affect_social --test cognition --test inventions`

Expected: all pass.

Run: `cargo test -p anabios-core --release --test all_scenarios --test save_load_roundtrip`

Expected: pass. The new scenario parses and runs with the flag stored and inert.

- [ ] **Step 11: Commit**

```bash
git add crates/anabios-core/src/territory.rs crates/anabios-core/src/lib.rs crates/anabios-core/src/world.rs crates/anabios-core/src/scenario.rs crates/anabios-core/src/snapshot.rs scenarios/habitat-territories.toml crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/affect.rs crates/anabios-core/tests/affect_play.rs crates/anabios-core/tests/affect_social.rs crates/anabios-core/tests/cognition.rs crates/anabios-core/tests/inventions.rs
git commit -m "feat(territory): territory_enabled flag + species_territories storage (FORMAT_VERSION 44)

Layout-only golden regen; trajectory guards unchanged.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Habitat movement gate, founder/newborn relocation, damped class mutation

**Files:**
- Modify: `crates/anabios-core/src/integrate.rs` (signature + body + 9 test call sites + new tests)
- Modify: `crates/anabios-core/tests/inventions.rs:501` (call site)
- Modify: `crates/anabios-core/src/tick.rs:60` (call site)
- Modify: `crates/anabios-core/src/scenario.rs` (founder relocation in `instantiate`)
- Modify: `crates/anabios-core/src/reproduce.rs` (~`:224-240`)

**Interfaces:**
- Consumes: `habitat::{Locomotion, gate_move, nearest_valid, damp_locomotion_mutation}`
- Produces: `integrate_all(…, max_radius: f32, habitat: Option<&BiomeField>)`, with a new last parameter.

- [ ] **Step 1: Write the failing tests** (in `integrate.rs` tests module)

```rust
    /// 256-unit world, all Grass except Water in columns >= 16 (coast at x=128).
    fn coast_world() -> World {
        let mut w = World::with_dims(1, 256.0, 32, 16);
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                w.biome.at_mut(col, row).terrain = if col >= 16 {
                    crate::biome::TerrainType::Water
                } else {
                    crate::biome::TerrainType::Grass
                };
            }
        }
        w
    }

    fn step_once(w: &mut World, id: u32, dir: Vec2) -> Vec2 {
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = dir;
        let biome = w.biome.clone();
        integrate_all(
            &mut w.agents,
            &desired,
            w.world_size,
            false,
            false,
            w.cognition_enabled,
            w.spatial.perception_max_radius(),
            Some(&biome),
        );
        w.agents.position[id as usize]
    }

    #[test]
    fn habitat_gate_keeps_land_agents_out_of_water() {
        let mut w = coast_world();
        let id = spawn_at_unit_speed(&mut w, Vec2::new(126.0, 100.0));
        let p = step_once(&mut w, id, Vec2::new(1.0, 0.0));
        assert_eq!(p, Vec2::new(126.0, 100.0), "blocked at the shore");
        let p = step_once(&mut w, id, Vec2::new(0.70710677, 0.70710677));
        assert!((p.x - 126.0).abs() < 1e-4 && p.y > 100.0, "slides north along the coast: {p:?}");
    }

    #[test]
    fn habitat_gate_keeps_water_agents_in_water_and_lets_air_cross() {
        let mut w = coast_world();
        let mut g = crate::genome::Genome::neutral();
        g.set(crate::genome::GenomeSlot::Locomotion, 0.1);
        let fish = spawn_at_unit_speed(&mut w, Vec2::new(130.0, 60.0));
        w.agents.genome[fish as usize] = g;
        assert_eq!(step_once(&mut w, fish, Vec2::new(-1.0, 0.0)), Vec2::new(130.0, 60.0));
        let bird = spawn_at_unit_speed(&mut w, Vec2::new(126.0, 30.0));
        w.agents.genome[bird as usize].set(crate::genome::GenomeSlot::Locomotion, 0.9);
        let p = step_once(&mut w, bird, Vec2::new(1.0, 0.0));
        assert!((p.x - 130.0).abs() < 1e-4, "air crosses the coast: {p:?}");
    }

    #[test]
    fn habitat_none_is_the_ungated_move() {
        let mut w = coast_world();
        let id = spawn_at_unit_speed(&mut w, Vec2::new(126.0, 100.0));
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        integrate_all(
            &mut w.agents,
            &desired,
            w.world_size,
            false,
            false,
            w.cognition_enabled,
            w.spatial.perception_max_radius(),
            None,
        );
        assert!((w.agents.position[id as usize].x - 130.0).abs() < 1e-4);
    }
```

In `crates/anabios-core/src/habitat.rs` tests add:

```rust
    #[test]
    fn instantiate_relocates_founders_onto_their_habitat() {
        let s = crate::scenario::Scenario::parse_toml(include_str!(
            "../../../scenarios/habitat-territories.toml"
        ))
        .expect("parse");
        let w = s.instantiate();
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let class = Locomotion::of(&w.agents.genome[i]);
            let t = w.biome.sample(w.agents.position[i]).terrain;
            assert!(class.can_occupy(t), "founder {id} ({class:?}) spawned on {t:?}");
        }
    }
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p anabios-core --lib habitat_gate habitat_none instantiate_relocates`

Expected: compile error (extra argument). After Step 3 the relocation test fails with "spawned on Water" until Step 4.

- [ ] **Step 3: Gate `integrate_all`**

Change the signature to add a last parameter, and extend the doc comment:

```rust
/// … `habitat` (territory layer) is `Some(biome)` when `territory_enabled`:
/// each move passes through `habitat::gate_move` for the agent's Locomotion
/// class; `None` applies the move ungated (exact identity).
#[allow(clippy::too_many_arguments)]
pub fn integrate_all(
    agents: &mut AgentBuffers,
    desired_direction: &[Vec2],
    world_size: f32,
    dimorphism_enabled: bool,
    gene_tech_coupling: bool,
    cognition_enabled: bool,
    max_radius: f32,
    habitat: Option<&crate::biome::BiomeField>,
) {
```

In the body, directly after:

```rust
            let v = direction
                * (SPEED_MAX_CAP * module_speed * speed_factor * inv_speed * affect_speed);
```

insert:

```rust
            // Habitat gate (territory layer): the move the agent's Locomotion
            // class allows — full, coastline slide, or none. `None` ⇒ `v`
            // untouched, so flag-off worlds are byte-identical.
            let v = match habitat {
                Some(biome) => crate::habitat::gate_move(
                    biome,
                    crate::habitat::Locomotion::of(&genome[i]),
                    *pos,
                    v,
                ),
                None => v,
            };
```

Update every existing call site by appending `None,` as the last argument:
- the nine in `integrate.rs` tests
- `tests/inventions.rs:501`

In `tick.rs` stage 4 append:

```rust
        world.territory_enabled.then_some(&world.biome),
```

- [ ] **Step 4: Relocate founders at instantiate**

In `scenario.rs` `instantiate`, the per-agent loop currently does `placed_positions.push(position);` right after the `let position = match …` block, then builds `g`. Move the push below the trait application and relocate first. Replace:

```rust
                placed_positions.push(position);
                let mut g = Genome::neutral();
```

with:

```rust
                let mut g = Genome::neutral();
```

and directly after `spec.traits.apply(&mut g);` insert:

```rust
                // Territory layer: move a founder onto terrain its Locomotion
                // class can occupy (no RNG). Flag off ⇒ position unchanged.
                let position = if w.territory_enabled {
                    crate::habitat::nearest_valid(
                        &w.biome,
                        position,
                        crate::habitat::Locomotion::of(&g),
                    )
                    .unwrap_or(position)
                } else {
                    position
                };
                placed_positions.push(position);
```

This must come after the `world_map` / `climate` match that finalizes `w.biome`; it does, since the agent loop runs after it.

- [ ] **Step 5: Damp class mutation and relocate newborns in `reproduce.rs`**

Replace:

```rust
        child_genome.mutate_in_place_scaled(&mut world.rng, sigma_mult);
```

with:

```rust
        let loco_before = child_genome.get(crate::genome::GenomeSlot::Locomotion);
        child_genome.mutate_in_place_scaled(&mut world.rng, sigma_mult);
        // Territory layer: class flips are rare — shrink this slot's drawn
        // delta (same draw count). Flag off ⇒ untouched.
        if world.territory_enabled {
            crate::habitat::damp_locomotion_mutation(loco_before, &mut child_genome);
        }
```

Replace:

```rust
        let child_pos = midpoint_torus(a_pos, b_pos, world.world_size);
```

with:

```rust
        let child_pos = midpoint_torus(a_pos, b_pos, world.world_size);
        // Territory layer: a newborn lands on terrain its class can occupy.
        let child_pos = if world.territory_enabled {
            crate::habitat::nearest_valid(
                &world.biome,
                child_pos,
                crate::habitat::Locomotion::of(&child_genome),
            )
            .unwrap_or(child_pos)
        } else {
            child_pos
        };
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p anabios-core --lib integrate habitat reproduce`

Expected: pass.

Run: `cargo test -p anabios-core --release --test determinism trajectory_unchanged --test inventions`

Expected: pass. Goldens don't move; the flag is off.

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/integrate.rs crates/anabios-core/src/tick.rs crates/anabios-core/src/scenario.rs crates/anabios-core/src/reproduce.rs crates/anabios-core/src/habitat.rs crates/anabios-core/tests/inventions.rs
git commit -m "feat(habitat): terrain-gated movement, founder/newborn relocation, damped class mutation

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Aquatic biomass, class-gated grazing, class-aware plant sensing

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (constants + two methods + tests)
- Modify: `crates/anabios-core/src/scenario.rs` (`seed_aquatic` call)
- Modify: `crates/anabios-core/src/tick.rs` (stage 10 + `sense_all` arg)
- Modify: `crates/anabios-core/src/interact.rs` (`feed_pass` gate + test call site `:679`)
- Modify: `crates/anabios-core/src/sense.rs` (`sense_all`/`sense_one`/`best_plant_direction` + test call sites `:511,:575,:591,:633`)
- Modify: `crates/anabios-core/benches/tick_bench.rs:109`, `tests/inventions.rs:413`, `tests/anthro_race.rs:121` (call sites)

**Interfaces:**
- Consumes: `habitat::Locomotion`
- Produces:
  - `biome::{AQUATIC_CAPACITY = 4.0, AQUATIC_REGROWTH_RATE = 0.01, AQUATIC_RESEED_FRAC = 0.01}`
  - `BiomeField::seed_aquatic(&mut self)`
  - `BiomeField::aquatic_regrow_step(&mut self)`
  - `sense_all(…, cognition_enabled: bool, territory_enabled: bool)`, with a new last parameter

- [ ] **Step 1: Write the failing tests**

In `biome.rs` tests:

```rust
    #[test]
    fn aquatic_biomass_seeds_and_regrows_only_water() {
        let mut f = BiomeField::generate(3, 32, 256.0);
        for (k, c) in f.cells.iter_mut().enumerate() {
            c.terrain = if k % 2 == 0 { TerrainType::Water } else { TerrainType::Grass };
            c.plant_biomass = 0.0;
        }
        f.seed_aquatic();
        for c in &f.cells {
            let want = if c.terrain == TerrainType::Water { AQUATIC_CAPACITY } else { 0.0 };
            assert_eq!(c.plant_biomass, want);
        }
        // Grazed-out water reseeds and climbs; land is untouched by this step.
        for c in f.cells.iter_mut() {
            c.plant_biomass = 0.0;
        }
        f.aquatic_regrow_step();
        for c in &f.cells {
            if c.terrain == TerrainType::Water {
                assert!(c.plant_biomass > 0.0 && c.plant_biomass <= AQUATIC_CAPACITY);
            } else {
                assert_eq!(c.plant_biomass, 0.0);
            }
        }
    }
```

In `interact.rs` tests:

```rust
    #[test]
    fn flyers_do_not_graze_aquatic_biomass() {
        let mut w = World::new(5);
        w.territory_enabled = true;
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                let c = w.biome.at_mut(col, row);
                c.terrain = crate::biome::TerrainType::Water;
                c.plant_biomass = crate::biome::AQUATIC_CAPACITY;
            }
        }
        let mut g = crate::genome::Genome::neutral();
        g.set(crate::genome::GenomeSlot::Locomotion, 0.9); // Air
        let bird = w.spawn_agent(crate::prelude::Vec2::new(500.0, 500.0), g);
        g.set(crate::genome::GenomeSlot::Locomotion, 0.1); // Water
        let fish = w.spawn_agent(crate::prelude::Vec2::new(300.0, 300.0), g);
        let before_bird = w.biome.sample(w.agents.position[bird as usize]).plant_biomass;
        let before_fish = w.biome.sample(w.agents.position[fish as usize]).plant_biomass;
        refresh_sensors(&mut w);
        interact_all(&mut w);
        assert_eq!(
            w.biome.sample(w.agents.position[bird as usize]).plant_biomass,
            before_bird,
            "an Air agent over water must not graze"
        );
        assert!(
            w.biome.sample(w.agents.position[fish as usize]).plant_biomass < before_fish,
            "a Water agent grazes aquatic biomass"
        );
    }
```

(`refresh_sensors` is the existing test helper at `interact.rs:676`; update its `sense_all` call in Step 4.)

In `sense.rs` tests:

```rust
    #[test]
    fn land_agents_do_not_sense_aquatic_biomass_when_territory_on() {
        let mut w = World::new(9);
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                let c = w.biome.at_mut(col, row);
                // Water everywhere (full aquatic biomass) except a bare grass cell
                // under the agent.
                c.terrain = crate::biome::TerrainType::Water;
                c.plant_biomass = crate::biome::AQUATIC_CAPACITY;
            }
        }
        let pos = Vec2::new(500.0, 500.0);
        let (col, row) = w.biome.cell_coords(pos);
        w.biome.at_mut(col, row).terrain = crate::biome::TerrainType::Grass;
        w.biome.at_mut(col, row).plant_biomass = 0.0;
        let id = w.spawn_agent(pos, crate::genome::Genome::neutral()); // Land
        w.resize_scratch();
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let run = |w: &mut World, on: bool| {
            sense_all(
                &w.agents,
                &w.biome,
                &w.pheromones,
                &w.spatial,
                &w.codex.hostility,
                &w.culture_mask,
                &mut w.sensors,
                w.world_size,
                false,
                false,
                on,
            );
            w.sensors[id as usize]
        };
        assert_ne!(run(&mut w, false).plant_direction, Vec2::ZERO, "flag off: water biomass visible");
        assert_eq!(run(&mut w, true).plant_direction, Vec2::ZERO, "flag on: land agent ignores water");
    }
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p anabios-core --lib aquatic flyers_do_not land_agents_do_not`

Expected: compile errors (missing items, wrong arity).

- [ ] **Step 3: Aquatic biome methods**

In `biome.rs`, after `pub const TEMP_LAPSE` add:

```rust
/// Aquatic biomass capacity of a Water cell when `World::territory_enabled`
/// (0.4 × Grass). `TerrainType::carrying_capacity` stays 0.0 for Water — the
/// aquatic pool is maintained only by `seed_aquatic`/`aquatic_regrow_step`.
pub const AQUATIC_CAPACITY: f32 = 4.0;
/// Logistic regrowth rate of aquatic biomass per biome step (Grass's rate).
pub const AQUATIC_REGROWTH_RATE: f32 = 0.01;
/// Floor a grazed-out Water cell reseeds from, as a fraction of capacity, so
/// the aquatic pool can never go permanently extinct.
pub const AQUATIC_RESEED_FRAC: f32 = 0.01;
```

In `impl BiomeField`, after `regrow_step_seasonal`, add:

```rust
    /// Fill every Water cell to `AQUATIC_CAPACITY` (territory layer; called
    /// once at instantiate). Deterministic, no RNG.
    pub fn seed_aquatic(&mut self) {
        for c in self.cells.iter_mut() {
            if c.terrain == TerrainType::Water {
                c.plant_biomass = AQUATIC_CAPACITY;
            }
        }
    }

    /// One biome step of logistic aquatic regrowth on Water cells only
    /// (territory layer), from at least the reseed floor. Land cells are
    /// untouched. Deterministic, no RNG.
    pub fn aquatic_regrow_step(&mut self) {
        let floor = AQUATIC_RESEED_FRAC * AQUATIC_CAPACITY;
        for c in self.cells.iter_mut() {
            if c.terrain != TerrainType::Water {
                continue;
            }
            let b = c.plant_biomass.max(floor);
            let next = b + AQUATIC_REGROWTH_RATE * b * (1.0 - b / AQUATIC_CAPACITY);
            c.plant_biomass = next.clamp(0.0, AQUATIC_CAPACITY);
        }
    }
```

In `scenario.rs` `instantiate`, directly after the `match &self.world_map { … }` block, add:

```rust
        // Territory layer: stock the oceans with aquatic biomass once the
        // final biome exists. Flag off ⇒ Water stays at 0.0 as before.
        if w.territory_enabled {
            w.biome.seed_aquatic();
        }
```

In `tick.rs` stage 10, after the `if world.season_period > 0 { … } else { … }` regrowth, add:

```rust
        if world.territory_enabled {
            world.biome.aquatic_regrow_step();
        }
```

- [ ] **Step 4: Class-aware sensing**

In `sense.rs`:
1. Add a `territory_enabled: bool` last parameter to `sense_all` and document it: "Territory layer: plant sensing only sees terrain the agent's Locomotion class can graze. `false` ⇒ identical to before."
2. Pass it through to `sense_one` as a new last parameter.
3. In `sense_one`, after `let genome = &agents.genome[i];`, add:

```rust
    // Territory layer: which terrain this agent may graze (None = any, flag off).
    let graze = territory_enabled.then(|| crate::habitat::Locomotion::of(genome));
```

Change `let plant_direction = best_plant_direction(biome, pos, radius);` to `best_plant_direction(biome, pos, radius, graze);`, and in the `SensorRegister` literal replace `local_plant_biomass: local_cell.plant_biomass,` with:

```rust
        local_plant_biomass: match graze {
            Some(c) if !c.can_graze(local_cell.terrain) => 0.0,
            _ => local_cell.plant_biomass,
        },
```

Change `fn best_plant_direction(biome: &BiomeField, pos: Vec2, radius: f32) -> Vec2` to take `graze: Option<crate::habitat::Locomotion>`. Inside the loop, right after the `if cell.plant_biomass <= 0.0 { continue; }` check, add:

```rust
            if let Some(c) = graze {
                if !c.can_graze(cell.terrain) {
                    continue;
                }
            }
```

Update every `sense_all` call:
- `tick.rs` passes `world.territory_enabled`.
- These pass `false`: `sense.rs` tests (4 sites), `interact.rs:679`, `benches/tick_bench.rs:109`, `tests/inventions.rs:413`, `tests/anthro_race.rs:121`.

- [ ] **Step 5: Class-gated grazing**

In `interact.rs` `feed_pass`, after the `asleep` check, add:

```rust
        // Territory layer: graze only terrain this agent's Locomotion class
        // may feed on (flyers over water, stranded agents). Flag off ⇒ skipped.
        if world.territory_enabled
            && !crate::habitat::Locomotion::of(&world.agents.genome[i])
                .can_graze(world.biome.sample(world.agents.position[i]).terrain)
        {
            continue;
        }
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p anabios-core --lib biome sense interact`

Expected: pass.

Run: `cargo test -p anabios-core --release --test determinism trajectory_unchanged --test inventions --test anthro_race`

Expected: pass.

Run: `cargo bench -p anabios-core --no-run`

Expected: compiles.

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/scenario.rs crates/anabios-core/src/tick.rs crates/anabios-core/src/interact.rs crates/anabios-core/src/sense.rs crates/anabios-core/benches/tick_bench.rs crates/anabios-core/tests/inventions.rs crates/anabios-core/tests/anthro_race.rs
git commit -m "feat(habitat): aquatic biomass, class-gated grazing and plant sensing

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Species territory step + soft-edge pull + habitat pull masking

**Files:**
- Modify: `crates/anabios-core/src/territory.rs` (constants + `radius_for` + `territory_pull` + `territory_step` + tests)
- Modify: `crates/anabios-core/src/tick.rs` (stage 8 call; `decide_all` pull + masking)

**Interfaces:**
- Consumes: `Territory`, `Locomotion`, `habitat::pull_allowed`, `crate::spatial::torus_delta(a, b, ws) -> Vec2` (= a − b wrapped)
- Produces:
  - `territory::{TERRITORY_CENTRE_RATE = 0.1, TERRITORY_K = 12.0, TERRITORY_R_MIN = 48.0, TERRITORY_R_MAX = 256.0, TERRITORY_PULL = 1.0}`
  - `radius_for(u32) -> f32`
  - `territory_pull(&Territory, pos: Vec2, terr: f32, ws: f32) -> Vec2`
  - `territory_step(&mut World)`

- [ ] **Step 1: Write the failing tests** (`territory.rs` tests module)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{Genome, GenomeSlot};
    use crate::world::World;

    #[test]
    fn pull_is_zero_inside_and_ramps_outside() {
        let t = Territory { cx: 500.0, cy: 500.0, r: 100.0, class: Locomotion::Land };
        assert_eq!(territory_pull(&t, Vec2::new(550.0, 500.0), 1.0, 1024.0), Vec2::ZERO);
        assert_eq!(territory_pull(&t, Vec2::new(600.0, 500.0), 1.0, 1024.0), Vec2::ZERO);
        let near = territory_pull(&t, Vec2::new(650.0, 500.0), 1.0, 1024.0);
        let far = territory_pull(&t, Vec2::new(750.0, 500.0), 1.0, 1024.0);
        assert!(near.x < 0.0 && near.y == 0.0, "points home: {near:?}");
        assert!((near.length() - 0.5 * TERRITORY_PULL).abs() < 1e-5);
        assert!((far.length() - TERRITORY_PULL).abs() < 1e-5, "capped at full pull");
        assert!((territory_pull(&t, Vec2::new(750.0, 500.0), 0.5, 1024.0).length()
            - 0.5 * TERRITORY_PULL).abs() < 1e-5, "scaled by Territoriality");
        assert_eq!(territory_pull(&Territory::default(), Vec2::ZERO, 1.0, 1024.0), Vec2::ZERO);
    }

    #[test]
    fn radius_follows_sqrt_members_and_clamps() {
        assert_eq!(radius_for(1), TERRITORY_R_MIN);
        assert!((radius_for(100) - 120.0).abs() < 1e-4);
        assert_eq!(radius_for(1_000_000), TERRITORY_R_MAX);
    }

    fn world_with(positions: &[(f32, f32)], loco: f32) -> World {
        let mut w = World::new(3);
        w.territory_enabled = true;
        for &(x, y) in positions {
            let mut g = Genome::neutral();
            g.set(GenomeSlot::Locomotion, loco);
            w.spawn_agent(Vec2::new(x, y), g);
        }
        w
    }

    #[test]
    fn step_initializes_centre_across_the_torus_seam() {
        // Members straddle x = 0: the torus mean is ~x = 0, not x = 512.
        let mut w = world_with(&[(1020.0, 300.0), (4.0, 300.0), (1016.0, 300.0), (8.0, 300.0)], 0.5);
        territory_step(&mut w);
        let t = w.species_territories[0];
        assert!(t.is_set());
        let dx = crate::spatial::torus_delta(Vec2::new(t.cx, 0.0), Vec2::new(0.0, 0.0), 1024.0).x;
        assert!(dx.abs() < 1e-3, "centre at the seam, got cx={}", t.cx);
        assert!((t.cy - 300.0).abs() < 1e-3);
        assert_eq!(t.class, Locomotion::Land);
        assert_eq!(t.r, radius_for(4));
    }

    #[test]
    fn step_moves_an_existing_centre_by_the_ema_rate() {
        let mut w = world_with(&[(600.0, 500.0)], 0.1);
        w.species_territories.push(Territory { cx: 500.0, cy: 500.0, r: 50.0, class: Locomotion::Land });
        territory_step(&mut w);
        let t = w.species_territories[0];
        assert!((t.cx - (500.0 + 100.0 * TERRITORY_CENTRE_RATE)).abs() < 1e-3);
        assert_eq!(t.class, Locomotion::Water, "majority class of members");
    }

    #[test]
    fn new_species_inherit_the_parent_record() {
        let mut w = world_with(&[(100.0, 100.0)], 0.5);
        territory_step(&mut w);
        let parent = w.species_territories[0];
        let child = w.push_species(Genome::neutral(), Some(0));
        territory_step(&mut w); // child has no members yet: row inherited, not updated
        assert_eq!(w.species_territories[child as usize], parent);
    }

    #[test]
    fn step_is_a_noop_with_the_flag_off() {
        let mut w = world_with(&[(100.0, 100.0)], 0.5);
        w.territory_enabled = false;
        territory_step(&mut w);
        assert!(w.species_territories.is_empty());
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p anabios-core --lib territory`

Expected: compile error (missing functions).

- [ ] **Step 3: Implement** (in `territory.rs`, below `impl Territory`)

```rust
/// EMA rate at which a territory centre follows its members' mean position,
/// per species step (`species::SPECIES_STEP_INTERVAL` ticks).
pub const TERRITORY_CENTRE_RATE: f32 = 0.1;
/// Radius per √member: `r = clamp(K·√n, R_MIN, R_MAX)`.
pub const TERRITORY_K: f32 = 12.0;
pub const TERRITORY_R_MIN: f32 = 48.0;
pub const TERRITORY_R_MAX: f32 = 256.0;
/// Homing pull at Territoriality = 1 once a member is ≥ 2r from the centre.
pub const TERRITORY_PULL: f32 = 1.0;

/// Territory radius for a species of `members` alive agents.
pub fn radius_for(members: u32) -> f32 {
    (TERRITORY_K * (members as f32).sqrt()).clamp(TERRITORY_R_MIN, TERRITORY_R_MAX)
}

/// Soft-edge homing pull toward the territory centre: zero inside radius `r`
/// (free roam), then `min((d − r)/r, 1) · TERRITORY_PULL · terr` toward the
/// centre. Unset rows pull nothing. Applied in `decide_all` behind the flag.
pub fn territory_pull(t: &Territory, pos: Vec2, terr: f32, ws: f32) -> Vec2 {
    if !t.is_set() {
        return Vec2::ZERO;
    }
    let d = crate::spatial::torus_delta(t.centre(), pos, ws);
    let dist = d.length();
    if dist <= t.r {
        return Vec2::ZERO;
    }
    let ramp = ((dist - t.r) / t.r).min(1.0);
    (d / dist) * (ramp * TERRITORY_PULL * terr)
}

/// Per-species territory update, run right after `species::species_step`.
/// Grows `species_territories` to `next_species_id` (a new species inherits
/// its parent's record), then for every species with members: centre ← EMA
/// toward the members' torus-safe mean position (mean offset from a reference
/// point — the current centre, or the first member for an unset row — so the
/// seam never splits a species), radius ← `radius_for(n)`, class ← majority
/// member class (lowest index wins ties). Ascending id order, f64 sums, no
/// RNG, no trig ⇒ deterministic. No-op with the flag off.
pub fn territory_step(world: &mut crate::world::World) {
    if !world.territory_enabled {
        return;
    }
    let ws = world.world_size;
    let n = world.next_species_id as usize;
    while world.species_territories.len() < n {
        let sid = world.species_territories.len();
        let inherited = world
            .species_parents
            .get(sid)
            .copied()
            .flatten()
            .and_then(|p| world.species_territories.get(p as usize).copied())
            .unwrap_or_default();
        world.species_territories.push(inherited);
    }

    #[derive(Clone, Copy, Default)]
    struct Acc {
        reference: Option<Vec2>,
        dx: f64,
        dy: f64,
        count: u32,
        votes: [u32; 3],
    }
    let mut acc = vec![Acc::default(); n];
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i] as usize;
        let pos = world.agents.position[i];
        let a = &mut acc[sid];
        let reference = *a.reference.get_or_insert_with(|| {
            let t = world.species_territories[sid];
            if t.is_set() {
                t.centre()
            } else {
                pos
            }
        });
        let d = crate::spatial::torus_delta(pos, reference, ws);
        a.dx += d.x as f64;
        a.dy += d.y as f64;
        a.count += 1;
        a.votes[Locomotion::of(&world.agents.genome[i]).index()] += 1;
    }

    for (sid, a) in acc.iter().enumerate() {
        let Some(reference) = a.reference else {
            continue;
        };
        let inv = 1.0 / a.count as f64;
        let centroid = Vec2::new(
            (reference.x + (a.dx * inv) as f32).rem_euclid(ws),
            (reference.y + (a.dy * inv) as f32).rem_euclid(ws),
        );
        let t = &mut world.species_territories[sid];
        if t.is_set() {
            let d = crate::spatial::torus_delta(centroid, t.centre(), ws);
            t.cx = (t.cx + d.x * TERRITORY_CENTRE_RATE).rem_euclid(ws);
            t.cy = (t.cy + d.y * TERRITORY_CENTRE_RATE).rem_euclid(ws);
        } else {
            t.cx = centroid.x;
            t.cy = centroid.y;
        }
        t.r = radius_for(a.count);
        let mut best = 0;
        for k in 1..3 {
            if a.votes[k] > a.votes[best] {
                best = k;
            }
        }
        t.class = [Locomotion::Land, Locomotion::Water, Locomotion::Air][best];
    }
}
```

(If the borrow checker rejects the `get_or_insert_with` closure reading `world.species_territories` while `world.agents` is iterated, hoist `let terr = &world.species_territories;` before the loop and read `terr[sid]`. Both are shared borrows of distinct fields.)

- [ ] **Step 4: Wire into `tick.rs`**

Stage 8:

```rust
    if world.tick.is_multiple_of(crate::species::SPECIES_STEP_INTERVAL) {
        crate::species::species_step(world);
        // Territory layer: re-centre/re-size each species' range from its
        // freshly reassigned members (no-op, zero state with the flag off).
        crate::territory::territory_step(world);
    }
```

In `decide_all`, next to the other captured flags, add:

```rust
    let territory_enabled = world.territory_enabled;
    let territories = &world.species_territories;
```

At the top of the per-agent closure body, after the dead-slot early return, add:

```rust
            // Territory layer: this agent's Locomotion class (None = flag off).
            let class = territory_enabled
                .then(|| crate::habitat::Locomotion::of(&agents.genome[i]));
```

In the `biome_adaptation` block wrap the two `action.move_* +=` lines:

```rust
                if crate::habitat::pull_allowed(biome, agents.position[i], pull, class) {
                    action.move_x += crate::culture::HABITAT_PULL * pull.x;
                    action.move_y += crate::culture::HABITAT_PULL * pull.y;
                }
```

Do the same in the `terrain_habitat` block, with the `TERRAIN_HABITAT_PULL` lines. Directly after the `settlement_enabled` anchor block, add:

```rust
            // Species territory (territory layer, opt-in): free roam inside the
            // species' range, a Territoriality-scaled pull home past its edge.
            if territory_enabled {
                if let Some(t) = territories.get(agents.species_id[i] as usize) {
                    let pull = crate::territory::territory_pull(
                        t,
                        agents.position[i],
                        agents.genome[i].get(crate::genome::GenomeSlot::Territoriality),
                        ws,
                    );
                    action.move_x += pull.x;
                    action.move_y += pull.y;
                }
            }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p anabios-core --lib territory tick`

Expected: pass.

Run: `cargo test -p anabios-core --release --test determinism trajectory_unchanged --test inventions`

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/territory.rs crates/anabios-core/src/tick.rs
git commit -m "feat(territory): per-species territory step + soft-edge pull + habitat pull masking

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Collision — fine hash, separation steering, stage-4' min-gap resolve

**Files:**
- Create: `crates/anabios-core/src/collision.rs`
- Modify: `crates/anabios-core/src/lib.rs` (`pub mod collision;` after `pub mod codex;`)
- Modify: `crates/anabios-core/src/spatial.rs` (`res()` accessor)
- Modify: `crates/anabios-core/src/world.rs` (two skip fields + `World::new`)
- Modify: `crates/anabios-core/src/tick.rs` (stage 1 rebuild, stage 4' resolve, decide steering)

**Interfaces:**
- Consumes: `Locomotion`, `UniformSpatialHash::{with_dims, rebuild, query}`, `crate::spatial::torus_delta`, `crate::prelude::wrap_torus`
- Produces:
  - `collision::{COLLISION_CELL = 4.0, BODY_R_BASE = 0.4, BODY_R_SIZE = 0.35, STEER_MARGIN = 1.25, SEP_PULL = 2.0, RESOLVE_PASSES = 2, MAX_PUSH = 1.0}`
  - `body_radius(&Genome) -> f32`
  - `rebuild_hash(&mut World)`
  - `separation_steer(&UniformSpatialHash, &AgentBuffers, i: usize, ws: f32) -> Vec2`
  - `resolve_overlaps(&mut World)`
  - `UniformSpatialHash::res() -> usize`
  - `World::{collision_spatial, collision_scratch}` (both `#[serde(skip)]`)

- [ ] **Step 1: Write the failing tests** (`collision.rs` test module; create the file with just the tests first)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{Genome, GenomeSlot};
    use crate::world::World;

    fn flat_world() -> World {
        let mut w = World::new(2);
        w.territory_enabled = true;
        for c in w.biome.cells.iter_mut() {
            c.terrain = crate::biome::TerrainType::Grass;
        }
        w
    }

    fn with_class(loco: f32) -> Genome {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Locomotion, loco);
        g
    }

    #[test]
    fn body_radius_spans_base_to_base_plus_size_term() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Size, 0.0);
        assert_eq!(body_radius(&g), BODY_R_BASE);
        g.set(GenomeSlot::Size, 1.0);
        assert!((body_radius(&g) - (BODY_R_BASE + BODY_R_SIZE)).abs() < 1e-6);
        assert!(2.0 * body_radius(&g) <= 1.5 + 1e-6, "max pair gap stays under contact range 2.0");
    }

    #[test]
    fn stacked_agents_end_at_least_the_gap_apart() {
        let mut w = flat_world();
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let gap = body_radius(&w.agents.genome[a as usize]) + body_radius(&w.agents.genome[b as usize]);
        resolve_overlaps(&mut w);
        let d = crate::spatial::torus_distance(
            w.agents.position[a as usize],
            w.agents.position[b as usize],
            w.world_size,
        );
        assert!(d >= gap - 1e-4, "d={d} gap={gap}");
    }

    #[test]
    fn air_and_land_pass_through_each_other() {
        let mut w = flat_world();
        let land = w.spawn_agent(Vec2::new(300.0, 300.0), with_class(0.5));
        let bird = w.spawn_agent(Vec2::new(300.2, 300.0), with_class(0.9));
        resolve_overlaps(&mut w);
        assert_eq!(w.agents.position[land as usize], Vec2::new(300.0, 300.0));
        assert_eq!(w.agents.position[bird as usize], Vec2::new(300.2, 300.0));
    }

    #[test]
    fn a_push_never_crosses_into_invalid_habitat() {
        let mut w = flat_world();
        // Coast at x = 304 (cell col 38 of 8-unit cells on the 1024 world): water east of it.
        let res = w.biome.res;
        for row in 0..res {
            for col in 38..res {
                w.biome.at_mut(col, row).terrain = crate::biome::TerrainType::Water;
            }
        }
        let a = w.spawn_agent(Vec2::new(303.8, 300.0), Genome::neutral()); // land, at the shore
        let _b = w.spawn_agent(Vec2::new(303.4, 300.0), Genome::neutral()); // pushes a east
        resolve_overlaps(&mut w);
        let t = w.biome.sample(w.agents.position[a as usize]).terrain;
        assert_ne!(t, crate::biome::TerrainType::Water, "land agent pushed into the sea");
    }

    #[test]
    fn resolve_is_a_noop_with_the_flag_off() {
        let mut w = flat_world();
        w.territory_enabled = false;
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let _b = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        resolve_overlaps(&mut w);
        assert_eq!(w.agents.position[a as usize], Vec2::new(300.0, 300.0));
    }

    #[test]
    fn steering_points_away_from_an_overlapping_neighbour() {
        let mut w = flat_world();
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let _b = w.spawn_agent(Vec2::new(300.5, 300.0), Genome::neutral());
        rebuild_hash(&mut w);
        let s = separation_steer(&w.collision_spatial, &w.agents, a as usize, w.world_size);
        assert!(s.x < 0.0 && s.y.abs() < 1e-6, "steer west, away from b: {s:?}");
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p anabios-core --lib collision`

Expected: compile error.

- [ ] **Step 3: `spatial.rs` accessor + `World` scratch fields**

In `impl UniformSpatialHash` add:

```rust
    /// Grid resolution per axis.
    #[inline]
    pub fn res(&self) -> usize {
        self.res
    }
```

In `World`, next to `resource_spatial`, add:

```rust
    /// Fine collision hash (`collision::COLLISION_CELL` cells), rebuilt at
    /// stage 1 and inside the stage-4' resolve when `territory_enabled`.
    /// `#[serde(skip)]` scratch — resized/rebuilt on first use after load.
    #[serde(skip)]
    pub collision_spatial: UniformSpatialHash,
    /// Jacobi position snapshot reused by `collision::resolve_overlaps`.
    /// Scratch, `#[serde(skip)]`.
    #[serde(skip)]
    pub collision_scratch: Vec<crate::prelude::Vec2>,
```

and in `World::new`:

```rust
            // Placeholder (3x3); `collision::rebuild_hash` sizes it on first use.
            collision_spatial: UniformSpatialHash::with_dims(crate::biome::WORLD_SIZE_DEFAULT, 3),
            collision_scratch: Vec::new(),
```

- [ ] **Step 4: Implement `collision.rs`** (above the tests)

```rust
//! Body collision (territory/habitat/collision layer). Each agent is a disc of
//! `body_radius` (from its Size gene); colliding pairs (see
//! `Locomotion::collides_with`) are kept at least `r_i + r_j` apart by:
//!
//! 1. a separation steering term added in `decide_all` (agents route around
//!    each other), and
//! 2. a hard post-integrate resolve (stage 4'): `RESOLVE_PASSES` Jacobi
//!    passes over a fine hash, each agent pushing itself out of its overlaps
//!    against a position snapshot, dropping any push into terrain its class
//!    can't occupy.
//!
//! Both read snapshots and write only their own slot, so the result is
//! independent of rayon thread count. Gated on `World::territory_enabled`.

use crate::agent::AgentBuffers;
use crate::genome::{Genome, GenomeSlot};
use crate::habitat::Locomotion;
use crate::prelude::{wrap_torus, Vec2};
use crate::spatial::{torus_delta, UniformSpatialHash};
use crate::world::World;

/// Collision hash cell size (world units); also the query reach.
pub const COLLISION_CELL: f32 = 4.0;
/// Body radius at Size 0; `+ BODY_R_SIZE · Size` on top (Size ∈ [0,1]).
pub const BODY_R_BASE: f32 = 0.4;
pub const BODY_R_SIZE: f32 = 0.35;
/// Steering starts at `STEER_MARGIN × (r_i + r_j)` — just before contact.
pub const STEER_MARGIN: f32 = 1.25;
/// Weight of the steering vector in `decide_all`.
pub const SEP_PULL: f32 = 2.0;
/// Jacobi passes per tick.
pub const RESOLVE_PASSES: usize = 2;
/// Largest per-pass push (keeps a pass inside the one-ring hash guarantee).
pub const MAX_PUSH: f32 = 1.0;

/// Fixed unit directions for exactly coincident pairs (no RNG).
const TIE_DIRS: [(f32, f32); 8] = [
    (1.0, 0.0),
    (0.707_106_77, 0.707_106_77),
    (0.0, 1.0),
    (-0.707_106_77, 0.707_106_77),
    (-1.0, 0.0),
    (-0.707_106_77, -0.707_106_77),
    (0.0, -1.0),
    (0.707_106_77, -0.707_106_77),
];

#[inline]
pub fn body_radius(g: &Genome) -> f32 {
    BODY_R_BASE + BODY_R_SIZE * g.get(GenomeSlot::Size)
}

/// Unit vector pushing `i` away from `j`, given `d = pos_i − pos_j` (torus)
/// and its length. Coincident pairs get opposite fixed directions keyed on the
/// unordered id pair.
#[inline]
fn away_dir(i: u32, j: u32, d: Vec2, dist: f32) -> Vec2 {
    if dist > 1e-4 {
        return d / dist;
    }
    let (lo, hi) = if i < j { (i, j) } else { (j, i) };
    let (x, y) = TIE_DIRS[(lo.wrapping_mul(31).wrapping_add(hi) % 8) as usize];
    let dir = Vec2::new(x, y);
    if i < j {
        -dir
    } else {
        dir
    }
}

fn collision_res(ws: f32) -> usize {
    ((ws / COLLISION_CELL) as usize).max(3)
}

/// (Re)size and rebuild `world.collision_spatial` from current positions.
pub fn rebuild_hash(world: &mut World) {
    let res = collision_res(world.world_size);
    if world.collision_spatial.res() != res {
        world.collision_spatial = UniformSpatialHash::with_dims(world.world_size, res);
    }
    let agents = &world.agents;
    world.collision_spatial.rebuild(&agents.position, |i| agents.is_alive(i as u32));
}

/// Separation steering for agent `i`: Σ over colliding neighbours within
/// `STEER_MARGIN·(r_i+r_j)` of `away · (reach − d)/reach`. Reads the collision
/// hash built at stage 1 this tick.
pub fn separation_steer(
    spatial: &UniformSpatialHash,
    agents: &AgentBuffers,
    i: usize,
    ws: f32,
) -> Vec2 {
    let pos = agents.position[i];
    let ri = body_radius(&agents.genome[i]);
    let ci = Locomotion::of(&agents.genome[i]);
    let mut acc = Vec2::ZERO;
    spatial.query(pos, COLLISION_CELL, |oid| {
        let j = oid as usize;
        if j == i || !ci.collides_with(Locomotion::of(&agents.genome[j])) {
            return;
        }
        let reach = (ri + body_radius(&agents.genome[j])) * STEER_MARGIN;
        let d = torus_delta(pos, agents.position[j], ws);
        let dist = d.length();
        if dist >= reach {
            return;
        }
        acc += away_dir(i as u32, oid, d, dist) * ((reach - dist) / reach);
    });
    acc
}

/// Stage 4': push overlapping colliding pairs apart to their minimum gap.
/// Rebuilds the fine hash from post-integrate positions, then runs
/// `RESOLVE_PASSES` Jacobi passes: each alive agent sums half of each overlap
/// along `away_dir` (read from the pass's snapshot), caps the push at
/// `MAX_PUSH`, and applies it only if the destination is valid for its class.
/// Between passes positions move ≤ `MAX_PUSH`, so a neighbour within the
/// max gap (1.5) is still within one hash cell (4.0) of the stale bucket.
/// No-op with the flag off.
pub fn resolve_overlaps(world: &mut World) {
    use rayon::prelude::*;
    if !world.territory_enabled {
        return;
    }
    rebuild_hash(world);
    let ws = world.world_size;
    let cap = world.agents.capacity();
    for _ in 0..RESOLVE_PASSES {
        let mut snap = std::mem::take(&mut world.collision_scratch);
        snap.clear();
        snap.extend_from_slice(&world.agents.position[..cap]);
        let spatial = &world.collision_spatial;
        let biome = &world.biome;
        let AgentBuffers { position, genome, alive, .. } = &mut world.agents;
        let (genome, alive) = (&*genome, &*alive);
        let snap_ref = &snap;
        position[..cap].par_iter_mut().enumerate().for_each(|(i, pos)| {
            if !alive[i] {
                return;
            }
            let p = snap_ref[i];
            let ci = Locomotion::of(&genome[i]);
            let ri = body_radius(&genome[i]);
            let mut push = Vec2::ZERO;
            spatial.query(p, COLLISION_CELL, |oid| {
                let j = oid as usize;
                if j == i || !ci.collides_with(Locomotion::of(&genome[j])) {
                    return;
                }
                let gap = ri + body_radius(&genome[j]);
                let d = torus_delta(p, snap_ref[j], ws);
                let dist = d.length();
                if dist >= gap {
                    return;
                }
                push += away_dir(i as u32, oid, d, dist) * ((gap - dist) * 0.5);
            });
            if push == Vec2::ZERO {
                return;
            }
            let len = push.length();
            if len > MAX_PUSH {
                push *= MAX_PUSH / len;
            }
            let target = wrap_torus(p + push, Vec2::splat(ws));
            if ci.can_occupy(biome.sample(target).terrain) {
                *pos = target;
            }
        });
        world.collision_scratch = snap;
    }
}
```

Note: the "stacked" test requires one pass to separate exactly-coincident equal-size agents. Each moves `gap/2` in opposite `TIE_DIRS` directions, so they end exactly `gap` apart after pass 1, and pass 2 sees no overlap. If `gap/2 > MAX_PUSH` this would take more passes, but `gap ≤ 1.5` so `gap/2 ≤ 0.75`.

- [ ] **Step 5: Wire into `tick.rs`**

Stage 1, after `world.spatial.rebuild(…)`:

```rust
    // Territory layer: fine collision hash for this tick's separation steer.
    if world.territory_enabled {
        crate::collision::rebuild_hash(world);
    }
```

Directly after the stage-4 `integrate_all(…);` call:

```rust
    // Stage 4': collision resolve (territory layer) — no two colliding bodies
    // end the move overlapping. Before needs/anchor/interact so every later
    // stage sees resolved positions. No-op with the flag off.
    crate::collision::resolve_overlaps(world);
```

In `decide_all`, add `let collision = &world.collision_spatial;` next to the other captures, and just before the final `// Normalize the movement intent` block (after the `affect_enabled` hijack), add:

```rust
            // Separation steering (territory layer): route around overlapping
            // bodies. Last in the stack so it still applies under the pen
            // override and the hijack; the stage-4' resolve backstops it.
            if territory_enabled {
                let sep = crate::collision::separation_steer(collision, agents, i, ws);
                action.move_x += crate::collision::SEP_PULL * sep.x;
                action.move_y += crate::collision::SEP_PULL * sep.y;
            }
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p anabios-core --lib collision spatial tick world`

Expected: pass.

Run: `cargo test -p anabios-core --release --test determinism trajectory_unchanged --test inventions`

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/collision.rs crates/anabios-core/src/lib.rs crates/anabios-core/src/spatial.rs crates/anabios-core/src/world.rs crates/anabios-core/src/tick.rs
git commit -m "feat(collision): body radii, separation steering, stage-4' min-gap resolve

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Flag-on integration coverage — habitat invariant, golden, round-trip, thread identity

**Files:**
- Modify: `crates/anabios-core/tests/invariants.rs`
- Modify: `crates/anabios-core/tests/determinism.rs`
- Modify: `crates/anabios-core/tests/save_load_roundtrip.rs`

**Interfaces:**
- Consumes: `scenarios/habitat-territories.toml`, `habitat::Locomotion`, `collision::body_radius`, `common::{world, run, ticks, assert_golden}`

- [ ] **Step 1: Habitat invariant** (append to `tests/invariants.rs`; add `mod common;` at the top if it's missing)

```rust
/// Territory layer: over a long flag-on run, no Land agent is ever on a Water
/// cell and no Water agent is ever on land (checked every 50 ticks).
#[test]
fn habitat_classes_never_leave_their_terrain() {
    use anabios_core::biome::TerrainType;
    use anabios_core::habitat::Locomotion;
    let mut w = common::world(include_str!("../../../scenarios/habitat-territories.toml"));
    assert!(w.territory_enabled);
    let horizon = common::ticks(2000);
    while w.tick < horizon {
        common::run(&mut w, 50);
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let class = Locomotion::of(&w.agents.genome[i]);
            let t = w.biome.sample(w.agents.position[i]).terrain;
            match class {
                Locomotion::Land => assert_ne!(t, TerrainType::Water, "tick {} land agent {id} in water", w.tick),
                Locomotion::Water => assert_eq!(t, TerrainType::Water, "tick {} water agent {id} on {t:?}", w.tick),
                Locomotion::Air => {}
            }
        }
    }
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p anabios-core --release --test invariants habitat_classes`

Expected: PASS. A failure means a path bypasses the gate; the likely suspects are the resolve push or relocation. Fix the root cause, don't loosen the test.

- [ ] **Step 3: Flag-on golden + thread identity**

In `tests/determinism.rs` add:

```rust
const HABITAT_SCENARIO: &str = include_str!("../../../scenarios/habitat-territories.toml");
const HABITAT_GOLDEN: &[(u64, u64)] = &[(0, 0), (100, 0), (1000, 0)];

#[test]
fn habitat_territories_matches_golden_hashes() {
    common::assert_golden("habitat-territories", HABITAT_SCENARIO, HABITAT_GOLDEN);
}
```

and add `include_str!("../../../scenarios/habitat-territories.toml"),` to the scenario list in `parallel_matches_serial_across_thread_counts`.

Pin the golden:

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --release --test determinism habitat_territories_matches_golden_hashes -- --nocapture`

Paste the printed table into `HABITAT_GOLDEN`.

- [ ] **Step 4: Round-trip entry** (in `save_load_roundtrip.rs`'s `roundtrip_tests!` table)

```rust
    territory_roundtrip:
        // Warm past two species steps (ticks 0/200/400) so territory EMA state
        // is non-trivial when saved.
        "../../../scenarios/habitat-territories.toml", 420,
        |w: &World| w.territory_enabled, "territory_enabled";
```

- [ ] **Step 5: Run the determinism set**

Run: `cargo test -p anabios-core --release --test determinism --test save_load_roundtrip territory habitat parallel`

Expected: pass, including 1 vs 2 vs 8 threads on the new scenario.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/tests/invariants.rs crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/save_load_roundtrip.rs
git commit -m "test(territory): habitat invariant, flag-on golden, round-trip, thread identity

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Performance bench + measurement probe + constant tuning

**Files:**
- Modify: `crates/anabios-core/benches/tick_bench.rs`
- Modify: `crates/anabios-core/tests/invariants.rs` (`#[ignore]` probe)
- Possibly modify: constants in `biome.rs` / `territory.rs` / `collision.rs` (tuning only), and `HABITAT_GOLDEN` if a constant changes

- [ ] **Step 1: Territory-on tick bench** (in `tick_bench.rs`, register the new fn in `criterion_group!`)

```rust
/// Territory layer overhead: the 10k tick with `territory_enabled` on vs the
/// same population off (founders relocated onto valid ground first, so the
/// comparison measures steady-state cost, not stranded agents).
fn bench_territory(c: &mut Criterion) {
    let mut group = c.benchmark_group("territory");
    group.sample_size(20);
    let off = build_population(10_000, 1);
    let mut on = off.clone();
    on.territory_enabled = true;
    for id in on.agents.iter_alive().collect::<Vec<_>>() {
        let i = id as usize;
        let class = anabios_core::habitat::Locomotion::of(&on.agents.genome[i]);
        if let Some(p) = anabios_core::habitat::nearest_valid(&on.biome, on.agents.position[i], class) {
            on.agents.position[i] = p;
        }
    }
    for (label, template) in [("off", off), ("on", on)] {
        group.bench_function(label, |b| {
            b.iter_batched(
                || template.clone(),
                |mut w| {
                    step(&mut w);
                    w
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}
```

Run: `cargo bench -p anabios-core --bench tick_bench -- territory`

Expected: the `on` median is ≤ 1.10 × the `off` median. Laptop timing drifts with heat, so re-run once if the two samples are within noise of the limit. If it's over budget, profile `resolve_overlaps` first. The likely fix is `RESOLVE_PASSES = 1`, justified by the probe in Step 3.

- [ ] **Step 2: Measurement probe** (append to `tests/invariants.rs`)

```rust
/// Measurement probe (not a gate): 8 seeds × 20k ticks of the flagship
/// scenario, reporting per-class populations, habitat violations, deep
/// overlaps (colliding pairs closer than half their gap), and the share of
/// members inside their species' territory. Run:
///   cargo test -p anabios-core --release --test invariants \
///     territory_measurement_probe -- --ignored --nocapture
#[test]
#[ignore]
fn territory_measurement_probe() {
    use anabios_core::biome::TerrainType;
    use anabios_core::collision::body_radius;
    use anabios_core::habitat::Locomotion;
    use anabios_core::scenario::Scenario;
    let base = include_str!("../../../scenarios/habitat-territories.toml");
    for seed in 1..=8u64 {
        let mut s = Scenario::parse_toml(base).expect("parse");
        s.seed = seed;
        let mut w = s.instantiate();
        common::run(&mut w, 20_000);
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        let mut pop = [0u32; 3];
        let (mut violations, mut deep, mut inside) = (0u32, 0u32, 0u32);
        for &id in &ids {
            let i = id as usize;
            let c = Locomotion::of(&w.agents.genome[i]);
            pop[c.index()] += 1;
            let t = w.biome.sample(w.agents.position[i]).terrain;
            if !c.can_occupy(t) {
                violations += 1;
            }
            if let Some(tr) = w.species_territories.get(w.agents.species_id[i] as usize) {
                let d = anabios_core::spatial::torus_distance(tr.centre(), w.agents.position[i], w.world_size);
                if tr.is_set() && d <= tr.r {
                    inside += 1;
                }
            }
        }
        for (k, &a) in ids.iter().enumerate() {
            for &b in &ids[k + 1..] {
                let (ga, gb) = (&w.agents.genome[a as usize], &w.agents.genome[b as usize]);
                if !Locomotion::of(ga).collides_with(Locomotion::of(gb)) {
                    continue;
                }
                let gap = body_radius(ga) + body_radius(gb);
                let d = anabios_core::spatial::torus_distance(
                    w.agents.position[a as usize],
                    w.agents.position[b as usize],
                    w.world_size,
                );
                if d < 0.5 * gap {
                    deep += 1;
                }
            }
        }
        let n = ids.len().max(1) as f32;
        println!(
            "seed={seed} alive={} land={} water={} air={} violations={violations} deep_overlaps={deep} inside_territory={:.1}% water_cells_with_biomass={}",
            ids.len(), pop[0], pop[1], pop[2], 100.0 * inside as f32 / n,
            w.biome.cells.iter().filter(|c| c.terrain == TerrainType::Water && c.plant_biomass > 0.1).count(),
        );
    }
}
```

`Scenario.seed` is a `pub` field; if it isn't, rebuild the TOML string with `base.replacen("seed = 11", &format!("seed = {seed}"), 1)`.

- [ ] **Step 3: Run the probe and record results**

Run: `cargo test -p anabios-core --release --test invariants territory_measurement_probe -- --ignored --nocapture`

Acceptance, per the spec's targets:
- `violations == 0` on every seed.
- `deep_overlaps` ≈ 0; a handful at 1500 agents is acceptable.
- `inside_territory ≥ 80%` on most seeds.
- `water > 0` and `air > 0` on at least 6 of 8 seeds.

If aquatic lineages collapse, raise `AQUATIC_CAPACITY` (try 6.0) or `AQUATIC_REGROWTH_RATE`. If inside-share is low, raise `TERRITORY_PULL` (try 1.5) or `TERRITORY_K`. Tune constants only, never the architecture. After any constant change:
- re-pin `HABITAT_GOLDEN` (Task 8 Step 3 command);
- re-run `--test determinism trajectory_unchanged` (must still pass untouched).

- [ ] **Step 4: Save the findings**

Write `docs/superpowers/specs/2026-09-25-territory-habitat-collision-findings.md` containing:
- the per-seed probe table (verbatim output);
- the bench `on`/`off` medians;
- any constants changed, with before and after values;
- a one-paragraph verdict.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/benches/tick_bench.rs crates/anabios-core/tests/invariants.rs docs/superpowers/specs/2026-09-25-territory-habitat-collision-findings.md
# plus any tuned source file and tests/determinism.rs if HABITAT_GOLDEN was re-pinned
git commit -m "perf+probe(territory): overhead bench, 8-seed measurement probe, findings

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Godot bridge accessors

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs` (3 `#[func]`s near `settlement_sites` ~`:1490`, 2 pure helpers near `body_tags_of` ~`:1663`, tests near `body_tags_match_module_counts` ~`:2520`)

**Interfaces:**
- Consumes: `World::{territory_enabled, species_territories, species_member_counts, species_centroids}`, `habitat::Locomotion`, `hsv_to_color(h, s, v) -> Color` (`lib.rs:2016`)
- Produces (GDScript-visible):
  - `territory_active() -> bool`
  - `alive_locomotion() -> PackedByteArray` (0 land / 1 water / 2 air, in `alive_positions` order; empty when off)
  - `species_territories() -> Array<VarDictionary>`, each with keys `species_id: int, pos: Vector2, radius: float, locomotion: int, color: Color`

- [ ] **Step 1: Write the failing tests** (in the `lib.rs` tests module)

```rust
    fn habitat_world() -> anabios_core::World {
        let toml = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/habitat-territories.toml"
        ))
        .expect("read habitat-territories.toml");
        let mut w = anabios_core::Scenario::parse_toml(&toml).unwrap().instantiate();
        for _ in 0..5 {
            anabios_core::tick::step(&mut w);
        }
        w
    }

    #[test]
    fn locomotion_export_is_empty_when_off_and_aligned_when_on() {
        assert!(super::locomotion_of(&minimal_world()).is_empty());
        let w = habitat_world();
        let loco = super::locomotion_of(&w);
        assert_eq!(loco.len(), w.agents.live_count() as usize);
        assert!(loco.contains(&0) && loco.contains(&1) && loco.contains(&2));
    }

    #[test]
    fn territory_export_lists_live_initialized_species() {
        assert!(super::territories_of(&minimal_world()).is_empty());
        let w = habitat_world();
        let t = super::territories_of(&w);
        assert!(t.len() >= 3, "three founder species, got {}", t.len());
        for v in &t {
            assert!(v.r >= anabios_core::territory::TERRITORY_R_MIN);
            assert!(w.species_member_counts[v.species_id as usize] > 0);
        }
    }
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p anabios-godot locomotion_export territory_export`

Expected: compile error.

- [ ] **Step 3: Implement the pure helpers** (module level, next to `body_tags_of`)

```rust
/// Locomotion class per alive agent (0 land / 1 water / 2 air), ascending id
/// order; empty when the territory layer is off.
fn locomotion_of(w: &anabios_core::World) -> Vec<u8> {
    if !w.territory_enabled {
        return Vec::new();
    }
    w.agents
        .iter_alive()
        .map(|id| anabios_core::habitat::Locomotion::of(&w.agents.genome[id as usize]) as u8)
        .collect()
}

/// One live, initialized species territory with its species colour (HSV of
/// the species' centroid genome, clamped like `alive_colors`).
struct TerritoryView {
    species_id: u32,
    cx: f32,
    cy: f32,
    r: f32,
    class: u8,
    hsv: (f32, f32, f32),
}

fn territories_of(w: &anabios_core::World) -> Vec<TerritoryView> {
    use anabios_core::genome::GenomeSlot;
    w.species_territories
        .iter()
        .enumerate()
        .filter(|(sid, t)| {
            t.is_set() && w.species_member_counts.get(*sid).copied().unwrap_or(0) > 0
        })
        .map(|(sid, t)| {
            let g = &w.species_centroids[sid];
            TerritoryView {
                species_id: sid as u32,
                cx: t.cx,
                cy: t.cy,
                r: t.r,
                class: t.class as u8,
                hsv: (
                    g.get(GenomeSlot::ColorHue),
                    g.get(GenomeSlot::ColorSat).clamp(0.4, 1.0),
                    g.get(GenomeSlot::ColorVal).clamp(0.5, 1.0),
                ),
            }
        })
        .collect()
}
```

- [ ] **Step 4: Implement the `#[func]`s** (after `settlement_sites`)

```rust
    /// Whether the territory/habitat/collision layer is enabled — gates the
    /// territory ground overlay and the airborne body lift.
    #[func]
    fn territory_active(&self) -> bool {
        self.inner.as_ref().map(|w| w.territory_enabled).unwrap_or(false)
    }

    /// Locomotion class per alive agent (0 land, 1 water, 2 air), same order
    /// as `alive_positions`. Empty when the territory layer is off.
    #[func]
    fn alive_locomotion(&self) -> PackedByteArray {
        let mut out = PackedByteArray::new();
        if let Some(w) = self.inner.as_ref() {
            for c in locomotion_of(w) {
                out.push(c);
            }
        }
        out
    }

    /// Live species territories: `{species_id, pos, radius, locomotion,
    /// color}` per species with members and an initialized range. Empty when
    /// the layer is off.
    #[func]
    fn species_territories(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(w) = self.inner.as_ref() else {
            return out;
        };
        for t in territories_of(w) {
            let mut d = VarDictionary::new();
            d.set("species_id", t.species_id as i64);
            d.set("pos", Vector2::new(t.cx, t.cy));
            d.set("radius", t.r);
            d.set("locomotion", t.class as i64);
            d.set("color", hsv_to_color(t.hsv.0, t.hsv.1, t.hsv.2));
            out.push(&d);
        }
        out
    }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p anabios-godot`

Expected: pass.

Run: `cargo build -p anabios-godot`

Expected: builds.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): territory_active / alive_locomotion / species_territories bridge

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Viewer — territory ground overlay + airborne lift

**Files:**
- Create: `game/scripts/territory_layer.gd`, `game/scripts/test_territory_layer.gd`
- Modify: `game/scripts/overlay_manager.gd`, `game/scripts/legend_panel.gd:6-8`, `game/scripts/main.gd` (after the caravan layer ~`:196`), `game/scripts/agent_layer.gd` (~`:130-140` consts, `refresh` ~`:388`, instance loop ~`:590-610`), `game/scripts/test_agent_layer.gd`, `.github/workflows/ci.yml:263`

**Interfaces:**
- Consumes: bridge `territory_active()`, `species_territories()`, `alive_locomotion()`, `world_size()`
- Produces:
  - `OverlayManager.GROUND_TERRITORY = 8`, `ground_is_territory() -> bool`
  - `TerritoryLayer.torus_copies(c: Vector2, world: float) -> PackedVector2Array` (9 points)
  - `AgentLayer.air_lift(sz: float, airborne: bool) -> Vector2`

- [ ] **Step 1: Write the failing GDScript tests**

`game/scripts/test_territory_layer.gd`:

```gdscript
extends SceneTree
# Headless unit test for territory_layer.gd's pure static helper:
# torus_copies(), the 3x3 world-shifted copies of a territory centre that
# let one ring draw across the torus seam at any camera position. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_territory_layer.gd
# Exits 0 on success, 1 on the first failed assertion.

const TerritoryLayer = preload("res://scripts/territory_layer.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	var c := Vector2(10.0, 1000.0)
	var copies: PackedVector2Array = TerritoryLayer.torus_copies(c, 1024.0)
	_check(copies.size() == 9, "nine copies, got %d" % copies.size())
	_check(copies.has(c), "the centre itself is a copy")
	_check(copies.has(c + Vector2(1024.0, -1024.0)), "diagonal shift present")
	_check(copies.has(c + Vector2(-1024.0, 0.0)), "west shift present")
	quit(1 if _failed else 0)
```

In `game/scripts/test_agent_layer.gd`, add a check function and call it from `_init` alongside the existing ones:

```gdscript
func _check_air_lift() -> void:
	_check(AgentLayer.air_lift(10.0, false) == Vector2.ZERO, "ground figures are not lifted")
	var lift: Vector2 = AgentLayer.air_lift(10.0, true)
	_check(lift.x == 0.0 and lift.y < 0.0, "flyers ride above their ground point")
	_check(
		is_equal_approx(lift.y, -10.0 * AgentLayer.AIR_LIFT), "lift scales with body size"
	)
```

- [ ] **Step 2: Run to confirm failure**

Run:
```bash
godot --headless --rendering-driver dummy --path game -s res://scripts/test_territory_layer.gd
```

Expected: a parse error or a non-zero exit, because the script is missing. Per the verifying-godot-headless skill, check the log for `SCRIPT ERROR`; don't trust exit 0 alone.

- [ ] **Step 3: Implement `territory_layer.gd`**

```gdscript
extends Node2D
# Territory ground overlay (territory/habitat/collision layer): a translucent
# disc + ring per live species at its territory centre and radius, in the
# species colour, shown only while the [G] ground mode is "territory". Plain
# _draw() primitives — no shader, so it is Metal-safe. Draws the 3x3 torus
# copies itself (torus_copies) instead of relying on main.gd's wrap clones.

const RING_WIDTH := 2.0
const FILL_ALPHA := 0.08
# Territories only change every species step (200 ticks); a slow refresh is
# visually identical and keeps the bridge walk off the hot path.
const REFRESH_FRAMES := 15

var _sim
var _overlay
var _sites: Array = []
var _world: float = 0.0
var _frame: int = 0


func setup(sim, overlay) -> void:
	_sim = sim
	_overlay = overlay
	z_index = 1


# The centre plus its eight one-world shifts, so a ring near the seam (or a
# camera looking across it) still shows. Pure; unit-tested.
static func torus_copies(c: Vector2, world: float) -> PackedVector2Array:
	var out := PackedVector2Array()
	for gy in range(-1, 2):
		for gx in range(-1, 2):
			out.append(c + Vector2(gx * world, gy * world))
	return out


func _process(_delta: float) -> void:
	var active: bool = (
		_sim != null and bool(_sim.territory_active()) and _overlay.ground_is_territory()
	)
	visible = active
	if not active:
		return
	_frame += 1
	if _frame % REFRESH_FRAMES == 1 or _sites.is_empty():
		_sites = _sim.species_territories()
		_world = float(_sim.world_size())
		queue_redraw()


func _draw() -> void:
	for s in _sites:
		var col: Color = s["color"]
		var r: float = s["radius"]
		var fill := Color(col.r, col.g, col.b, FILL_ALPHA)
		for c in torus_copies(s["pos"], _world):
			draw_circle(c, r, fill)
			draw_arc(c, r, 0.0, TAU, 96, col, RING_WIDTH, true)
```

- [ ] **Step 4: Ground mode, legend, wiring**

`overlay_manager.gd`:
- Add `const GROUND_TERRITORY := 8` after `GROUND_MARKETS`, and change `GROUND_MAX := 9`.
- Add:

```gdscript
func ground_is_territory() -> bool:
	return ground_mode == GROUND_TERRITORY
```

- In `_cycle_ground()` append:

```gdscript
	# Skip TERRITORY when the territory layer is disabled.
	if ground_mode == GROUND_TERRITORY and not bool(sim.territory_active()):
		ground_mode = GROUND_BIOME
```

`biome_renderer.gd` needs no change: the territory mode falls through to `mode = -1`, the plain biome view under the rings.

`legend_panel.gd`: extend `GROUND_NAMES` with `"territory"` as the ninth entry. Keep `gdformat` line width; if it overflows, break the array one entry per line.

`main.gd`, after the `caravan_layer` block:

```gdscript
	# Species territory rings ([G] ground mode "territory"; territory layer).
	var territory_layer = preload("res://scripts/territory_layer.gd").new()
	territory_layer.name = "TerritoryLayer"
	add_child(territory_layer)
	move_child(territory_layer, module_layers.get_index())
	territory_layer.setup(sim, overlay)
```

- [ ] **Step 5: Airborne lift in `agent_layer.gd`**

Next to the `SHADOW_*` consts:

```gdscript
# Airborne figures (territory layer, Air locomotion) ride AIR_LIFT body sizes
# above their ground point; their contact shadow stays on the ground, shrunk
# by AIR_SHADOW_SCALE and never cut at the waterline.
const AIR_LIFT := 0.9
const AIR_SHADOW_SCALE := 0.6
const LOCO_AIR := 2
```

A static helper next to `crowd_cell_for`:

```gdscript
static func air_lift(sz: float, airborne: bool) -> Vector2:
	return Vector2(0.0, -sz * AIR_LIFT) if airborne else Vector2.ZERO
```

In `refresh()`, next to the other per-agent arrays (after `inv_masks`):

```gdscript
	# Locomotion class per agent (empty unless the territory layer is on).
	var loco: PackedByteArray = sim.alive_locomotion()
	var have_loco: bool = loco.size() == n
```

In the instance loop, replace the transform, wading and shadow lines:

```gdscript
		var airborne: bool = have_loco and loco[i] == LOCO_AIR
		var t: Transform2D = Transform2D(0.0, Vector2(sz, sz), 0.0, smooth[i] + air_lift(sz, airborne))
		mm.set_instance_transform_2d(j, t)
		var wading: bool = not airborne and wading_check and _biome.is_water_at(smooth[i])
		# A wading figure casts no contact shadow on the water; a flyer's shrinks.
		var sh_w: float = 0.0 if wading else sz * SHADOW_W * (AIR_SHADOW_SCALE if airborne else 1.0)
```

Keep the existing `shadows.set_instance_transform_2d(…)` call as is, since it already uses `sh_w` and the ground point `smooth[i]`. Run `gdformat game/scripts/agent_layer.gd` to rewrap long lines.

- [ ] **Step 6: CI smoke list**

In `.github/workflows/ci.yml:263` append ` test_territory_layer` to the `scripts/godot-smoke.sh scripts …` list.

- [ ] **Step 7: Run the viewer checks**

```bash
gdformat game/scripts/territory_layer.gd game/scripts/test_territory_layer.gd game/scripts/agent_layer.gd game/scripts/test_agent_layer.gd game/scripts/overlay_manager.gd game/scripts/legend_panel.gd game/scripts/main.gd
gdformat --check game/scripts/ && gdlint game/scripts/
cargo build -p anabios-godot
scripts/godot-smoke.sh scripts test_territory_layer test_agent_layer
scripts/godot-smoke.sh scenes res://scenes/menu.tscn res://scenes/main.tscn
```

Expected: all clean. Confirm no `SCRIPT ERROR` appears in the smoke logs, and `wc -l game/scripts/agent_layer.gd game/scripts/main.gd` is ≤ 1000 each.

- [ ] **Step 8: Visual verification**

Use the `running-the-viewer` skill:
1. Run `scenarios/habitat-territories.toml`.
2. Screenshot at 1× and 3× zoom in the default ground mode, and with `[G]` cycled to `territory`.
3. Confirm the rings are visible, flyers are lifted over water with small ground shadows, swimmers are in the sea, and there are no stacked figures at 3×.

Save the screenshots to the scratchpad and attach them to the PR description in Task 12.

- [ ] **Step 9: Commit**

```bash
git add game/scripts/territory_layer.gd game/scripts/test_territory_layer.gd game/scripts/overlay_manager.gd game/scripts/legend_panel.gd game/scripts/main.gd game/scripts/agent_layer.gd game/scripts/test_agent_layer.gd .github/workflows/ci.yml
git commit -m "feat(viewer): territory ground overlay + airborne lift for Air locomotion

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Docs, full local gate, PR

**Files:**
- Modify: `docs/determinism-contract.md`, `docs/scenarios.md`

- [ ] **Step 1: Docs**

In `docs/determinism-contract.md`, in the skip-rules list, add entries in the file's existing format:
- `World.collision_spatial` and `World.collision_scratch` are per-tick scratch (`#[serde(skip)]`, rebuilt before every read).
- `World.species_territories` is path-dependent EMA state (serialized).
- The Locomotion class is derived from the genome, not stored.

In `docs/scenarios.md` add a `habitat-territories` entry in the same shape as its neighbours: purpose, the flag, the three pinned classes, and the probe command from Task 9.

- [ ] **Step 2: Full local gate**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --lib
cargo test -p anabios-core --release --test determinism --test save_load_roundtrip --test invariants --test all_scenarios
cargo bench --workspace --no-run
gdformat --check game/scripts/ && gdlint game/scripts/
```

Expected: all clean. For `cfg(coverage)`-only paths, run `RUSTFLAGS="--cfg coverage" cargo check -p anabios-core --tests`.

- [ ] **Step 3: Commit docs, push, open the PR**

```bash
git add docs/determinism-contract.md docs/scenarios.md
git commit -m "docs: territory layer determinism rules + habitat-territories scenario

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git push -u origin claude/territory-habitat-collision
gh pr create --title "Territory, habitat & collision layer (FORMAT_VERSION 44)" --body-file <scratchpad>/pr-body.md
```

The PR body should cover:
- what shipped;
- the flag-off guarantee (the trajectory guards plus layout-only golden regen);
- the probe table from the findings doc;
- bench numbers;
- viewer screenshots;
- a `FORMAT_VERSION 44` collision warning for other open branches.

End it with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
