# Big Five (OCEAN) Personality Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give agents heritable, normally-distributed Big Five personalities (each a signed `[-1,+1]` genome trait) that dictate concrete behavior through one hard-coded modulation pass.

**Architecture:** Repurpose 5 inert `GenomeSlot`s as OCEAN traits with signed accessors and Gaussian init. A new `personality.rs` owns the tuning constants and the formulas; `apply_personality` nudges each agent's `ActionRegister` in the decide stage, and two factor helpers modulate speed (integrate) and reproduction threshold. At neutral traits the pass is an identity, so wiring is determinism-neutral; only Gaussian init changes the golden hashes.

**Tech Stack:** Rust (`anabios-core`), deterministic seeded RNG.

## Global Constraints

- **Deterministic core.** At neutral traits (stored `0.5` → signed `0.0`) every personality effect MUST be an exact identity (all factors `1.0`, all biases `0`). Wiring the pass in (Tasks 1–3) MUST leave the three golden hashes byte-identical: `(0, 0x58807132956798b1)`, `(100, 0xa020c143eccfb4eb)`, `(1000, 0xfd21efef4e1619e4)`. Only Gaussian init (Task 4) changes them — that task carries the deliberate golden refresh.
- **Genome slot indices never move.** Rename variants in place; `GENOME_LEN = 50` unchanged. Stored values stay clamped `[0,1]`.
- **Signed trait mapping:** trait value `= 2·get(slot) − 1 ∈ [-1,+1]`.
- **All new sim code lives in `anabios-core`.**
- **CI gate — stable toolchain (matches CI):** `rustup run stable cargo fmt --all --check`; `rustup run stable cargo clippy --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc --workspace --no-deps --document-private-items`; `rustup run stable cargo test --workspace --lib --tests`. **Commit `cargo fmt` output.** Escape `[0,1]`/`[N]` as `` `[0,1]` `` in doc comments.
- **Tuning constants (verbatim):** in `personality.rs`: `K_O = 0.5`, `K_C = 0.5`, `K_E = 0.6`, `K_N = 0.8`, `K_A = 1.0`, `K_C_FEED = 0.5`, `COMFORT_FRAC = 0.5`, `N_DAMPEN = 0.5`. In `genome.rs`: `INIT_SIGMA = 0.2`.
- **Sentinels:** `program::NO_TARGET == u32::MAX`; `sense::NO_NEIGHBOR_ID == u32::MAX`. `SPAWN_ENERGY == 50.0` (`agent.rs`).

---

### Task 1: Genome — rename slots, signed accessors, Gaussian sampler

Rename 5 inert slots to OCEAN names (indices unchanged), add signed accessors and a Gaussian personality sampler.

**Files:**
- Modify: `crates/anabios-core/src/genome.rs`

**Interfaces:**
- Produces: `GenomeSlot::{Openness=12, Conscientiousness=21, Extraversion=13, Agreeableness=10, Neuroticism=11}`; `Genome::{openness, conscientiousness, extraversion, agreeableness, neuroticism}(&self) -> f32`; `Genome::sample_personality_in_place(&mut self, rng: &mut Rng)`; `pub const INIT_SIGMA: f32`.

- [ ] **Step 1: Rename the 5 slots in `GenomeSlot`**

In `crates/anabios-core/src/genome.rs`, change these enum lines (keep the `= N` indices exactly):

```rust
    // Drive levels (10..20)
    Agreeableness = 10,   // was Aggression; signed: +peaceful / -antagonistic
    Neuroticism = 11,     // was Fearfulness
    Openness = 12,        // was Curiosity
    Extraversion = 13,    // was SocialAffinity
    KinPreference = 14,
    Territoriality = 15,
```

and in the Behavioral block:

```rust
    ExploreVsExploit = 20,
    Conscientiousness = 21,   // was RiskTolerance
    AmbushPreference = 22,
```

- [ ] **Step 2: Fix the 5 in-file test references to the old names**

In the same file's `#[cfg(test)] mod tests`, the clamp/crossover tests use the old names as arbitrary slots. Update them:
- line ~258: `g.set(GenomeSlot::Aggression, -1.0);` → `g.set(GenomeSlot::Agreeableness, -1.0);`
- line ~259: `g.set(GenomeSlot::Curiosity, 2.0);` → `g.set(GenomeSlot::Openness, 2.0);`
- line ~260: `assert_eq!(g.get(GenomeSlot::Aggression), 0.0);` → `GenomeSlot::Agreeableness`
- line ~261: `assert_eq!(g.get(GenomeSlot::Curiosity), 1.0);` → `GenomeSlot::Openness`
- line ~340: `b.set(GenomeSlot::Aggression, 1.0);` → `GenomeSlot::Agreeableness`

- [ ] **Step 3: Write the failing tests (accessors + sampler)**

Add to the `#[cfg(test)] mod tests` block in `genome.rs`:

```rust
    #[test]
    fn personality_accessors_are_signed_minus1_to_plus1() {
        let mut g = Genome::neutral(); // all 0.5
        assert!((g.openness() - 0.0).abs() < 1e-6);
        assert!((g.agreeableness() - 0.0).abs() < 1e-6);
        g.set(GenomeSlot::Openness, 1.0);
        g.set(GenomeSlot::Neuroticism, 0.0);
        assert!((g.openness() - 1.0).abs() < 1e-6);
        assert!((g.neuroticism() - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn sample_personality_is_centered_clamped_and_varied() {
        let mut rng = crate::rng::Rng::from_seed(42);
        let mut sum = 0.0f32;
        let mut min = 1.0f32;
        let mut max = 0.0f32;
        let n = 2000;
        for _ in 0..n {
            let mut g = Genome::neutral();
            g.sample_personality_in_place(&mut rng);
            let v = g.get(GenomeSlot::Openness);
            assert!((0.0..=1.0).contains(&v));
            sum += v;
            min = min.min(v);
            max = max.max(v);
        }
        let mean = sum / n as f32;
        assert!((mean - 0.5).abs() < 0.05, "mean {mean} not ~0.5");
        assert!(max - min > 0.3, "spread too small: {min}..{max}");
        // Non-personality slot is untouched by the sampler.
        let mut g = Genome::neutral();
        g.sample_personality_in_place(&mut rng);
        assert_eq!(g.get(GenomeSlot::Size), 0.5);
    }
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `rustup run stable cargo test -p anabios-core --lib genome::`
Expected: FAIL to compile (`openness`, `sample_personality_in_place`, `INIT_SIGMA` undefined).

- [ ] **Step 5: Implement accessors, sampler, and constant**

Near the top constants of `genome.rs` (after `MUTATION_SIGMA_MAX`):

```rust
/// Std-dev for the Gaussian initial distribution of the 5 OCEAN personality
/// slots (stored space `[0,1]`, centered on the neutral 0.5).
pub const INIT_SIGMA: f32 = 0.2;
```

In `impl Genome` (near `get`/`set`):

```rust
    /// Openness in `[-1,+1]` (`2·slot − 1`). +1 novelty-seeking, −1 routine.
    pub fn openness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Openness) - 1.0
    }
    /// Conscientiousness in `[-1,+1]`. +1 prudent/careful, −1 impulsive.
    pub fn conscientiousness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Conscientiousness) - 1.0
    }
    /// Extraversion in `[-1,+1]`. +1 social/seeking, −1 solitary.
    pub fn extraversion(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Extraversion) - 1.0
    }
    /// Agreeableness in `[-1,+1]`. +1 cooperative/peaceful, −1 antagonistic.
    pub fn agreeableness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Agreeableness) - 1.0
    }
    /// Neuroticism in `[-1,+1]`. +1 anxious/reactive, −1 stable/bold.
    pub fn neuroticism(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Neuroticism) - 1.0
    }

    /// Overwrite the 5 OCEAN slots with `N(0.5, INIT_SIGMA)` clamped to `[0,1]`,
    /// giving a normally-distributed personality. Other slots are untouched.
    pub fn sample_personality_in_place(&mut self, rng: &mut Rng) {
        for slot in [
            GenomeSlot::Openness,
            GenomeSlot::Conscientiousness,
            GenomeSlot::Extraversion,
            GenomeSlot::Agreeableness,
            GenomeSlot::Neuroticism,
        ] {
            let v = rng.gaussian(0.5, INIT_SIGMA).clamp(0.0, 1.0);
            self.set(slot, v);
        }
    }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `rustup run stable cargo test -p anabios-core --lib genome::`
Expected: PASS (all genome tests, including the two new ones).

- [ ] **Step 7: Format, lint, commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/genome.rs
git commit -m "feat(core): rename 5 inert slots to OCEAN traits + signed accessors + Gaussian sampler

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: `personality.rs` — modulation pass + factor helpers

The formulas: `apply_personality` (E/N/A/C-feed on the `ActionRegister`) plus `personality_speed_factor` (O) and `personality_reproduction_factor` (C). Pure, deterministic, unit-tested.

**Files:**
- Create: `crates/anabios-core/src/personality.rs`
- Modify: `crates/anabios-core/src/lib.rs` (add `pub mod personality;`)

**Interfaces:**
- Consumes: `Genome` accessors (Task 1); `program::{ActionRegister, NO_TARGET}`; `sense::{SensorRegister, NO_NEIGHBOR_ID}`; `agent::SPAWN_ENERGY`.
- Produces: `apply_personality(action: &mut ActionRegister, genome: &Genome, sensors: &SensorRegister, energy: f32)`; `personality_speed_factor(genome: &Genome) -> f32`; `personality_reproduction_factor(genome: &Genome) -> f32`.

- [ ] **Step 1: Register the module**

In `crates/anabios-core/src/lib.rs`, add alongside the other `pub mod` lines:

```rust
pub mod personality;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/anabios-core/src/personality.rs` with (tests first, impl in Step 4):

```rust
//! Big Five (OCEAN) personality: hard-coded modulation of an agent's action
//! intents from its signed `[-1,+1]` personality traits. At neutral traits
//! (value 0.0) every function here is an exact identity, so wiring it into the
//! pipeline is determinism-neutral until genomes are given non-neutral traits.

use crate::agent::SPAWN_ENERGY;
use crate::genome::Genome;
use crate::program::{ActionRegister, NO_TARGET};
use crate::sense::{SensorRegister, NO_NEIGHBOR_ID};

/// Openness → movement-speed gain (applied in `integrate`).
pub const K_O: f32 = 0.5;
/// Conscientiousness → reproduction-threshold gain (applied in `reproduce`).
pub const K_C: f32 = 0.5;
/// Extraversion → same-species approach bias + broadcast gain.
pub const K_E: f32 = 0.6;
/// Neuroticism → flee bias from other-species neighbors.
pub const K_N: f32 = 0.8;
/// Agreeableness → same-species attack suppression gain.
pub const K_A: f32 = 1.0;
/// Conscientiousness → feed-intent boost when below comfort energy.
pub const K_C_FEED: f32 = 0.5;
/// Comfort energy fraction (of `SPAWN_ENERGY`) below which C boosts feeding.
pub const COMFORT_FRAC: f32 = 0.5;
/// Neuroticism → feed/mate dampening under threat.
pub const N_DAMPEN: f32 = 0.5;

/// Movement-speed multiplier from Openness. `1.0` at neutral.
pub fn personality_speed_factor(genome: &Genome) -> f32 {
    (1.0 + K_O * genome.openness()).max(0.0)
}

/// Reproduction energy-threshold multiplier from Conscientiousness. `1.0` at neutral.
pub fn personality_reproduction_factor(genome: &Genome) -> f32 {
    (1.0 + K_C * genome.conscientiousness()).max(0.0)
}

/// Modulate an action from personality + current percepts (E, N, A, C-feed).
/// Openness (speed) and Conscientiousness (repro threshold) are applied at
/// their own sites via the factor helpers above.
pub fn apply_personality(
    action: &mut ActionRegister,
    genome: &Genome,
    sensors: &SensorRegister,
    energy: f32,
) {
    let c = genome.conscientiousness();
    let e = genome.extraversion();
    let a = genome.agreeableness();
    let n = genome.neuroticism();

    // Extraversion: bias movement toward the nearest same-species neighbor and
    // scale broadcasts. (Introverts, e<0, bias away.)
    if sensors.nearest_same_id != NO_NEIGHBOR_ID {
        action.move_x += K_E * e * sensors.nearest_same_dir.x;
        action.move_y += K_E * e * sensors.nearest_same_dir.y;
    }
    let bcast = (1.0 + K_E * e.max(0.0)).max(0.0);
    for ch in action.broadcast_intent.iter_mut() {
        *ch *= bcast;
    }

    // Neuroticism: flee nearby other-species neighbors; dampen feed/mate under threat.
    if sensors.nearest_other_id != NO_NEIGHBOR_ID {
        let flee = K_N * n.max(0.0);
        action.move_x -= flee * sensors.nearest_other_dir.x;
        action.move_y -= flee * sensors.nearest_other_dir.y;
        let damp = (1.0 - N_DAMPEN * n.max(0.0)).max(0.0);
        action.feed_intent *= damp;
        action.mate_intent *= damp;
    }

    // Agreeableness: scale sharing (+A shares more, −A none); suppress attacks on
    // kin (+A peaceful → ×0; −A antagonistic → up to ×2).
    action.share_intent *= (1.0 + a).clamp(0.0, 2.0);
    if sensors.nearest_same_id != NO_NEIGHBOR_ID
        && action.target_id != NO_TARGET
        && action.target_id == sensors.nearest_same_id
    {
        action.fire_intent *= (1.0 - K_A * a).clamp(0.0, 2.0);
    }

    // Conscientiousness: boost feeding when below comfort energy (provisioning).
    if energy < COMFORT_FRAC * SPAWN_ENERGY {
        action.feed_intent *= 1.0 + K_C_FEED * c.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::GenomeSlot;
    use crate::prelude::Vec2;

    fn neutral() -> Genome {
        Genome::neutral()
    }

    #[test]
    fn identity_at_neutral_traits() {
        let g = neutral();
        assert!((personality_speed_factor(&g) - 1.0).abs() < 1e-6);
        assert!((personality_reproduction_factor(&g) - 1.0).abs() < 1e-6);
        let mut a = ActionRegister::default();
        a.feed_intent = 1.0;
        a.fire_intent = 1.0;
        a.share_intent = 1.0;
        a.move_x = 0.3;
        let before = a;
        let mut s = SensorRegister::default();
        s.nearest_same_id = 7;
        s.nearest_other_id = 9;
        s.nearest_same_dir = Vec2::new(1.0, 0.0);
        s.nearest_other_dir = Vec2::new(0.0, 1.0);
        a.target_id = 7;
        apply_personality(&mut a, &g, &s, SPAWN_ENERGY); // energy above comfort
        assert!((a.feed_intent - before.feed_intent).abs() < 1e-6);
        assert!((a.fire_intent - before.fire_intent).abs() < 1e-6);
        assert!((a.share_intent - before.share_intent).abs() < 1e-6);
        assert!((a.move_x - before.move_x).abs() < 1e-6);
    }

    #[test]
    fn openness_and_conscientiousness_factors_scale_with_trait() {
        let mut g = neutral();
        g.set(GenomeSlot::Openness, 1.0);
        assert!(personality_speed_factor(&g) > 1.0);
        g.set(GenomeSlot::Openness, 0.0);
        assert!(personality_speed_factor(&g) < 1.0);
        let mut g2 = neutral();
        g2.set(GenomeSlot::Conscientiousness, 1.0);
        assert!(personality_reproduction_factor(&g2) > 1.0);
    }

    #[test]
    fn extraversion_biases_toward_same_neighbor() {
        let mut g = neutral();
        g.set(GenomeSlot::Extraversion, 1.0);
        let mut a = ActionRegister::default();
        let mut s = SensorRegister::default();
        s.nearest_same_id = 3;
        s.nearest_same_dir = Vec2::new(1.0, 0.0);
        apply_personality(&mut a, &g, &s, SPAWN_ENERGY);
        assert!(a.move_x > 0.0, "extravert should bias toward same neighbor");
    }

    #[test]
    fn agreeableness_raises_share_and_suppresses_kin_fire() {
        let mut hi = neutral();
        hi.set(GenomeSlot::Agreeableness, 1.0);
        let mut a = ActionRegister::default();
        a.share_intent = 1.0;
        a.fire_intent = 1.0;
        a.target_id = 5;
        let mut s = SensorRegister::default();
        s.nearest_same_id = 5;
        apply_personality(&mut a, &hi, &s, SPAWN_ENERGY);
        assert!(a.share_intent > 1.0, "agreeable shares more");
        assert!(a.fire_intent < 1.0, "agreeable suppresses kin attack");

        let mut lo = neutral();
        lo.set(GenomeSlot::Agreeableness, 0.0); // antagonistic
        let mut a2 = ActionRegister::default();
        a2.fire_intent = 1.0;
        a2.target_id = 5;
        apply_personality(&mut a2, &lo, &s, SPAWN_ENERGY);
        assert!(a2.fire_intent > 1.0, "antagonist attacks kin more");
    }

    #[test]
    fn neuroticism_flees_other_species_and_dampens_feed() {
        let mut g = neutral();
        g.set(GenomeSlot::Neuroticism, 1.0);
        let mut a = ActionRegister::default();
        a.feed_intent = 1.0;
        let mut s = SensorRegister::default();
        s.nearest_other_id = 8;
        s.nearest_other_dir = Vec2::new(1.0, 0.0);
        apply_personality(&mut a, &g, &s, SPAWN_ENERGY);
        assert!(a.move_x < 0.0, "neurotic flees away from other-species");
        assert!(a.feed_intent < 1.0, "neurotic dampens feeding under threat");
    }

    #[test]
    fn conscientiousness_boosts_feed_when_hungry() {
        let mut g = neutral();
        g.set(GenomeSlot::Conscientiousness, 1.0);
        let mut a = ActionRegister::default();
        a.feed_intent = 1.0;
        let s = SensorRegister::default();
        apply_personality(&mut a, &g, &s, 0.1 * SPAWN_ENERGY); // below comfort
        assert!(a.feed_intent > 1.0, "conscientious boosts feeding when hungry");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `rustup run stable cargo test -p anabios-core --lib personality::`
Expected: FAIL to compile (functions not yet defined — they are in the same file's non-test section, so this step confirms the test file compiles once impl is present; if the impl block above is included, tests should build).

(The impl and tests are in the same Step-2 file. If you split them, add the impl now.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `rustup run stable cargo test -p anabios-core --lib personality::`
Expected: PASS — 6 tests ok.

- [ ] **Step 5: Format, lint, doc, commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc -p anabios-core --no-deps --document-private-items
git add crates/anabios-core/src/personality.rs crates/anabios-core/src/lib.rs
git commit -m "feat(core): personality modulation pass + speed/reproduction factor helpers

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Wire personality into the tick pipeline (determinism-neutral)

Hook the three call sites. Because traits are still neutral-init here, this MUST NOT change the golden hashes — that is the verification.

**Files:**
- Modify: `crates/anabios-core/src/tick.rs` (`decide_all`)
- Modify: `crates/anabios-core/src/integrate.rs` (`integrate_all`)
- Modify: `crates/anabios-core/src/reproduce.rs` (threshold site ~line 156)

**Interfaces:**
- Consumes: Task 2's three functions.

- [ ] **Step 1: Hook `apply_personality` in `decide_all`**

In `crates/anabios-core/src/tick.rs`, `decide_all`, change the loop body so the action is mutable and modulated before normalization:

```rust
        let mut action = decide(
            &world.agents.program[i],
            &world.agents.genome[i],
            &world.sensors[i],
            &world.agents.meme_vector[i],
            world.agents.energy[i],
            world.agents.age[i],
            &mut world.eval_stack,
        );
        crate::personality::apply_personality(
            &mut action,
            &world.agents.genome[i],
            &world.sensors[i],
            world.agents.energy[i],
        );
        // Normalize the movement intent to a unit direction (identical to the
        // pre-M11 logic that lived inside `decide`).
        let v = Vec2::new(action.move_x, action.move_y);
        let len = v.length();
        world.desired_direction[i] = if len < 1e-4 { Vec2::ZERO } else { v / len };
        world.actions[i] = action;
```

- [ ] **Step 2: Hook `personality_speed_factor` in `integrate_all`**

In `crates/anabios-core/src/integrate.rs`, in the moving branch, multiply speed by the factor:

```rust
        let direction = desired_direction[i];
        let module_speed = crate::module::effective_speed_max(&agents.modules[i]).clamp(0.0, 1.0);
        let speed_factor = crate::personality::personality_speed_factor(&agents.genome[i]);
        let v = direction * (SPEED_MAX_CAP * module_speed * speed_factor);
        agents.velocity[i] = v;
```

- [ ] **Step 3: Hook `personality_reproduction_factor` in `reproduce.rs`**

At the threshold computation (~line 156):

```rust
    let threshold = SPAWN_ENERGY
        * agents.genome[i].get(GenomeSlot::ReproductionThreshold)
        * 1.5
        * crate::personality::personality_reproduction_factor(&agents.genome[i]);
    agents.energy[i] >= threshold
```

- [ ] **Step 4: Verify the golden hashes are UNCHANGED**

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS — `minimal_scenario_matches_golden_hashes` still green (neutral traits → identity pass → byte-identical hashes). If it FAILS, a modulation is not a true identity at neutral; fix the formula (do NOT refresh hashes in this task).

- [ ] **Step 5: Run the full core suite**

Run: `rustup run stable cargo test -p anabios-core --lib --tests`
Expected: PASS.

- [ ] **Step 6: Format, lint, commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/tick.rs crates/anabios-core/src/integrate.rs crates/anabios-core/src/reproduce.rs
git commit -m "feat(core): wire personality pass into decide/integrate/reproduce (golden unchanged)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Gaussian personality init + TraitOverrides + golden refresh

Give every spawned agent a normally-distributed personality. This changes evolution → deliberate golden-hash refresh.

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs` (`instantiate`, `TraitOverrides` struct + `apply`)
- Modify: `crates/anabios-core/tests/determinism.rs` (refreshed hashes)

**Interfaces:**
- Consumes: `Genome::sample_personality_in_place` (Task 1).
- Produces: `TraitOverrides.{openness, conscientiousness, extraversion, agreeableness, neuroticism}: Option<f32>` (stored `[0,1]`).

- [ ] **Step 1: Sample personality at spawn**

In `crates/anabios-core/src/scenario.rs` `instantiate`, in the per-agent loop, insert the sampler right after `let mut g = Genome::neutral();` and before `archetype_genome`:

```rust
            let mut g = Genome::neutral();
            g.sample_personality_in_place(&mut w.rng);
            if let Some(name) = &spec.archetype {
                archetype_genome(name, &mut g);
            }
            spec.traits.apply(&mut g);
```

(Sampling before `traits.apply` means an explicit personality override in a scenario wins over the random draw — Step 2.)

- [ ] **Step 2: Extend `TraitOverrides`**

Add fields to the `TraitOverrides` struct:

```rust
    /// Big Five personality overrides (stored `[0,1]`; `0.5` = neutral/0.0
    /// signed). When present, they pin the slot instead of the random draw.
    pub openness: Option<f32>,
    pub conscientiousness: Option<f32>,
    pub extraversion: Option<f32>,
    pub agreeableness: Option<f32>,
    pub neuroticism: Option<f32>,
```

And in `TraitOverrides::apply`, add:

```rust
        if let Some(v) = self.openness { g.set(GenomeSlot::Openness, v); }
        if let Some(v) = self.conscientiousness { g.set(GenomeSlot::Conscientiousness, v); }
        if let Some(v) = self.extraversion { g.set(GenomeSlot::Extraversion, v); }
        if let Some(v) = self.agreeableness { g.set(GenomeSlot::Agreeableness, v); }
        if let Some(v) = self.neuroticism { g.set(GenomeSlot::Neuroticism, v); }
```

- [ ] **Step 3: Confirm the golden test now FAILS (expected)**

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: FAIL — hashes changed (personality draws + non-neutral behavior). This is expected and intended.

- [ ] **Step 4: Regenerate the golden hashes**

Run: `UPDATE_HASHES=1 rustup run stable cargo test -p anabios-core --test determinism -- --nocapture`
Copy the printed `(tick, 0x...)` values into `crates/anabios-core/tests/determinism.rs` `GOLDEN`, replacing the three old constants. Update the surrounding comment to note: "Refreshed 2026-07-14 for Big Five personality (Gaussian init)."

- [ ] **Step 5: Verify determinism holds against the refreshed hashes**

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS. Run it a second time — still PASS (reproducible; the new state is deterministic).

- [ ] **Step 6: Full core suite + format + commit**

Run: `rustup run stable cargo test -p anabios-core --lib --tests`
Expected: PASS.

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/scenario.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): Gaussian personality init for all scenarios + trait overrides (golden refresh)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Behavioral integration tests (personality dictates behavior)

Prove at population scale that pinned traits produce the predicted differences. Same-seed high-vs-low comparisons so any difference is attributable to the trait. Covers Openness (dispersal), Extraversion (clustering), Conscientiousness (energy). Neuroticism/Agreeableness directional correctness is covered by the Task 2 unit tests (their population-scale emergence needs combat/two-species tuning and is left to scenario exploration).

**Files:**
- Create: `crates/anabios-core/tests/personality_behavior.rs`

**Interfaces:**
- Consumes: `Scenario::parse_toml`, `tick::step`, `Genome` accessors, `SensorRegister`.

- [ ] **Step 1: Write the integration tests**

Create `crates/anabios-core/tests/personality_behavior.rs`:

```rust
//! Population-scale proof that pinned OCEAN traits change behavior. Each test
//! runs two same-seed populations differing only in one pinned trait and
//! asserts the predicted difference in an aggregate metric.

use anabios_core::prelude_test::Vec2;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

fn run(toml: &str, ticks: u64) -> anabios_core::world::World {
    let mut w = Scenario::parse_toml(toml).expect("parse").instantiate();
    for _ in 0..ticks {
        step(&mut w);
    }
    w
}

fn mean_dist_from_centroid(w: &anabios_core::world::World) -> f32 {
    let ids: Vec<u32> = w.agents.iter_alive().collect();
    if ids.is_empty() {
        return 0.0;
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for &id in &ids {
        let p = w.agents.position[id as usize];
        cx += p.x;
        cy += p.y;
    }
    let n = ids.len() as f32;
    let c = Vec2::new(cx / n, cy / n);
    let mut s = 0.0;
    for &id in &ids {
        s += (w.agents.position[id as usize] - c).length();
    }
    s / n
}

fn mean_crowding(w: &anabios_core::world::World) -> f32 {
    let ids: Vec<u32> = w.agents.iter_alive().collect();
    if ids.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    for &id in &ids {
        s += w.sensors[id as usize].crowding as f32;
    }
    s / ids.len() as f32
}

fn mean_energy(w: &anabios_core::world::World) -> f32 {
    let ids: Vec<u32> = w.agents.iter_alive().collect();
    if ids.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    for &id in &ids {
        s += w.agents.energy[id as usize];
    }
    s / ids.len() as f32
}

fn scenario(trait_line: &str) -> String {
    format!(
        "name = \"p\"\nseed = 7\n\n[[agents]]\ncount = 120\nplacement = {{ kind = \"cluster\", center_x = 512.0, center_y = 512.0, radius = 80.0 }}\n[agents.traits]\n{trait_line}\n"
    )
}

#[test]
fn openness_increases_dispersal() {
    let hi = run(&scenario("openness = 0.95"), 300);
    let lo = run(&scenario("openness = 0.05"), 300);
    let (dh, dl) = (mean_dist_from_centroid(&hi), mean_dist_from_centroid(&lo));
    assert!(dh > dl, "high-O dispersal {dh} should exceed low-O {dl}");
}

#[test]
fn extraversion_increases_clustering() {
    let hi = run(&scenario("extraversion = 0.95"), 300);
    let lo = run(&scenario("extraversion = 0.05"), 300);
    let (ch, cl) = (mean_crowding(&hi), mean_crowding(&lo));
    assert!(ch > cl, "high-E crowding {ch} should exceed low-E {cl}");
}

#[test]
fn conscientiousness_raises_mean_energy() {
    let hi = run(&scenario("conscientiousness = 0.95"), 300);
    let lo = run(&scenario("conscientiousness = 0.05"), 300);
    let (eh, el) = (mean_energy(&hi), mean_energy(&lo));
    assert!(eh > el, "high-C mean energy {eh} should exceed low-C {el}");
}
```

- [ ] **Step 2: Run to verify they fail first (guard against tautology), then pass**

Run: `rustup run stable cargo test -p anabios-core --test personality_behavior`
Expected: PASS. If any assertion fails, the effect direction or gain is wrong — inspect the metric and, only if a formula is genuinely off, adjust the relevant `K_*` in `personality.rs` (re-run Task 4 golden refresh if a constant changes) — do not weaken an assertion to force a pass. If a run goes extinct (0 alive), lengthen `radius`/lower `ticks` so the metric is meaningful, and note it.

- [ ] **Step 3: Format, commit**

```bash
rustup run stable cargo fmt --all
git add crates/anabios-core/tests/personality_behavior.rs
git commit -m "test(core): behavioral integration tests for OCEAN traits (dispersal/clustering/energy)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Full verification pass

Final CI gate across the workspace; confirm determinism against the refreshed hashes.

**Files:** none (verification only).

- [ ] **Step 1: Full CI gate (stable toolchain)**

```bash
rustup run stable cargo fmt --all --check
rustup run stable cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc --workspace --no-deps --document-private-items
rustup run stable cargo test --workspace --lib --tests
```
Expected: all PASS. (If `fmt --check` fails, `cargo fmt --all` and commit.)

- [ ] **Step 2: Determinism reproducibility**

Run: `rustup run stable cargo test -p anabios-core --test determinism` (twice)
Expected: PASS both times against the refreshed hashes.

- [ ] **Step 3: Sanity — personality varies and evolves**

Run a scenario headless and confirm non-degenerate personality spread:
`rustup run stable cargo run -p anabios-headless --release -- run --scenario scenarios/divergent.toml --ticks 500 --seed 0`
Expected: clean run, non-zero alive. (Spot-check via a temporary print of `genome.openness()` spread is optional — remove any debug print before committing.)

- [ ] **Step 4: Commit any formatting-only changes**

```bash
git add -A
git commit -m "chore(core): final formatting for Big Five personality" || echo "nothing to commit"
```

---

## Self-Review

**Spec coverage:**
- Repurpose 5 inert slots as signed OCEAN traits → Task 1 (rename + accessors). ✅
- `[-1,+1]` mapping (`2·g−1`) → Task 1 accessors + test. ✅
- Gaussian distribution, default all scenarios → Task 4 (`sample_personality_in_place` in `instantiate`). ✅
- Heritability/evolution → free via existing crossover + `mutate_in_place` (all slots); no task needed, noted. ✅
- Hard-coded modulation per trait (O/C/E/A/N) → Task 2 formulas + Task 3 wiring. ✅
- Openness via speed in integrate (not magnitude) → Task 2 `personality_speed_factor` + Task 3 Step 2. ✅
- C via reproduction threshold → Task 2 `personality_reproduction_factor` + Task 3 Step 3. ✅
- `TraitOverrides` for pinning → Task 4 Step 2. ✅
- Identity-at-neutral / golden unchanged on wiring; refresh only on init → Task 3 Step 4 + Task 4 Steps 3–5. ✅
- Unit tests (mapping, sampler, each modulation direction) → Tasks 1–2. ✅
- Behavioral integration tests → Task 5 (O/E/C fully; N/A directional at unit level — **conscious scope reduction** for test robustness, since population-scale N/A need combat/two-species tuning; flagged, not silently dropped). ✅ (partial — see note)
- Golden refresh → Task 4. ✅ CI gate → Task 6. ✅

**Placeholder scan:** No TBD/TODO in requirements. Task 5's N/A integration reduction is explicit, not a placeholder.

**Type consistency:** Slot names (`Openness/Conscientiousness/Extraversion/Agreeableness/Neuroticism`) identical across Tasks 1/2/4. Function names (`apply_personality`, `personality_speed_factor`, `personality_reproduction_factor`, `sample_personality_in_place`) and signatures identical across Tasks 2/3/1/4. Constants (`K_O/K_C/K_E/K_N/K_A/K_C_FEED/COMFORT_FRAC/N_DAMPEN`, `INIT_SIGMA`) match Global Constraints. Sentinels (`NO_TARGET`, `NO_NEIGHBOR_ID`) used consistently.
