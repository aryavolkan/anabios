//! Invention-tree mechanism tests: discovery gating, prereqs, social spread,
//! per-invention buffs/debuffs, codex detectors, and end-to-end determinism
//! with the mechanism enabled.

use anabios_core::codex::{observe_all, EventType};
use anabios_core::culture::SKILL_CHANNEL;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::invention::{self, channel, INVENTION_COUNT};
use anabios_core::module::Module;
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program};
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
use anabios_core::world::World;

/// A kit with a Communicator (so meme/invention ops are enabled) + basics.
fn comm_kit() -> anabios_core::module::ModuleList {
    let mut m = anabios_core::module::ModuleList::new();
    m.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
    m.push(Module::Sensor {
        sensor_type: anabios_core::module::SensorType::Vision,
        radius: 0.6,
        acuity: 0.6,
    });
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 });
    m.push(Module::Communicator { range: 10.0, channel_id: 0 });
    m
}

fn set_held(w: &mut World, id: u32, inv: usize) {
    w.agents.meme_vector[id as usize][channel(inv)] = 1.0;
}

fn level_of(w: &World, id: u32, inv: usize) -> f32 {
    w.agents.meme_vector[id as usize][channel(inv)]
}

/// `World::resize_scratch` is crate-private; tests size the tick scratch
/// buffers directly (same defaults).
fn size_scratch(w: &mut World) {
    let cap = w.agents.capacity();
    w.sensors.resize(cap, Default::default());
    w.desired_direction.resize(cap, Vec2::ZERO);
    w.actions.resize(cap, Default::default());
    w.combat_damaged.resize(cap, false);
    w.combat_attacker.resize(cap, 0);
}

// --- Gating -----------------------------------------------------------------

#[test]
fn flag_off_never_discovers_and_consumes_no_invention_rng() {
    let mut w = World::new(7);
    // inventions_enabled defaults to false.
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    w.agents.modules[id as usize] = comm_kit();
    w.agents.meme_vector[id as usize][SKILL_CHANNEL] = 1.0;
    let hash_before = state_hash(&w);
    for _ in 0..50 {
        invention::invention_step(&mut w);
    }
    assert_eq!(invention::held_mask(&w.agents.meme_vector[id as usize]), 0);
    // invention_step is a strict no-op with the flag off: identical state.
    assert_eq!(state_hash(&w), hash_before);
}

#[test]
fn discovery_requires_communicator() {
    let mut w = World::new(11);
    w.inventions_enabled = true;
    // Plain starter kit (no Communicator), max skill and openness — should
    // still never discover.
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    w.agents.meme_vector[id as usize][SKILL_CHANNEL] = 1.0;
    let mut g = w.agents.genome[id as usize];
    g.set(GenomeSlot::Openness, 1.0);
    w.agents.genome[id as usize] = g;
    for _ in 0..500 {
        invention::invention_step(&mut w);
    }
    assert_eq!(invention::held_mask(&w.agents.meme_vector[id as usize]), 0);
}

#[test]
fn communicators_eventually_discover_stone_tools() {
    let mut w = World::new(13);
    w.inventions_enabled = true;
    let mut ids = Vec::new();
    for n in 0..8 {
        let id = w.spawn_agent(Vec2::new(500.0 + n as f32 * 3.0, 500.0), Genome::neutral());
        w.agents.modules[id as usize] = comm_kit();
        w.agents.meme_vector[id as usize][SKILL_CHANNEL] = 1.0;
        let mut g = w.agents.genome[id as usize];
        g.set(GenomeSlot::Openness, 1.0);
        w.agents.genome[id as usize] = g;
        ids.push(id);
    }
    let mut discovered = false;
    for _ in 0..20_000 {
        invention::invention_step(&mut w);
        if ids
            .iter()
            .any(|&id| invention::has(&w.agents.meme_vector[id as usize], invention::STONE_TOOLS))
        {
            discovered = true;
            break;
        }
    }
    assert!(discovered, "skilled, open communicators should discover Stone Tools");
    // Nothing beyond era 1 can be held yet (prereqs chain through stone).
    for &id in &ids {
        let mask = invention::held_mask(&w.agents.meme_vector[id as usize]);
        assert!(mask & !invention::bit(invention::STONE_TOOLS) == 0);
    }
}

// --- Prereqs & atrophy --------------------------------------------------------

#[test]
fn unsupported_invention_atrophies_away() {
    let mut w = World::new(17);
    w.inventions_enabled = true;
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    // Farming without Fire: violates the prereq chain (knowledge inherited
    // without its foundations).
    set_held(&mut w, id, invention::FARMING);
    let before = level_of(&w, id, invention::FARMING);
    invention::invention_step(&mut w);
    let after = level_of(&w, id, invention::FARMING);
    assert!(
        (before - after - invention::ATROPHY_RATE).abs() < 1e-6,
        "unsupported tech decays by ATROPHY_RATE per tick: {before} -> {after}"
    );
    // With the foundations held, no decay.
    set_held(&mut w, id, invention::STONE_TOOLS);
    set_held(&mut w, id, invention::FIRE);
    let stable = level_of(&w, id, invention::FARMING);
    invention::invention_step(&mut w);
    assert_eq!(level_of(&w, id, invention::FARMING), stable, "supported tech must not decay");
}

// --- Spread -------------------------------------------------------------------

#[test]
fn spread_copies_toward_holder_neighbour_and_respects_prereqs() {
    let mut w = World::new(19);
    w.inventions_enabled = true;
    let holder = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let receiver = w.spawn_agent(Vec2::new(505.0, 500.0), Genome::neutral());
    w.agents.modules[holder as usize] = comm_kit();
    w.agents.modules[receiver as usize] = comm_kit();
    w.agents.program[holder as usize] = Program::from_slice(&[Node::Idle]);
    w.agents.program[receiver as usize] = Program::from_slice(&[Node::Idle]);
    // Holder knows the full chain through Farming.
    set_held(&mut w, holder, invention::STONE_TOOLS);
    set_held(&mut w, holder, invention::FIRE);
    set_held(&mut w, holder, invention::FARMING);
    // Receiver knows nothing: only Stone Tools (no prereqs) may spread.
    w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
    size_scratch(&mut w);
    anabios_core::culture::culture_step(&mut w);
    let stone = level_of(&w, receiver, invention::STONE_TOOLS);
    assert!(
        (stone - invention::INVENTION_SPREAD_RATE).abs() < 1e-6,
        "receiver lerps toward holder's Stone Tools at the spread rate, got {stone}"
    );
    assert_eq!(level_of(&w, receiver, invention::FIRE), 0.0, "Fire needs Stone Tools first");
    assert_eq!(level_of(&w, receiver, invention::FARMING), 0.0, "Farming needs Fire first");
}

#[test]
fn writing_doubles_generic_meme_copy_rate() {
    let mut w = World::new(23);
    w.inventions_enabled = true;
    let sender = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let literate = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral());
    let illiterate = w.spawn_agent(Vec2::new(505.0, 500.0), Genome::neutral());
    for id in [sender, literate, illiterate] {
        w.agents.modules[id as usize] = comm_kit();
    }
    // Sender broadcasts 4.0 on channel 1 (decide stage sets broadcast_intent).
    w.agents.program[sender as usize] =
        Program::from_slice(&[Node::Const(4.0), Node::Broadcast(1)]);
    set_held(&mut w, literate, invention::WRITING);
    // Full tick so the sender's broadcast_intent is populated.
    step(&mut w);
    let got_literate = w.agents.meme_vector[literate as usize][1];
    let got_illiterate = w.agents.meme_vector[illiterate as usize][1];
    assert!(got_illiterate > 0.0, "plain receiver adopts the broadcast meme");
    assert!(
        (got_literate - 2.0 * got_illiterate).abs() < 1e-5,
        "Writing holder copies at 2x rate: literate={got_literate} illiterate={got_illiterate}"
    );
}

// --- Buffs --------------------------------------------------------------------

/// Two worlds side by side differ only in one agent's held inventions; feed
/// both through `interact_all` and compare energy gained from grazing.
fn graze_energy_with(inv: Option<usize>) -> f32 {
    let mut w = World::new(29);
    // Find a grass cell to stand on.
    let mut pos = Vec2::ZERO;
    'outer: for row in 0..anabios_core::biome::BIOME_RES {
        for col in 0..anabios_core::biome::BIOME_RES {
            if w.biome.at(col, row).terrain == anabios_core::biome::TerrainType::Grass {
                pos = Vec2::new(
                    (col as f32 + 0.5) * anabios_core::biome::CELL_SIZE,
                    (row as f32 + 0.5) * anabios_core::biome::CELL_SIZE,
                );
                break 'outer;
            }
        }
    }
    let id = w.spawn_agent(pos, Genome::neutral());
    if let Some(k) = inv {
        set_held(&mut w, id, k);
    }
    w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
    size_scratch(&mut w);
    let before = w.agents.energy[id as usize];
    anabios_core::interact::interact_all(&mut w);
    w.agents.energy[id as usize] - before
}

#[test]
fn stone_tools_and_farming_and_fire_raise_grazing_gain() {
    let plain = graze_energy_with(None);
    let stone = graze_energy_with(Some(invention::STONE_TOOLS));
    let farm = graze_energy_with(Some(invention::FARMING));
    let fire = graze_energy_with(Some(invention::FIRE));
    assert!(stone > plain, "Stone Tools increase grazing gain: {plain} -> {stone}");
    assert!(farm > stone, "Farming beats Stone Tools alone: {stone} -> {farm}");
    assert!(fire > plain, "Fire increases energy per biomass: {plain} -> {fire}");
}

#[test]
fn metalworking_raises_combat_damage() {
    let damage_with = |inv: Option<usize>| -> f32 {
        let mut w = World::new(31);
        let attacker = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let target = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
        // Attacker: Mouth + Weapon; Target: distinct species so it's "other".
        let mut kit = anabios_core::module::starter_kit();
        kit.push(Module::Weapon { damage: 4.0, energy_cost: 0.0 });
        w.agents.modules[attacker as usize] = kit;
        let sid = anabios_core::prelude_test::reassign_to_new_species(&mut w, target);
        assert_ne!(sid, 0);
        if let Some(k) = inv {
            set_held(&mut w, attacker, k);
        }
        // Fire unconditionally at the nearest other.
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        size_scratch(&mut w);
        anabios_core::sense::sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &mut w.sensors,
            w.world_size,
        );
        w.actions[attacker as usize].fire_intent = 1.0;
        let before = w.agents.energy[target as usize];
        anabios_core::interact::interact_all(&mut w);
        before - w.agents.energy[target as usize]
    };
    let plain = damage_with(None);
    let metal = damage_with(Some(invention::METALWORKING));
    assert!(plain > 0.0, "weapon should deal damage: {plain}");
    // The target grazes too, so compare the DELTA: Metalworking adds
    // METALWORKING_DAMAGE × base damage (4.0) on top.
    let extra = metal - plain;
    assert!(
        (extra - invention::METALWORKING_DAMAGE * 4.0).abs() < 1e-4,
        "Metalworking adds 50% of base damage: plain={plain} metal={metal}"
    );
}

// --- Debuffs ------------------------------------------------------------------

#[test]
fn flat_upkeep_and_nuclear_income_apply_in_invention_step() {
    let mut w = World::new(37);
    w.inventions_enabled = true;
    let writer = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
    set_held(&mut w, writer, invention::WRITING);
    set_held(&mut w, writer, invention::STONE_TOOLS);
    set_held(&mut w, writer, invention::FIRE);
    set_held(&mut w, writer, invention::FARMING);
    let e0 = w.agents.energy[writer as usize];
    invention::invention_step(&mut w);
    let drained = e0 - w.agents.energy[writer as usize];
    assert!(
        (drained - invention::WRITING_UPKEEP).abs() < 1e-5,
        "Writing holder pays upkeep (fire/husbandry metabolism not in this stage): {drained}"
    );

    // Nuclear: full prereq chain held; net is income - upkeep (positive).
    let nuke = w.spawn_agent(Vec2::new(600.0, 600.0), Genome::neutral());
    for k in 0..INVENTION_COUNT {
        set_held(&mut w, nuke, k);
    }
    let e0 = w.agents.energy[nuke as usize];
    invention::invention_step(&mut w);
    let gained = w.agents.energy[nuke as usize] - e0;
    let expected = invention::NUCLEAR_INCOME
        - invention::WRITING_UPKEEP
        - invention::MEDICINE_UPKEEP
        - invention::ELECTRICITY_UPKEEP
        - invention::NUCLEAR_UPKEEP;
    assert!(
        (gained - expected).abs() < 1e-4,
        "full tree nets Nuclear income minus upkeeps: {gained} vs {expected}"
    );
}

#[test]
fn fire_holder_pays_extra_metabolism() {
    let drain_with = |inv: Option<usize>| -> f32 {
        let mut w = World::new(41);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        if let Some(k) = inv {
            set_held(&mut w, id, k);
        }
        let desired = vec![Vec2::ZERO; w.agents.capacity()];
        let before = w.agents.energy[id as usize];
        anabios_core::integrate::integrate_all(&mut w.agents, &desired, w.world_size);
        before - w.agents.energy[id as usize]
    };
    let plain = drain_with(None);
    let fire = drain_with(Some(invention::FIRE));
    assert!(
        (fire - plain * (1.0 + invention::FIRE_METABOLISM)).abs() < 1e-5,
        "Fire scales basal metabolism: {plain} -> {fire}"
    );
}

#[test]
fn medicine_extends_effective_lifespan() {
    let mut w = World::new(43);
    let mut g = Genome::neutral();
    g.set(GenomeSlot::LifespanBias, 0.0); // base lifespan = LIFESPAN_MIN_TICKS
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), g);
    set_held(&mut w, id, invention::MEDICINE);
    // Age the agent to just past the base lifespan; a Medicine holder must
    // survive (1.5x), a plain agent would die.
    w.agents.age[id as usize] = anabios_core::age::LIFESPAN_MIN_TICKS;
    w.agents.energy[id as usize] = 10.0;
    anabios_core::age::age_and_starve(&mut w);
    assert!(w.agents.is_alive(id), "Medicine extends lifespan past the base minimum");
}

#[test]
fn pollution_penalizes_biome_regrowth() {
    let mut w = World::new(47);
    // Pick a grass cell, drain it, pollute it, and compare one regrow step
    // against an unpolluted grass cell at the same biomass.
    let mut target = None;
    'outer: for row in 0..anabios_core::biome::BIOME_RES {
        for col in 0..anabios_core::biome::BIOME_RES {
            if w.biome.at(col, row).terrain == anabios_core::biome::TerrainType::Grass {
                target = Some((col, row));
                break 'outer;
            }
        }
    }
    let (col, row) = target.expect("grass cell exists");
    let idx = w.biome.cell_index(col, row);
    w.biome.cells[idx].plant_biomass = 1.0;
    let unpolluted_step = {
        let mut b = w.biome.clone();
        b.regrow_step();
        b.cells[idx].plant_biomass - 1.0
    };
    w.biome.cells[idx].pollution = 0.5;
    let polluted_step = {
        let mut b = w.biome.clone();
        b.regrow_step();
        b.cells[idx].plant_biomass - 1.0
    };
    assert!(unpolluted_step > 0.0, "grass regrows");
    assert!(
        polluted_step < unpolluted_step * 0.6,
        "pollution 0.5 roughly halves regrowth: {unpolluted_step} vs {polluted_step}"
    );
}

#[test]
fn crowding_stress_applies_to_established_holder_when_capacity_grew_this_tick() {
    // `invention_step` (tick stage 6c) runs before the second `resize_scratch`,
    // so on a tick where reproduction grew capacity the sensors buffer is still
    // sized to the top-of-tick population. The per-agent bounds check must keep
    // charging crowding stress to the established Farming holder (index 0) —
    // only the just-born slot beyond the buffer is skipped. Regression guard for
    // the old all-or-nothing `sensors_ok = len >= capacity` gate, which dropped
    // the debuff for the WHOLE population on every growth tick.
    let mut w = World::new(61);
    w.inventions_enabled = true;
    let holder = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let _newborn = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral());
    assert!(w.agents.capacity() >= 2, "two agents → capacity ≥ 2");
    // Full chain so Farming is supported (no atrophy) and pays no flat upkeep,
    // isolating crowding stress as the only energy change this tick.
    set_held(&mut w, holder, invention::STONE_TOOLS);
    set_held(&mut w, holder, invention::FIRE);
    set_held(&mut w, holder, invention::FARMING);
    // Sensors sized to ONLY the holder (len 1 < capacity 2) — the newborn slot
    // sits beyond the buffer, exactly the mid-growth-tick condition.
    w.sensors.resize(1, Default::default());
    let crowding = invention::FARMING_CROWDING_FREE + 10;
    w.sensors[holder as usize].crowding = crowding;
    let mask = invention::held_mask(&w.agents.meme_vector[holder as usize]);
    let expected = invention::crowding_stress(mask, crowding);
    assert!(expected > 0.0, "test setup: crowding must exceed the free allowance");
    let e0 = w.agents.energy[holder as usize];
    invention::invention_step(&mut w);
    let drained = e0 - w.agents.energy[holder as usize];
    assert!(
        (drained - expected).abs() < 1e-6,
        "established holder pays crowding stress on a growth tick: drained={drained} expected={expected}"
    );
}

// --- Codex ----------------------------------------------------------------------

#[test]
fn invention_discovered_fires_once_and_adopted_fires_at_majority() {
    let mut w = World::new(53);
    w.inventions_enabled = true;
    // Six agents, species 0; three hold Fire (3/6 = 50% → adopted).
    let mut ids = Vec::new();
    for n in 0..6 {
        let id = w.spawn_agent(Vec2::new(500.0 + n as f32, 500.0), Genome::neutral());
        ids.push(id);
    }
    for &id in &ids[0..3] {
        set_held(&mut w, id, invention::STONE_TOOLS);
        set_held(&mut w, id, invention::FIRE);
    }
    observe_all(&mut w);
    let events: Vec<_> = w.codex.drain_events().collect();
    let discovered: Vec<_> =
        events.iter().filter(|e| e.event_type == EventType::InventionDiscovered).collect();
    // Stone Tools and Fire each discovered exactly once (global latch).
    assert_eq!(discovered.len(), 2, "one discovery event per invention: {discovered:?}");
    let adopted: Vec<_> =
        events.iter().filter(|e| e.event_type == EventType::InventionAdopted).collect();
    assert_eq!(adopted.len(), 2, "both inventions adopted at 50%: {adopted:?}");
    assert!(adopted.iter().all(|e| e.species_id == 0));

    // Second observation: no duplicates (latched).
    observe_all(&mut w);
    let again: Vec<_> = w.codex.drain_events().collect();
    assert!(
        !again.iter().any(|e| matches!(
            e.event_type,
            EventType::InventionDiscovered | EventType::InventionAdopted
        )),
        "latches must not re-fire"
    );

    // Drop Fire adoption below 50% (2/6 holders) and back up: re-arms and
    // re-fires.
    w.agents.meme_vector[ids[0] as usize][channel(invention::FIRE)] = 0.0;
    observe_all(&mut w);
    w.codex.drain_events().for_each(drop);
    w.agents.meme_vector[ids[0] as usize][channel(invention::FIRE)] = 1.0;
    observe_all(&mut w);
    let refired: Vec<_> = w
        .codex
        .drain_events()
        .filter(|e| {
            e.event_type == EventType::InventionAdopted && e.value as usize == invention::FIRE
        })
        .collect();
    assert_eq!(refired.len(), 1, "adoption latch re-arms on drop below threshold");
}

#[test]
fn detectors_do_nothing_with_flag_off() {
    let mut w = World::new(59);
    // inventions_enabled = false; hand-plant a held invention anyway (no
    // mechanism could produce it, but the detector must still stay silent —
    // and the agg table skips invention counts entirely).
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    set_held(&mut w, id, invention::STONE_TOOLS);
    observe_all(&mut w);
    let events: Vec<_> = w.codex.drain_events().collect();
    assert!(
        !events.iter().any(|e| matches!(
            e.event_type,
            EventType::InventionDiscovered | EventType::InventionAdopted
        )),
        "no invention events with the flag off"
    );
}

#[test]
fn meme_sweep_does_not_double_fire_on_invention_channels() {
    // A species whose invention-channel mean sweeps 0 → ≥0.6 must fire
    // InventionAdopted, but NOT MemeSweep (which would double-count the same
    // phenomenon on the widened channels).
    let mut w = World::new(61);
    w.inventions_enabled = true;
    let mut ids = Vec::new();
    for n in 0..6 {
        let id = w.spawn_agent(Vec2::new(500.0 + n as f32, 500.0), Genome::neutral());
        w.agents.modules[id as usize] = comm_kit();
        ids.push(id);
    }
    // Drive MEME_SWEEP_WINDOW+ ticks with the channel low…
    for _ in 0..anabios_core::codex::MEME_SWEEP_WINDOW {
        observe_all(&mut w);
    }
    w.codex.drain_events().for_each(drop);
    // …then high for another full window.
    for &id in &ids {
        set_held(&mut w, id, invention::STONE_TOOLS);
    }
    for _ in 0..anabios_core::codex::MEME_SWEEP_WINDOW {
        observe_all(&mut w);
    }
    let events: Vec<_> = w.codex.drain_events().collect();
    assert!(
        events.iter().any(|e| e.event_type == EventType::InventionAdopted),
        "adoption reported explicitly"
    );
    assert!(
        !events.iter().any(|e| e.event_type == EventType::MemeSweep),
        "MemeSweep must not fire on an invention channel: {events:?}"
    );
}

// --- End-to-end -------------------------------------------------------------------

const INVENTIONS_SCENARIO: &str = include_str!("../../../scenarios/inventions.toml");

#[test]
fn inventions_scenario_is_deterministic() {
    let scenario = Scenario::parse_toml(INVENTIONS_SCENARIO).expect("parse inventions scenario");
    assert!(scenario.inventions_enabled);
    let run = |ticks: u64| {
        let mut w = scenario.instantiate();
        for _ in 0..ticks {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(300), run(300), "same seed + flag on → bit-identical");
}

/// Pinned golden hashes for the flag-ON inventions scenario. `determinism.rs`
/// only locks the flag-OFF `minimal.toml`, so the entire invention mechanism —
/// discovery RNG draws, copy-toward-best spread, atrophy, pollution, per-holder
/// upkeep — would be free to drift silently while `inventions_scenario_is_
/// deterministic` (self-consistency only) still passed. These hashes lock the
/// mechanism's actual behavior. Regenerate deliberately with `UPDATE_HASHES=1`
/// (prints new values to copy in) whenever an invention change is intentional.
// Refreshed 2026-07-19: MemeSweep no longer fires on invention channels (the
// InventionAdopted detector already reports those sweeps explicitly) — the
// codex event stream is serialized into the hash, so ticks 100/300 moved.
// Refreshed 2026-07-19 (2): biome trade goods added AgentBuffers.inventory,
// World.{resources,resources_enabled}, CodexState.first_cross_species_trade.
// Flag off = byte-identical trajectory; only serialized layout grew, so all
// three hashes moved.
// Refreshed 2026-07-19 (3): added World.terrain_habitat flag (geographic
// trade routes). Flag off = byte-identical trajectory; only serialized
// layout grew, so all three hashes moved.
const INVENTIONS_GOLDEN: &[(u64, u64)] =
    &[(0, 0xf691b5efa48827f6), (100, 0x24898a39cd314b21), (300, 0x87c995f8dadfdc2a)];

#[test]
fn inventions_scenario_matches_golden_hashes() {
    let scenario = Scenario::parse_toml(INVENTIONS_SCENARIO).expect("parse inventions scenario");
    let mut w = scenario.instantiate();

    let max_tick = INVENTIONS_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < INVENTIONS_GOLDEN.len() && INVENTIONS_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }

    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated inventions hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }

    for ((exp_tick, exp_hash), (got_tick, got_hash)) in INVENTIONS_GOLDEN.iter().zip(&observed) {
        assert_eq!(exp_tick, got_tick, "tick mismatch");
        assert_eq!(
            *exp_hash, *got_hash,
            "invention hash drift at tick {exp_tick}: expected 0x{exp_hash:016x}, got 0x{got_hash:016x}.\n\
             If intentional, rerun with UPDATE_HASHES=1 and copy the printed values.",
        );
    }
}

#[test]
fn innovators_discover_before_traditionalists_in_demo_scenario() {
    // The demo's core promise: with the flag on, the high-Openness culture
    // produces discoveries and the tree's first era appears within a few
    // hundred ticks.
    let scenario = Scenario::parse_toml(INVENTIONS_SCENARIO).expect("parse inventions scenario");
    let mut w = scenario.instantiate();
    let mut first_discovery_tick = None;
    let mut stone_seen = false;
    for _ in 0..2000 {
        step(&mut w);
        for ev in w.codex.drain_events() {
            if ev.event_type == EventType::InventionDiscovered {
                if first_discovery_tick.is_none() {
                    first_discovery_tick = Some(ev.tick);
                }
                if ev.value as usize == invention::STONE_TOOLS {
                    stone_seen = true;
                }
            }
        }
        if stone_seen {
            break;
        }
    }
    assert!(stone_seen, "Stone Tools should be discovered within 2000 ticks");
    let t = first_discovery_tick.unwrap();
    assert!(t > 0 && t < 1500, "first discovery reasonably early, got tick {t}");
}
