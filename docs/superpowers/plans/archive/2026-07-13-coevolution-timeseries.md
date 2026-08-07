# Gene↔Culture Co-evolution Time-Series — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a live, determinism-safe time-series panel to the Godot frontend that plots gene-side and meme-side signals over time, so you can watch genome and meme complexes co-evolve.

**Architecture:** Pure Rust aggregate helpers compute per-tick scalar metrics from a read-only `&World`. The gdext `Simulation` node samples them every tick inside `step_n` into a view-only history buffer it owns (outside `World`), plus a persistent codex event log. A new GDScript `CoevolutionPanel` (`Control` + custom `_draw`) reads that history and renders a vertical stack of small-multiple charts with codex event markers.

**Tech Stack:** Rust (gdext binding, `anabios-godot` crate), GDScript (Godot 4.7), `anabios-core` (read-only).

## Global Constraints

- **Determinism invariant (LOAD-BEARING):** No new `World` fields. No `&mut World`. All `World` access is `&self`/`&World`. All new mutable state (history buffer, event log) lives on the `Simulation` gdext node, which is NOT `World`. Golden hashes must stay byte-identical: `(0, 0x58807132956798b1)`, `(100, 0xa020c143eccfb4eb)`, `(1000, 0xfd21efef4e1619e4)`. No golden refresh.
- **All new Rust code lives in `crates/anabios-godot`** (the binding). `anabios-core` is not modified.
- **CI gate — run locally with the stable toolchain to match CI** (local default toolchain differs): `rustup run stable cargo fmt --all --check`; `rustup run stable cargo clippy --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc --workspace --no-deps --document-private-items`; `rustup run stable cargo test --workspace --lib --tests`. **Commit the exact `cargo fmt` output** (CI checks the committed tree).
- **Doc-comment gotcha:** escape `[0,1]` / `[N]` as `` `[0,1]` `` in doc comments or rustdoc treats them as broken intra-doc links and fails `-D warnings`.
- **GDScript has no unit-test harness here.** GDScript task verification = clean headless boot (`godot --headless --path game res://scenes/main.tscn --quit-after <frames>`) + manual visual check. macOS has no `timeout`; use `--quit-after` (Godot frames).
- **Godot binary:** the project's Godot 4.7 executable (invoke as `godot`; if not on PATH, the user runs it — surface the command).

---

### Task 1: Pure metric helper functions (Rust, TDD)

Pure functions that compute each scalar from compact live-agent slices. No Godot, no `World` — trivially unit-testable. `coevo_metrics` (Task 2) composes them.

**Files:**
- Create: `crates/anabios-godot/src/coevo.rs`
- Modify: `crates/anabios-godot/src/lib.rs` (add `mod coevo;` near the top, after the `use godot::prelude::*;` line)

**Interfaces:**
- Consumes: `anabios_core::genome::{Genome, GenomeSlot, GENOME_LEN}`, `anabios_core::program::MEME_CHANNELS`, `anabios_core::culture::technique_match`.
- Produces (all `pub(crate)`):
  - `fn frac_true(flags: &[bool]) -> f32`
  - `fn mean_slot(genomes: &[Genome], slot: GenomeSlot) -> f32`
  - `fn mean_channel_over(memes: &[[f32; MEME_CHANNELS]], keep: &[bool], ch: usize) -> f32`
  - `fn mean_tech_match(memes: &[[f32; MEME_CHANNELS]], keep: &[bool], opt: f32) -> f32`
  - `fn genetic_diversity(genomes: &[Genome]) -> f32`
  - `fn species_max_meme_divergence(memes: &[[f32; MEME_CHANNELS]], species: &[u32], xs: &[f32], comm: &[bool]) -> f32`

- [ ] **Step 1: Write the failing tests**

Create `crates/anabios-godot/src/coevo.rs`:

```rust
//! Pure per-tick co-evolution metric helpers. Each takes compact slices over
//! the *live* agents (already filtered/parallel) and returns one scalar. Kept
//! free of Godot and `World` types so they unit-test in isolation.

use anabios_core::culture::{technique_match, TECH_CHANNEL};
use anabios_core::genome::{Genome, GenomeSlot, GENOME_LEN};
use anabios_core::program::MEME_CHANNELS;

/// Minimum members per spatial half for a species to count toward dialect
/// divergence. Mirrors `DIALECT_MIN_HALF` in `anabios_core::codex`.
const DIALECT_MIN_HALF: usize = 3;

/// Fraction of `flags` that are true, in `[0,1]`. Empty slice → 0.0.
pub(crate) fn frac_true(flags: &[bool]) -> f32 {
    if flags.is_empty() {
        return 0.0;
    }
    flags.iter().filter(|&&b| b).count() as f32 / flags.len() as f32
}

/// Mean of one genome slot over all `genomes`. Empty → 0.0.
pub(crate) fn mean_slot(genomes: &[Genome], slot: GenomeSlot) -> f32 {
    if genomes.is_empty() {
        return 0.0;
    }
    genomes.iter().map(|g| g.get(slot)).sum::<f32>() / genomes.len() as f32
}

/// Mean of meme channel `ch` over agents where `keep[i]` is true. No kept
/// agents (or bad channel) → 0.0.
pub(crate) fn mean_channel_over(
    memes: &[[f32; MEME_CHANNELS]],
    keep: &[bool],
    ch: usize,
) -> f32 {
    if ch >= MEME_CHANNELS {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut n = 0u32;
    for (m, &k) in memes.iter().zip(keep) {
        if k {
            sum += m[ch];
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

/// Mean `technique_match(meme[TECH], opt)` over kept agents. No kept agents → 0.0.
pub(crate) fn mean_tech_match(
    memes: &[[f32; MEME_CHANNELS]],
    keep: &[bool],
    opt: f32,
) -> f32 {
    let mut sum = 0.0;
    let mut n = 0u32;
    for (m, &k) in memes.iter().zip(keep) {
        if k {
            sum += technique_match(m[TECH_CHANNEL], opt);
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

/// Mean per-slot variance across `genomes` (summed variance over the 50 slots
/// divided by 50). Empty → 0.0. A cheap scalar for genetic spread.
pub(crate) fn genetic_diversity(genomes: &[Genome]) -> f32 {
    if genomes.is_empty() {
        return 0.0;
    }
    let n = genomes.len() as f32;
    let mut total_var = 0.0;
    for slot in 0..GENOME_LEN {
        let mut mean = 0.0;
        for g in genomes {
            mean += g.0[slot];
        }
        mean /= n;
        let mut var = 0.0;
        for g in genomes {
            let d = g.0[slot] - mean;
            var += d * d;
        }
        total_var += var / n;
    }
    total_var / GENOME_LEN as f32
}

/// Maximum, over Communicator-bearing species, of the west/east per-channel
/// mean-meme L2 distance — the same kernel the `DialectFormed` detector uses,
/// aggregated to one scalar. A species contributes only if each half (split at
/// its members' mean x) has ≥ `DIALECT_MIN_HALF` members. None qualify → 0.0.
pub(crate) fn species_max_meme_divergence(
    memes: &[[f32; MEME_CHANNELS]],
    species: &[u32],
    xs: &[f32],
    comm: &[bool],
) -> f32 {
    use std::collections::BTreeMap;
    // Group live indices by species; note which species have a communicator.
    let mut members: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut has_comm: BTreeMap<u32, bool> = BTreeMap::new();
    for i in 0..memes.len() {
        members.entry(species[i]).or_default().push(i);
        let e = has_comm.entry(species[i]).or_insert(false);
        *e = *e || comm[i];
    }
    let mut best = 0.0f32;
    for (sid, idxs) in members.iter() {
        if !has_comm.get(sid).copied().unwrap_or(false) {
            continue;
        }
        let cx = idxs.iter().map(|&i| xs[i]).sum::<f32>() / idxs.len() as f32;
        let (mut west, mut east): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        for &i in idxs {
            if xs[i] < cx {
                west.push(i);
            } else {
                east.push(i);
            }
        }
        if west.len() < DIALECT_MIN_HALF || east.len() < DIALECT_MIN_HALF {
            continue;
        }
        let mut wm = [0.0f32; MEME_CHANNELS];
        let mut em = [0.0f32; MEME_CHANNELS];
        for &i in &west {
            for ch in 0..MEME_CHANNELS {
                wm[ch] += memes[i][ch];
            }
        }
        for &i in &east {
            for ch in 0..MEME_CHANNELS {
                em[ch] += memes[i][ch];
            }
        }
        let (wn, en) = (west.len() as f32, east.len() as f32);
        let mut l2 = 0.0f32;
        for ch in 0..MEME_CHANNELS {
            let d = wm[ch] / wn - em[ch] / en;
            l2 += d * d;
        }
        best = best.max(l2.sqrt());
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::genome::{Genome, GenomeSlot, GENOME_LEN};
    use anabios_core::program::MEME_CHANNELS;

    fn genome_with(slot: GenomeSlot, v: f32) -> Genome {
        let mut a = [0.0f32; GENOME_LEN];
        a[slot as usize] = v;
        Genome(a)
    }

    #[test]
    fn frac_true_counts_and_bounds() {
        assert_eq!(frac_true(&[]), 0.0);
        assert_eq!(frac_true(&[true, false, false, false]), 0.25);
        assert_eq!(frac_true(&[true, true]), 1.0);
    }

    #[test]
    fn mean_slot_averages_named_slot() {
        let gs = [
            genome_with(GenomeSlot::SocialLearning, 0.2),
            genome_with(GenomeSlot::SocialLearning, 0.8),
        ];
        assert!((mean_slot(&gs, GenomeSlot::SocialLearning) - 0.5).abs() < 1e-6);
        assert_eq!(mean_slot(&[], GenomeSlot::SocialLearning), 0.0);
    }

    #[test]
    fn mean_channel_respects_keep_mask() {
        let mut a = [0.0f32; MEME_CHANNELS];
        a[5] = 1.0;
        let mut b = [0.0f32; MEME_CHANNELS];
        b[5] = 0.0;
        let memes = [a, b];
        // Only the first agent is a communicator → mean over kept = 1.0.
        assert_eq!(mean_channel_over(&memes, &[true, false], 5), 1.0);
        // No communicators → 0.0, not NaN.
        assert_eq!(mean_channel_over(&memes, &[false, false], 5), 0.0);
        // Bad channel → 0.0.
        assert_eq!(mean_channel_over(&memes, &[true, true], 99), 0.0);
    }

    #[test]
    fn genetic_diversity_zero_for_identical_and_positive_for_spread() {
        let same = [genome_with(GenomeSlot::Size, 0.5), genome_with(GenomeSlot::Size, 0.5)];
        assert_eq!(genetic_diversity(&same), 0.0);
        let spread = [genome_with(GenomeSlot::Size, 0.0), genome_with(GenomeSlot::Size, 1.0)];
        assert!(genetic_diversity(&spread) > 0.0);
    }

    #[test]
    fn divergence_needs_comm_and_min_half() {
        // Two species. Species 7 has 3 west (meme0=0) + 3 east (meme0=1) comms.
        let lo = [0.0f32; MEME_CHANNELS];
        let mut hi = [0.0f32; MEME_CHANNELS];
        hi[0] = 1.0;
        let memes = vec![lo, lo, lo, hi, hi, hi];
        let species = vec![7u32; 6];
        let xs = vec![0.0, 1.0, 2.0, 10.0, 11.0, 12.0]; // mean x = 6 → 3 west, 3 east
        let comm = vec![true; 6];
        let d = species_max_meme_divergence(&memes, &species, &xs, &comm);
        assert!((d - 1.0).abs() < 1e-6, "expected L2 ~1.0, got {d}");
        // No communicators → 0.0.
        let none = vec![false; 6];
        assert_eq!(species_max_meme_divergence(&memes, &species, &xs, &none), 0.0);
    }
}
```

- [ ] **Step 2: Wire the module in**

In `crates/anabios-godot/src/lib.rs`, add after `use godot::prelude::*;` (around line 11):

```rust
mod coevo;
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `rustup run stable cargo test -p anabios-godot coevo:: -- --nocapture`
Expected: FAIL to compile first only if `mod coevo;` missing; once wired, tests build and PASS (these are self-contained). If any assertion fails, fix the helper. Expected end state after Step 4: PASS.

- [ ] **Step 4: Run tests to verify they pass**

Run: `rustup run stable cargo test -p anabios-godot coevo::`
Expected: PASS — `test result: ok. 5 passed`.

- [ ] **Step 5: Format, lint, commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-godot --all-targets -- -D warnings
git add crates/anabios-godot/src/coevo.rs crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): pure per-tick co-evolution metric helpers

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: History buffer + `coevo_metrics` + per-tick sampling + series exports

Compose the Task 1 helpers into a `CoevoSample`, sample it every tick inside `step_n` into a view-only `history` on the `Simulation` node, and expose read-only series/scrub queries. Prove golden hashes unchanged.

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs` (`Simulation` struct + `init` + `new_world`/`load_scenario`/`load_scenario_with_seed`/`step_n`; add exports)
- Test: `crates/anabios-godot/src/lib.rs` (`#[cfg(test)]` — reuse existing test module) + run `crates/anabios-core/tests/determinism.rs`

**Interfaces:**
- Consumes: Task 1 helpers (`crate::coevo::*`), `anabios_core::culture::{env_optimum_at, SKILL_CHANNEL}`, `anabios_core::genome::GenomeSlot`, `anabios_core::module::{self, ModuleType}`.
- Produces (GDScript-visible `#[func]`):
  - `coevo_metrics(&self) -> Dictionary` — current-tick scalar bundle (keys per spec table).
  - `coevo_history_len(&self) -> i64`
  - `coevo_series(&self, key: GString) -> PackedFloat32Array` — full history of one key.
  - `coevo_sample_at(&self, index: i64) -> Dictionary` — one historical sample (scrub readout).
- Produces (internal): `struct CoevoSample { … }`, `Simulation.history: Vec<CoevoSample>`, `fn sample_now(w: &World) -> CoevoSample`.

- [ ] **Step 1: Add the sample struct and buffer field**

In `crates/anabios-godot/src/lib.rs`, above the `Simulation` struct add:

```rust
/// One per-tick co-evolution metric snapshot. Plain data, lives OUTSIDE
/// `World` (on the `Simulation` node) so determinism is untouched.
#[derive(Clone, Copy)]
struct CoevoSample {
    tick: f32,
    communicator_frac: f32,
    mean_social_learning: f32,
    mean_individual_learning: f32,
    genetic_diversity: f32,
    mean_skill: f32,
    mean_tech_match: f32,
    meme_divergence: f32,
    live_count: f32,
    species_count: f32,
    env_optimum: f32,
}

/// Soft cap on retained samples (~tens of KB each thousand ticks). Past this we
/// stop appending and log once, rather than grow without bound.
const COEVO_HISTORY_CAP: usize = 200_000;
```

Extend the struct and `init`:

```rust
pub struct Simulation {
    #[allow(dead_code)]
    base: Base<Node>,
    inner: Option<anabios_core::World>,
    history: Vec<CoevoSample>,
    history_capped_logged: bool,
}
```

```rust
    fn init(base: Base<Node>) -> Self {
        Self { base, inner: None, history: Vec::new(), history_capped_logged: false }
    }
```

- [ ] **Step 2: Add `sample_now` and sample inside `step_n`; clear on (re)load**

Add a free function near the bottom of the `impl Simulation` block's file (outside `#[godot_api]`, e.g. after it):

```rust
/// Compute a `CoevoSample` from a read-only world. Builds compact live-only
/// slices once, then delegates to the pure `coevo` helpers.
fn sample_now(w: &anabios_core::World) -> CoevoSample {
    use anabios_core::culture::{env_optimum_at, SKILL_CHANNEL};
    use anabios_core::genome::GenomeSlot;
    use anabios_core::module::{self, ModuleType};

    let mut memes: Vec<[f32; anabios_core::program::MEME_CHANNELS]> = Vec::new();
    let mut genomes: Vec<anabios_core::genome::Genome> = Vec::new();
    let mut species: Vec<u32> = Vec::new();
    let mut xs: Vec<f32> = Vec::new();
    let mut comm: Vec<bool> = Vec::new();
    let mut species_set = std::collections::BTreeSet::new();
    for id in w.agents.iter_alive() {
        let i = id as usize;
        memes.push(w.agents.meme_vector[i]);
        genomes.push(w.agents.genome[i]);
        species.push(w.agents.species_id[i]);
        xs.push(w.agents.position[i].x);
        comm.push(module::has(&w.agents.modules[i], ModuleType::Communicator));
        species_set.insert(w.agents.species_id[i]);
    }
    let active = w.env_period > 0;
    let opt = if active { env_optimum_at(w.tick, w.env_period) } else { 0.0 };
    CoevoSample {
        tick: w.tick as f32,
        communicator_frac: crate::coevo::frac_true(&comm),
        mean_social_learning: crate::coevo::mean_slot(&genomes, GenomeSlot::SocialLearning),
        mean_individual_learning: crate::coevo::mean_slot(&genomes, GenomeSlot::IndividualLearning),
        genetic_diversity: crate::coevo::genetic_diversity(&genomes),
        mean_skill: crate::coevo::mean_channel_over(&memes, &comm, SKILL_CHANNEL),
        mean_tech_match: if active {
            crate::coevo::mean_tech_match(&memes, &comm, opt)
        } else {
            0.0
        },
        meme_divergence: crate::coevo::species_max_meme_divergence(&memes, &species, &xs, &comm),
        live_count: memes.len() as f32,
        species_count: species_set.len() as f32,
        env_optimum: if active { opt } else { -1.0 },
    }
}
```

Change `step_n` to sample each tick (respecting the cap):

```rust
    #[func]
    fn step_n(&mut self, n: i64) {
        for _ in 0..n.max(0) {
            let Some(w) = self.inner.as_mut() else { return };
            anabios_core::tick::step(w);
            if self.history.len() < COEVO_HISTORY_CAP {
                let s = sample_now(self.inner.as_ref().unwrap());
                self.history.push(s);
            } else if !self.history_capped_logged {
                godot_warn!("coevo history hit {} samples; no longer recording", COEVO_HISTORY_CAP);
                self.history_capped_logged = true;
            }
        }
    }
```

In each of `new_world`, `load_scenario`, `load_scenario_with_seed`, after setting `self.inner`, add:

```rust
        self.history.clear();
        self.history_capped_logged = false;
```

- [ ] **Step 3: Add the read-only exports**

Add inside the `#[godot_api] impl Simulation` block (near `species_stats`):

```rust
    /// Current-tick co-evolution scalars (see plan/spec for key meanings).
    /// All frequencies/means in `[0,1]`; `env_optimum` is `-1.0` when inactive.
    #[func]
    fn coevo_metrics(&self) -> Dictionary {
        let mut d = Dictionary::new();
        let Some(w) = self.inner.as_ref() else { return d };
        let s = sample_now(w);
        d.set("tick", s.tick as i64);
        d.set("communicator_frac", s.communicator_frac);
        d.set("mean_social_learning", s.mean_social_learning);
        d.set("mean_individual_learning", s.mean_individual_learning);
        d.set("genetic_diversity", s.genetic_diversity);
        d.set("mean_skill", s.mean_skill);
        d.set("mean_tech_match", s.mean_tech_match);
        d.set("meme_divergence", s.meme_divergence);
        d.set("live_count", s.live_count);
        d.set("species_count", s.species_count);
        d.set("env_optimum", s.env_optimum);
        d
    }

    /// Number of recorded per-tick samples.
    #[func]
    fn coevo_history_len(&self) -> i64 {
        self.history.len() as i64
    }

    /// Full history of one series, oldest-first. Unknown key → empty.
    #[func]
    fn coevo_series(&self, key: GString) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        let k = String::from(key);
        let pick: fn(&CoevoSample) -> f32 = match k.as_str() {
            "tick" => |s| s.tick,
            "communicator_frac" => |s| s.communicator_frac,
            "mean_social_learning" => |s| s.mean_social_learning,
            "mean_individual_learning" => |s| s.mean_individual_learning,
            "genetic_diversity" => |s| s.genetic_diversity,
            "mean_skill" => |s| s.mean_skill,
            "mean_tech_match" => |s| s.mean_tech_match,
            "meme_divergence" => |s| s.meme_divergence,
            "live_count" => |s| s.live_count,
            "species_count" => |s| s.species_count,
            "env_optimum" => |s| s.env_optimum,
            _ => return out,
        };
        for s in &self.history {
            out.push(pick(s));
        }
        out
    }

    /// One historical sample as a Dictionary (for the scrub readout). Out-of-range
    /// index → empty.
    #[func]
    fn coevo_sample_at(&self, index: i64) -> Dictionary {
        let mut d = Dictionary::new();
        let Some(s) = (index >= 0).then(|| self.history.get(index as usize)).flatten() else {
            return d;
        };
        d.set("tick", s.tick as i64);
        d.set("communicator_frac", s.communicator_frac);
        d.set("mean_social_learning", s.mean_social_learning);
        d.set("mean_individual_learning", s.mean_individual_learning);
        d.set("genetic_diversity", s.genetic_diversity);
        d.set("mean_skill", s.mean_skill);
        d.set("mean_tech_match", s.mean_tech_match);
        d.set("meme_divergence", s.meme_divergence);
        d.set("live_count", s.live_count);
        d.set("species_count", s.species_count);
        d.set("env_optimum", s.env_optimum);
        d
    }
```

- [ ] **Step 4: Add a Rust test for the buffer via a scenario**

In the existing `#[cfg(test)] mod tests` in `lib.rs` (or add one), add a test that drives the core directly (not through Godot node lifecycle):

```rust
    #[test]
    fn sample_now_is_bounded_and_nonneg() {
        // Instantiate the shipped minimal scenario and step a few ticks.
        let toml = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/minimal.toml"),
        )
        .expect("read minimal.toml");
        let mut w = anabios_core::Scenario::parse_toml(&toml).unwrap().instantiate();
        for _ in 0..25 {
            anabios_core::tick::step(&mut w);
        }
        let s = super::sample_now(&w);
        for v in [
            s.communicator_frac,
            s.mean_social_learning,
            s.mean_individual_learning,
            s.mean_skill,
            s.mean_tech_match,
        ] {
            assert!((0.0..=1.0).contains(&v), "expected [0,1], got {v}");
        }
        assert!(s.meme_divergence >= 0.0);
        assert!(s.genetic_diversity >= 0.0);
        assert!(s.live_count >= 0.0);
    }
```

(If the existing test module can't see `sample_now`, mark `sample_now` `pub(crate)` and call `super::sample_now`. The path in the test above assumes `crates/anabios-godot/` → repo `scenarios/` is `../../scenarios`; adjust if the manifest dir differs.)

- [ ] **Step 5: Run tests + the golden determinism test**

Run: `rustup run stable cargo test -p anabios-godot`
Expected: PASS including `sample_now_is_bounded_and_nonneg`.

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS — `minimal_scenario_matches_golden_hashes` still green (World untouched; proves determinism).

- [ ] **Step 6: Lint, doc, format, commit**

```bash
rustup run stable cargo clippy -p anabios-godot --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc -p anabios-godot --no-deps --document-private-items
rustup run stable cargo fmt --all
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): per-tick co-evolution history buffer + series exports

Samples coevo_metrics every tick inside step_n into a view-only buffer on
the Simulation node (outside World). Golden hashes unchanged.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Persistent codex event log + non-draining cursor read

`codex_panel.gd` currently *drains* `take_codex_events()` each frame, so a second consumer would race it. Move draining into `step_n` (per tick, into a view-only log on `Simulation`) and convert the export to a non-draining cursor read. Update `codex_panel.gd` to keep working. This gives the co-evolution panel access to the *full* event history for markers.

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs` (`Simulation` struct + `step_n` + replace `take_codex_events`)
- Modify: `game/scripts/codex_panel.gd` (use the cursor API)

**Interfaces:**
- Consumes: Task 2's `step_n` loop (adds a drain line to it).
- Produces (GDScript-visible):
  - `codex_events_since(&self, cursor: i64) -> Array<Dictionary>` — events at log index ≥ `cursor`; each `{ index, type, tick, species_id, value, loc }`. Non-draining.
  - `codex_event_count(&self) -> i64`
- Removes: `take_codex_events(&mut self)` (replaced).

- [ ] **Step 1: Add the event log field + struct**

In `crates/anabios-godot/src/lib.rs`, add above `Simulation`:

```rust
/// A codex event retained for the timeline. Plain data on the `Simulation`
/// node (outside `World`).
#[derive(Clone, Copy)]
struct StoredEvent {
    event_type: i64,
    tick: i64,
    species_id: i64,
    value: f32,
    loc_x: f32,
    loc_y: f32,
}
```

Extend `Simulation` + `init`:

```rust
    events: Vec<StoredEvent>,
```
```rust
        Self { base, inner: None, history: Vec::new(), history_capped_logged: false, events: Vec::new() }
```

Clear `self.events.clear();` in `new_world`/`load_scenario`/`load_scenario_with_seed` alongside the `history.clear()` added in Task 2.

- [ ] **Step 2: Drain codex per tick in `step_n`**

Inside the `step_n` loop from Task 2, after the `sample_now` push, drain and store:

```rust
            // Persist codex events for the timeline (single drain site).
            {
                let w = self.inner.as_mut().unwrap();
                for ev in w.codex.drain_events() {
                    if self.events.len() < COEVO_HISTORY_CAP {
                        self.events.push(StoredEvent {
                            event_type: ev.event_type as u8 as i64,
                            tick: ev.tick as i64,
                            species_id: ev.species_id as i64,
                            value: ev.value,
                            loc_x: ev.loc_x,
                            loc_y: ev.loc_y,
                        });
                    }
                }
            }
```

(Determinism note: draining is an *output* read; it does not feed back into `step`. Per-tick vs per-frame draining changes nothing in `World` evolution.)

- [ ] **Step 3: Replace `take_codex_events` with the cursor API**

Delete the `take_codex_events(&mut self)` function (lines ~252–269) and add:

```rust
    /// Total codex events recorded so far.
    #[func]
    fn codex_event_count(&self) -> i64 {
        self.events.len() as i64
    }

    /// Codex events at log index ≥ `cursor`, non-draining. Callers track their
    /// own cursor (use the returned `index` + 1, or `codex_event_count`). Each:
    /// `{ index, type, tick, species_id, value, loc }`.
    #[func]
    fn codex_events_since(&self, cursor: i64) -> Array<Dictionary> {
        let mut out = Array::<Dictionary>::new();
        let start = cursor.max(0) as usize;
        for (idx, ev) in self.events.iter().enumerate().skip(start) {
            let mut d = Dictionary::new();
            d.set("index", idx as i64);
            d.set("type", ev.event_type);
            d.set("tick", ev.tick);
            d.set("species_id", ev.species_id);
            d.set("value", ev.value);
            d.set("loc", Vector2::new(ev.loc_x, ev.loc_y));
            out.push(&d);
        }
        out
    }
```

- [ ] **Step 4: Update `codex_panel.gd` to the cursor API**

Modify `game/scripts/codex_panel.gd`. Replace the drain in `_process` with a cursor read; add a `var _cursor: int = 0`:

```gdscript
var _cursor: int = 0

func _process(_delta: float) -> void:
	var events: Array = sim.codex_events_since(_cursor)
	if events.is_empty():
		return
	for ev in events:
		_cursor = int(ev["index"]) + 1
		var t: int = int(ev["type"])
		if t >= 0 and t < _counts.size():
			_counts[t] += 1
		_recent.append(ev)
		while _recent.size() > MAX_RECENT:
			_recent.pop_front()
	_render()
```

(Reset is automatic: reloading a scenario clears `self.events` in Rust, so the log restarts at 0 and `_cursor` naturally re-reads from a fresh empty log. If the scene is fully reloaded, `_cursor` re-inits to 0 anyway.)

- [ ] **Step 5: Build, lint, boot-check**

Run: `rustup run stable cargo build -p anabios-godot`
Expected: builds clean (no references to removed `take_codex_events`).

Run: `rustup run stable cargo clippy -p anabios-godot --all-targets -- -D warnings`
Expected: PASS.

Boot check (headless, confirms GDScript still parses/runs against the new API):
Run: `godot --headless --path game res://scenes/main.tscn --quit-after 120`
Expected: no parse/script errors; process exits cleanly. (If `godot` isn't on PATH, surface this command for the user to run.)

- [ ] **Step 6: Format + commit**

```bash
rustup run stable cargo fmt --all
git add crates/anabios-godot/src/lib.rs game/scripts/codex_panel.gd
git commit -m "refactor(godot): persistent codex event log + non-draining cursor read

Move codex draining into step_n's single site; codex_panel reads via a
cursor so the co-evolution timeline can share the full event history.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: `CoevolutionPanel` — charts, legend, scrub, hotkey (no markers yet)

The GDScript chart. A `Control` with custom `_draw()` rendering a vertical stack of small-multiple mini-charts from the Rust series, with legend toggles, a scrub cursor, and a `T` hotkey toggle. Markers come in Task 5.

**Files:**
- Create: `game/scripts/coevolution_panel.gd`
- Modify: `game/scenes/main.tscn` (add a `Control` node under `UI` with this script)

**Interfaces:**
- Consumes (Task 2): `sim.coevo_history_len()`, `sim.coevo_series(key)`, `sim.coevo_sample_at(index)`.
- Produces: a self-contained panel node; no other task consumes it (Task 5 edits this same file).

- [ ] **Step 1: Write the panel script**

Create `game/scripts/coevolution_panel.gd`:

```gdscript
extends Control

# Gene↔culture co-evolution time-series. Reads the Rust per-tick history and
# draws a vertical stack of small-multiple charts sharing one time axis.
# Toggle with [Y]. (Read-only; no World mutation.)

@onready var sim = get_node("/root/Main/Simulation")

# Series grouped into stacked sub-charts. Each entry: {key, label, color}.
# unit "01" charts share a fixed [0,1] axis; "auto" charts self-scale.
const CHARTS := [
	{
		"title": "gene vs culture",
		"unit": "01",
		"series": [
			{"key": "communicator_frac", "label": "Communicator gene", "color": Color(0.35, 0.75, 1.0)},
			{"key": "mean_skill", "label": "skill meme", "color": Color(1.0, 0.75, 0.25)},
			{"key": "mean_social_learning", "label": "SocialLearning", "color": Color(0.55, 0.9, 0.55, 0.7)},
			{"key": "mean_individual_learning", "label": "IndividualLearning", "color": Color(0.9, 0.55, 0.85, 0.7)},
		],
	},
	{
		"title": "cultural divergence",
		"unit": "01",
		"series": [
			{"key": "meme_divergence", "label": "dialect L2", "color": Color(1.0, 0.4, 0.4)},
			{"key": "mean_tech_match", "label": "tech match", "color": Color(0.5, 1.0, 0.8)},
		],
	},
	{
		"title": "population",
		"unit": "auto",
		"series": [
			{"key": "live_count", "label": "alive", "color": Color(0.8, 0.8, 0.85)},
			{"key": "species_count", "label": "species", "color": Color(0.6, 0.7, 1.0)},
		],
	},
	{
		"title": "genetic diversity",
		"unit": "auto",
		"series": [
			{"key": "genetic_diversity", "label": "mean slot var", "color": Color(0.7, 0.9, 0.6)},
		],
	},
]

var _visible: bool = false
var _hidden_keys: Dictionary = {}          # key -> true when toggled off
var _scrub_index: int = -1                  # -1 = live (right edge)
var _font: Font

func _ready() -> void:
	visible = false
	_font = ThemeDB.fallback_font
	# Fill the right third of the screen; adjust in the scene as desired.
	set_anchors_preset(Control.PRESET_RIGHT_WIDE)

func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_Y:
		_visible = not _visible
		visible = _visible

func _process(_delta: float) -> void:
	if _visible:
		queue_redraw()

func _gui_input(event: InputEvent) -> void:
	# Click a legend row to toggle a series; click a chart to move the scrub cursor.
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		_handle_click(event.position)

func _handle_click(pos: Vector2) -> void:
	var n: int = sim.coevo_history_len()
	if n <= 0:
		return
	# Map x within the plot area to a sample index for scrubbing.
	var pad := 90.0
	var plot_w: float = max(1.0, size.x - pad - 10.0)
	if pos.x >= pad:
		var frac: float = clampf((pos.x - pad) / plot_w, 0.0, 1.0)
		_scrub_index = int(round(frac * float(n - 1)))

func _draw() -> void:
	var n: int = sim.coevo_history_len()
	draw_rect(Rect2(Vector2.ZERO, size), Color(0.05, 0.06, 0.09, 0.85))
	if n <= 1:
		draw_string(_font, Vector2(12, 24), "co-evolution — waiting for data…",
			HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color.WHITE)
		return

	var ticks: PackedFloat32Array = sim.coevo_series("tick")
	var pad := 90.0
	var plot_w: float = max(1.0, size.x - pad - 10.0)
	var chart_h: float = (size.y - 24.0) / float(CHARTS.size())
	var y0 := 20.0

	for c in CHARTS:
		_draw_chart(c, ticks, pad, plot_w, y0, chart_h - 8.0)
		y0 += chart_h

	# Scrub cursor + readout across the full height.
	if _scrub_index >= 0 and _scrub_index < n:
		var sx: float = pad + plot_w * (float(_scrub_index) / float(n - 1))
		draw_line(Vector2(sx, 16), Vector2(sx, size.y - 4), Color(1, 1, 1, 0.4), 1.0)
		_draw_readout(_scrub_index)

func _draw_chart(c: Dictionary, ticks: PackedFloat32Array, pad: float, plot_w: float,
		top: float, h: float) -> void:
	var n: int = ticks.size()
	# Determine y-scale.
	var vmax := 1.0
	var vmin := 0.0
	if c["unit"] == "auto":
		vmax = 0.0001
		for s in c["series"]:
			if _hidden_keys.has(s["key"]):
				continue
			var arr: PackedFloat32Array = sim.coevo_series(s["key"])
			for v in arr:
				vmax = max(vmax, v)
	# Frame + title.
	draw_rect(Rect2(Vector2(pad, top), Vector2(plot_w, h)), Color(1, 1, 1, 0.06))
	draw_string(_font, Vector2(pad + 4, top + 12), str(c["title"]),
		HORIZONTAL_ALIGNMENT_LEFT, -1, 11, Color(0.8, 0.85, 0.95))
	# Series polylines (adaptive downsample to pixel columns).
	var cols: int = int(min(plot_w, float(n)))
	var legend_y: float = top + 12.0
	for s in c["series"]:
		var key: String = s["key"]
		var col: Color = s["color"]
		var off: bool = _hidden_keys.has(key)
		# Legend row (right-aligned label; click toggles via _handle_click? — use labels col).
		draw_string(_font, Vector2(6, legend_y), s["label"], HORIZONTAL_ALIGNMENT_LEFT, 80, 10,
			Color(col.r, col.g, col.b, 0.35) if off else col)
		legend_y += 12.0
		if off:
			continue
		var arr: PackedFloat32Array = sim.coevo_series(key)
		if arr.size() < 2:
			continue
		var pts := PackedVector2Array()
		for cx in range(cols):
			var idx: int = int(float(cx) / float(max(1, cols - 1)) * float(n - 1))
			var v: float = arr[idx]
			var ny: float = clampf((v - vmin) / max(0.0001, (vmax - vmin)), 0.0, 1.0)
			var px: float = pad + (float(cx) / float(max(1, cols - 1))) * plot_w
			var py: float = top + h - ny * h
			pts.push_back(Vector2(px, py))
		if pts.size() >= 2:
			draw_polyline(pts, col, 1.5, true)

func _draw_readout(index: int) -> void:
	var s: Dictionary = sim.coevo_sample_at(index)
	if s.is_empty():
		return
	var lines := PackedStringArray()
	lines.append("t=%d" % int(s.get("tick", 0)))
	lines.append("comm=%.2f skill=%.2f" % [float(s.get("communicator_frac", 0)), float(s.get("mean_skill", 0))])
	lines.append("div=%.2f match=%.2f" % [float(s.get("meme_divergence", 0)), float(s.get("mean_tech_match", 0))])
	lines.append("alive=%d sp=%d" % [int(s.get("live_count", 0)), int(s.get("species_count", 0))])
	var box := Vector2(150, 8 + lines.size() * 13)
	var origin := Vector2(size.x - box.x - 8, 20)
	draw_rect(Rect2(origin, box), Color(0, 0, 0, 0.7))
	var y := origin.y + 14
	for ln in lines:
		draw_string(_font, Vector2(origin.x + 6, y), ln, HORIZONTAL_ALIGNMENT_LEFT, -1, 10, Color.WHITE)
		y += 13
```

(Legend click-to-toggle is wired minimally via `_handle_click` scrub; a follow-up may add per-row hit-testing. The `_hidden_keys` mechanism is in place for it. Keep this task's scope to draw + scrub + hotkey.)

- [ ] **Step 2: Add the node to the scene**

Edit `game/scenes/main.tscn`. Under the `UI` `CanvasLayer` node, add a `Control` child named `CoevolutionPanel` with `script = ExtResource` pointing at `res://scripts/coevolution_panel.gd`. Mirror how `PopulationPanel`/`DitPanel` are declared (ext_resource for the script, a node entry parented to `UI`). Minimal node block:

```
[ext_resource type="Script" path="res://scripts/coevolution_panel.gd" id="X_coevo"]

[node name="CoevolutionPanel" type="Control" parent="UI"]
anchors_preset = 6
anchor_left = 0.66
anchor_right = 1.0
anchor_bottom = 1.0
offset_left = 0.0
script = ExtResource("X_coevo")
```

(Use the next free `id` string; match the file's existing `ext_resource` id style. The panel hides itself in `_ready`, so it won't crowd the HUD until `Y` is pressed.)

- [ ] **Step 3: Headless boot check**

Run: `godot --headless --path game res://scenes/main.tscn --quit-after 180`
Expected: no parse errors, no runtime errors referencing `coevolution_panel.gd`; clean exit. (Surface for the user if `godot` isn't on PATH.)

- [ ] **Step 4: Manual visual check (windowed)**

Run: `godot --path game res://scenes/main.tscn` (or launch via the menu with the `gene-culture-skill` scenario).
Verify: press `Y` → panel appears on the right; over ~1000 ticks the "gene vs culture" chart shows `communicator_frac` and `mean_skill` lines moving; clicking a chart drops a scrub line with a value readout; press `Y` again → hides.

- [ ] **Step 5: Commit**

```bash
git add game/scripts/coevolution_panel.gd game/scenes/main.tscn
git commit -m "feat(godot): co-evolution time-series panel (charts + scrub + hotkey)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Codex event markers on the timeline

Overlay color-coded vertical lines at Speciation / MemeSweep / DialectFormed / Extinction ticks across all sub-charts, so a sweep visibly lines up with the curve that moved.

**Files:**
- Modify: `game/scripts/coevolution_panel.gd`

**Interfaces:**
- Consumes (Task 3): `sim.codex_events_since(cursor)`, `sim.codex_event_count()`. Consumes (Task 2): `sim.coevo_series("tick")` for tick→x mapping.
- Produces: markers drawn in `_draw`.

- [ ] **Step 1: Accumulate marker events**

Add to `coevolution_panel.gd` near the other vars:

```gdscript
# EventType ids we mark, → color. (0=Extinction, 2=Speciation, 11=DialectFormed, 12=MemeSweep.)
const MARKER_COLORS := {
	0: Color(1.0, 0.3, 0.3, 0.5),    # Extinction
	2: Color(0.6, 0.9, 1.0, 0.5),    # Speciation
	11: Color(1.0, 0.6, 0.2, 0.6),   # DialectFormed
	12: Color(1.0, 0.9, 0.3, 0.6),   # MemeSweep
}
var _marks: Array[Dictionary] = []       # {tick:int, type:int}
var _mark_cursor: int = 0
```

Add marker polling in `_process` (runs regardless of visibility so the log stays complete):

```gdscript
func _poll_marks() -> void:
	var evs: Array = sim.codex_events_since(_mark_cursor)
	for ev in evs:
		_mark_cursor = int(ev["index"]) + 1
		var t: int = int(ev["type"])
		if MARKER_COLORS.has(t):
			_marks.append({"tick": int(ev["tick"]), "type": t})
```

Call it at the top of `_process`:

```gdscript
func _process(_delta: float) -> void:
	_poll_marks()
	if _visible:
		queue_redraw()
```

(Independent `_mark_cursor` — the panel and `codex_panel.gd` each track their own cursor over the shared non-draining log, so both see every event.)

- [ ] **Step 2: Draw the markers**

In `_draw`, after the charts loop and before the scrub cursor, map each marker's tick to an x and draw a full-height line. Add a helper and call it:

```gdscript
func _draw_marks(ticks: PackedFloat32Array, pad: float, plot_w: float) -> void:
	var n: int = ticks.size()
	if n < 2 or _marks.is_empty():
		return
	var t_first: float = ticks[0]
	var t_last: float = ticks[n - 1]
	var span: float = max(1.0, t_last - t_first)
	for m in _marks:
		var mt: float = float(m["tick"])
		if mt < t_first or mt > t_last:
			continue
		var mx: float = pad + ((mt - t_first) / span) * plot_w
		draw_line(Vector2(mx, 16), Vector2(mx, size.y - 4), MARKER_COLORS[m["type"]], 1.0)
```

Call inside `_draw` (right after the `for c in CHARTS` loop):

```gdscript
	_draw_marks(ticks, pad, plot_w)
```

- [ ] **Step 3: Headless boot check**

Run: `godot --headless --path game res://scenes/main.tscn --quit-after 180`
Expected: clean boot, no errors.

- [ ] **Step 4: Manual visual check**

Run the `dialects` scenario windowed; press `Y`. Expected: yellow `MemeSweep` vertical lines appear (~tick 79 per prior metrics) and line up on the divergence/skill charts; blue `Speciation` lines appear where species split.

- [ ] **Step 5: Commit**

```bash
git add game/scripts/coevolution_panel.gd
git commit -m "feat(godot): codex event markers on the co-evolution timeline

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Full verification pass + legend hotkey doc

Final gate: determinism, CI, headless boot across the richest gene-culture scenarios, and add the `Y` hotkey to the legend so it's discoverable.

**Files:**
- Modify: `game/scripts/legend_panel.gd` (add the `[Y]` line)

**Interfaces:**
- Consumes: everything above. Produces: nothing new.

- [ ] **Step 1: Advertise the hotkey in the legend**

In `game/scripts/legend_panel.gd`, extend the expanded `label.text` to include the co-evolution toggle. Change the format string:

```gdscript
		label.text = (
			"[G] ground: %s\n[C] body: %s\n[Y] co-evolution chart\n[H] hide this\nWASD/drag pan · wheel zoom · click inspect"
		) % [GROUND_NAMES[g], BODY_NAMES[b]]
```

- [ ] **Step 2: Full CI gate (stable toolchain, matches CI)**

```bash
rustup run stable cargo fmt --all --check
rustup run stable cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc --workspace --no-deps --document-private-items
rustup run stable cargo test --workspace --lib --tests
```
Expected: all PASS. (If `fmt --check` fails, run `cargo fmt --all` and commit the result.)

- [ ] **Step 3: Determinism golden**

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: `minimal_scenario_matches_golden_hashes` PASS — hashes byte-identical, no refresh.

- [ ] **Step 4: Headless boot across scenarios**

For each of `minimal`, `gene-culture-skill`, `dialects`, `cooperation`, `dit-env-slow`:
Run: `godot --headless --path game res://scenes/main.tscn --quit-after 240`
(The scene boots the menu-default scenario; to exercise each, launch via the menu or set `game_config.gd` defaults.) Expected: clean boot, no script errors.

- [ ] **Step 5: Manual visual pass (the acceptance criterion)**

Windowed, run `gene-culture-skill` and `dialects`. Confirm the spec's success criterion: press `Y`, watch `communicator_frac` and `mean_skill` rise together, see a `MemeSweep`/`Speciation` marker land on the tick a curve jumps, scrub to read exact values, series legend colors correct. Note any visual issues for a follow-up (not a blocker for the instrument's correctness).

- [ ] **Step 6: Commit**

```bash
git add game/scripts/legend_panel.gd
git commit -m "docs(godot): advertise [Y] co-evolution chart in the legend

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- Read-only `coevo_metrics` reusing detector math → Task 1 (`species_max_meme_divergence` mirrors the `DialectFormed` kernel) + Task 2. ✅
- Per-tick history buffer on `Simulation` (outside `World`) sampled in `step_n` → Task 2. ✅
- Full signal set (gene-side, meme-side, population, all in the sample) → Task 1/2 helpers + Task 4 charts. ✅
- Codex event markers (Speciation/MemeSweep/DialectFormed/Extinction) → Task 3 (log) + Task 5 (draw). ✅
- Small-multiples stack, flagship overlay, legend toggles, scrub cursor, hotkey → Task 4. ✅
- Full-run history w/ adaptive downsampling + soft cap → Task 2 (`COEVO_HISTORY_CAP`, logged) + Task 4 (column decimation). ✅
- Determinism unchanged / no golden refresh → Global Constraints + Task 2 Step 5 + Task 6 Step 3. ✅
- Single codex drain site (no consumer race) → Task 3. ✅
- Tests: Rust unit tests, headless boot, manual pass, CI gate → Tasks 1/2/6. ✅
- Out-of-scope (biome/domestication/writing/other views) → not present. ✅

**Placeholder scan:** No TBD/TODO in requirements. Two acknowledged follow-ups are explicitly out of this cycle's scope (per-row legend hit-testing; per-scenario headless exercise via menu) and are noted, not left as blocking gaps.

**Type consistency:** Series keys are identical strings across `CoevoSample` fields, `coevo_series`/`coevo_sample_at` match arms, and the GDScript `CHARTS`/readout (`communicator_frac`, `mean_skill`, `mean_social_learning`, `mean_individual_learning`, `genetic_diversity`, `mean_tech_match`, `meme_divergence`, `live_count`, `species_count`, `env_optimum`, `tick`). Marker type ids (0/2/11/12) match the `EventType` enum. `codex_events_since`/`codex_event_count`/`_cursor`/`_mark_cursor` consistent between Task 3 and Tasks 3–5.
