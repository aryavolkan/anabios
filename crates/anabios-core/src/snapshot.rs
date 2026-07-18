//! World snapshot save/load + deterministic state hash.
//!
//! The serialized format is a versioned envelope around bincode-encoded
//! `World` bytes. `format_version` exists so future code can refuse or
//! migrate old snapshots cleanly.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::world::World;

/// Current snapshot format version. Bump on any breaking change to the
/// serialized layout.
pub const FORMAT_VERSION: u32 = 3;

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
}
