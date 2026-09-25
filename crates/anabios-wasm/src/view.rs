//! Pure, allocation-reusing "view" builders over a read-only [`World`].
//!
//! Everything here mirrors the draw data the Godot bridge
//! (`crates/anabios-godot/src/lib.rs`) and the web recorder
//! (`crates/anabios-headless/src/record.rs`) export, packed into flat,
//! fixed-stride buffers a JavaScript caller can view directly in wasm linear
//! memory. Nothing in this module mutates the world or draws RNG, so reading
//! a view between ticks leaves the run bit-identical (asserted by the unit
//! tests below via `state_hash`).

use std::collections::BTreeMap;

use anabios_core::affect::arousal;
use anabios_core::agent::AGENT_NULL;
use anabios_core::biome::{cell_color, BiomeCell, TerrainType, SEA_LEVEL};
use anabios_core::codex::{CodexEvent, EventType};
use anabios_core::culture::{SKILL_CHANNEL, TECH_CHANNEL};
use anabios_core::genome::{GenomeSlot, GENOME_LEN, SLOT_NAMES};
use anabios_core::invention::{
    bit, for_each_set_bit, held_mask, level, tech_era, INVENTIONS, INVENTION_CHANNEL_BASE,
    INVENTION_COUNT,
};
use anabios_core::module::effective_diet_carnivory;
use anabios_core::mood::MOOD_COUNT;
use anabios_core::practice::PRACTICES;
use anabios_core::resource::Good;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::World;
use glam::Vec2;
use serde_json::{json, Value};

/// Floats per agent in [`fill_agents`]:
/// `[id, x, y, rot, size, diet, hue, sat, val, dialect_hue, energy,
///   species_id, mood, flags, arousal, infection]`.
///
/// `flags` bits: 0 = livestock, 1 = asleep, 2 = male (only meaningful when
/// the matching scenario flag is on — see [`world_flags`]).
pub const AGENT_STRIDE: usize = 16;

/// Floats per line segment in [`fill_segments`]: `[x1, y1, x2, y2, hue]`.
pub const SEGMENT_STRIDE: usize = 5;

/// Floats per settlement site in [`fill_sites`]: `[species_id, x, y, members]`.
pub const SITE_STRIDE: usize = 4;

/// Floats per trade hub in [`fill_hubs`]: `[x, y, goods_mask]` (bit k = good k).
pub const HUB_STRIDE: usize = 3;

/// Floats per codex event in [`push_event`]:
/// `[type, tick, species_id (-1 = global), value, x, y]`.
pub const EVENT_STRIDE: usize = 6;

/// Scenario-flag bitmask returned by [`world_flags`]. Bit positions are part
/// of the JS contract (`web/src/sim.js`).
pub const FLAG_INVENTIONS: u32 = 1 << 0;
pub const FLAG_AFFECT: u32 = 1 << 1;
pub const FLAG_DISEASE: u32 = 1 << 2;
pub const FLAG_DOMESTICATION: u32 = 1 << 3;
pub const FLAG_RESOURCES: u32 = 1 << 4;
pub const FLAG_DIMORPHISM: u32 = 1 << 5;
pub const FLAG_BASIC_NEEDS: u32 = 1 << 6;
pub const FLAG_COGNITION: u32 = 1 << 7;
pub const FLAG_SETTLEMENT: u32 = 1 << 8;
pub const FLAG_WAR: u32 = 1 << 9;

/// Which opt-in subsystems the loaded world runs, so the frontend can hide
/// overlays whose columns would read as all-zero.
pub fn world_flags(w: &World) -> u32 {
    let mut f = 0;
    if w.inventions_enabled {
        f |= FLAG_INVENTIONS;
    }
    if w.affect_enabled {
        f |= FLAG_AFFECT;
    }
    if w.disease_enabled {
        f |= FLAG_DISEASE;
    }
    if w.domestication_enabled {
        f |= FLAG_DOMESTICATION;
    }
    if w.resources_enabled {
        f |= FLAG_RESOURCES;
    }
    if w.sexual_dimorphism_enabled {
        f |= FLAG_DIMORPHISM;
    }
    if w.basic_needs_enabled {
        f |= FLAG_BASIC_NEEDS;
    }
    if w.cognition_enabled {
        f |= FLAG_COGNITION;
    }
    if w.settlement_enabled {
        f |= FLAG_SETTLEMENT;
    }
    if w.war_enabled {
        f |= FLAG_WAR;
    }
    f
}

/// Project a meme vector onto a stable hue in `[0,1)` (same weights as the
/// Godot bridge's `dialect_hue`, dialect channels only).
pub fn dialect_hue(meme: &[f32]) -> f32 {
    let n = meme.len().min(INVENTION_CHANNEL_BASE);
    let mut acc = 0.0_f32;
    let mut wsum = 0.0_f32;
    for (k, v) in meme[..n].iter().enumerate() {
        let w = 0.37 + 0.11 * k as f32;
        acc += v * w;
        wsum += w;
    }
    if wsum <= 0.0 {
        return 0.0;
    }
    (acc / wsum).rem_euclid(1.0)
}

/// Fill `out` with [`AGENT_STRIDE`] floats per alive agent, ascending id
/// order. Returns the agent count.
pub fn fill_agents(w: &World, out: &mut Vec<f32>) -> usize {
    out.clear();
    let n = w.agents.live_count() as usize;
    out.reserve(n * AGENT_STRIDE);
    for id in w.agents.iter_alive() {
        let i = id as usize;
        let p = w.agents.position[i];
        let v = w.agents.velocity[i];
        let g = &w.agents.genome[i];
        let rot = if v.length_squared() > 1e-6 { v.y.atan2(v.x) } else { 0.0 };
        let mut flags = 0u32;
        if w.domestication_enabled && w.agents.livestock_of[i] != AGENT_NULL {
            flags |= 1;
        }
        if w.agents.asleep.get(i).map(|b| *b).unwrap_or(false) {
            flags |= 2;
        }
        if w.sexual_dimorphism_enabled && w.agents.sex.get(i).map(|b| *b).unwrap_or(false) {
            flags |= 4;
        }
        out.extend_from_slice(&[
            id as f32,
            p.x,
            p.y,
            rot,
            0.5 + 2.5 * g.get(GenomeSlot::Size),
            effective_diet_carnivory(&w.agents.modules[i]).clamp(0.0, 1.0),
            g.get(GenomeSlot::ColorHue),
            g.get(GenomeSlot::ColorSat).clamp(0.4, 1.0),
            g.get(GenomeSlot::ColorVal).clamp(0.5, 1.0),
            dialect_hue(&w.agents.meme_vector[i]),
            w.agents.energy[i],
            w.agents.species_id[i] as f32,
            w.agents.mood[i] as f32,
            flags as f32,
            arousal(&w.agents.affect[i]),
            w.agents.infection[i],
        ]);
    }
    n
}

/// River presentation blend (viewer-only), same constants as the Godot bridge.
fn river_tint(rgb: [f32; 3], river_flow: f32) -> [f32; 3] {
    const RIVER_BLUE: [f32; 3] = [0.18, 0.42, 0.72];
    if river_flow <= 0.0 {
        return rgb;
    }
    let mix = (0.55 + 0.45 * river_flow.max(0.0).sqrt()).clamp(0.0, 1.0);
    [
        rgb[0] + (RIVER_BLUE[0] - rgb[0]) * mix,
        rgb[1] + (RIVER_BLUE[1] - rgb[1]) * mix,
        rgb[2] + (RIVER_BLUE[2] - rgb[2]) * mix,
    ]
}

/// RGBA8 view colour of one biome cell: `cell_color` → river tint, alpha =
/// elevation. Byte-identical to the Godot bridge's `cell_view_rgba8`.
pub fn cell_view_rgba8(cell: &BiomeCell) -> [u8; 4] {
    let rgb = river_tint(cell_color(cell), cell.river_flow);
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
    [to_u8(rgb[0]), to_u8(rgb[1]), to_u8(rgb[2]), to_u8(cell.elevation)]
}

/// Fill `out` with `res² × 4` RGBA8 bytes, row-major (`row = y`, `col = x`).
pub fn fill_biome_rgba(w: &World, out: &mut Vec<u8>) {
    out.clear();
    out.reserve(w.biome.cells.len() * 4);
    for cell in w.biome.cells.iter() {
        out.extend_from_slice(&cell_view_rgba8(cell));
    }
}

/// Fill `out` with one elevation float per biome cell, row-major.
pub fn fill_elevation(w: &World, out: &mut Vec<f32>) {
    out.clear();
    out.extend(w.biome.cells.iter().map(|c| c.elevation.clamp(0.0, 1.0)));
}

/// Fill `out` with one `TerrainType` id per biome cell (river cells report
/// `Water`, matching the Godot bridge's `biome_terrain_ids`).
pub fn fill_terrain(w: &World, out: &mut Vec<u8>) {
    out.clear();
    out.extend(w.biome.cells.iter().map(|c| {
        if c.river_flow > 0.0 {
            TerrainType::Water as u8
        } else {
            c.terrain as u8
        }
    }));
}

/// Viewer sea level: the highest elevation still classified as water (the
/// tight lower bound on the generator's `sea_level`), or `SEA_LEVEL` for an
/// all-land field.
pub fn sea_level(w: &World) -> f32 {
    let mut max_water = f32::MIN;
    for cell in w.biome.cells.iter() {
        if cell.terrain == TerrainType::Water && cell.elevation > max_water {
            max_water = cell.elevation;
        }
    }
    if max_water == f32::MIN {
        SEA_LEVEL
    } else {
        max_water
    }
}

/// Pack a `(from, to, hue)` scratch list (combat streaks / trade routes) into
/// [`SEGMENT_STRIDE`] floats each. Returns the segment count.
pub fn fill_segments(list: &[(Vec2, Vec2, f32)], out: &mut Vec<f32>) -> usize {
    out.clear();
    out.reserve(list.len() * SEGMENT_STRIDE);
    for (from, to, hue) in list {
        out.extend_from_slice(&[from.x, from.y, to.x, to.y, *hue]);
    }
    list.len()
}

/// Settlement sites from the codex latch: per settled species, the centroid
/// of its alive members' home anchors and the anchored member count. Returns
/// the site count.
pub fn fill_sites(w: &World, out: &mut Vec<f32>) -> usize {
    out.clear();
    if w.codex.settlement_active.is_empty() {
        return 0;
    }
    let mut acc: BTreeMap<u32, (f64, f64, u32)> = BTreeMap::new();
    for id in w.agents.iter_alive() {
        let sid = w.agents.species_id[id as usize];
        if !w.codex.settlement_active.contains(&sid) {
            continue;
        }
        let a = w.agents.anchor[id as usize];
        let e = acc.entry(sid).or_insert((0.0, 0.0, 0));
        e.0 += a.x as f64;
        e.1 += a.y as f64;
        e.2 += 1;
    }
    let mut n = 0;
    for (sid, (sx, sy, count)) in acc {
        if count == 0 {
            continue;
        }
        out.extend_from_slice(&[
            sid as f32,
            (sx / count as f64) as f32,
            (sy / count as f64) as f32,
            count as f32,
        ]);
        n += 1;
    }
    n
}

/// Fixed trade hubs (empty unless `resources_enabled`). Returns the hub count.
pub fn fill_hubs(w: &World, out: &mut Vec<f32>) -> usize {
    out.clear();
    for h in &w.trade_hubs {
        let mask = h.goods.iter().fold(0u32, |m, g| m | (1 << g.index()));
        out.extend_from_slice(&[h.pos.x, h.pos.y, mask as f32]);
    }
    w.trade_hubs.len()
}

/// Append one codex event as [`EVENT_STRIDE`] floats.
pub fn push_event(ev: &CodexEvent, out: &mut Vec<f32>) {
    let sid = if ev.species_id == u32::MAX { -1.0 } else { ev.species_id as f32 };
    out.extend_from_slice(&[
        ev.event_type as u8 as f32,
        ev.tick as f32,
        sid,
        ev.value,
        ev.loc_x,
        ev.loc_y,
    ]);
}

/// Species id → archetype display name, assigned in declaration order exactly
/// as `Scenario::instantiate` reserves species ids (specs without an
/// archetype share species 0; dynamically speciated splinters are absent).
/// Same convention as the web recorder.
pub fn species_labels(scenario: &Scenario) -> BTreeMap<u32, String> {
    let mut labels = BTreeMap::new();
    let mut next_sid = 1u32;
    for spec in &scenario.agents {
        if let Some(name) = &spec.archetype {
            labels.insert(next_sid, name.clone());
            next_sid += 1;
        }
    }
    labels
}

/// Static catalog the frontend needs once per module load: event-type names,
/// the invention tree, practice keys, mood names, genome slot names, goods.
pub fn catalog_json() -> String {
    let events: Vec<&str> = EventType::ALL.iter().map(|t| t.name()).collect();
    let inventions: Vec<Value> = INVENTIONS
        .iter()
        .enumerate()
        .map(|(k, inv)| {
            let mut prereqs = Vec::new();
            for_each_set_bit(inv.prereqs, |p| prereqs.push(INVENTIONS[p].key));
            json!({
                "id": k, "key": inv.key, "name": inv.name, "era": inv.era,
                "prereqs": prereqs, "buff": inv.buff, "debuff": inv.debuff,
            })
        })
        .collect();
    let practices: Vec<Value> =
        PRACTICES.iter().map(|p| json!({ "key": p.key, "name": p.name })).collect();
    let moods: Vec<&str> = (0..MOOD_COUNT as u8).map(anabios_core::mood::name).collect();
    let goods: Vec<String> = Good::ALL.iter().map(|g| format!("{g:?}")).collect();
    json!({
        "events": events,
        "inventions": inventions,
        "practices": practices,
        "moods": moods,
        "genome_slots": SLOT_NAMES.to_vec(),
        "goods": goods,
        "agent_stride": AGENT_STRIDE,
        "segment_stride": SEGMENT_STRIDE,
        "site_stride": SITE_STRIDE,
        "hub_stride": HUB_STRIDE,
        "event_stride": EVENT_STRIDE,
    })
    .to_string()
}

/// Per-live-species aggregate rows: `{ id, name, count, mean_energy,
/// tech_era, adopted: [keys] }` sorted by id, plus the world-level flags.
pub fn species_json(w: &World, labels: &BTreeMap<u32, String>) -> String {
    let mut count: BTreeMap<u32, u32> = BTreeMap::new();
    let mut energy: BTreeMap<u32, f32> = BTreeMap::new();
    let mut inv_counts: BTreeMap<u32, [u32; INVENTION_COUNT]> = BTreeMap::new();
    for id in w.agents.iter_alive() {
        let i = id as usize;
        let sp = w.agents.species_id[i];
        *count.entry(sp).or_insert(0) += 1;
        *energy.entry(sp).or_insert(0.0) += w.agents.energy[i];
        if w.inventions_enabled {
            let counts = inv_counts.entry(sp).or_insert([0; INVENTION_COUNT]);
            for_each_set_bit(held_mask(&w.agents.meme_vector[i]), |k| counts[k] += 1);
        }
    }
    let rows: Vec<Value> = count
        .iter()
        .map(|(sp, n)| {
            let nf = *n as f32;
            let mut adopted = Vec::new();
            let mut mask = 0u32;
            if let Some(counts) = inv_counts.get(sp) {
                for (k, &holders) in counts.iter().enumerate() {
                    if holders as f32 / nf >= 0.5 {
                        adopted.push(INVENTIONS[k].key);
                        mask |= bit(k);
                    }
                }
            }
            json!({
                "id": sp,
                "name": labels.get(sp).cloned().unwrap_or_else(|| format!("species {sp}")),
                "count": n,
                "mean_energy": energy[sp] / nf,
                "tech_era": tech_era(mask),
                "adopted": adopted,
            })
        })
        .collect();
    json!({ "species": rows, "labels": labels }).to_string()
}

/// Inspector view of one agent (`{}` when the id is dead or out of range).
pub fn agent_json(w: &World, id: u32, labels: &BTreeMap<u32, String>) -> String {
    if !w.agents.is_alive(id) {
        return "{}".to_string();
    }
    let i = id as usize;
    let g = &w.agents.genome[i];
    let meme = &w.agents.meme_vector[i];
    let p = w.agents.position[i];
    let sid = w.agents.species_id[i];
    let modules: Vec<String> =
        w.agents.modules[i].iter().map(|m| format!("{:?}", m.module_type())).collect();
    let mask = held_mask(meme);
    let mut held = Vec::new();
    for_each_set_bit(mask, |k| {
        held.push(json!({ "name": INVENTIONS[k].name, "level": level(meme, k) }));
    });
    let owner = w.agents.livestock_of.get(i).copied().unwrap_or(AGENT_NULL);
    let genome: Vec<f32> = (0..GENOME_LEN).map(|s| g.raw(s)).collect();
    json!({
        "id": id,
        "x": p.x, "y": p.y,
        "energy": w.agents.energy[i],
        "age": w.agents.age[i],
        "lineage_id": w.agents.lineage_id[i],
        "species_id": sid,
        "species": labels.get(&sid).cloned().unwrap_or_else(|| format!("species {sid}")),
        "size": 0.5 + 2.5 * g.get(GenomeSlot::Size),
        "diet_carnivory": effective_diet_carnivory(&w.agents.modules[i]),
        "skill": meme[SKILL_CHANNEL],
        "technique": meme[TECH_CHANNEL],
        "iq": w.agents.iq[i],
        "indiv_learn": g.get(GenomeSlot::IndividualLearning) > 0.5,
        "social_learn": g.get(GenomeSlot::SocialLearning) > 0.5,
        "sex_male": w.agents.sex.get(i).map(|b| *b).unwrap_or(false),
        "livestock_of": if owner == AGENT_NULL { -1i64 } else { owner as i64 },
        "arousal": arousal(&w.agents.affect[i]),
        "mood": anabios_core::mood::name(w.agents.mood[i]),
        "infection": w.agents.infection[i],
        "thirst": w.agents.thirst[i],
        "fatigue": w.agents.fatigue[i],
        "asleep": w.agents.asleep.get(i).map(|b| *b).unwrap_or(false),
        "dialect_hue": dialect_hue(meme),
        "modules": modules,
        "program_len": w.agents.program[i].len(),
        "inventions": held,
        "tech_era": tech_era(mask),
        "genome": genome,
    })
    .to_string()
}

/// FNV-1a basis / prime (64-bit).
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Fold LE bytes into an FNV-1a accumulator.
fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(FNV_PRIME);
    }
}

/// A pointer-width-independent trajectory fingerprint.
///
/// `snapshot::state_hash` hashes the bincode payload of the whole `World`,
/// and that payload is *not* portable across 32- and 64-bit targets: the
/// `BitVec<usize>` columns (`alive`, `asleep`, `sex`) serialize their raw
/// `usize` store words, so the same bit pattern encodes differently on wasm32
/// than on x86-64 even when the simulation is identical. This fingerprint
/// instead folds the trajectory-defining state (tick, seed, every alive
/// agent's kinematics / energy / age / genome / memes / drives, the species
/// tables, and the dynamic biome columns) into FNV-1a over fixed-width LE
/// bytes, so a wasm run and a native run of the same scenario and seed can be
/// compared bit-for-bit (`web/test/wasm-smoke.mjs` vs the `fingerprint`
/// example in this crate).
pub fn fingerprint(w: &World) -> u64 {
    let mut h = FNV_OFFSET;
    fnv(&mut h, &w.tick.to_le_bytes());
    fnv(&mut h, &w.seed.to_le_bytes());
    fnv(&mut h, &w.agents.live_count().to_le_bytes());
    fnv(&mut h, &w.next_lineage_id.to_le_bytes());
    fnv(&mut h, &w.next_species_id.to_le_bytes());
    for c in &w.species_member_counts {
        fnv(&mut h, &c.to_le_bytes());
    }
    for id in w.agents.iter_alive() {
        let i = id as usize;
        fnv(&mut h, &id.to_le_bytes());
        let p = w.agents.position[i];
        let v = w.agents.velocity[i];
        for f in [p.x, p.y, v.x, v.y, w.agents.energy[i]] {
            fnv(&mut h, &f.to_bits().to_le_bytes());
        }
        fnv(&mut h, &w.agents.age[i].to_le_bytes());
        fnv(&mut h, &w.agents.species_id[i].to_le_bytes());
        fnv(&mut h, &w.agents.lineage_id[i].to_le_bytes());
        for f in w.agents.genome[i].as_slice() {
            fnv(&mut h, &f.to_bits().to_le_bytes());
        }
        for f in &w.agents.meme_vector[i] {
            fnv(&mut h, &f.to_bits().to_le_bytes());
        }
        for f in [w.agents.iq[i], w.agents.infection[i], w.agents.thirst[i], w.agents.fatigue[i]] {
            fnv(&mut h, &f.to_bits().to_le_bytes());
        }
        let asleep = w.agents.asleep.get(i).map(|b| *b).unwrap_or(false);
        let male = w.agents.sex.get(i).map(|b| *b).unwrap_or(false);
        fnv(&mut h, &[w.agents.mood[i], asleep as u8, male as u8]);
    }
    for c in w.biome.cells.iter() {
        fnv(&mut h, &c.plant_biomass.to_bits().to_le_bytes());
        fnv(&mut h, &c.pollution.to_bits().to_le_bytes());
        fnv(&mut h, &[c.succession]);
    }
    h
}

/// Run metadata: scenario name, seed, tick, dims, alive count, flags and the
/// determinism receipt (`state_hash`, formatted like the headless CLI).
pub fn meta_json(w: &World, scenario_name: &str) -> String {
    json!({
        "scenario": scenario_name,
        "seed": w.seed,
        "tick": w.tick,
        "alive": w.agents.live_count(),
        "world_size": w.world_size,
        "biome_res": w.biome_res,
        "sea_level": sea_level(w),
        "flags": world_flags(w),
        "total_trades": w.total_trades,
        "state_hash": format!("0x{:016x}", state_hash(w)),
        "fingerprint": format!("0x{:016x}", fingerprint(w)),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::tick::step;

    fn world(name: &str, seed: u64) -> (Scenario, World) {
        let path = format!("{}/../../scenarios/{name}.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).expect("scenario");
        let mut s = Scenario::parse_toml(&text).expect("parse");
        s.seed = seed;
        let w = s.instantiate();
        (s, w)
    }

    /// Reading every view between ticks must leave the run bit-identical.
    #[test]
    fn views_are_side_effect_free() {
        let (s, mut viewed) = world("predator-prey", 7);
        let mut plain = viewed.clone();
        let labels = species_labels(&s);
        let (mut a, mut b, mut seg, mut site, mut hub, mut ev) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut f = Vec::new();
        for _ in 0..200 {
            step(&mut viewed);
            for e in viewed.codex.drain_events() {
                push_event(&e, &mut ev);
            }
            fill_agents(&viewed, &mut a);
            fill_biome_rgba(&viewed, &mut b);
            fill_elevation(&viewed, &mut f);
            fill_segments(&viewed.combat_streaks, &mut seg);
            fill_sites(&viewed, &mut site);
            fill_hubs(&viewed, &mut hub);
            let _ = species_json(&viewed, &labels);
            let _ = agent_json(&viewed, 0, &labels);
            let _ = meta_json(&viewed, &s.name);
            step(&mut plain);
            for _ in plain.codex.drain_events() {}
        }
        assert_eq!(state_hash(&viewed), state_hash(&plain));
    }

    #[test]
    fn agent_buffer_is_well_formed() {
        let (_, mut w) = world("predator-prey", 3);
        for _ in 0..50 {
            step(&mut w);
        }
        let mut out = Vec::new();
        let n = fill_agents(&w, &mut out);
        assert_eq!(n, w.agents.live_count() as usize);
        assert_eq!(out.len(), n * AGENT_STRIDE);
        for row in out.chunks(AGENT_STRIDE) {
            let id = row[0] as u32;
            assert!(w.agents.is_alive(id));
            assert!((0.0..=w.world_size).contains(&row[1]), "x {}", row[1]);
            assert!((0.0..=w.world_size).contains(&row[2]), "y {}", row[2]);
            assert!((0.0..=1.0).contains(&row[5]), "diet {}", row[5]);
            assert!((0.0..1.0).contains(&row[9]), "dialect hue {}", row[9]);
        }
    }

    #[test]
    fn biome_buffers_match_resolution() {
        let (_, w) = world("minimal", 1);
        let (mut rgba, mut elev, mut ter) = (Vec::new(), Vec::new(), Vec::new());
        fill_biome_rgba(&w, &mut rgba);
        fill_elevation(&w, &mut elev);
        fill_terrain(&w, &mut ter);
        let n = w.biome_res * w.biome_res;
        assert_eq!(rgba.len(), n * 4);
        assert_eq!(elev.len(), n);
        assert_eq!(ter.len(), n);
        assert!(ter.iter().all(|&t| t <= TerrainType::Tundra as u8));
        let sl = sea_level(&w);
        assert!((0.0..=1.0).contains(&sl));
    }

    #[test]
    fn catalog_lists_every_event_type() {
        let v: Value = serde_json::from_str(&catalog_json()).unwrap();
        let events = v["events"].as_array().unwrap();
        assert_eq!(events.len(), anabios_core::codex::EVENT_TYPE_COUNT);
        assert_eq!(events[0], "extinction");
        assert_eq!(events[38], "war");
        assert_eq!(v["inventions"].as_array().unwrap().len(), INVENTION_COUNT);
        assert_eq!(v["moods"].as_array().unwrap().len(), MOOD_COUNT);
    }

    #[test]
    fn species_labels_follow_declaration_order() {
        let (s, w) = world("predator-prey", 0);
        let labels = species_labels(&s);
        assert_eq!(labels.get(&1).map(String::as_str), Some("grazer"));
        assert_eq!(labels.get(&2).map(String::as_str), Some("stalker"));
        let v: Value = serde_json::from_str(&species_json(&w, &labels)).unwrap();
        let rows = v["species"].as_array().unwrap();
        assert_eq!(rows.iter().map(|r| r["count"].as_u64().unwrap()).sum::<u64>(), 68);
    }

    #[test]
    fn agent_json_is_empty_for_dead_ids() {
        let (s, w) = world("minimal", 1);
        let labels = species_labels(&s);
        assert_eq!(agent_json(&w, u32::MAX - 1, &labels), "{}");
        let v: Value = serde_json::from_str(&agent_json(&w, 0, &labels)).unwrap();
        assert_eq!(v["id"], 0);
        assert_eq!(v["genome"].as_array().unwrap().len(), GENOME_LEN);
    }

    #[test]
    fn fingerprint_is_stable_and_tick_sensitive() {
        let (_, mut a) = world("minimal", 1);
        let (_, mut b) = world("minimal", 1);
        assert_eq!(fingerprint(&a), fingerprint(&b));
        step(&mut a);
        assert_ne!(fingerprint(&a), fingerprint(&b));
        step(&mut b);
        assert_eq!(fingerprint(&a), fingerprint(&b));
        let (_, other_seed) = world("minimal", 2);
        assert_ne!(fingerprint(&b), fingerprint(&other_seed));
    }

    #[test]
    fn event_rows_encode_global_species_as_minus_one() {
        let ev = CodexEvent {
            event_type: EventType::WarOrRaid,
            tick: 12,
            species_id: u32::MAX,
            value: 0.5,
            loc_x: 1.0,
            loc_y: 2.0,
        };
        let mut out = Vec::new();
        push_event(&ev, &mut out);
        assert_eq!(out, vec![38.0, 12.0, -1.0, 0.5, 1.0, 2.0]);
    }
}
