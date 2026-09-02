//! Disease & epidemiology subsystem (flag `disease_enabled`): a crowding-seeded
//! SIS pathogen. Infection spills over in crowded populations, spreads by
//! proximity, and drains energy per tick — mortality funnels through the
//! existing `age::age_and_starve` path (carcasses, scavenging, war attribution
//! all keep working untouched). Medicine finally has a pressure to counter:
//! holders transmit less and recover faster.
//!
//! Everything here is gated on `World::disease_enabled`: with the flag off the
//! stage early-returns with zero state change and zero RNG draws, so every
//! pre-disease scenario stays byte-identical. Design:
//! `docs/superpowers/specs/2026-09-01-disease-epidemiology-design.md`.

use crate::world::World;
use std::collections::BTreeMap;

/// Crowding probe radius for zoonotic spillover.
pub const SPILLOVER_RADIUS: f32 = 8.0;
/// Minimum neighbors within `SPILLOVER_RADIUS` for spillover to be possible.
pub const SPILLOVER_MIN_NEIGHBORS: u32 = 6;
/// Per-tick spillover probability for a crowded susceptible agent.
pub const SPILLOVER_P: f32 = 0.0002;
/// Contact radius for agent-to-agent transmission.
pub const TRANSMISSION_RADIUS: f32 = 4.0;
/// Per-contact per-tick transmission probability (before medicine adjustment).
pub const TRANSMIT_P: f32 = 0.05;
/// Infection intensity a freshly infected agent starts at.
pub const INFECTION_SEED: f32 = 0.3;
/// Minimum intensity for an agent to shed to neighbors.
pub const SHED_MIN: f32 = 0.1;
/// Deterministic per-tick recovery (before the medicine multiplier).
pub const RECOVERY_RATE: f32 = 0.01;
/// Per-tick energy drain at infection intensity 1.0.
pub const DISEASE_DRAIN: f32 = 0.02;
/// Recovery-rate multiplier for Medicine holders.
pub const MEDICINE_RECOVERY_MULT: f32 = 3.0;
/// Susceptibility multiplier for Medicine holders.
pub const MEDICINE_TRANSMIT_MULT: f32 = 0.25;
/// Minimum live members of a species for the outbreak detector to engage.
pub const OUTBREAK_MIN_POP: u32 = 20;
/// Infected-fraction threshold at which `EpidemicOutbreak` fires.
pub const OUTBREAK_FRACTION: f32 = 0.25;
/// Infected-fraction threshold below which an outbreak latch re-arms (and a
/// medicine-bearing species resolving its wave fires `MedicineContainment`).
pub const OUTBREAK_REARM: f32 = 0.125;

/// Tick stage 6g (after knowledge 6f, before age+starve 7). Flag-gated
/// early return with zero RNG draws when off. All iteration is ascending id /
/// `BTreeMap`, and RNG draws happen only in the two resolve passes below, in
/// ascending-agent order, so the draw stream is a pure function of the state.
pub fn disease_step(world: &mut World) {
    if !world.disease_enabled {
        return;
    }
    let medicine_bit = crate::invention::bit(crate::invention::MEDICINE);
    let ws = world.world_size;

    // 1. Recovery & drain (ascending id; deterministic, no draws). Recovery
    //    runs first so a just-cured agent sheds nothing and pays no drain this
    //    tick. Deaths are NOT applied here — the energy drain is resolved by
    //    `age::age_and_starve` later in the tick.
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
    for &id in &alive_ids {
        let i = id as usize;
        let inf = world.agents.infection[i];
        if inf <= 0.0 {
            continue;
        }
        let held = crate::invention::held_mask(&world.agents.meme_vector[i]);
        let rec = if held & medicine_bit != 0 {
            RECOVERY_RATE * MEDICINE_RECOVERY_MULT
        } else {
            RECOVERY_RATE
        };
        let next = (inf - rec).max(0.0);
        world.agents.infection[i] = next;
        if next > 0.0 {
            world.agents.energy[i] -= next * DISEASE_DRAIN;
        }
    }

    // 2. Spillover: crowded susceptibles develop infection spontaneously. One
    //    `world.rng` draw per crowded susceptible, in ascending id order. No
    //    draws for uncrowded agents.
    let mut crowded: Vec<u32> = Vec::new();
    for &id in &alive_ids {
        let i = id as usize;
        if world.agents.infection[i] > 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let mut count: u32 = 0;
        world.spatial.query(pos, SPILLOVER_RADIUS, |other| {
            let o = other as usize;
            if o != i
                && world.agents.alive[o]
                && crate::spatial::torus_distance_sq(pos, world.agents.position[o], ws)
                    <= SPILLOVER_RADIUS * SPILLOVER_RADIUS
            {
                count += 1;
            }
        });
        if count >= SPILLOVER_MIN_NEIGHBORS {
            crowded.push(id);
        }
    }
    for id in crowded {
        if world.rng.f32_unit() < SPILLOVER_P {
            world.agents.infection[id as usize] = INFECTION_SEED;
        }
    }

    // 3. Transmission: shedders expose nearby susceptibles. Two-pass so the
    //    result is order-independent: collect candidates keyed by target
    //    (dedup keeps the strongest source's probability), then resolve in
    //    ascending-target order with one draw each.
    let mut candidates: BTreeMap<u32, f32> = BTreeMap::new();
    for &id in &alive_ids {
        let i = id as usize;
        if world.agents.infection[i] < SHED_MIN {
            continue;
        }
        let pos = world.agents.position[i];
        world.spatial.query(pos, TRANSMISSION_RADIUS, |other| {
            let o = other as usize;
            if o == i || !world.agents.alive[o] || world.agents.infection[o] > 0.0 {
                return;
            }
            if crate::spatial::torus_distance_sq(pos, world.agents.position[o], ws)
                > TRANSMISSION_RADIUS * TRANSMISSION_RADIUS
            {
                return;
            }
            let held = crate::invention::held_mask(&world.agents.meme_vector[o]);
            let p = if held & medicine_bit != 0 {
                TRANSMIT_P * MEDICINE_TRANSMIT_MULT
            } else {
                TRANSMIT_P
            };
            candidates.entry(other).and_modify(|best| *best = best.max(p)).or_insert(p);
        });
    }
    for (target, p) in candidates {
        if world.rng.f32_unit() < p {
            world.agents.infection[target as usize] = INFECTION_SEED;
        }
    }

    world.agents.scratch_ids = alive_ids;
}
