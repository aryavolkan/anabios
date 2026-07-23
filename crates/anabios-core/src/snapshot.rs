//! World snapshot save/load + deterministic state hash.
//!
//! The serialized format is a versioned envelope around bincode-encoded
//! `World` bytes. `format_version` exists so future code can refuse or
//! migrate old snapshots cleanly.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::world::World;

/// Current snapshot format version. Bump on any breaking change to the
/// serialized layout — and note bincode is NOT self-describing: adding,
/// removing, or reordering a serialized field anywhere in `World` (or a type
/// it contains) changes the byte layout, so such changes MUST bump this
/// constant. `#[serde(default)]` on a new field does not let bincode read
/// old payloads; it only helps self-describing formats.
/// v2: BiomeCell.env climate field + World.biome_adaptation/env_period.
/// v3: World.max_population.
/// v4: World.cultural_inventions flag (superseded by v5).
/// v5: the cultural-inventions ratchet is replaced by the full invention
///     tree: MEME_CHANNELS widened 8→18 (inventions ride meme channels),
///     BiomeCell.pollution, World.inventions_enabled (renamed from
///     cultural_inventions), CodexState invention latches.
/// v6: biome trade goods — AgentBuffers.inventory, World.{resources,
///     resources_enabled}, CodexState.first_cross_species_trade. Behavior
///     unchanged with resources_enabled off; only serialized layout grew.
/// v6 (main, merged): cognitive layer Phase 1 — World.cognition_enabled +
///     AgentBuffers realized-IQ phenotype fields (iq / iq_enrich_acc /
///     iq_enrich_ticks).
/// v7: geographic trade routes — World.terrain_habitat flag (opt-in terrain
///     habitat selection). Behavior unchanged with the flag off; only the
///     serialized layout grew.
/// v7 (main, merged): cognitive layer Phase 3/4 — MEME_CHANNELS widened 18→20
///     for the maladaptive-practice block (channels 18-19) + CodexState practice
///     latches (practices_discovered / practices_adopted).
/// v8: merge of the biome-trade-goods branch with main's cognitive layer — the
///     combined serialized layout carries BOTH feature sets' new fields, so the
///     version advances past both branches' v7.
/// v9: E3 population-dynamics detectors — CodexState cycle/plateau/cascade
///     scratch (cycle_history, cycle/boom/carrying latches, cascade state).
///     Agent behavior unchanged; the event stream gains the four new types.
pub const FORMAT_VERSION: u32 = 9;

#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
    format_version: u32,
    payload: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("unsupported snapshot format version {found}, expected {expected}")]
    Version { found: u32, expected: u32 },
}

pub fn save_to_bytes(world: &World) -> Result<Vec<u8>, SnapshotError> {
    let payload = bincode::serialize(world)?;
    let env = Envelope { format_version: FORMAT_VERSION, payload };
    Ok(bincode::serialize(&env)?)
}

pub fn load_from_bytes(bytes: &[u8]) -> Result<World, SnapshotError> {
    let env: Envelope = bincode::deserialize(bytes)?;
    if env.format_version != FORMAT_VERSION {
        return Err(SnapshotError::Version { found: env.format_version, expected: FORMAT_VERSION });
    }
    let mut world: World = bincode::deserialize(&env.payload)?;
    world.pheromones.refresh_nonzero();
    Ok(world)
}

/// A 64-bit fingerprint of the world's persistent state. Uses FNV-1a over
/// the bincode-serialized payload. Suitable for golden-tick replay tests.
pub fn state_hash(world: &World) -> u64 {
    // Don't include scratch buffers; only persistent fields are serialized.
    let payload = bincode::serialize(world).expect("world is always serializable");
    fnv1a_64(&payload)
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut h = FNV_OFFSET;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::tick::step;

    #[test]
    fn roundtrip_preserves_state() {
        let mut w = World::new(123);
        for _ in 0..5 {
            let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        for _ in 0..20 {
            step(&mut w);
        }
        let bytes = save_to_bytes(&w).expect("save");
        let w2 = load_from_bytes(&bytes).expect("load");
        assert_eq!(w.tick, w2.tick);
        assert_eq!(w.agents.live_count(), w2.agents.live_count());
        assert_eq!(state_hash(&w), state_hash(&w2));
    }

    #[test]
    fn state_hash_differs_after_a_tick() {
        let mut w = World::new(7);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let h0 = state_hash(&w);
        step(&mut w);
        let h1 = state_hash(&w);
        assert_ne!(h0, h1);
    }

    #[test]
    fn version_mismatch_is_rejected() {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::ZERO, Genome::neutral());
        let bytes = save_to_bytes(&w).expect("save");
        // Mutate the version byte. The Envelope is `{format_version: u32,
        // payload: Vec<u8>}`; bincode encodes the u32 LE first.
        let mut tampered = bytes.clone();
        tampered[0] = 99;
        let err = load_from_bytes(&tampered).expect_err("should error");
        assert!(matches!(err, SnapshotError::Version { .. }));
    }

    #[test]
    fn old_format_version_is_rejected_cleanly() {
        // Forge a previous-version envelope around an otherwise-valid payload:
        // the version gate must reject it with the clean `Version` error —
        // not the cryptic bincode EOF error the payload parse would produce
        // (bincode is not self-describing, so an older `World` layout can
        // never reach deserialization).
        let mut w = World::new(3);
        let _ = w.spawn_agent(Vec2::ZERO, Genome::neutral());
        let env = Envelope {
            format_version: FORMAT_VERSION - 1,
            payload: bincode::serialize(&w).unwrap(),
        };
        let bytes = bincode::serialize(&env).unwrap();
        let err = load_from_bytes(&bytes).expect_err("old version must be rejected");
        match err {
            SnapshotError::Version { found, expected } => {
                assert_eq!(found, FORMAT_VERSION - 1);
                assert_eq!(expected, FORMAT_VERSION);
            }
            other => panic!("expected Version error, got {other}"),
        }
    }

    #[test]
    fn pheromone_decay_continues_after_roundtrip() {
        let mut w = World::new(9);
        w.pheromones.deposit(Vec2::new(100.0, 100.0), 0, 1.0);
        let mut w2 = load_from_bytes(&save_to_bytes(&w).expect("save")).expect("load");
        // The serde-skipped `nonzero` cache must be refreshed on load, or the
        // loaded world's decay_step would silently become a no-op.
        w.pheromones.decay_step();
        w2.pheromones.decay_step();
        assert_eq!(w.pheromones.cells, w2.pheromones.cells);
        assert!(
            w2.pheromones.sample(Vec2::new(100.0, 100.0), 0) < 1.0,
            "loaded world keeps decaying its pheromone field"
        );
    }

    #[test]
    fn loaded_world_continues_bit_identically() {
        let mut w = World::new(77);
        // Populate the subsystems whose state lives behind serde(skip)
        // scratch: two species (codex agg), pheromones (nonzero flag), a
        // carcass (carcass_spatial).
        for k in 0..5 {
            let _ = w.spawn_agent(Vec2::new(500.0 + k as f32, 500.0), Genome::neutral());
        }
        let migrant = w.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
        crate::prelude_test::reassign_to_new_species(&mut w, migrant);
        w.pheromones.deposit(Vec2::new(500.0, 500.0), 0, 2.0);
        w.carcasses.push(crate::carcass::Carcass {
            pos: Vec2::new(501.0, 500.0),
            flesh: 5.0,
            age: 0,
            species_id: 0,
        });
        for _ in 0..30 {
            step(&mut w);
        }
        let mut w2 = load_from_bytes(&save_to_bytes(&w).expect("save")).expect("load");
        for _ in 0..30 {
            step(&mut w);
            step(&mut w2);
        }
        // Every #[serde(skip)] scratch buffer (agent + carcass spatial hashes,
        // codex agg, sensors, pheromone flag) must rebuild itself on the fly.
        assert_eq!(state_hash(&w), state_hash(&w2));
    }
}
