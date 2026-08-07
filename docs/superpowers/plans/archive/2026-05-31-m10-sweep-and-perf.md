# M10 — Headless Sweep Tooling + Perf Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close out the original roadmap. Add a `sweep` subcommand to `anabios-headless` that runs N seeds of a scenario in parallel, writes per-run codex events as JSONL plus an aggregate CSV summary suitable for spreadsheets or W&B upload. Land one targeted perf cleanup (the long-standing M1 I2 follow-up: reuse a single `Vec<u32>` of alive ids in the tick stages instead of allocating a fresh one each tick).

**Architecture:** `anabios-headless sweep` takes `--scenario`, `--seeds N`, `--ticks N`, `--out <dir>`, `--threads N`. It runs the matrix with `rayon`, each worker calling `Scenario::instantiate` with its assigned seed, ticking, then dumping events + a stats dict. A `summary.csv` index in `--out/` lists per-seed final alive / total events / per-chapter counts / state_hash. A `runs/seed_N.events.jsonl` per seed holds the full event stream. Perf cleanup: hoist the `Vec<u32>` snapshot of alive ids into a `World` scratch field reused across decide/integrate/interact/reproduce/age stages, eliminating ~5 allocations per tick.

**Tech Stack:** Same as M9 (Rust). Adds rayon for the sweep parallelism (already a workspace dep).

**Branch:** `m10-sweep-and-perf` from `main`.

**Working directory:** `/Users/aryasen/projects/anabios/`.

**Scope note (medium effort):** Sweep CLI + per-run JSONL + CSV summary + one perf cleanup. Deferred: live W&B HTTP streaming (needs a Python wrapper or wandb-rs), parameter-grid sweeps (only seed-grid in this milestone), automatic event-rarity ranking, the unfinished M1 I3 dead-velocity-field cleanup.

---

## File structure after M10

New:
```
crates/anabios-headless/src/sweep.rs       # sweep subcommand implementation
```
Modified:
```
crates/anabios-headless/Cargo.toml         # +rayon
crates/anabios-headless/src/main.rs        # +Sweep subcommand
crates/anabios-core/src/world.rs           # +alive_ids_scratch field
crates/anabios-core/src/tick.rs            # decide_all reuses scratch
crates/anabios-core/src/integrate.rs       # integrate_all takes &mut World, reuses scratch
crates/anabios-core/src/interact.rs        # interact_all takes &mut World
crates/anabios-core/src/age.rs             # age_and_starve already takes &mut World — reuse scratch
crates/anabios-core/src/reproduce.rs       # already takes &mut World — reuse scratch
crates/anabios-core/src/module.rs          # upkeep_all reuses scratch
README.md                                  # document sweep command
```

---

## Task 0: Branch

- [ ] `git checkout main && git pull && git checkout -b m10-sweep-and-perf`
- [ ] `cargo test --workspace --lib 2>&1 | tail -3` — baseline green.

---

## Task 1: Hoist alive-ids scratch onto World

**Goal:** Add `pub alive_ids_scratch: Vec<u32>` (skip-serialized) to `World`. Refactor the 5 tick stages that currently do `let alive_ids: Vec<u32> = agents.iter_alive().collect()` to use the scratch buffer.

**Files:** `crates/anabios-core/src/world.rs`, `tick.rs`, `integrate.rs`, `interact.rs`, `reproduce.rs`, `age.rs`, `module.rs`.

- [ ] **Step 1.1: Add scratch field**

In `World`:

```rust
    #[serde(skip)]
    pub alive_ids_scratch: Vec<u32>,
```

Initialize `Vec::new()` in `World::new`. Add a helper:

```rust
    /// Fill the scratch buffer with current alive agent ids in ascending order.
    /// Caller can then iterate `&self.alive_ids_scratch` while mutating other
    /// agent fields (the borrow split is clean because alive_ids_scratch is a
    /// separate Vec).
    pub fn snapshot_alive_ids(&mut self) {
        self.alive_ids_scratch.clear();
        self.alive_ids_scratch.extend(self.agents.iter_alive());
    }
```

- [ ] **Step 1.2: Refactor `integrate_all`**

It currently takes `&mut AgentBuffers` and snapshots its own Vec. Change signature to `&mut World`, snapshot once via `world.snapshot_alive_ids()`, iterate `&world.alive_ids_scratch`. Update the call site in `tick.rs`.

- [ ] **Step 1.3: Refactor `interact_all`**

Same pattern: `&mut World`, reuse scratch.

- [ ] **Step 1.4: Refactor `module::upkeep_all`**

Same: `&mut World`, reuse scratch.

- [ ] **Step 1.5: Refactor `age_and_starve` + `reproduce_all` + `tick::decide_all`**

These already take `&mut World`. Replace their inline `let alive_ids: Vec<u32> = ...` with `world.snapshot_alive_ids(); for id in &world.alive_ids_scratch { ... }`. **Important**: when one stage calls another stage that ALSO snapshots, the scratch is overwritten — but since each stage runs to completion before the next, this is fine. The only caveat is `reproduce_all` calls `agents.spawn` which extends buffers but the snapshot was taken before spawn, so we still iterate only original agents this tick (intended).

- [ ] **Step 1.6: Update existing unit tests + agent.rs tests**

Any test calling `integrate_all`/`interact_all`/`upkeep_all` directly with `&mut agents` instead of `&mut world` needs updating. Walk the test failures and update each. (Most are accessed via `World` already.)

- [ ] **Step 1.7: Run lib tests + golden + commit**

```bash
cargo test -p anabios-core --lib
cargo test -p anabios-core --tests
```

If the golden hashes drift (they shouldn't — same RNG order, same logic), regenerate via UPDATE_HASHES=1 and verify the cause is just scratch ordering, then commit.

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "perf(core): hoist alive_ids snapshot onto World; eliminate per-tick Vec allocs"
```

---

## Task 2: Bench perf check

- [ ] **Step 2.1:** `cargo bench -p anabios-core --bench tick_bench` — compare to M7/M8 baselines (~1.9 ms/1k, ~12 ms/10k). Should hold or improve slightly.
- [ ] **Step 2.2:** Commit only if a follow-up tuning landed.

---

## Task 3: Sweep CLI implementation

**Goal:** Add a `sweep` subcommand to `anabios-headless`.

**Files:**
- Modify: `crates/anabios-headless/Cargo.toml` (+ rayon)
- Create: `crates/anabios-headless/src/sweep.rs`
- Modify: `crates/anabios-headless/src/main.rs` (+ Sweep subcommand, route to `sweep::run`)

- [ ] **Step 3.1: Add rayon dep**

```toml
rayon = { workspace = true }
```

- [ ] **Step 3.2: Implement sweep.rs**

```rust
//! Sweep multiple seeds of a scenario in parallel, writing per-run codex
//! event JSONL files plus an aggregate CSV summary.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use anabios_core::codex::{drain_events_drain_stub_unused as _, EventType};
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
struct RunSummary {
    seed: u64,
    ticks: u64,
    final_alive: u32,
    final_biomass: f32,
    state_hash: u64,
    counts: BTreeMap<&'static str, u64>,
}

pub fn run(scenario_path: PathBuf, seeds: u64, ticks: u64, out_dir: PathBuf, threads: Option<usize>) -> Result<()> {
    if let Some(n) = threads {
        rayon::ThreadPoolBuilder::new().num_threads(n).build_global()
            .context("failed to configure rayon thread pool")?;
    }
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;

    let summaries: Vec<RunSummary> = (0..seeds)
        .into_par_iter()
        .map(|seed| run_one(&text, seed, ticks, &out_dir))
        .collect::<Result<Vec<_>>>()?;

    write_summary_csv(&out_dir, &summaries)?;
    println!(
        "sweep complete: {} runs × {} ticks → {}",
        seeds, ticks, out_dir.display()
    );
    Ok(())
}

fn run_one(scenario_text: &str, seed: u64, ticks: u64, out_dir: &Path) -> Result<RunSummary> {
    let mut scenario = Scenario::parse_toml(scenario_text)?;
    scenario.seed = seed;
    let mut world = scenario.instantiate();

    let events_path = out_dir.join(format!("seed_{seed:08}.events.jsonl"));
    let mut f = File::create(&events_path)
        .with_context(|| format!("creating {}", events_path.display()))?;

    let mut counts: BTreeMap<&'static str, u64> = BTreeMap::new();
    for _ in 0..ticks {
        step(&mut world);
        for ev in world.codex.drain_events() {
            let name = event_name(ev.event_type);
            *counts.entry(name).or_insert(0) += 1;
            serde_json::to_writer(&mut f, &ev)?;
            f.write_all(b"\n")?;
        }
    }

    Ok(RunSummary {
        seed,
        ticks,
        final_alive: world.agents.live_count(),
        final_biomass: world.plant_biomass_total(),
        state_hash: state_hash(&world),
        counts,
    })
}

fn event_name(t: EventType) -> &'static str {
    match t {
        EventType::Extinction => "extinction",
        EventType::PopulationCrash => "pop_crash",
        EventType::SpeciationEvent => "speciation",
        EventType::Migration => "migration",
        EventType::NovelModuleAppeared => "novel_module",
        EventType::NovelBehaviorPattern => "novel_behavior",
    }
}

fn write_summary_csv(out_dir: &Path, runs: &[RunSummary]) -> Result<()> {
    let path = out_dir.join("summary.csv");
    let mut f = File::create(&path)
        .with_context(|| format!("creating {}", path.display()))?;
    writeln!(
        f,
        "seed,ticks,final_alive,final_biomass,state_hash,extinction,pop_crash,speciation,migration,novel_module,novel_behavior"
    )?;
    for r in runs {
        let g = |k: &str| r.counts.get(k).copied().unwrap_or(0);
        writeln!(
            f,
            "{},{},{},{:.1},0x{:016x},{},{},{},{},{},{}",
            r.seed, r.ticks, r.final_alive, r.final_biomass, r.state_hash,
            g("extinction"), g("pop_crash"), g("speciation"),
            g("migration"), g("novel_module"), g("novel_behavior"),
        )?;
    }
    Ok(())
}

// Suppress unused-import lint for the EventType import in case the codex
// re-exports rename.
#[allow(dead_code)]
mod _unused {
    use anabios_core::codex::EventType as _;
}
```

(Remove the `drain_events_drain_stub_unused` and `_unused` helpers — they were placeholders. The real imports needed are just `EventType`, `Scenario`, `state_hash`, `step`. If the implementer hits a "no such item `drain_events_drain_stub_unused`" error, just remove that line and the `_unused` module.)

- [ ] **Step 3.3: Wire into main.rs**

Extend the `Command` enum with:

```rust
    /// Sweep N seeds of a scenario in parallel; write per-run events + CSV summary.
    Sweep {
        #[arg(long)]
        scenario: PathBuf,
        #[arg(long, default_value_t = 16)]
        seeds: u64,
        #[arg(long, default_value_t = 2000)]
        ticks: u64,
        #[arg(long)]
        out: PathBuf,
        /// Override the rayon thread pool size; defaults to logical CPUs.
        #[arg(long)]
        threads: Option<usize>,
    },
```

Add the matching match arm in `main`:

```rust
        Command::Sweep { scenario, seeds, ticks, out, threads } => sweep::run(scenario, seeds, ticks, out, threads),
```

Add `mod sweep;` at the top of `main.rs`.

- [ ] **Step 3.4: Smoke test the sweep**

```bash
cargo build --release --bin anabios-headless
./target/release/anabios-headless sweep --scenario scenarios/minimal.toml --seeds 4 --ticks 500 --out /tmp/anabios_sweep
ls /tmp/anabios_sweep/
cat /tmp/anabios_sweep/summary.csv
```

Expected: 4 JSONL files + one CSV with 4 data rows.

- [ ] **Step 3.5: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-headless/ README.md
git commit -m "feat(headless): sweep subcommand — parallel per-seed runs with JSONL + CSV summary"
```

- [ ] **Step 3.6: Document in README**

Append a "Running a sweep" section showing the example command from Step 3.4.

---

## Task 4: Final + tag

- [ ] **Step 4.1:** `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
- [ ] **Step 4.2:** Determinism smoke (release):

```bash
cargo build --release --bin anabios-headless
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/m10_a.txt
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/m10_b.txt
diff /tmp/m10_a.txt /tmp/m10_b.txt && echo deterministic
```

- [ ] **Step 4.3:** Tag:

```bash
git tag -a m10 -m "M10: sweep CLI + perf hoist (alive_ids scratch)"
```

---

## Post-implementation expectations

- `anabios-headless sweep --scenario X --seeds N --ticks N --out DIR` runs N parallel sims and writes per-seed event JSONL + summary CSV
- Per-tick cost drops slightly (no per-tick `Vec<u32>` allocations across 5 stages)
- Determinism preserved
- README documents the sweep workflow

Deferred (genuinely out of original roadmap):
- Live W&B streaming (needs Python wrapper or wandb-rs crate)
- Parameter-grid sweeps (not just seed grid — e.g., sweep over `MUTATION_RATE`)
- Event-rarity ranking + automatic "interesting world" selection
- Snapshot replay system
- Cross-platform Godot exports + notarization
