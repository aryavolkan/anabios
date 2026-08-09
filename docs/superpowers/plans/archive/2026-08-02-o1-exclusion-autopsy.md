# O1 — Competitive-Exclusion Autopsy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> This plan has **two halves**. Tasks 1–3 are TDD **code** tasks that build a
> reusable, headless-only diagnostic instrument (no core-sim change, no golden
> movement). Tasks 4–7 are an **experiment protocol** that runs the instrument to
> produce the O1 findings document. Protocol steps are measurements and decision
> gates, not code — record numbers, don't invent them.

**Goal:** Quantify *why* culture loses in the Out-of-Africa world — a per-strategy fitness ledger plus bidirectional invasion analysis — and name the single dominant lever, confirmed by one targeted intervention.

**Architecture:** A headless-only instrument reads per-strategy aggregates from a `&World` each window (zero sim impact, like `soak.rs`). "Strategy" is keyed on **Communicator-module presence** (cultural-capable vs asocial), which is heritable and robust to species reclustering. A pure invasion-fitness function computes a rare strategy's per-capita growth rate. A new `autopsy` subcommand wires them together over a scenario. The experiment protocol then measures the baseline, runs two dedicated invasion scenarios, scans one-variable scenario levers, and writes the findings.

**Tech Stack:** Rust (workspace crate `anabios-headless`), `clap` subcommands, `rayon` (already wired), `anabios-core` read-only API.

## Global Constraints

- **Determinism gate stays green.** O1 is diagnosis only: **no `anabios-core` behavior change, no golden-hash movement.** The instrument reads `&World`; it never mutates sim state. If any protocol step tempts you to change a core constant, STOP — that belongs in O2/O3 behind a scenario flag, not here.
- **Opt-in / baseline-stable.** New scenarios are new files under `scenarios/`; never edit a shipped scenario's counts/flags. Shipped goldens must not move.
- **Evidence discipline.** Exclusion is confirmed only by **bidirectional** invasion (cultural cannot invade asocial-dominated world AND asocial can re-invade cultural world). A one-directional growth curve is not proof. Record negative results honestly.
- **Heavy runs on the release binary.** Build `--release`; let sweeps parallelize over rayon. Pilot at reduced seeds/ticks before a full run. Do not run the full determinism/emergence suite locally each commit — fast checks locally, heavy suite in PR CI.
- **Exact per-agent reads (use verbatim, these are verified against the codebase):**
  - alive iteration: `world.agents.iter_alive()` yields `AgentId` (`u32`)
  - species: `world.agents.species_id[i as usize]`
  - energy: `world.agents.energy[i as usize]` (`f32`)
  - IQ: `world.agents.iq[i as usize]` (`f32`; `0.0` when `cognition_enabled` off)
  - cultural skill: `world.agents.meme_vector[i as usize][anabios_core::culture::SKILL_CHANNEL]` (`f32` in `[0,1]`)
  - tech era: `anabios_core::invention::tech_era(anabios_core::invention::held_mask(&world.agents.meme_vector[i as usize]))` → `u8`
  - cultural-capable test: `anabios_core::module::has(&world.agents.modules[i as usize], anabios_core::module::ModuleType::Communicator)` → `bool`

---

## Task 1: Strategy sampler (headless instrument, read-only)

Build the per-strategy aggregate read over a `&World`. Strategy = Cultural (has Communicator module) vs Asocial (does not).

**Files:**
- Create: `crates/anabios-headless/src/ledger.rs`
- Modify: `crates/anabios-headless/src/main.rs` (add `mod ledger;` next to the other `mod` lines near the top, e.g. after `mod demo;`)
- Test: inline `#[cfg(test)]` module in `crates/anabios-headless/src/ledger.rs`

**Interfaces:**
- Consumes: `anabios_core::world::World`, `anabios_core::scenario::Scenario`, the verified per-agent reads in Global Constraints.
- Produces:
  - `pub enum StrategyKind { Cultural, Asocial }` (derives `Clone, Copy, PartialEq, Eq, Debug`)
  - `pub const STRATEGY_KINDS: [StrategyKind; 2] = [StrategyKind::Cultural, StrategyKind::Asocial];`
  - `pub fn strategy_label(k: StrategyKind) -> &'static str` (`"cultural"` / `"asocial"`)
  - `pub struct StrategyStat { pub kind: StrategyKind, pub count: u32, pub mean_energy: f64, pub mean_skill: f64, pub mean_iq: f64, pub mean_era: f64, pub max_era: u8 }` (derives `Clone, Copy, Debug`)
  - `pub fn sample_strategies(world: &World) -> [StrategyStat; 2]` — index 0 = Cultural, index 1 = Asocial. Means are `0.0` when `count == 0`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::scenario::Scenario;

    // 3 asocial foragers + 2 communicators (culture-capable). At tick 0 the
    // sampler must bucket them purely by Communicator-module presence.
    const MIX: &str = "\
name = \"t\"
seed = 1
[[agents]]
count = 3
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
[[agents]]
count = 2
archetype = \"communicator\"
placement = { kind = \"uniform\" }
";

    #[test]
    fn sample_buckets_by_communicator_module() {
        let world = Scenario::parse_toml(MIX).unwrap().instantiate();
        let stats = sample_strategies(&world);
        assert_eq!(stats[0].kind, StrategyKind::Cultural);
        assert_eq!(stats[1].kind, StrategyKind::Asocial);
        assert_eq!(stats[0].count, 2, "two communicator agents are Cultural");
        assert_eq!(stats[1].count, 3, "three asocial foragers are Asocial");
    }

    #[test]
    fn empty_strategy_has_zero_means_not_nan() {
        // All-asocial world: the Cultural bucket is empty and must read 0.0,
        // never NaN (a NaN would poison the CSV and the invasion math).
        let only_asocial = "\
name = \"t\"
seed = 1
[[agents]]
count = 4
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
";
        let world = Scenario::parse_toml(only_asocial).unwrap().instantiate();
        let stats = sample_strategies(&world);
        assert_eq!(stats[0].count, 0);
        assert_eq!(stats[0].mean_energy, 0.0);
        assert_eq!(stats[0].mean_skill, 0.0);
        assert_eq!(stats[0].max_era, 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-headless ledger:: -- --nocapture`
Expected: FAIL to compile (`sample_strategies`, `StrategyKind` not found).

- [ ] **Step 3: Write minimal implementation**

```rust
//! O1 diagnostic instrument: per-strategy aggregates read from a `&World`.
//!
//! Headless-only and read-only — never mutates sim state, so it adds zero
//! determinism/golden risk (mirrors `soak.rs`'s telemetry stance). "Strategy"
//! is keyed on Communicator-module presence: culture-capable cognition vs the
//! asocial baseline. That key is heritable and survives species reclustering,
//! unlike `species_id`.

use anabios_core::culture::SKILL_CHANNEL;
use anabios_core::invention::{held_mask, tech_era};
use anabios_core::module::{self, ModuleType};
use anabios_core::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StrategyKind {
    Cultural,
    Asocial,
}

pub const STRATEGY_KINDS: [StrategyKind; 2] = [StrategyKind::Cultural, StrategyKind::Asocial];

pub fn strategy_label(k: StrategyKind) -> &'static str {
    match k {
        StrategyKind::Cultural => "cultural",
        StrategyKind::Asocial => "asocial",
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StrategyStat {
    pub kind: StrategyKind,
    pub count: u32,
    pub mean_energy: f64,
    pub mean_skill: f64,
    pub mean_iq: f64,
    pub mean_era: f64,
    pub max_era: u8,
}

impl StrategyStat {
    fn empty(kind: StrategyKind) -> Self {
        StrategyStat {
            kind,
            count: 0,
            mean_energy: 0.0,
            mean_skill: 0.0,
            mean_iq: 0.0,
            mean_era: 0.0,
            max_era: 0,
        }
    }
}

// Running accumulator, folded into a StrategyStat at the end.
#[derive(Default)]
struct Acc {
    count: u64,
    energy: f64,
    skill: f64,
    iq: f64,
    era_sum: f64,
    max_era: u8,
}

impl Acc {
    fn finish(self, kind: StrategyKind) -> StrategyStat {
        if self.count == 0 {
            return StrategyStat::empty(kind);
        }
        let n = self.count as f64;
        StrategyStat {
            kind,
            count: self.count as u32,
            mean_energy: self.energy / n,
            mean_skill: self.skill / n,
            mean_iq: self.iq / n,
            mean_era: self.era_sum / n,
            max_era: self.max_era,
        }
    }
}

/// Per-strategy aggregate over the currently-alive agents. Index 0 = Cultural,
/// index 1 = Asocial. Read-only; safe to call every window during a run.
pub fn sample_strategies(world: &World) -> [StrategyStat; 2] {
    let mut cultural = Acc::default();
    let mut asocial = Acc::default();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let is_cultural = module::has(&world.agents.modules[i], ModuleType::Communicator);
        let acc = if is_cultural { &mut cultural } else { &mut asocial };
        acc.count += 1;
        acc.energy += world.agents.energy[i] as f64;
        acc.skill += world.agents.meme_vector[i][SKILL_CHANNEL] as f64;
        acc.iq += world.agents.iq[i] as f64;
        let era = tech_era(held_mask(&world.agents.meme_vector[i]));
        acc.era_sum += era as f64;
        acc.max_era = acc.max_era.max(era);
    }
    [cultural.finish(StrategyKind::Cultural), asocial.finish(StrategyKind::Asocial)]
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-headless ledger:: -- --nocapture`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-headless/src/ledger.rs crates/anabios-headless/src/main.rs
git commit -m "feat(o1): per-strategy sampler (cultural vs asocial), read-only" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Invasion-fitness metric (pure function)

Compute a rare strategy's per-capita growth rate while it is at low frequency — the formal invasion criterion.

**Files:**
- Modify: `crates/anabios-headless/src/ledger.rs` (append)
- Test: inline `#[cfg(test)]` in the same file

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub struct InvasionWindow { pub tick: u64, pub mutant_n: u32, pub total_n: u32 }` (derives `Clone, Copy, Debug`)
  - `pub fn invasion_fitness(windows: &[InvasionWindow], rare_frac_max: f64) -> Option<f64>` — mean of `ln(mutant_n[k+1] / mutant_n[k])` over consecutive window pairs where window `k`'s frequency `mutant_n/total_n <= rare_frac_max` and both counts `> 0`. `None` if no qualifying pair (mutant never present while rare, or went straight to extinction). Positive ⇒ invades; ≤ 0 ⇒ excluded.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod invasion_tests {
    use super::*;

    fn w(tick: u64, mutant_n: u32, total_n: u32) -> InvasionWindow {
        InvasionWindow { tick, mutant_n, total_n }
    }

    #[test]
    fn rare_mutant_that_grows_has_positive_invasion_fitness() {
        // Mutant stays under 10% of the population but its count climbs.
        let windows = [w(0, 10, 1000), w(100, 20, 1000), w(200, 40, 1000)];
        let r = invasion_fitness(&windows, 0.10).unwrap();
        assert!(r > 0.0, "growing rare mutant must invade, got {r}");
    }

    #[test]
    fn rare_mutant_that_shrinks_is_excluded() {
        let windows = [w(0, 40, 1000), w(100, 20, 1000), w(200, 10, 1000)];
        let r = invasion_fitness(&windows, 0.10).unwrap();
        assert!(r < 0.0, "shrinking rare mutant is excluded, got {r}");
    }

    #[test]
    fn windows_above_rare_threshold_are_ignored() {
        // Every window is >10% frequency → no qualifying rare pair → None.
        let windows = [w(0, 500, 1000), w(100, 600, 1000)];
        assert!(invasion_fitness(&windows, 0.10).is_none());
    }

    #[test]
    fn extinction_pair_is_skipped_not_neg_infinity() {
        // mutant_n[k+1] == 0 would make ln(0) = -inf; that pair must be skipped.
        // The only surviving valid pair here is (10 -> 20): positive.
        let windows = [w(0, 10, 1000), w(100, 20, 1000), w(200, 0, 1000)];
        let r = invasion_fitness(&windows, 0.10).unwrap();
        assert!(r.is_finite() && r > 0.0, "got {r}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-headless ledger::invasion_tests -- --nocapture`
Expected: FAIL to compile (`invasion_fitness`, `InvasionWindow` not found).

- [ ] **Step 3: Write minimal implementation**

```rust
/// One measurement window for invasion analysis: the mutant strategy's count
/// and the total live population at `tick`.
#[derive(Clone, Copy, Debug)]
pub struct InvasionWindow {
    pub tick: u64,
    pub mutant_n: u32,
    pub total_n: u32,
}

/// Invasion fitness: the mutant's mean per-window log-growth rate while rare.
///
/// Averages `ln(n[k+1]/n[k])` over consecutive window pairs where window `k` is
/// below `rare_frac_max` frequency and both counts are positive. Pairs touching
/// a zero count are skipped (an `ln(0)` would be `-inf`). Returns `None` when no
/// qualifying rare pair exists. Positive ⇒ the rare strategy can invade.
pub fn invasion_fitness(windows: &[InvasionWindow], rare_frac_max: f64) -> Option<f64> {
    let mut sum = 0.0_f64;
    let mut pairs = 0_u64;
    for pair in windows.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a.total_n == 0 {
            continue;
        }
        let freq = a.mutant_n as f64 / a.total_n as f64;
        if freq > rare_frac_max {
            continue;
        }
        if a.mutant_n == 0 || b.mutant_n == 0 {
            continue;
        }
        sum += (b.mutant_n as f64 / a.mutant_n as f64).ln();
        pairs += 1;
    }
    if pairs == 0 {
        None
    } else {
        Some(sum / pairs as f64)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-headless ledger::invasion_tests -- --nocapture`
Expected: PASS (all four).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-headless/src/ledger.rs
git commit -m "feat(o1): invasion-fitness metric (rare-strategy log-growth rate)" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `autopsy` subcommand (ledger CSV + invasion readout)

Run a scenario, sample both strategies every window into a CSV, and print the invasion fitness of a chosen mutant strategy.

**Files:**
- Create: `crates/anabios-headless/src/autopsy.rs`
- Modify: `crates/anabios-headless/src/main.rs` (add `mod autopsy;`, a `Command::Autopsy { … }` variant, and its match arm)
- Test: inline `#[cfg(test)]` in `crates/anabios-headless/src/autopsy.rs`

**Interfaces:**
- Consumes: `ledger::{sample_strategies, strategy_label, invasion_fitness, InvasionWindow, StrategyKind}`, `anabios_core::scenario::Scenario`, `anabios_core::tick::step`.
- Produces:
  - `pub fn run(scenario: PathBuf, seed: Option<u64>, ticks: u64, window: u64, out: PathBuf, mutant: StrategyKind) -> anyhow::Result<()>`
  - CSV at `out` with header `tick,strategy,count,freq,mean_energy,mean_skill,mean_iq,mean_era,max_era` and one row per strategy per window.
  - stdout line: `invasion_fitness mutant=<label> r=<value|none>` plus a one-line verdict (`INVADES` / `EXCLUDED` / `no rare-phase data`).

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::scenario::Scenario;
    use std::io::Write;

    // Minimal mixed world; a couple hundred ticks is enough to emit >1 window.
    const MIX: &str = "\
name = \"t\"
seed = 7
[[agents]]
count = 30
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
[[agents]]
count = 8
archetype = \"cultural_forager\"
placement = { kind = \"uniform\" }
";

    #[test]
    fn autopsy_writes_ledger_rows_for_both_strategies() {
        let dir = std::env::temp_dir().join("anabios_o1_autopsy_test");
        std::fs::create_dir_all(&dir).unwrap();
        let scen = dir.join("mix.toml");
        std::fs::File::create(&scen).unwrap().write_all(MIX.as_bytes()).unwrap();
        let csv = dir.join("ledger.csv");

        run(scen, Some(7), 200, 50, csv.clone(), StrategyKind::Cultural).unwrap();

        let body = std::fs::read_to_string(&csv).unwrap();
        let mut lines = body.lines();
        assert_eq!(
            lines.next().unwrap(),
            "tick,strategy,count,freq,mean_energy,mean_skill,mean_iq,mean_era,max_era"
        );
        assert!(body.contains(",cultural,"), "ledger has a cultural row");
        assert!(body.contains(",asocial,"), "ledger has an asocial row");
        // 200 ticks / 50-tick window = at least 4 sampled windows × 2 strategies.
        assert!(lines.count() >= 8, "expected >=8 data rows");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-headless autopsy:: -- --nocapture`
Expected: FAIL to compile (`run` not found / `Command::Autopsy` missing).

- [ ] **Step 3: Write minimal implementation**

Create `crates/anabios-headless/src/autopsy.rs`:

```rust
//! O1 `autopsy` subcommand: run a scenario, log per-strategy aggregates each
//! window, and report the invasion fitness of a chosen mutant strategy.
//!
//! Diagnosis only — reads the world, never changes the sim.

use std::io::Write;
use std::path::PathBuf;

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anyhow::{Context, Result};

use crate::ledger::{
    invasion_fitness, sample_strategies, strategy_label, InvasionWindow, StrategyKind,
};

/// Frequency below which a strategy counts as "rare" for invasion analysis.
const RARE_FRAC_MAX: f64 = 0.10;

pub fn run(
    scenario_path: PathBuf,
    seed: Option<u64>,
    ticks: u64,
    window: u64,
    out: PathBuf,
    mutant: StrategyKind,
) -> Result<()> {
    let window = window.max(1);
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }
    let mut world = scenario.instantiate();

    let mut csv = std::fs::File::create(&out)
        .with_context(|| format!("creating ledger {}", out.display()))?;
    writeln!(csv, "tick,strategy,count,freq,mean_energy,mean_skill,mean_iq,mean_era,max_era")?;

    let mut invasion: Vec<InvasionWindow> = Vec::new();

    for t in 0..ticks {
        step(&mut world);
        if (t + 1) % window == 0 {
            let stats = sample_strategies(&world);
            let total: u32 = stats.iter().map(|s| s.count).sum();
            for s in &stats {
                let freq = if total == 0 { 0.0 } else { s.count as f64 / total as f64 };
                writeln!(
                    csv,
                    "{},{},{},{:.4},{:.3},{:.4},{:.4},{:.3},{}",
                    world.tick,
                    strategy_label(s.kind),
                    s.count,
                    freq,
                    s.mean_energy,
                    s.mean_skill,
                    s.mean_iq,
                    s.mean_era,
                    s.max_era,
                )?;
                if s.kind == mutant {
                    invasion.push(InvasionWindow {
                        tick: world.tick,
                        mutant_n: s.count,
                        total_n: total,
                    });
                }
            }
        }
    }
    csv.flush().with_context(|| format!("flushing {}", out.display()))?;

    match invasion_fitness(&invasion, RARE_FRAC_MAX) {
        Some(r) => {
            let verdict = if r > 0.0 { "INVADES" } else { "EXCLUDED" };
            println!("invasion_fitness mutant={} r={:.5} {}", strategy_label(mutant), r, verdict);
        }
        None => {
            println!(
                "invasion_fitness mutant={} r=none (no rare-phase data)",
                strategy_label(mutant)
            );
        }
    }
    println!("ledger written: {}", out.display());
    Ok(())
}
```

Then wire it into `crates/anabios-headless/src/main.rs`:

1. Add `mod autopsy;` with the other module declarations.
2. Add a `use` for the strategy enum near the top: `use ledger::StrategyKind;` (add after the existing `use` lines).
3. Add a clap `ValueEnum` bridge and the subcommand variant inside `enum Command`:

```rust
/// CLI spelling of the mutant strategy for `autopsy`.
#[derive(Clone, Copy, clap::ValueEnum)]
enum MutantArg {
    Cultural,
    Asocial,
}

impl From<MutantArg> for StrategyKind {
    fn from(m: MutantArg) -> Self {
        match m {
            MutantArg::Cultural => StrategyKind::Cultural,
            MutantArg::Asocial => StrategyKind::Asocial,
        }
    }
}
```

```rust
    /// O1 diagnosis: run a scenario, log per-strategy (cultural vs asocial)
    /// aggregates each window to a CSV, and report the invasion fitness of the
    /// chosen rare strategy. Reads the world only; no sim/golden impact.
    Autopsy {
        #[arg(long)]
        scenario: PathBuf,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 20000)]
        ticks: u64,
        #[arg(long, default_value_t = 500)]
        window: u64,
        #[arg(long, default_value = "ledger.csv")]
        out: PathBuf,
        /// Which strategy is the rare mutant whose invasion fitness we report.
        #[arg(long, value_enum, default_value_t = MutantArg::Cultural)]
        mutant: MutantArg,
    },
```

4. Add the match arm in `main()`:

```rust
        Command::Autopsy { scenario, seed, ticks, window, out, mutant } => {
            autopsy::run(scenario, seed, ticks, window, out, mutant.into())
        }
```

- [ ] **Step 4: Run test + build to verify green**

Run: `cargo test -p anabios-headless autopsy:: -- --nocapture && cargo build -p anabios-headless`
Expected: test PASS; binary builds. Then smoke-run:
`cargo run -p anabios-headless -- autopsy --scenario scenarios/inventions.toml --ticks 500 --window 100 --out /tmp/o1-smoke.csv`
Expected: prints an `invasion_fitness …` line and `ledger written: …`; `/tmp/o1-smoke.csv` has the header + rows.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-headless/src/autopsy.rs crates/anabios-headless/src/main.rs
git commit -m "feat(o1): autopsy subcommand — per-strategy ledger CSV + invasion readout" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Baseline autopsy — who wins, on what margin (protocol)

Run the instrument on the real Out-of-Africa world and quantify the exclusion. **Measurement only — record the actual numbers.**

- [ ] **Step 1: Build the release binary.**
`cargo build --release -p anabios-headless`

- [ ] **Step 2: Baseline ledger run.**
`./target/release/anabios-headless autopsy --scenario scenarios/out-of-africa.toml --seed 318 --ticks 20000 --window 500 --out runs/o1/ooa-baseline-ledger.csv --mutant cultural`
Record the printed `invasion_fitness` line.

- [ ] **Step 3: Read the ledger.** From `runs/o1/ooa-baseline-ledger.csv`, extract per-window: cultural vs asocial `count`, `freq`, `mean_energy`, `mean_skill`, `mean_era`, `max_era`. Identify:
  - the **winning strategy** by end-of-run frequency,
  - the **tick and margin** at which the losing strategy's frequency starts its monotone decline (the exclusion onset),
  - whether cultural agents ever reach `mean_skill`/`mean_era` above the asocial baseline *before* being excluded (culture "works but loses" vs "never gets going").

- [ ] **Step 4: Cross-check against the IQ-ceiling hypothesis.** The existing `2026-08-01-out-of-africa-climb-experiment.md` plan blames the era-3 IQ gate. From the ledger, record `max_era` reached by *either* strategy. If `max_era` never approaches era-3, the IQ ceiling is **not** the binding constraint (it is never tested) — corroborating the exclusion hypothesis. Write this comparison down; it is a headline O1 finding.

- [ ] **Step 5: Commit the artifacts.**

```bash
git add runs/o1/ooa-baseline-ledger.csv
git commit -m "chore(o1): baseline OoA strategy ledger (seed 318, 20k ticks)" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

*(If `runs/` is gitignored, instead copy the CSV under `docs/superpowers/data/o1/` and commit there; note the location in the findings doc.)*

---

## Task 5: Bidirectional invasion analysis (protocol) — the exclusion proof

Author two dedicated invasion scenarios (resident large, mutant rare) and measure invasion fitness both directions. This is the milestone's core evidence.

- [ ] **Step 1: Author cultural-into-asocial.** Create `scenarios/o1-invasion-cultural-into-asocial.toml`: copy the header flags of `scenarios/out-of-africa.toml` (all the `*_enabled` lines + climate/economy knobs) but replace the agent list with a single dense cradle cluster: **~1000 `asocial_forager`** (resident) + **~20 `cultural_forager`** (rare mutant, ≤2% start frequency), same `center_x/center_y`, overlapping radius so they genuinely mix. Keep `max_population = 3000`. Example mutant block:

```toml
[[agents]]
count = 20
archetype = "cultural_forager"
placement = { kind = "cluster", center_x = 150.0, center_y = 440.0, radius = 90.0 }
[agents.traits]
lifespan_bias = 1.0
reproduction_threshold = 0.3
```

- [ ] **Step 2: Author asocial-into-cultural (the reverse).** Create `scenarios/o1-invasion-asocial-into-cultural.toml`: same flags, but **~1000 `cultural_forager`** resident + **~20 `asocial_forager`** rare mutant in the same mixed cluster.

- [ ] **Step 3: Run both, several seeds.** For seeds `318 1 2 3 4` (pilot with 2 seeds first if wall-clock is tight):
```
./target/release/anabios-headless autopsy --scenario scenarios/o1-invasion-cultural-into-asocial.toml --seed <s> --ticks 20000 --window 500 --out runs/o1/inv-cul-into-aso-<s>.csv --mutant cultural
./target/release/anabios-headless autopsy --scenario scenarios/o1-invasion-asocial-into-cultural.toml --seed <s> --ticks 20000 --window 500 --out runs/o1/inv-aso-into-cul-<s>.csv --mutant asocial
```
Record each run's `invasion_fitness r` value.

- [ ] **Step 4: Apply the exclusion test.** Competitive exclusion of culture is **confirmed** iff, across seeds: cultural-mutant `r ≤ 0` (cannot invade asocial world) **AND** asocial-mutant `r > 0` (can invade cultural world). Tabulate `r` per direction per seed. If the pattern is mixed or reversed, record that instead — a surprising result is a valid O1 finding and reshapes O2/O3.

- [ ] **Step 5: Commit scenarios + data.**

```bash
git add scenarios/o1-invasion-cultural-into-asocial.toml scenarios/o1-invasion-asocial-into-cultural.toml runs/o1/inv-*.csv
git commit -m "chore(o1): bidirectional invasion scenarios + results" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: One-variable lever scan (protocol)

Vary one scenario-level knob at a time on the cultural-into-asocial invasion scenario; measure how each moves the cultural mutant's invasion fitness. **One knob per variant — attributable effects only.** All knobs here are scenario-level (no core-constant edits), so no golden movement.

- [ ] **Step 1: Enumerate the lever variants.** Copy `scenarios/o1-invasion-cultural-into-asocial.toml` to one file per lever, changing exactly one thing:
  - `…-density.toml` — shrink the mutant cluster `radius` (150.0→60.0) so cultural agents pack together (more transmission partners / collective-brain density).
  - `…-ceiling.toml` — set `resources_enabled = false` (removes the era-gated material economy → tests whether asocial's resource ceiling is what culture needs to escape).
  - `…-cognition.toml` — set `cognition_enabled = false` (removes the IQ gate entirely → isolates whether the gate matters at these eras).
  - `…-mixing.toml` — raise the mutant `count` from 20→120 (still a minority, but tests the Allan-Wilson/critical-mass threshold: does culture invade only above a founding-density floor?).

- [ ] **Step 2: Run each variant** (seeds `318 1 2`, mutant `cultural`, identical ticks/window), writing `runs/o1/lever-<name>-<seed>.csv`, recording each `invasion_fitness r`.

- [ ] **Step 3: Fill the knob×margin table.** For each lever: median cultural-mutant `r` across seeds, and Δ vs the Task 5 baseline `r`. The lever with the largest positive Δ (turns `r` from ≤0 to >0, or most increases it) is the **candidate dominant lever**.

- [ ] **Step 4: Confound guard.** Confirm each variant differs from baseline in exactly one line (`git diff --no-index` the baseline scenario against each variant; it should show a single changed knob). Record the diffs.

- [ ] **Step 5: Commit.**

```bash
git add scenarios/o1-invasion-cultural-into-asocial-*.toml runs/o1/lever-*.csv
git commit -m "chore(o1): one-variable lever scan on the invasion margin" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Dominant-lever confirmation + findings write-up (protocol → deliverable)

Confirm the candidate lever with one targeted intervention and write the O1 findings document — the milestone deliverable.

- [ ] **Step 1: Targeted confirmation.** Take the candidate dominant lever from Task 6 and push it one step further (e.g. if density won, tighten radius again / raise founding count to the point predicted to flip `r`), as `scenarios/o1-confirm-<lever>.toml`. Run seeds `318 1 2`. **Pass criterion:** the intervention moves cultural-mutant `r` in the predicted direction and, ideally, flips it positive — demonstrating the lever *causally* controls the invasion margin. Record the result honestly whether or not it flips.

- [ ] **Step 2: Write the findings doc.** Create `docs/superpowers/specs/2026-08-02-o1-exclusion-findings.md` containing:
  - **Fitness ledger summary** (Task 4): winning strategy, exclusion-onset tick/margin, whether culture "works but loses," and the `max_era`-vs-IQ-ceiling cross-check.
  - **Bidirectional invasion result** (Task 5): the per-direction `r` table and the exclusion verdict (confirmed / not).
  - **IQ-ceiling vs exclusion adjudication:** state plainly which hypothesis the data support, citing the `max_era` reached vs `IQ_REQ_BY_ERA[2]=0.55`.
  - **Knob×margin table** (Task 6) and the **named dominant lever**.
  - **Confirmation** (Task 1 of this task): the targeted intervention's effect on `r`.
  - **Handoff to O2/O3:** one paragraph — which lever O2 (lifetime learning) and O3 (niche construction) should target first, given the diagnosis.

- [ ] **Step 3: Update the arc index.** In `docs/superpowers/specs/2026-08-02-open-ended-complexity-arc-design.md`, mark O1's open question #1 (IQ ceiling vs exclusion) resolved with a one-line pointer to the findings doc.

- [ ] **Step 4: Record the finding in memory.** Update the `anabios-ooa-climb` memory (or add a linked `anabios-o1-exclusion-autopsy` memory) with the named dominant lever and the bidirectional-invasion verdict, so later sessions start from the diagnosis, not the hypothesis.

- [ ] **Step 5: Commit the deliverable.**

```bash
git add docs/superpowers/specs/2026-08-02-o1-exclusion-findings.md docs/superpowers/specs/2026-08-02-open-ended-complexity-arc-design.md scenarios/o1-confirm-*.toml runs/o1/*.csv
git commit -m "docs(o1): competitive-exclusion autopsy findings + named dominant lever" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Done when

- The `autopsy` instrument (Tasks 1–3) is committed with green tests and **no golden-hash movement** (`cargo test -p anabios-core --test determinism` unchanged).
- The findings doc exists with: the fitness ledger, the bidirectional invasion table, the IQ-ceiling-vs-exclusion adjudication, the knob×margin table, a named dominant lever, and a confirming intervention.
- O1's open question in the arc spec is marked resolved and the memory updated.
- Every "culture loses because X" claim is backed by a recorded number, and the exclusion claim specifically rests on the **bidirectional** invasion result — not a one-directional curve.

## Notes / risks

- **Strategy key = Communicator module, not species_id.** Species reclustering reassigns `species_id`; the Communicator-module key is heritable and stable, which is why the ledger uses it. If a later milestone needs finer archetype resolution, add a `species_id` column then — don't retrofit it into the invasion metric.
- **Wall-clock.** 20k-tick autopsy runs at 3000 agents on `out-of-africa.toml` are heavy. Pilot at `--ticks 4000` and 2 seeds to sanity-check the pipeline before the full runs; the invasion scenarios are single dense clusters and run lighter than the full OoA map.
- **Zero-sim-impact is a hard line.** If any step seems to require editing an `anabios-core` constant to move the margin, that is a *finding* ("the dominant lever is a core constant, X"), not a task — record it and hand it to O2/O3. O1 does not change sim behavior.
- **`runs/` gitignore.** Check `.gitignore` before committing CSVs; if `runs/` is ignored, relocate artifacts under `docs/superpowers/data/o1/` and reference that path in the findings doc.
```
