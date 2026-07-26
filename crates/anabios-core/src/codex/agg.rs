//! Per-tick per-species aggregation: `SpeciesAggTable` / `SpeciesAgg`.
//!
//! One fused `iter_alive` pass at the top of `observe_all` builds dense,
//! reused per-species storage shared by every detector — replacing the ~7
//! hand-rolled per-detector population scans (and their per-tick `BTreeMap`
//! churn). All accumulations visit agents in ascending id order, so every
//! f32/f64 sum is bit-identical to the scans this replaced. Extracted verbatim
//! from `codex/mod.rs`.

use super::*;

/// Per-species aggregates for one tick, built in a single `iter_alive` pass
/// at the top of `observe_all` and shared by every detector. Replaces the
/// ~7 hand-rolled per-detector population scans (and their per-tick
/// `BTreeMap` churn) with one pass over dense, reused storage.
///
/// Lives on `World` behind `#[serde(skip)]` — never part of the snapshot or
/// the state hash. All accumulations visit agents in ascending id order so
/// every f32/f64 sum is bit-identical to the scans this replaced.
#[derive(Debug, Clone, Default)]
pub struct SpeciesAggTable {
    /// Dense per-species entries, indexed by species id. Grows as needed.
    /// `pub(super)` (= visible within `codex`): the detectors reach the table
    /// through `get`/`active`, but the codex tests construct/inspect it
    /// directly — as they could when this type lived in `mod.rs`.
    pub(super) entries: Vec<SpeciesAgg>,
    /// Species with ≥1 alive member this tick, ascending.
    pub(super) active: Vec<u32>,
}

/// Aggregates for one species for one tick.
#[derive(Debug, Clone)]
pub struct SpeciesAgg {
    pub count: u32,
    pub sum_x: f64,
    pub sum_y: f64,
    /// Alive member agent indices, ascending.
    pub member_idx: Vec<usize>,
    pub has_comm: bool,
    pub has_pheromone: bool,
    /// Bitmask of present `ModuleType` discriminants (< 16 types).
    pub module_mask: u16,
    /// Bitmask of present program `node_kind` discriminants (≤ 42 kinds).
    pub node_mask: u64,
    /// Raw per-terrain member counts (normalized by `count` on read).
    pub terrain_counts: [f32; TERRAIN_SLOTS],
    pub meme_sums: [f64; MEME_CHANNELS],
    /// Sum of `SensorRegister::crowding`; 0 when the sensors scratch is
    /// undersized (standalone `observe_all` calls outside the tick).
    pub crowding_sum: f64,
    /// Sum of per-member `effective_speed_max` (module speed; for the E6
    /// flight detector's relative-speed test).
    pub speed_sum: f64,
    pub weapon_sum: f64,
    pub armor_sum: f64,
    /// Sum of per-member `effective_diet_carnivory` (0 = herbivore …
    /// 1 = carnivore); mean classifies the species' trophic level for the
    /// cascade detector.
    pub diet_sum: f64,
    /// Per-invention count of members at or above the held threshold (for
    /// `InventionAdopted`). All zero when the invention tree is inactive.
    pub invention_counts: [u32; crate::invention::INVENTION_COUNT],
    /// Per-practice count of members holding each maladaptive practice (for
    /// `PracticeAdopted`). All zero when cognition is inactive.
    pub practice_counts: [u32; crate::practice::PRACTICE_COUNT],
    /// Distinct biome cells occupied by members this tick (scratch for the
    /// E4 spatial detectors). Cell index = row * res + col. Held as a
    /// capacity-retaining `Vec` (cleared, not freed, each tick) that `build`
    /// leaves **sorted and deduplicated** — so `.len()` is the distinct-cell
    /// count and iteration is deterministic (ascending), preserving the
    /// codex's "ordered iteration, never a hash container" invariant while
    /// avoiding the per-tick `BTreeSet` node churn.
    pub occ_cells: Vec<u32>,
    /// Per-slot genome sums / squared sums over members (for the E5
    /// genome-moment history). 50 slots each.
    pub genome_sums: [f64; 50],
    pub genome_sumsq: [f64; 50],
}

impl Default for SpeciesAgg {
    fn default() -> Self {
        Self {
            count: 0,
            sum_x: 0.0,
            sum_y: 0.0,
            member_idx: Vec::new(),
            has_comm: false,
            has_pheromone: false,
            module_mask: 0,
            node_mask: 0,
            terrain_counts: [0.0; TERRAIN_SLOTS],
            meme_sums: [0.0; MEME_CHANNELS],
            crowding_sum: 0.0,
            speed_sum: 0.0,
            weapon_sum: 0.0,
            armor_sum: 0.0,
            diet_sum: 0.0,
            invention_counts: [0; crate::invention::INVENTION_COUNT],
            practice_counts: [0; crate::practice::PRACTICE_COUNT],
            occ_cells: Vec::new(),
            genome_sums: [0.0; 50],
            genome_sumsq: [0.0; 50],
        }
    }
}

impl SpeciesAgg {
    fn reset(&mut self) {
        self.count = 0;
        self.sum_x = 0.0;
        self.sum_y = 0.0;
        self.member_idx.clear();
        self.has_comm = false;
        self.has_pheromone = false;
        self.module_mask = 0;
        self.node_mask = 0;
        self.terrain_counts = [0.0; TERRAIN_SLOTS];
        self.meme_sums = [0.0; MEME_CHANNELS];
        self.crowding_sum = 0.0;
        self.speed_sum = 0.0;
        self.weapon_sum = 0.0;
        self.armor_sum = 0.0;
        self.diet_sum = 0.0;
        self.invention_counts = [0; crate::invention::INVENTION_COUNT];
        self.practice_counts = [0; crate::practice::PRACTICE_COUNT];
        self.occ_cells.clear();
        self.genome_sums = [0.0; 50];
        self.genome_sumsq = [0.0; 50];
    }

    /// This tick's centroid (mean alive position), `(0,0)` when empty.
    /// f64 accumulator divided by member count — identical to the former
    /// `compute_centroids`.
    #[inline]
    pub fn centroid(&self) -> (f32, f32) {
        let nf = self.count.max(1) as f64;
        ((self.sum_x / nf) as f32, (self.sum_y / nf) as f32)
    }
}

impl SpeciesAggTable {
    /// Entry for `sid`, if the species has ≥1 alive member this tick.
    #[inline]
    pub fn get(&self, sid: u32) -> Option<&SpeciesAgg> {
        self.entries.get(sid as usize).filter(|e| e.count > 0)
    }

    /// Species ids with ≥1 alive member, ascending.
    #[inline]
    pub fn active(&self) -> &[u32] {
        &self.active
    }

    /// Rebuild from current world state. One `iter_alive` pass.
    pub fn build(&mut self, world: &World) {
        use crate::module::{self, ModuleType};
        for &sid in &self.active {
            if let Some(e) = self.entries.get_mut(sid as usize) {
                e.reset();
            }
        }
        self.active.clear();
        let sensors_ok = world.sensors.len() >= world.agents.capacity();
        for id in world.agents.iter_alive() {
            let i = id as usize;
            let sid = world.agents.species_id[i];
            let idx = sid as usize;
            if idx >= self.entries.len() {
                self.entries.resize(idx + 1, SpeciesAgg::default());
            }
            let e = &mut self.entries[idx];
            if e.count == 0 {
                self.active.push(sid);
            }
            e.count += 1;
            let pos = world.agents.position[i];
            e.sum_x += pos.x as f64;
            e.sum_y += pos.y as f64;
            e.member_idx.push(i);
            let modules = &world.agents.modules[i];
            if !e.has_comm && module::has(modules, ModuleType::Communicator) {
                e.has_comm = true;
            }
            if !e.has_pheromone && module::has(modules, ModuleType::Pheromone) {
                e.has_pheromone = true;
            }
            for m in modules.iter() {
                e.module_mask |= 1u16 << (m.module_type() as u8);
            }
            for node in world.agents.program[i].nodes.iter().copied() {
                e.node_mask |= 1u64 << crate::program::Program::node_kind(node);
            }
            let (col, row) = world.biome.cell_coords(pos);
            let terrain = world.biome.at(col, row).terrain as usize;
            e.terrain_counts[terrain.min(TERRAIN_SLOTS - 1)] += 1.0;
            e.occ_cells.push(world.biome.cell_index(col, row) as u32);
            for (ch, s) in e.meme_sums.iter_mut().enumerate() {
                *s += world.agents.meme_vector[i][ch] as f64;
            }
            if sensors_ok {
                e.crowding_sum += world.sensors[i].crowding as f64;
            }
            e.diet_sum += module::effective_diet_carnivory(modules) as f64;
            e.speed_sum += module::effective_speed_max(modules) as f64;
            for (slot, gv) in world.agents.genome[i].0.iter().enumerate() {
                let x = *gv as f64;
                e.genome_sums[slot] += x;
                e.genome_sumsq[slot] += x * x;
            }
            if world.inventions_enabled {
                let inv_mask = crate::invention::held_mask(&world.agents.meme_vector[i]);
                crate::invention::for_each_set_bit(inv_mask, |k| e.invention_counts[k] += 1);
            }
            if world.cognition_enabled {
                for (p, c) in e.practice_counts.iter_mut().enumerate() {
                    if crate::practice::has(&world.agents.meme_vector[i], p) {
                        *c += 1;
                    }
                }
            }
            e.weapon_sum +=
                module::effective_weapon(modules).map(|w| w.damage).unwrap_or(0.0) as f64;
            e.armor_sum += module::effective_armor_protection(modules) as f64;
        }
        // `occ_cells` was pushed once per member (with duplicates when members
        // share a cell); collapse each active species' list to the sorted set
        // of distinct cells so `.len()` and iteration match the former
        // `BTreeSet` exactly.
        for &sid in &self.active {
            let cells = &mut self.entries[sid as usize].occ_cells;
            cells.sort_unstable();
            cells.dedup();
        }
        self.active.sort_unstable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;

    /// Drift guard for the serde-skip accumulator footgun. `SpeciesAggTable`
    /// reuses `SpeciesAgg` entries across ticks: `build` populates them and
    /// `reset` is supposed to zero them again. If a field is ever added to
    /// `build` (and `Default`) but not to `reset`, a reused entry would carry
    /// stale scratch into the next tick — and because those values feed hashed
    /// detector events, that silently corrupts determinism.
    ///
    /// Rather than enumerate fields (which would drift with the struct), this
    /// populates an entry through a real `build`, then asserts `reset` returns
    /// it to exactly `Default`. Any field `build` writes but `reset` misses
    /// shows up as a mismatch here. Debug-string comparison is intentional:
    /// it ignores `Vec`/`BTreeSet` spare capacity (which `reset`'s `clear`
    /// retains) and compares only observable contents.
    #[test]
    fn reset_restores_default_state() {
        let mut w = World::new(7);
        for k in 0..8u32 {
            let pos = Vec2::new(100.0 + k as f32 * 13.0, 200.0 + k as f32 * 7.0);
            w.spawn_agent(pos, Genome::neutral());
        }
        let mut table = SpeciesAggTable::default();
        table.build(&w);
        assert!(!table.active.is_empty(), "expected at least one active species");

        let mut entry = table.entries[table.active[0] as usize].clone();
        let default_dbg = format!("{:?}", SpeciesAgg::default());
        // Guard against a vacuous test: build must actually have populated it.
        assert_ne!(format!("{entry:?}"), default_dbg, "build() should populate the entry");

        entry.reset();
        assert_eq!(
            format!("{entry:?}"),
            default_dbg,
            "reset() must zero every field build() populates — a field added to \
             build()+Default but not reset() would leak stale scratch across ticks"
        );
    }
}
