//! Cultural-advancement demo: run a scenario and narrate the invention race
//! between populations — discovery/adoption events as they fire, plus a
//! periodic per-culture tech table and a final era summary.
//!
//! Populations are tracked as CULTURES (the founding archetype of each
//! lineage), not species ids: free reproduction speciates the founders into
//! hundreds of one-member genetic splinters, but a splinter still belongs to
//! its founders' culture. Culture membership is resolved by lineage ancestry
//! (`parent_ids` chains back to a labelled founder).

use std::collections::BTreeMap;
use std::path::PathBuf;

use anabios_core::agent::LineageId;
use anabios_core::codex::EventType;
use anabios_core::invention::{self, INVENTIONS, INVENTION_COUNT};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::world::World;
use anyhow::{Context, Result};

/// Per-culture tech snapshot used for the periodic report and final summary.
struct CultureTech {
    alive: u32,
    /// Mean energy of members (shows tech upkeep/stress burden).
    mean_energy: f32,
    /// Adopted (≥50% of members) invention ids.
    adopted: Vec<usize>,
    /// In-progress invention id → adoption percent (0..50).
    learning: Vec<(usize, u32)>,
    era: u8,
}

/// Tracks which founding culture each lineage belongs to.
struct CultureMap {
    /// species id → display label for the founding populations.
    species_label: BTreeMap<u32, String>,
    /// lineage id → culture label (resolved lazily via parent chains).
    lineage_culture: BTreeMap<LineageId, String>,
    /// species id → culture label, inferred from resolved members (fallback
    /// for lineages whose parents died before resolution).
    species_culture: BTreeMap<u32, String>,
}

impl CultureMap {
    fn new(world: &World, scenario: &Scenario) -> Self {
        // Map species ids to archetype labels: each archetype spec claims a
        // fresh species id in declaration order (archetype-free specs stay
        // species 0).
        let mut species_label: BTreeMap<u32, String> = BTreeMap::new();
        let mut next_sid = 1u32;
        for spec in &scenario.agents {
            if let Some(name) = &spec.archetype {
                species_label.insert(next_sid, name.clone());
                next_sid += 1;
            }
        }
        let mut lineage_culture = BTreeMap::new();
        let mut species_culture = BTreeMap::new();
        for id in world.agents.iter_alive() {
            let i = id as usize;
            let label = species_label
                .get(&world.agents.species_id[i])
                .cloned()
                .unwrap_or_else(|| format!("species-{}", world.agents.species_id[i]));
            lineage_culture.insert(world.agents.lineage_id[i], label.clone());
            species_culture.insert(world.agents.species_id[i], label);
        }
        Self { species_label, lineage_culture, species_culture }
    }

    /// Refresh all mappings from the live world. Cheap enough to run every
    /// few dozen ticks: each lineage/species is resolved once, so parents are
    /// always captured while still alive.
    fn resolve(&mut self, world: &World) {
        for id in world.agents.iter_alive() {
            let i = id as usize;
            let lineage = world.agents.lineage_id[i];
            if !self.lineage_culture.contains_key(&lineage) {
                let parents = world.agents.parent_ids[i];
                let label = parents
                    .iter()
                    .find_map(|p| self.lineage_culture.get(p).cloned())
                    .or_else(|| self.species_culture.get(&world.agents.species_id[i]).cloned())
                    .unwrap_or_else(|| format!("species-{}", world.agents.species_id[i]));
                self.lineage_culture.insert(lineage, label);
            }
            let sid = world.agents.species_id[i];
            if !self.species_culture.contains_key(&sid) {
                let label = self.lineage_culture[&lineage].clone();
                self.species_culture.insert(sid, label);
            }
        }
    }

    /// Culture label of one live agent (already resolved by [`Self::resolve`]).
    fn culture_of(&self, world: &World, agent_idx: usize) -> String {
        self.lineage_culture
            .get(&world.agents.lineage_id[agent_idx])
            .cloned()
            .unwrap_or_else(|| format!("species-{}", world.agents.species_id[agent_idx]))
    }
}

pub fn run(scenario_path: PathBuf, ticks: u64, seed: Option<u64>, report_every: u64) -> Result<()> {
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario file {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }

    let mut world = scenario.instantiate();
    let mut cultures = CultureMap::new(&world, &scenario);
    println!(
        "=== anabios invention-tree demo ===\nscenario={} seed={} ticks={} agents={}",
        scenario.name,
        world.seed,
        ticks,
        world.agents.live_count()
    );
    for (sid, label) in &cultures.species_label {
        println!("  species {sid}: {label}");
    }
    println!("tree:");
    for inv in &INVENTIONS {
        let prereq = if inv.prereqs == 0 {
            "—".to_string()
        } else {
            let mut names = Vec::new();
            invention::for_each_set_bit(inv.prereqs, |p| names.push(INVENTIONS[p].key));
            names.join("+")
        };
        println!(
            "  [era {}] {:<14} needs {:<22} buff: {:<28} debuff: {}",
            inv.era, inv.key, prereq, inv.buff, inv.debuff
        );
    }
    println!();

    // Adoption lines already narrated per (culture, invention) — speciation
    // splinters re-fire the codex latch, so dedupe for readability.
    let mut narrated: std::collections::BTreeSet<(String, usize)> =
        std::collections::BTreeSet::new();
    for t in 1..=ticks {
        step(&mut world);
        // Resolve newborn lineages into cultures while their parents are
        // still alive; every tick so event labels below map splinter species
        // back to their founding culture.
        cultures.resolve(&world);
        for ev in world.codex.drain_events() {
            let label = cultures
                .species_label
                .get(&ev.species_id)
                .or_else(|| cultures.species_culture.get(&ev.species_id))
                .cloned()
                .unwrap_or_else(|| format!("species-{}", ev.species_id));
            match ev.event_type {
                EventType::InventionDiscovered => {
                    let inv = &INVENTIONS[ev.value as usize];
                    println!(
                        "tick {:>5} | * {label} DISCOVERED {} — {} / -{}",
                        ev.tick,
                        inv.key.to_uppercase(),
                        inv.buff,
                        inv.debuff
                    );
                }
                EventType::InventionAdopted => {
                    let k = ev.value as usize;
                    if narrated.insert((label.clone(), k)) {
                        let inv = &INVENTIONS[k];
                        println!(
                            "tick {:>5} | > {label} adopted {} (majority of members)",
                            ev.tick,
                            inv.key.to_uppercase()
                        );
                    }
                }
                // Only the founding cultures' own extinction is news;
                // one-member genetic splinters wink out constantly.
                EventType::Extinction if cultures.species_label.contains_key(&ev.species_id) => {
                    println!("tick {:>5} | x {label} went EXTINCT", ev.tick);
                }
                _ => {}
            }
        }
        if t % report_every == 0 && t < ticks {
            print_report(&world, &mut cultures, t);
        }
    }

    // Final standings.
    println!("\n=== final standings (tick {ticks}) ===");
    cultures.resolve(&world);
    let table = tech_table(&world, &cultures);
    for (label, st) in &table {
        println!(
            "  {label:<16} alive={:<4} era={} adopted={}/{}",
            st.alive,
            st.era,
            st.adopted.len(),
            INVENTION_COUNT
        );
        if !st.adopted.is_empty() {
            let names: Vec<&str> = st.adopted.iter().map(|&k| INVENTIONS[k].key).collect();
            println!("    techs: {}", names.join(", "));
        }
    }
    if let Some((winner, st)) =
        table.iter().max_by_key(|(_, st)| (st.era, st.adopted.len(), st.alive))
    {
        println!(
            "\nmost advanced culture: {winner} (era {}, {} techs, {} alive)",
            st.era,
            st.adopted.len(),
            st.alive
        );
    }
    Ok(())
}

fn print_report(world: &World, cultures: &mut CultureMap, tick: u64) {
    println!("--- tick {tick} ---");
    cultures.resolve(world);
    for (label, st) in tech_table(world, cultures) {
        let adopted: Vec<&str> = st.adopted.iter().map(|&k| INVENTIONS[k].key).collect();
        let learning: Vec<String> =
            st.learning.iter().map(|&(k, pct)| format!("{}:{}%", INVENTIONS[k].key, pct)).collect();
        println!(
            "  {label:<16} alive={:<4} nrg={:<6.1} era={} adopted=[{}] learning={}",
            st.alive,
            st.mean_energy,
            st.era,
            adopted.join(","),
            learning.join(",")
        );
    }
    // Machinery's debuff, made visible once it matters.
    let pollution: f32 = world.biome.cells.iter().map(|c| c.pollution).sum();
    if pollution > 0.1 {
        println!("  world: pollution={pollution:.1} (machinery regrowth penalty)");
    }
}

/// Compute per-culture adoption from current agent meme vectors, grouping
/// agents by their founding culture (see [`CultureMap`]).
fn tech_table(world: &World, cultures: &CultureMap) -> BTreeMap<String, CultureTech> {
    let mut counts: BTreeMap<String, (u32, f32, [u32; INVENTION_COUNT])> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let label = cultures.culture_of(world, i);
        let e = counts.entry(label).or_insert((0, 0.0, [0; INVENTION_COUNT]));
        e.0 += 1;
        e.1 += world.agents.energy[i];
        let mask = invention::held_mask(&world.agents.meme_vector[i]);
        invention::for_each_set_bit(mask, |k| e.2[k] += 1);
    }
    let mut out = BTreeMap::new();
    for (label, (alive, energy_sum, inv_counts)) in counts {
        let mut adopted = Vec::new();
        let mut learning = Vec::new();
        let mut mask = 0u32;
        for (k, &holders) in inv_counts.iter().enumerate() {
            let pct = holders * 100 / alive.max(1);
            if pct >= 50 {
                adopted.push(k);
                mask |= invention::bit(k);
            } else if pct > 0 {
                learning.push((k, pct));
            }
        }
        let mean_energy = if alive > 0 { energy_sum / alive as f32 } else { 0.0 };
        out.insert(
            label,
            CultureTech { alive, mean_energy, adopted, learning, era: invention::tech_era(mask) },
        );
    }
    out
}
