//! Soak / novelty-decay harness: run ONE scenario for a long horizon and,
//! every window, record emergence + telemetry to measure whether the world
//! keeps generating novelty (the E10 payoff of drifting climate).
//!
//! Pure post-processing over the sim — like `sweep`, it only drains events and
//! samples read-only state (`snapshot::save_to_bytes` for a size proxy). It
//! never mutates sim behavior.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anabios_core::codex::EVENT_TYPE_COUNT;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::save_to_bytes;
use anabios_core::tick::step;
use anyhow::{Context, Result};

use crate::score::event_name;

/// One window's worth of soak measurements.
#[derive(Debug, Clone)]
pub struct WindowRow {
    /// 1-based window index.
    pub window: u64,
    /// Absolute tick at the end of this window.
    pub end_tick: u64,
    /// Total codex events fired during this window.
    pub events: u64,
    /// Event types first seen in THIS window (the novelty signal).
    pub new_types: u64,
    /// Distinct event types seen so far, cumulative (coverage numerator).
    pub cumulative_types: u64,
    /// Cumulative distinct types as a fraction of all event types.
    pub coverage: f64,
    /// Live agent count sampled at the end of the window.
    pub live_agents: u32,
    /// `snapshot::save_to_bytes(&world).len()` sampled once at window end —
    /// a cheap serialized-state-size proxy (watch for pathological growth).
    pub state_bytes: u64,
    /// Wall-clock throughput over the window's ticks.
    pub ticks_per_sec: f64,
}

/// Run a soak over `ticks` ticks in `window`-sized chunks, returning one row
/// per completed window. A trailing partial window (when `ticks % window != 0`)
/// is reported as a final shorter window.
pub fn soak(scenario_text: &str, seed: u64, ticks: u64, window: u64) -> Result<Vec<WindowRow>> {
    anyhow::ensure!(window > 0, "window must be > 0");
    let mut scenario = Scenario::parse_toml(scenario_text)?;
    scenario.seed = seed;
    let mut world = scenario.instantiate();

    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    let mut rows = Vec::new();

    let mut done = 0_u64;
    let mut window_idx = 0_u64;
    while done < ticks {
        let this_window = window.min(ticks - done);
        window_idx += 1;

        let mut events = 0_u64;
        let mut new_types = 0_u64;
        let start = Instant::now();
        for _ in 0..this_window {
            step(&mut world);
            for ev in world.codex.drain_events() {
                events += 1;
                let name = event_name(ev.event_type);
                if seen.insert(name) {
                    new_types += 1;
                }
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        let ticks_per_sec =
            if elapsed > 0.0 { this_window as f64 / elapsed } else { f64::INFINITY };

        let state_bytes = save_to_bytes(&world).map(|b| b.len() as u64).unwrap_or(0);

        done += this_window;
        rows.push(WindowRow {
            window: window_idx,
            end_tick: world.tick,
            events,
            new_types,
            cumulative_types: seen.len() as u64,
            coverage: seen.len() as f64 / EVENT_TYPE_COUNT as f64,
            live_agents: world.agents.live_count(),
            state_bytes,
            ticks_per_sec,
        });
    }
    Ok(rows)
}

/// CLI entry point: run the soak, print a summary table, and write
/// `soak-curve.csv` under `out`.
pub fn run(
    scenario_path: PathBuf,
    ticks: u64,
    window: u64,
    seed: Option<u64>,
    out: PathBuf,
) -> Result<()> {
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    let seed = seed.unwrap_or(scenario.seed);
    scenario.seed = seed;

    eprintln!(
        "[soak] scenario={} seed={} ticks={} window={} ({} event types tracked)",
        scenario.name, seed, ticks, window, EVENT_TYPE_COUNT
    );

    let rows = soak(&text, seed, ticks, window)?;

    write_csv(&out, scenario.name.as_str(), seed, &rows)?;
    print_table(&rows);
    eprintln!("[soak] wrote {}", out.display());
    Ok(())
}

fn print_table(rows: &[WindowRow]) {
    println!(
        "{:>6}  {:>10}  {:>8}  {:>9}  {:>9}  {:>8}  {:>7}  {:>10}  {:>10}",
        "window",
        "end_tick",
        "events",
        "new_types",
        "cum_types",
        "coverage",
        "alive",
        "state_kb",
        "ticks/s"
    );
    for r in rows {
        println!(
            "{:>6}  {:>10}  {:>8}  {:>9}  {:>9}  {:>7.3}  {:>7}  {:>10.1}  {:>10.0}",
            r.window,
            r.end_tick,
            r.events,
            r.new_types,
            r.cumulative_types,
            r.coverage,
            r.live_agents,
            r.state_bytes as f64 / 1024.0,
            r.ticks_per_sec,
        );
    }
    let total_events: u64 = rows.iter().map(|r| r.events).sum();
    let final_cov = rows.last().map(|r| r.coverage).unwrap_or(0.0);
    let last_new: u64 = rows.last().map(|r| r.new_types).unwrap_or(0);
    println!(
        "summary: {} windows, {} total events, final coverage {:.3}, new_types in last window {}",
        rows.len(),
        total_events,
        final_cov,
        last_new
    );
}

fn write_csv(out: &Path, scenario: &str, seed: u64, rows: &[WindowRow]) -> Result<()> {
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating output dir {}", parent.display()))?;
        }
    }
    let mut f = File::create(out).with_context(|| format!("creating {}", out.display()))?;
    writeln!(
        f,
        "scenario,seed,window,end_tick,events,new_types,cumulative_types,coverage,live_agents,state_bytes,ticks_per_sec"
    )?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{:.6},{},{},{:.2}",
            scenario,
            seed,
            r.window,
            r.end_tick,
            r.events,
            r.new_types,
            r.cumulative_types,
            r.coverage,
            r.live_agents,
            r.state_bytes,
            r.ticks_per_sec,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_math_is_sane() {
        // 3 full windows of a tiny scenario: monotonic cumulative types,
        // positive throughput, end_tick alignment.
        let text =
            std::fs::read_to_string("../../scenarios/minimal.toml").expect("read minimal.toml");
        let window = 200;
        let ticks = 600;
        let rows = soak(&text, 7, ticks, window).expect("soak runs");

        assert_eq!(rows.len(), 3, "3 windows of {window} ticks");
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r.window, i as u64 + 1);
            assert_eq!(r.end_tick, (i as u64 + 1) * window);
            assert!(r.ticks_per_sec > 0.0, "throughput must be positive");
            assert!(r.state_bytes > 0, "state size proxy must be nonzero");
            assert!(r.coverage >= 0.0 && r.coverage <= 1.0);
        }
        // Cumulative distinct types is monotonic non-decreasing.
        for pair in rows.windows(2) {
            assert!(
                pair[1].cumulative_types >= pair[0].cumulative_types,
                "cumulative types must not decrease"
            );
        }
        // New types summed across windows equals the final cumulative count.
        let summed_new: u64 = rows.iter().map(|r| r.new_types).sum();
        assert_eq!(summed_new, rows.last().unwrap().cumulative_types);
    }

    #[test]
    fn trailing_partial_window_is_reported() {
        let text =
            std::fs::read_to_string("../../scenarios/minimal.toml").expect("read minimal.toml");
        // 500 ticks in 200-tick windows -> 200, 200, 100.
        let rows = soak(&text, 1, 500, 200).expect("soak runs");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2].end_tick, 500);
    }
}
