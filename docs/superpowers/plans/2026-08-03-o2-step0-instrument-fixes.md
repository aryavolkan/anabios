# O2 Step 0 — Instrument Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the two O1-flagged instrument gaps so O2a's measurement and O2b's evolution claim are honest — a share-relative `invasion_fitness` variant (robust to non-stationary population) and a lineage-locked founder tag (robust to per-birth module mutation).

**Architecture:** Both fixes are **headless-only** additions to the `anabios-headless` `autopsy` tooling — no `anabios-core` change, so **zero determinism/golden impact**. The share metric is a pure function alongside the existing one. The lineage-locked tag is a headless-side `FounderTracker` (a `HashMap<LineageId, StrategyKind>`) seeded at t0 from initial strategy and inherited through `parent_ids` — reconstructing lineage descent by observing the sim, without adding a sim field.

**Tech Stack:** Rust (`anabios-headless`), clap, `anabios-core` read-only API.

## Global Constraints

- **Diagnosis-only / headless-only.** No `anabios-core` source changes. No `FORMAT_VERSION` bump, no golden rehash. The instrument reads `&World`; it never mutates sim state.
- **Additive, back-compatible.** The existing `invasion_fitness` and `sample_strategies` stay and keep their current behavior/signatures; new capability is added alongside. The `autopsy` default output must remain a superset (existing lines still printed).
- **Exact per-agent reads (verified against current `main`):**
  - alive iteration: `world.agents.iter_alive()` → `AgentId` (`u32`)
  - module presence: `anabios_core::module::has(&world.agents.modules[i as usize], anabios_core::module::ModuleType::Communicator)` → `bool`
  - lineage: `world.agents.lineage_id[i]` → `LineageId` (`u64`); `world.agents.parent_ids[i][0]` → mother's `LineageId`; founder parents are `anabios_core::agent::LINEAGE_NONE` (`0`)
  - `LineageId` / `LINEAGE_NONE` are re-exported at `anabios_core::{LineageId, LINEAGE_NONE}`.
- **Existing types to reuse (in `crates/anabios-headless/src/ledger.rs`):** `StrategyKind` (`Cultural`/`Asocial`, derives `Clone,Copy,PartialEq,Eq,Debug`), `StrategyStat`, `InvasionWindow { mutant_n: u32, total_n: u32 }`, `invasion_fitness(&[InvasionWindow], f64) -> Option<f64>`, `sample_strategies(&World) -> [StrategyStat; 2]` (index 0 = Cultural, 1 = Asocial), `strategy_label`.

---

## Task 1: Share-relative invasion fitness (pure function)

Add a frequency/share-based invasion metric next to the absolute-count one. This is the fix for O1's "absolute-count `invasion_fitness` is confounded by non-stationary population."

**Files:**
- Modify: `crates/anabios-headless/src/ledger.rs` (append after `invasion_fitness`)
- Test: inline `#[cfg(test)]` in the same file (the existing `invasion_tests` module)

**Interfaces:**
- Consumes: existing `InvasionWindow { mutant_n, total_n }`.
- Produces: `pub fn invasion_fitness_share(windows: &[InvasionWindow], rare_frac_max: f64) -> Option<f64>` — mean of `ln(freq[k+1] / freq[k])` over consecutive window pairs where window `k`'s frequency `mutant_n/total_n <= rare_frac_max` and both windows have positive `total_n` and positive `mutant_n`. `None` if no qualifying pair. Positive ⇒ the rare strategy gains *share* while rare (invades), independent of whether total population is growing or collapsing.

- [ ] **Step 1: Write the failing test**

Add to the existing `invasion_tests` module in `ledger.rs`:

```rust
#[test]
fn share_metric_ignores_global_population_change() {
    // Population collapses (mutant AND total both shrink each window) but the
    // mutant's SHARE stays flat at 0.05. Absolute invasion fitness reads
    // negative (raw count falling); share fitness must read ~0 (no share change).
    // This is the exact confound O1 flagged.
    let windows =
        [w(5, 100), w(2, 40), w(1, 20)]; // shares 0.05, 0.05, 0.05; counts 5→2→1
    let abs = invasion_fitness(&windows, 0.10).unwrap();
    let share = invasion_fitness_share(&windows, 0.10).unwrap();
    assert!(abs < 0.0, "absolute metric is confounded by the collapse: {abs}");
    assert!(share.abs() < 1e-9, "share metric cancels the collapse: {share}");
}

#[test]
fn share_metric_positive_when_share_grows() {
    // Mutant share rises 0.02 → 0.04 → 0.06 while rare.
    let windows = [w(2, 100), w(4, 100), w(6, 100)];
    let r = invasion_fitness_share(&windows, 0.10).unwrap();
    assert!(r > 0.0, "growing share must be positive, got {r}");
}

#[test]
fn share_metric_none_when_never_rare() {
    // Every window above the rare threshold → no qualifying pair.
    let windows = [w(500, 1000), w(600, 1000)];
    assert!(invasion_fitness_share(&windows, 0.10).is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-headless ledger::invasion_tests -- --nocapture`
Expected: FAIL to compile (`invasion_fitness_share` not found).

- [ ] **Step 3: Write minimal implementation**

Append to `ledger.rs` after `invasion_fitness`:

```rust
/// Share-relative invasion fitness: the mutant's mean per-window log-growth of
/// *frequency* (share) while rare. Unlike [`invasion_fitness`] (raw count),
/// this cancels global population change — a collapsing or booming world does
/// not bias it, because both mutant and total move together in the ratio.
///
/// Averages `ln(freq[k+1]/freq[k])` over consecutive pairs where window `k`'s
/// frequency is `<= rare_frac_max` and both windows have positive total and
/// mutant counts. Returns `None` when no qualifying rare pair exists. Positive
/// ⇒ the rare strategy gains share (invades).
pub fn invasion_fitness_share(windows: &[InvasionWindow], rare_frac_max: f64) -> Option<f64> {
    let mut sum = 0.0_f64;
    let mut pairs = 0_u64;
    for pair in windows.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a.total_n == 0 || b.total_n == 0 {
            continue;
        }
        let fa = a.mutant_n as f64 / a.total_n as f64;
        if fa > rare_frac_max {
            continue;
        }
        if a.mutant_n == 0 || b.mutant_n == 0 {
            continue;
        }
        let fb = b.mutant_n as f64 / b.total_n as f64;
        sum += (fb / fa).ln();
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
Expected: PASS (all invasion_tests, including the three new ones).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-headless/src/ledger.rs
git commit -m "feat(o2-step0): share-relative invasion_fitness (population-change robust)" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Lineage-locked founder tag

Add a headless-side `FounderTracker` that tags each lineage by the strategy it *descends from* (fixed at t0, inherited through births), and a `sample_by_founder` that buckets by it. This is the fix for O1's "the module-presence strategy tag is not lineage-locked."

**Files:**
- Modify: `crates/anabios-headless/src/ledger.rs` (extract a classifier-parameterized sampler)
- Create: `crates/anabios-headless/src/founder.rs`
- Modify: `crates/anabios-headless/src/main.rs` (add `mod founder;` beside the other module declarations)
- Test: inline `#[cfg(test)]` in `founder.rs`

**Interfaces:**
- Consumes: `ledger::{StrategyKind, StrategyStat, sample_strategies_by}`, the verified lineage reads.
- Produces (in `ledger.rs`):
  - `pub fn sample_strategies_by<F: Fn(&World, usize) -> StrategyKind>(world: &World, classify: F) -> [StrategyStat; 2]` — the existing bucketing, but the Cultural/Asocial decision comes from `classify`. Index 0 = Cultural, 1 = Asocial. `sample_strategies` is redefined to call this with the module-presence classifier (behavior unchanged — the existing `ledger::tests` must still pass).
- Produces (in `founder.rs`):
  - `pub struct FounderTracker` (holds `HashMap<LineageId, StrategyKind>`)
  - `pub fn init(world: &World) -> FounderTracker` — tag every alive founder lineage by its module presence at t0.
  - `pub fn observe(&mut self, world: &World)` — tag any newly-appeared lineage with its mother's tag (fallback: current module presence if the mother is untracked). Call once per tick.
  - `pub fn kind_of(&self, world: &World, i: usize) -> StrategyKind` — the lineage-locked tag for alive agent slot `i`.
  - `pub fn sample_by_founder(world: &World, tracker: &FounderTracker) -> [StrategyStat; 2]` — `ledger::sample_strategies_by(world, |w, i| tracker.kind_of(w, i))`.

- [ ] **Step 1: Refactor `sample_strategies` to a classifier-parameterized form (behavior-preserving)**

In `ledger.rs`, replace the body of `sample_strategies` and add the generic form. The `Acc`/`StrategyStat` accumulation is unchanged — only the Cultural/Asocial decision is now injected:

```rust
/// Per-strategy aggregate, bucketing each alive agent by `classify`. Index 0 =
/// Cultural, index 1 = Asocial. Read-only. `sample_strategies` is this with the
/// Communicator-module-presence classifier.
pub fn sample_strategies_by<F>(world: &World, classify: F) -> [StrategyStat; 2]
where
    F: Fn(&World, usize) -> StrategyKind,
{
    let mut cultural = Acc::default();
    let mut asocial = Acc::default();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let acc = match classify(world, i) {
            StrategyKind::Cultural => &mut cultural,
            StrategyKind::Asocial => &mut asocial,
        };
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

/// Per-strategy aggregate keyed on Communicator-module presence (the per-tick
/// phenotype). Index 0 = Cultural, index 1 = Asocial.
pub fn sample_strategies(world: &World) -> [StrategyStat; 2] {
    sample_strategies_by(world, |world, i| {
        if module::has(&world.agents.modules[i], ModuleType::Communicator) {
            StrategyKind::Cultural
        } else {
            StrategyKind::Asocial
        }
    })
}
```

- [ ] **Step 2: Verify the refactor is behavior-preserving**

Run: `cargo test -p anabios-headless ledger::tests -- --nocapture`
Expected: PASS (the existing `sample_buckets_by_communicator_module` and `empty_strategy_has_zero_means_not_nan` still pass — the refactor changed structure, not behavior).

- [ ] **Step 3: Write the failing founder test**

Create `crates/anabios-headless/src/founder.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const MIX: &str = "\
name = \"t\"
seed = 3
[[agents]]
count = 4
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
[[agents]]
count = 2
archetype = \"communicator\"
placement = { kind = \"uniform\" }
";

    #[test]
    fn init_tags_founders_by_module_presence() {
        let world = Scenario::parse_toml(MIX).unwrap().instantiate();
        let t = init(&world);
        let stats = sample_by_founder(&world, &t);
        assert_eq!(stats[0].count, 2, "two communicator founders → Cultural-descended");
        assert_eq!(stats[1].count, 4, "four asocial founders → Asocial-descended");
    }

    #[test]
    fn descendants_keep_founder_tag_even_if_module_mutates() {
        // All-asocial-founder world. Run it; every descendant MUST remain
        // Asocial-descended by founder tag, regardless of any Communicator
        // module a birth mutates in. If module mutation ever produces a
        // Communicator (module readout disagrees), that disagreement proves the
        // lineage-locked tag is doing its job.
        const ASOCIAL: &str = "\
name = \"t\"
seed = 7
[[agents]]
count = 40
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
";
        let mut world = Scenario::parse_toml(ASOCIAL).unwrap().instantiate();
        let mut t = init(&world);
        for _ in 0..1500 {
            step(&mut world);
            t.observe(&world);
        }
        let by_founder = sample_by_founder(&world, &t);
        let by_module = crate::ledger::sample_strategies(&world);
        // Invariant: no lineage descends from a Cultural founder here.
        assert_eq!(by_founder[0].count, 0, "no Cultural-descended lineages exist");
        // If the module readout found any Communicator (mutation), the two tags
        // must disagree — which is the whole point of the lineage-locked tag.
        if by_module[0].count > 0 {
            assert_ne!(
                by_module[0].count, by_founder[0].count,
                "module tag drifted via mutation; founder tag held"
            );
        }
    }
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test -p anabios-headless founder:: -- --nocapture`
Expected: FAIL to compile (`init`, `sample_by_founder`, `FounderTracker` not found; `mod founder;` not yet in `main.rs`).

- [ ] **Step 5: Write the implementation**

Prepend the implementation above the test module in `founder.rs`:

```rust
//! Lineage-locked founder tags for O2 invasion analysis.
//!
//! The raw cultural/asocial key (Communicator-module presence) is re-read every
//! tick and the module is recombined/mutated at every birth, so it drifts — O1
//! showed this confounds the "who is cultural" readout. This tracker fixes each
//! lineage's tag at t0 (by initial module presence) and inherits it through
//! `parent_ids`, so it reports which founding population an agent DESCENDS from,
//! immune to later module mutation. Headless-only; reads `&World`, never mutates.

use std::collections::HashMap;

use anabios_core::agent::{LineageId, LINEAGE_NONE};
use anabios_core::module::{self, ModuleType};
use anabios_core::world::World;

use crate::ledger::{sample_strategies_by, StrategyKind, StrategyStat};

fn module_kind(world: &World, i: usize) -> StrategyKind {
    if module::has(&world.agents.modules[i], ModuleType::Communicator) {
        StrategyKind::Cultural
    } else {
        StrategyKind::Asocial
    }
}

/// Maps each lineage id to the founding strategy it descends from.
pub struct FounderTracker {
    tag: HashMap<LineageId, StrategyKind>,
}

/// Seed founder tags from the initial population's module presence. Call once on
/// the freshly-instantiated world (before any `step`).
pub fn init(world: &World) -> FounderTracker {
    let mut tag = HashMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        tag.insert(world.agents.lineage_id[i], module_kind(world, i));
    }
    FounderTracker { tag }
}

impl FounderTracker {
    /// Tag any newly-appeared lineage with its mother's founder tag. Call once
    /// per tick: a birth's mother was tagged on a prior tick and lineage ids are
    /// never reused, so the map only grows and every new lineage finds its
    /// parent. Fallback (untracked mother / founder with no parent): current
    /// module presence.
    pub fn observe(&mut self, world: &World) {
        for id in world.agents.iter_alive() {
            let i = id as usize;
            let lid = world.agents.lineage_id[i];
            if self.tag.contains_key(&lid) {
                continue;
            }
            let mother = world.agents.parent_ids[i][0];
            let kind = if mother != LINEAGE_NONE {
                self.tag.get(&mother).copied()
            } else {
                None
            }
            .unwrap_or_else(|| module_kind(world, i));
            self.tag.insert(lid, kind);
        }
    }

    /// The lineage-locked tag for alive agent slot `i` (falls back to module
    /// presence for any untracked lineage — shouldn't happen if `observe` ran).
    pub fn kind_of(&self, world: &World, i: usize) -> StrategyKind {
        self.tag
            .get(&world.agents.lineage_id[i])
            .copied()
            .unwrap_or_else(|| module_kind(world, i))
    }
}

/// Per-strategy aggregate keyed on the lineage-locked founder tag instead of the
/// per-tick module readout. Index 0 = Cultural-descended, 1 = Asocial-descended.
pub fn sample_by_founder(world: &World, tracker: &FounderTracker) -> [StrategyStat; 2] {
    sample_strategies_by(world, |w, i| tracker.kind_of(w, i))
}
```

Then add `mod founder;` to `crates/anabios-headless/src/main.rs` beside the other `mod` declarations (e.g. after `mod ledger;`).

- [ ] **Step 6: Run test + build**

Run: `cargo test -p anabios-headless founder:: -- --nocapture && cargo build -p anabios-headless`
Expected: both founder tests PASS; crate builds.

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-headless/src/founder.rs crates/anabios-headless/src/ledger.rs crates/anabios-headless/src/main.rs
git commit -m "feat(o2-step0): lineage-locked founder tag (mutation-robust strategy key)" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Wire both instruments into `autopsy`

Make `autopsy` report the share-relative fitness alongside the absolute one, and add a `--tag founder|module` option that selects the lineage-locked sampler.

**Files:**
- Modify: `crates/anabios-headless/src/autopsy.rs`
- Modify: `crates/anabios-headless/src/main.rs` (add the `--tag` arg to `Command::Autopsy` + a `TagArg` value-enum, mirroring the existing `MutantArg` pattern)
- Test: inline `#[cfg(test)]` in `autopsy.rs` (extend the existing test)

**Interfaces:**
- Consumes: `ledger::{invasion_fitness, invasion_fitness_share, sample_strategies, strategy_label, InvasionWindow, StrategyKind}`, `founder::{init, sample_by_founder, FounderTracker}`.
- Produces: `autopsy::run(scenario, seed, ticks, window, out, mutant, tag)` where `tag: FounderTagMode` selects `module` (default, existing behavior) or `founder` sampling. stdout gains a second line: `invasion_fitness_share mutant=<label> r=<value|none> <VERDICT>`. The per-window CSV is unchanged.

- [ ] **Step 1: Write the failing test**

Extend the `#[cfg(test)] mod tests` in `autopsy.rs`. Add a `FounderTagMode` param to the `run` call and assert both invasion lines are producible. Replace the existing single test call site and add a founder-mode variant:

```rust
    #[test]
    fn autopsy_reports_share_and_supports_founder_tag() {
        let dir = std::env::temp_dir().join("anabios_o2_step0_autopsy");
        std::fs::create_dir_all(&dir).unwrap();
        let scen = dir.join("mix.toml");
        std::fs::File::create(&scen).unwrap().write_all(MIX.as_bytes()).unwrap();

        // Both tag modes must run and write a ledger with both strategies.
        for mode in [FounderTagMode::Module, FounderTagMode::Founder] {
            let csv = dir.join(format!("ledger-{mode:?}.csv"));
            run(scen.clone(), Some(7), 200, 50, csv.clone(), StrategyKind::Cultural, mode)
                .unwrap();
            let body = std::fs::read_to_string(&csv).unwrap();
            assert!(body.contains(",cultural,"), "ledger has a cultural row ({mode:?})");
            assert!(body.contains(",asocial,"), "ledger has an asocial row ({mode:?})");
        }
    }
```

(Keep `MIX` and the `use` lines from the existing test module; `FounderTagMode` comes in via `use super::*`.)

**Also update the pre-existing test** `autopsy_writes_ledger_rows_for_both_strategies` in the same module: its `run(...)` call currently passes 6 args and will not compile against the new signature. Add `FounderTagMode::Module` as the 7th argument, keeping all its existing assertions (header line, both-strategy rows, `>= 8` data rows). Both tests must compile and pass.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-headless autopsy:: -- --nocapture`
Expected: FAIL to compile (`FounderTagMode` not found; the new test *and* the pre-existing `autopsy_writes_ledger_rows_for_both_strategies` both mismatch the new `run` arity — both get fixed in Step 3).

- [ ] **Step 3: Implement in `autopsy.rs`**

Add the mode enum and thread it through `run`:

```rust
/// Which strategy key `autopsy` buckets by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FounderTagMode {
    /// Per-tick Communicator-module presence (the original O1 key).
    Module,
    /// Lineage-locked founder tag (mutation-robust; O2 Step 0).
    Founder,
}
```

Change the signature to
`pub fn run(scenario_path: PathBuf, seed: Option<u64>, ticks: u64, window: u64, out: PathBuf, mutant: StrategyKind, tag: FounderTagMode) -> Result<()>`
and inside:
- After `let mut world = scenario.instantiate();`, add
  `let mut tracker = (tag == FounderTagMode::Founder).then(|| crate::founder::init(&world));`
- In the tick loop, immediately after `step(&mut world);`, add
  `if let Some(t) = tracker.as_mut() { t.observe(&world); }`
- Replace the `let stats = sample_strategies(&world);` line with
  ```rust
  let stats = match &tracker {
      Some(t) => crate::founder::sample_by_founder(&world, t),
      None => sample_strategies(&world),
  };
  ```
- After the existing absolute `invasion_fitness` print block, add a share print block:
  ```rust
  match invasion_fitness_share(&invasion, RARE_FRAC_MAX) {
      Some(r) => {
          let verdict = if r > 0.0 { "INVADES" } else { "EXCLUDED" };
          println!(
              "invasion_fitness_share mutant={} r={:.5} {}",
              strategy_label(mutant), r, verdict
          );
      }
      None => println!(
          "invasion_fitness_share mutant={} r=none (no rare-phase data)",
          strategy_label(mutant)
      ),
  }
  ```
- Add `invasion_fitness_share` and `crate::founder` to the `use` imports at the top of `autopsy.rs`.

- [ ] **Step 4: Wire the `--tag` arg in `main.rs`**

Mirror the existing `MutantArg` value-enum pattern. Add:

```rust
/// CLI spelling of the strategy-key mode for `autopsy`.
#[derive(Clone, Copy, clap::ValueEnum)]
enum TagArg {
    Module,
    Founder,
}

impl From<TagArg> for autopsy::FounderTagMode {
    fn from(t: TagArg) -> Self {
        match t {
            TagArg::Module => autopsy::FounderTagMode::Module,
            TagArg::Founder => autopsy::FounderTagMode::Founder,
        }
    }
}
```

Add to the `Command::Autopsy { … }` variant:

```rust
        /// Strategy key: `module` (per-tick Communicator presence, original) or
        /// `founder` (lineage-locked founding-population tag, mutation-robust).
        #[arg(long, value_enum, default_value_t = TagArg::Module)]
        tag: TagArg,
```

And update the match arm to pass it:

```rust
        Command::Autopsy { scenario, seed, ticks, window, out, mutant, tag } => {
            autopsy::run(scenario, seed, ticks, window, out, mutant.into(), tag.into())
        }
```

- [ ] **Step 5: Run test + build + smoke**

Run: `cargo test -p anabios-headless autopsy:: -- --nocapture && cargo build -p anabios-headless`
Expected: test PASS; builds. Smoke both modes:
```
cargo run -p anabios-headless -- autopsy --scenario scenarios/inventions.toml --ticks 500 --window 100 --out /tmp/o2-smoke.csv --tag founder
```
Expected: prints BOTH an `invasion_fitness …` and an `invasion_fitness_share …` line, plus `ledger written`.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-headless/src/autopsy.rs crates/anabios-headless/src/main.rs
git commit -m "feat(o2-step0): autopsy reports share-relative fitness + --tag founder|module" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Validation — the fixed instruments reproduce O1's finding (protocol)

Confirm the two new instruments give the same (or cleaner) O1 conclusion on a real scenario, so O2a/O2b can rely on them. **Measurement only — record real numbers; do not invent.**

- [ ] **Step 1: Build release.** `cargo build --release -p anabios-headless`

- [ ] **Step 2: Run baseline vs practices-off with BOTH new instruments.** For seeds `1 2 3` on each of `scenarios/o1-invasion-cultural-into-asocial.toml` (practices on) and `scenarios/o1-lever-practices-off.toml` (practices off), run:
```
./target/release/anabios-headless autopsy --scenario <scen> --seed <s> --ticks 2500 --window 500 --tag founder --out docs/superpowers/data/o2/step0-<cond>-<s>.csv --mutant cultural
```
Record both the `invasion_fitness` and `invasion_fitness_share` lines for each. (`docs/superpowers/data/o2/` — create it; do NOT use `runs/`, which is gitignored.)

- [ ] **Step 3: Compare tags.** For one baseline seed, run once with `--tag module` and once with `--tag founder`; from the two ledger CSVs, record whether the late-window cultural counts differ (they will if module mutation occurred). Note the magnitude — this quantifies how much the old tag was drifting.

- [ ] **Step 4: Confirm the conclusion holds.** Assert the O1 result survives the cleaner instruments: practices-off should still show the cultural strategy gaining share (share-fitness less negative / positive) relative to baseline. If the cleaner metric *changes* the conclusion, that is itself an important finding — record it prominently.

- [ ] **Step 5: Write a short validation note + commit.** Create `docs/superpowers/data/o2/step0-validation.md` with the recorded numbers, the module-vs-founder drift magnitude, and the "conclusion holds / changed" verdict. Commit the data + note:

```bash
git add docs/superpowers/data/o2/
git commit -m "chore(o2-step0): validate fixed instruments reproduce the O1 practices finding" \
  -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Done when

- `invasion_fitness_share` exists with a test proving it cancels the population-change confound (share-flat-under-collapse → ~0 while absolute < 0).
- `FounderTracker` + `sample_by_founder` exist with a test proving descendants keep their founder tag under module mutation.
- `autopsy` prints both metrics and accepts `--tag founder|module`, default `module` (back-compatible).
- The validation note records that the fixed instruments reproduce (or, if not, honestly revise) the O1 practices finding, plus the measured module-vs-founder drift.
- **No `anabios-core` change, no `FORMAT_VERSION` bump, no golden movement** (`cargo test -p anabios-core --test determinism` unchanged). `cargo fmt --check` + `cargo clippy -p anabios-headless --all-targets -- -D warnings` clean.

## Notes / risks

- **`FounderTracker` memory grows** one entry per lineage ever born — fine for invasion-scale runs (thousands of agents, a few thousand ticks). If reused for soak runs later, add eviction of dead-and-childless lineages; out of scope here.
- **`observe` must run every tick.** A birth whose mother dies before the next `observe` would lose its inheritance link; calling `observe` immediately after each `step` (as Task 3 wires it) guarantees the mother was tagged the prior tick. Do not sample-time-only.
- **Back-compat is a hard requirement.** `--tag module` must reproduce the exact O1 numbers; the refactor in Task 2 is behavior-preserving and the existing `ledger::tests` guard it.
```
