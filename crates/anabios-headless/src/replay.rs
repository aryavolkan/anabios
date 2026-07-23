//! Replay verification: re-simulate codex events from periodic snapshots and
//! assert bit-identical reproduction — the detector-regression harness.
//!
//! Pass 1 records periodic snapshots and per-event state hashes. Pass 2
//! rewinds to the nearest snapshot at or before an event's tick, re-sims
//! forward, and checks both the state hash at the event tick and that the
//! same event type re-fires at the same tick. Determinism (design §7.2)
//! makes both checks strict equality.

use std::path::PathBuf;

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;
use anyhow::{Context, Result};

use crate::score;

/// One codex event recorded during pass 1, with the world hash at its tick.
pub struct EventRecord {
    pub index: usize,
    pub event_type: EventType,
    pub tick: u64,
    pub hash: u64,
}

/// Verification result for one replayed event.
pub struct ReplayOutcome {
    pub index: usize,
    pub event_type: EventType,
    pub tick: u64,
    /// Re-simmed world hash equals the pass-1 hash at the event tick.
    pub hash_ok: bool,
    /// The same event type re-fired at the same tick during re-sim.
    pub refired: bool,
}

impl ReplayOutcome {
    pub fn passed(&self) -> bool {
        self.hash_ok && self.refired
    }
}

/// Periodic rewind points: `(tick, snapshot bytes)`, ascending by tick.
pub type SnapshotLog = Vec<(u64, Vec<u8>)>;

/// Pass 1: run `ticks` ticks, snapshotting at tick 0 and every `every`
/// ticks, recording each codex event with the world hash at its tick.
pub fn record_run(
    scenario_text: &str,
    seed: Option<u64>,
    ticks: u64,
    every: u64,
) -> Result<(SnapshotLog, Vec<EventRecord>)> {
    anyhow::ensure!(every > 0, "snapshot interval must be > 0 (got {every})");
    let mut scenario = Scenario::parse_toml(scenario_text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }
    let mut world = scenario.instantiate();

    let mut snaps: Vec<(u64, Vec<u8>)> = Vec::new();
    snaps.push((0, save_to_bytes(&world).context("snapshot at tick 0")?));
    let mut records = Vec::new();

    for _ in 0..ticks {
        step(&mut world);
        let drained: Vec<_> = world.codex.drain_events().collect();
        for ev in drained {
            let index = records.len();
            records.push(EventRecord {
                index,
                event_type: ev.event_type,
                tick: ev.tick,
                hash: state_hash(&world),
            });
        }
        if world.tick % every == 0 {
            snaps.push((world.tick, save_to_bytes(&world).context("periodic snapshot")?));
        }
    }
    Ok((snaps, records))
}

/// Pass 2: replay one event from the nearest snapshot at or before its tick.
///
/// Tick convention: detectors run before `world.tick += 1` (`tick.rs`), so an
/// event stamped `tick = T` is emitted by the step that ends at
/// `world.tick == T + 1` — which is also the state pass 1 hashed. Re-sim
/// therefore runs through `world.tick == T + 1`.
pub fn verify(snaps: &SnapshotLog, rec: &EventRecord) -> Result<ReplayOutcome> {
    let (_, bytes) = snaps
        .iter()
        .filter(|(t, _)| *t <= rec.tick)
        .max_by_key(|(t, _)| *t)
        .context("no snapshot at or before event tick")?;
    let mut world = load_from_bytes(bytes).context("loading rewind snapshot")?;

    let mut refired = false;
    while world.tick <= rec.tick {
        step(&mut world);
        for ev in world.codex.drain_events() {
            if ev.event_type == rec.event_type && ev.tick == rec.tick {
                refired = true;
            }
        }
    }
    Ok(ReplayOutcome {
        index: rec.index,
        event_type: rec.event_type,
        tick: rec.tick,
        hash_ok: state_hash(&world) == rec.hash,
        refired,
    })
}

pub fn run(
    scenario_path: PathBuf,
    seed: Option<u64>,
    ticks: u64,
    snapshot_every: u64,
    event: Option<usize>,
    all: bool,
) -> Result<bool> {
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;
    let (snaps, records) = record_run(&text, seed, ticks, snapshot_every)?;
    eprintln!(
        "[replay] pass 1: {} ticks, {} snapshots, {} events",
        ticks,
        snaps.len(),
        records.len()
    );

    let selected: Vec<&EventRecord> = match (event, all) {
        (Some(k), _) => records.iter().filter(|r| r.index == k).collect(),
        (None, _) => records.iter().collect(),
    };
    if selected.is_empty() {
        eprintln!("[replay] no events to replay");
        return Ok(true);
    }

    let mut failures = 0;
    for (i, rec) in selected.iter().enumerate() {
        let out = verify(&snaps, rec)?;
        if !out.passed() {
            failures += 1;
        }
        // Cap log spam: every outcome is still verified, only printing is cut.
        if i < 50 || !out.passed() {
            println!(
                "{} event=#{:<4} type={:<20} tick={:<6} hash_ok={} refired={}",
                if out.passed() { "PASS" } else { "FAIL" },
                out.index,
                score::event_name(out.event_type),
                out.tick,
                out.hash_ok,
                out.refired
            );
        }
    }
    let checked = selected.len();
    println!("[replay] {checked} events replayed, {failures} failures");
    Ok(failures == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn predator_prey_text() -> String {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/predator-prey.toml");
        std::fs::read_to_string(path).expect("predator-prey scenario")
    }

    #[test]
    fn zero_snapshot_interval_errors_not_panics() {
        let text = predator_prey_text();
        match record_run(&text, Some(7), 10, 0) {
            Ok(_) => panic!("every=0 must error, not panic or succeed"),
            Err(e) => assert!(e.to_string().contains("snapshot interval")),
        }
    }

    #[test]
    fn replay_reproduces_events_bit_identically() {
        let text = predator_prey_text();
        let (snaps, records) = record_run(&text, Some(7), 500, 100).expect("record");
        assert!(!records.is_empty(), "expected at least one event in 500 ticks of predator-prey");
        // Verify every recorded event (500-tick runs produce a handful).
        for rec in &records {
            let out = verify(&snaps, rec).expect("verify");
            assert!(
                out.passed(),
                "event #{} ({:?} @ {}) failed: hash_ok={} refired={}",
                out.index,
                out.event_type,
                out.tick,
                out.hash_ok,
                out.refired
            );
        }
    }

    #[test]
    fn replay_detects_trajectory_divergence() {
        let text = predator_prey_text();
        let (snaps, records) = record_run(&text, Some(7), 500, 100).expect("record");
        let rec = records.first().expect("at least one event");
        let out = verify(&snaps, rec).expect("verify");
        assert!(out.hash_ok);

        // Tamper: simulate a detector/pipeline regression by demanding the
        // hash of a different tick — the equality check must fail.
        let wrong = EventRecord {
            hash: rec.hash ^ 0xdead,
            ..EventRecord {
                index: rec.index,
                event_type: rec.event_type,
                tick: rec.tick,
                hash: rec.hash,
            }
        };
        let out2 = verify(&snaps, &wrong).expect("verify tampered");
        assert!(!out2.hash_ok, "perturbed target hash must not match");
        assert!(!out2.passed());
    }
}
