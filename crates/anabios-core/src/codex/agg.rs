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
    /// Count of clinically sick members (infection ≥ `disease::SHED_MIN`) —
    /// the disease detectors' "infected fraction" numerator. Zero when
    /// `disease_enabled` is off.
    pub infected_count: u32,
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
    /// Alive male members (E12). Zero when sexual dimorphism is off (every
    /// agent is sex=false then); the SexRatioCollapse detector reads this
    /// only under the flag.
    pub male_count: u32,
    /// Alive tamed members (E13, `livestock_of != AGENT_NULL`). Zero when
    /// domestication is off; the LivestockHerd detector reads this only
    /// under the flag.
    pub livestock_count: u32,
    /// Per-affect-system activation sums over alive members (for species
    /// means; M-F). Zero when the affect layer is off — the affect column is
    /// all-zero then anyway.
    pub affect_sum: [f64; crate::affect::AFFECT_SYSTEMS],
    /// Member counts at/above the per-system HIGH_* thresholds (M-F).
    pub high_fear: u32,
    pub high_seek: u32,
    pub high_rage: u32,
    pub high_panic: u32,
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
            infected_count: 0,
            occ_cells: Vec::new(),
            genome_sums: [0.0; 50],
            genome_sumsq: [0.0; 50],
            male_count: 0,
            livestock_count: 0,
            affect_sum: [0.0; crate::affect::AFFECT_SYSTEMS],
            high_fear: 0,
            high_seek: 0,
            high_rage: 0,
            high_panic: 0,
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
        self.infected_count = 0;
        self.occ_cells.clear();
        self.genome_sums = [0.0; 50];
        self.genome_sumsq = [0.0; 50];
        self.male_count = 0;
        self.livestock_count = 0;
        self.affect_sum = [0.0; crate::affect::AFFECT_SYSTEMS];
        self.high_fear = 0;
        self.high_seek = 0;
        self.high_rage = 0;
        self.high_panic = 0;
    }

    /// Head count, centroid accumulators and the ascending member index list.
    fn absorb_membership(&mut self, world: &World, i: usize) {
        self.count += 1;
        let pos = world.agents.position[i];
        self.sum_x += pos.x as f64;
        self.sum_y += pos.y as f64;
        self.member_idx.push(i);
        if world.agents.sex.get(i).map(|b| *b).unwrap_or(false) {
            self.male_count += 1;
        }
        if world.agents.livestock_of[i] != crate::agent::AGENT_NULL {
            self.livestock_count += 1;
        }
        if world.disease_enabled && world.agents.infection[i] >= crate::disease::SHED_MIN {
            self.infected_count += 1;
        }
    }

    /// Body plan: which modules and program nodes the lineage carries, and the
    /// continuous morphology sums the detectors compare across species.
    fn absorb_morphology(&mut self, world: &World, i: usize) {
        use crate::module::{self, ModuleType};
        let modules = &world.agents.modules[i];
        if !self.has_comm && module::has(modules, ModuleType::Communicator) {
            self.has_comm = true;
        }
        if !self.has_pheromone && module::has(modules, ModuleType::Pheromone) {
            self.has_pheromone = true;
        }
        for m in modules.iter() {
            self.module_mask |= 1u16 << (m.module_type() as u8);
        }
        for node in world.agents.program[i].nodes.iter().copied() {
            self.node_mask |= 1u64 << crate::program::Program::node_kind(node);
        }
        self.diet_sum += module::effective_diet_carnivory(modules) as f64;
        self.speed_sum += module::effective_speed_max(modules) as f64;
        self.weapon_sum +=
            module::effective_weapon(modules).map(|w| w.damage).unwrap_or(0.0) as f64;
        self.armor_sum += module::effective_armor_protection(modules) as f64;
    }

    /// Where the lineage lives: per-terrain member counts and the occupied
    /// biome cells (deduplicated once the pass completes).
    fn absorb_habitat(&mut self, world: &World, i: usize) {
        let pos = world.agents.position[i];
        let (col, row) = world.biome.cell_coords(pos);
        let terrain = world.biome.at(col, row).terrain as usize;
        self.terrain_counts[terrain.min(TERRAIN_SLOTS - 1)] += 1.0;
        self.occ_cells.push(world.biome.cell_index(col, row) as u32);
    }

    /// Per-slot genome sums and sums-of-squares (mean and variance on read).
    fn absorb_genome(&mut self, world: &World, i: usize) {
        for (slot, gv) in world.agents.genome[i].as_slice().iter().enumerate() {
            let x = *gv as f64;
            self.genome_sums[slot] += x;
            self.genome_sumsq[slot] += x * x;
        }
    }

    /// Meme channel sums plus the held-invention and held-practice head counts.
    /// The two counts are flag-gated, so a scenario with the trees off leaves
    /// them at zero exactly as before those trees existed.
    fn absorb_culture(&mut self, world: &World, i: usize) {
        for (ch, s) in self.meme_sums.iter_mut().enumerate() {
            *s += world.agents.meme_vector[i][ch] as f64;
        }
        if world.inventions_enabled {
            let inv_mask = crate::invention::held_mask(&world.agents.meme_vector[i]);
            crate::invention::for_each_set_bit(inv_mask, |k| self.invention_counts[k] += 1);
        }
        if world.cognition_enabled {
            for (p, c) in self.practice_counts.iter_mut().enumerate() {
                if crate::practice::has(&world.agents.meme_vector[i], p) {
                    *c += 1;
                }
            }
        }
    }

    /// Affect-layer sums and the four "how many members are running hot"
    /// counters the mass-emotion detectors threshold on.
    fn absorb_affect(&mut self, world: &World, i: usize) {
        if !world.affect_enabled || world.agents.affect.len() <= i {
            return;
        }
        use crate::affect::{FEAR, PANIC, RAGE, SEEK};
        let af = &world.agents.affect[i];
        for (k, s) in self.affect_sum.iter_mut().enumerate() {
            *s += af[k] as f64;
        }
        if af[FEAR] >= HIGH_FEAR {
            self.high_fear += 1;
        }
        if af[SEEK] >= HIGH_SEEK {
            self.high_seek += 1;
        }
        if af[RAGE] >= HIGH_RAGE {
            self.high_rage += 1;
        }
        if af[PANIC] >= HIGH_PANIC {
            self.high_panic += 1;
        }
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
    ///
    /// The per-agent work is split into `absorb_*` groups by concern below.
    /// They are pure additions into disjoint fields of one entry, so the
    /// grouping is order-independent — but the *outer* pass still visits
    /// agents in ascending id order, which is what keeps every f32/f64 sum
    /// bit-identical to the per-detector scans this replaced.
    pub fn build(&mut self, world: &World) {
        for &sid in &self.active {
            if let Some(e) = self.entries.get_mut(sid as usize) {
                e.reset();
            }
        }
        self.active.clear();
        // The sensors scratch is undersized on standalone `observe_all` calls
        // made outside the tick; crowding is simply not accumulated then.
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
            e.absorb_membership(world, i);
            e.absorb_morphology(world, i);
            e.absorb_habitat(world, i);
            e.absorb_genome(world, i);
            e.absorb_culture(world, i);
            e.absorb_affect(world, i);
            if sensors_ok {
                e.crowding_sum += world.sensors[i].crowding as f64;
            }
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

    #[test]
    fn build_aggregates_affect_activations() {
        use crate::affect::{FEAR, SEEK};
        let mut w = World::new(11);
        w.affect_enabled = true;
        let a = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(120.0, 100.0), Genome::neutral());
        // Stamp distinctive activations directly on the serialized column.
        w.agents.affect[a as usize][FEAR] = 0.9;
        w.agents.affect[b as usize][FEAR] = 0.1;
        w.agents.affect[a as usize][SEEK] = 0.8;
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        let e = agg.get(0).expect("species 0");
        assert!((e.affect_sum[FEAR] - 1.0).abs() < 1e-5);
        assert!((e.affect_sum[SEEK] - 0.8).abs() < 1e-5);
        // Only `a` is above the HIGH-fear threshold (see params HIGH_* consts).
        assert_eq!(e.high_fear, 1);
    }

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
