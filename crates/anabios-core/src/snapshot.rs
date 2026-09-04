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
/// v10: E4 disturbance & succession — BiomeCell.succession,
///     World.{disasters_enabled, disasters}, CodexState disturbance-detector
///     scratch. `disasters_enabled` off in all golden scenarios ⇒ behavior
///     unchanged; only the serialized layout grew.
/// v11: E5 trait-evolution instruments — SpeciesAgg genome moments,
///     CodexState trait-detector scratch (genome_moments, fixation/rapid/
///     convergence state). Pure observers; behavior unchanged.
/// v12: E6 named behaviors — CombatHit ambush/tool_boosted context,
///     CodexState signature-detector scratch. Observability only; behavior
///     unchanged.
/// v13: E6 fix — `World.{still_ticks, prev_desired_direction}` are now
///     serialized (were `#[serde(skip)]`). They are path-dependent
///     accumulators feeding serialized codex state (ambush/signal detection),
///     so dropping them on load made restore-and-continue diverge from a
///     continuous run. Behavior unchanged; only the serialized layout grew.
/// v14: E7 kin & war — CodexState hostility records + war/alliance/kin
///     detector scratch, share_events gains recipient species, program
///     Node::SenseHostility (appended, serde-stable), World.war_enabled.
///     The mutation pool only widens behind the flag (off in all golden
///     scenarios) — layout growth only, behavior byte-identical. Merged onto
///     the E6 v13 still_ticks serialization, so the combined layout is v14.
/// v15: E8 settlements & economy — AgentBuffers.{anchor, harvest_exp},
///     World.{settlement_enabled, market_field}, anchor Sense nodes
///     (appended, mutation pool gated on settlement_enabled), CodexState
///     settlement/market/specialization scratch. Flags off in golden
///     scenarios — layout growth only. Merged onto the combined E6+E7 v14.
/// v16: E9 traditions — AgentBuffers.meme_lineage, CodexState variant
///     registry + tradition/radiation/ratchet scratch, inherit_meme fidelity
///     parameter (1.0 baseline = byte-identical draw values). Layout growth
///     only; the fidelity effect is gated on settlement latches (off in all
///     golden scenarios), so behavior is byte-identical there.
/// v17: E10 drifting climate — World.climate_drift_rate (radians/tick of a slow
///     secular drift added to culture::env_optimum_at on top of the seasonal
///     cycle). `0.0` in every golden scenario ⇒ env_optimum_at short-circuits to
///     the exact undrifted value; only the serialized layout grew by one f32.
/// v18: E11 climate maladaptation — CodexState.{maladapt_streak, maladapt_active}
///     + EventType::MaladaptationLag. The detector short-circuits when
///     `env_period == 0` (every golden scenario), so it never fires there;
///     behavior is byte-identical and only the serialized layout grew.
/// v19: TG1 gene↔tech coupling — World.gene_tech_coupling flag. Off in every
///     golden scenario ⇒ byte-identical behavior; only the layout grew.
/// v20: E12 sexual dimorphism — AgentBuffers.sex bit column,
///     World.sexual_dimorphism_enabled flag, CodexState dimorphism-detector
///     latches, EventType::{SexualSelection, SexRatioCollapse}. Flag off ⇒
///     zero extra RNG draws and identity stat factors; only the serialized
///     layout grew.
/// v21: E13 domestication — AgentBuffers.livestock_of column,
///     World.domestication_enabled flag, CodexState.{domesticated_species,
///     livestock_herd_streak, livestock_herd_active}, EventType::
///     {AnimalDomesticated, LivestockHerd}. Flag off ⇒ the tick stage
///     early-returns and zero RNG is drawn; only the serialized layout grew.
/// v22: climate-driven worldgen — BiomeCell.moisture (new f32); env now holds
///     temperature; TerrainType gained Savanna/Rainforest/Taiga/Tundra;
///     `generate` replaced by the gradient-noise + Whittaker pipeline. This
///     changes every world's terrain, so ALL golden hashes are regenerated
///     (a genuine trajectory change, not a byte-identical layout growth).
/// v23: hard genetic invention prerequisites — World.gene_requirements flag
///     (gates `invention::GeneReq` on discovery + social copy). Off in every
///     golden scenario ⇒ byte-identical behavior; only the layout grew.
/// v24: supply-side trade fix — World.conserve_goods_on_death flag. Off in
///      every existing scenario; serialized layout grew by one byte.
/// v25: maladaptive-practices toggle — World.practices_enabled flag (gates
///     `practice::discover_step`). Defaults `true`, so practices still run
///     wherever cognition is on ⇒ behavior unchanged in every golden scenario;
///     only the serialized layout grew (layered on v24's conserve_goods field).
///     (Reproducible O1-autopsy lever.)
/// v26: knowledge-accumulation subsystem — World.knowledge_enabled flag +
///      CodexState.{knowledge_by_species, knowledge_ratchet_fired}. Off in every
///      golden scenario; only the serialized layout grew.
/// v27: M-A+M-B subcortical affect layer — AgentBuffers.affect column (7
///      serialized f32 per agent), World.affect_enabled flag, genome temperament
///      slots 17/18/19/34/35 renamed in place (values/indices unchanged), and
///      EventType::MassFright (appended after KnowledgeRatchet). affect_enabled
///      off in every existing golden scenario ⇒ develop_all no-ops (zero RNG),
///      read-side hooks are identity, speed factor exactly 1.0, and the FEAR/
///      hijack paths are gated off — behavior byte-identical; only the serialized
///      layout grew (stacked on v26's knowledge layout).
/// v28: affect layer M-D — AgentBuffers.affect_prev_crowding (new serialized
///      f32 column: one-tick crowding memory for PANIC/GRIEF separation
///      detection). affect_enabled is off in every golden scenario ⇒
///      develop_all early-returns, the column stays 0.0, and behaviour is
///      byte-identical; only the serialized layout grew.
/// v29: M-F affect observability — CodexState.{frenzy_active, rage_streak,
///      rage_active, fear_count_history, cascade_active, grief_active} +
///      EventType::{PanicCascade, FeedingFrenzy, TerritorialRage, MassGrief}.
///      All detectors are gated on `affect_enabled` (off in every golden
///      scenario), so they never fire there — behavior byte-identical; only
///      the serialized layout grew. (The affect columns themselves were added
///      in M-A.)
/// 30: BiomeCell.elevation + river_flow (continental worldgen).
/// v30: O2 payoff-biased learning — World.payoff_biased_learning flag.
/// v31 (main): unilateral trade — World.unilateral_trade flag (surplus-gift
///      fallback in `interact::trade_pass`). Off in every golden scenario ⇒
///      byte-identical; only the serialized layout grew.
/// v31 (this branch, parallel): merge of continental-worldgen BiomeCell fields
///      (elevation/river_flow) with O2 payoff-biased learning.
/// 32: merge of this branch's v31 with main's v31 — all three layout growths
///     (worldgen BiomeCell fields, payoff-biased learning, unilateral trade)
///     stacked. Every flag is off/inert in every golden scenario ⇒ trajectories
///     byte-identical; only the serialized layout grew. Bumped to 32 to
///     distinguish the merged layout from either parent's v31.
/// 33: add `World.trade_hubs` (predetermined trade-hub placements), populated
///     from the finalized biome at scenario instantiate when
///     `resources_enabled`. Layout growth only.
/// 34: add `World.anthro_race_enabled` + `World.culture_roots` and
///     `CodexState.hunted_baselines`/`hunted_active` (anthropogenic arms race).
///     Flag off in every golden scenario ⇒ trajectories byte-identical;
///     only the serialized layout grew.
/// 35: O3 repro-biased learning — `World.repro_biased_learning` flag +
///     `AgentBuffers.births_ok`/`births_failed` columns (merged first, PR #145).
/// 36: basic needs (thirst + sleep) — AgentBuffers.{thirst, fatigue, asleep}
///     columns + World.basic_needs_enabled flag + EventType::Dehydration
///     (appended) + genome slots 8/9 renamed in place
///     (ThirstTolerance/SleepNeed; indices/values unchanged). Flag off in
///     every golden scenario ⇒ needs_step no-ops (zero RNG), the columns stay
///     0/false, and every read-side hook is exact identity — behavior
///     byte-identical; only the serialized layout grew. Re-bumped from 35 at
///     merge time (v31/v32 protocol). ⚠ The parallel disease branch (PR #143)
///     also claims v35 — it must re-bump to 37 when it merges.
pub const FORMAT_VERSION: u32 = 36;

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
    // Re-derive the (serde-skipped) livestock gate from the persisted flag so a
    // reloaded domestication world clears orphaned owners exactly like the live one.
    world.agents.track_livestock = world.domestication_enabled;
    // Re-derive the (serde-skipped) spatial hashes from the persisted world
    // dims. They reset to `Default` on load — the 1024/64 grid — which is only
    // correct for default-dim worlds: a custom `world_size`/`hash_res` world
    // would reload with the wrong `cell_size` (clamping perception radii wrong)
    // and wrong torus extent, silently diverging on the next step. Mirror
    // `World::with_dims` exactly.
    world.spatial = crate::spatial::UniformSpatialHash::with_dims(world.world_size, world.hash_res);
    world.carcass_spatial =
        crate::spatial::UniformSpatialHash::with_dims(world.world_size, world.hash_res);
    world.resource_spatial =
        crate::spatial::UniformSpatialHash::with_dims(world.world_size, world.hash_res);
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

    #[test]
    fn ambush_and_signal_accumulators_survive_roundtrip() {
        // Regression: `still_ticks` / `prev_desired_direction` are
        // path-dependent accumulators that feed SERIALIZED codex state
        // (`sig_hit_log`/`ambush_active` via combat_pass, `signal_responses`
        // via detect_structured_signaling). If they are dropped on load, a
        // restored world computes a different `ambush`/signaling verdict for
        // the next several ticks and its state hash diverges from a continuous
        // run — silently breaking replay. They must persist across a snapshot.
        let mut w = World::new(5);
        for _ in 0..8 {
            let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        for _ in 0..12 {
            step(&mut w);
        }
        // Stamp distinctive accumulator content (some agents past the
        // AMBUSH_STILL_MIN threshold) so the assertions are meaningful.
        for (i, v) in w.still_ticks.iter_mut().enumerate() {
            *v = (i as u32 * 13) % 90;
        }
        for (i, d) in w.prev_desired_direction.iter_mut().enumerate() {
            *d = Vec2::new(0.1 * i as f32, -0.2);
        }
        let w2 = load_from_bytes(&save_to_bytes(&w).expect("save")).expect("load");
        assert_eq!(w.still_ticks, w2.still_ticks, "still_ticks must persist across a snapshot");
        assert_eq!(
            w.prev_desired_direction, w2.prev_desired_direction,
            "prev_desired_direction must persist across a snapshot"
        );
        assert_eq!(state_hash(&w), state_hash(&w2));
    }
}
