# M-A: Subcortical Framework + SEEKING — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the deterministic subcortical affect substrate for anabios and wire the first Panksepp system (SEEKING) end-to-end: a Layer-0 homeostatic energy drive, a serialized per-agent Layer-1 activation column, an `affect::develop_all` compute stage (post-sense / pre-decide, zero RNG), the `affect_enabled` world/scenario flag, the ~5 heritable temperament genes in reserved genome slots, an `apply_affect` bias hook, and the SEEKING movement-speed factor — all opt-in and byte-identical when the flag is off.

**Architecture:** New `affect.rs` module holds all Layer-0/Layer-1 logic + read-side hooks + factor helpers (mirroring `iq.rs`). A serialized `AgentBuffers.affect: Vec<[f32;7]>` column stores per-agent activations. `affect::develop_all(world)` runs each tick between `sense_all` and `decide_all`, updating SEEK as a leaky integrator of the energy deficit (index-disjoint `par_iter`, zero RNG, strict no-op when the flag is off). `affect::apply_affect` biases the `ActionRegister` right after `apply_personality`; `affect::affect_speed_factor` scales movement speed in `integrate.rs`. Temperament genes reuse reserved slots 17/18/19/34/35 with signed `[-1,+1]` accessors. Determinism is preserved by the established playbook: flag-off byte-identity, serialized (never `#[serde(skip)]`) state, a one-time golden refresh for layout growth, and a `FORMAT_VERSION` bump.

**Tech Stack:** Rust, anabios-core, rayon, serde/bincode, cargo test.

## Global Constraints
- Single Xoshiro256++ RNG (`world.rng`); the affect stage draws ZERO RNG (even flag-on), preserving the tick's RNG draw order.
- Flag-off (`!world.affect_enabled`) is byte-identical: `develop_all` early-returns as a strict no-op, and every read-side hook is exact identity at neutral (all-zero) affect.
- Neutral-identity on the read side is guarded `if x != 0.0` (personality.rs idiom); factor helpers use `1.0 + k·x` and are exactly `1.0` at neutral.
- Affect state is serialized (never `#[serde(skip)]`) — it is a path-dependent accumulator feeding hashed movement (the still-ticks v13 footgun).
- Append enum variants at END only (no new enum variants in M-A; temperament reuses reserved slots, no index shift).
- Golden refresh via `UPDATE_HASHES=1`, done ONCE for layout growth, with a dated "flag off ⇒ byte-identical" changelog note.
- Controller (not implementer subagent) runs all cargo/git/golden-refresh/commit gates.
- Adding the serialized `affect` column (Task 3) turns the three pre-existing golden tests (`determinism.rs`, `cognition.rs`, `inventions.rs`) RED from that point on. This is expected layout drift; do NOT "fix" them mid-stream — they are refreshed exactly once, deliberately, in Task 9. Every intermediate task's "expect PASS" refers to that task's OWN new test.

---

## File Structure

**Created**
- `crates/anabios-core/src/affect.rs` — Layer-0 drive + Layer-1 leaky-integrator activations, read-side hooks (`apply_affect`), factor helpers (`affect_speed_factor`, `affect_reproduction_factor`), `homeostatic_drive`, `arousal`, `develop_all`. Constants + system indices.
- `scenarios/affect-seeking.toml` — flag-ON scenario (`affect_enabled = true`) that exercises SEEKING; the flag-ON golden anchor.
- `crates/anabios-core/tests/affect.rs` — flag-ON golden hashes + parse assertion + self-consistency + save→load→step for the affect column.

**Modified**
- `crates/anabios-core/src/lib.rs` — register `pub mod affect;`.
- `crates/anabios-core/src/genome.rs` — rename reserved slots 17/18/19/34/35 to Boldness/Aggressiveness/Nurturance/Sociality/Reactivity; signed accessors; `SLOT_NAMES`; tests (names + distance).
- `crates/anabios-core/src/agent.rs` — serialized `affect: Vec<AffectState>` column; init in both spawn branches.
- `crates/anabios-core/src/world.rs` — `affect_enabled: bool` field + `World::new` default.
- `crates/anabios-core/src/scenario.rs` — `affect_enabled` field + `instantiate()` threading + test.
- `crates/anabios-core/src/tick.rs` — call `affect::develop_all` between sense and decide; call `affect::apply_affect` after `apply_personality`.
- `crates/anabios-core/src/integrate.rs` — multiply speed by `affect::affect_speed_factor`.
- `crates/anabios-core/src/snapshot.rs` — bump `FORMAT_VERSION` 23→24 + changelog line.
- `crates/anabios-core/tests/determinism.rs` — refresh minimal golden (layout); add `affect-seeking.toml` to the parallel-vs-serial scenario set.
- `crates/anabios-core/tests/cognition.rs` — refresh cognitive golden (layout).
- `crates/anabios-core/tests/inventions.rs` — refresh inventions golden (layout).

---

## Task 1 — `affect.rs` module skeleton: constants, drive, arousal, factors

**Files:**
- Create `crates/anabios-core/src/affect.rs` (constants + pure functions + `#[cfg(test)] mod tests`).
- Modify `crates/anabios-core/src/lib.rs` (register the module; alphabetical slot is between `age` and `agent` — place `pub mod affect;` right after `pub mod age;`).
- Test: `crates/anabios-core/src/affect.rs` unit tests.

**Interfaces:**
- Produces: `AFFECT_SYSTEMS: usize`, `SEEK/FEAR/RAGE/LUST/CARE/PANIC/PLAY: usize`, `HIJACK_AROUSAL_THRESHOLD: f32`, `LAMBDA_DEFAULT: f32`, `type AffectState = [f32; AFFECT_SYSTEMS]`, `fn homeostatic_drive(energy: f32) -> f32`, `fn arousal(affect: &AffectState) -> f32`, `fn affect_speed_factor(affect: &AffectState) -> f32`, `fn affect_reproduction_factor(_affect: &AffectState) -> f32`.
- Consumes: `crate::agent::SPAWN_ENERGY` (full-path; no `use`).

Steps:

- [ ] **Step 1 — Write failing test.** Create `crates/anabios-core/src/affect.rs` with the module doc + a `#[cfg(test)] mod tests` containing:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homeostatic_drive_is_zero_when_sated_and_one_when_empty() {
        let e = crate::agent::SPAWN_ENERGY;
        assert_eq!(homeostatic_drive(e), 0.0);
        assert_eq!(homeostatic_drive(0.0), 1.0);
        assert!((homeostatic_drive(e * 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(homeostatic_drive(e * 2.0), 0.0, "surplus clamps to 0");
    }

    #[test]
    fn arousal_is_zero_at_neutral_and_maxes_the_defensive_systems() {
        assert_eq!(arousal(&[0.0; AFFECT_SYSTEMS]), 0.0);
        let mut a = [0.0; AFFECT_SYSTEMS];
        a[FEAR] = 0.4;
        a[RAGE] = 0.7;
        a[PANIC] = 0.1;
        assert!((arousal(&a) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn factors_are_identity_at_neutral() {
        let neutral = [0.0; AFFECT_SYSTEMS];
        assert_eq!(affect_speed_factor(&neutral), 1.0);
        assert_eq!(affect_reproduction_factor(&neutral), 1.0);
        let mut a = neutral;
        a[SEEK] = 1.0;
        assert!(affect_speed_factor(&a) > 1.0, "SEEKING speeds foraging up");
    }
}
```
Also add `pub mod affect;` to `lib.rs` right after `pub mod age;`.

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core affect::tests` — fails to compile: `cannot find value AFFECT_SYSTEMS`/`function homeostatic_drive not found` (the items above the test module do not exist yet).

- [ ] **Step 3 — Minimal impl.** Prepend to `affect.rs` (above the test module):
```rust
//! Subcortical affect layer (Panksepp primary-process systems) — Layer 0
//! homeostatic drive + Layer 1 leaky-integrator activations that bias (and, in
//! later milestones, override) the evolved program. Gated on
//! `World::affect_enabled`: with the flag off `develop_all` is a strict no-op,
//! every read-side hook is exact identity at neutral (all-zero) affect, and the
//! stage draws ZERO RNG — so a flag-off world is byte-identical.
//!
//! Biological framing (design spec §1.2): functional/temporal layering
//! (subsumption), NOT evolutionary strata. We model survival/motivational
//! circuits and make no claim that agents *feel* anything.

/// Number of Panksepp primary-process systems tracked per agent.
pub const AFFECT_SYSTEMS: usize = 7;

// Activation indices into `AffectState`.
pub const SEEK: usize = 0;
pub const FEAR: usize = 1;
pub const RAGE: usize = 2;
pub const LUST: usize = 3;
pub const CARE: usize = 4;
pub const PANIC: usize = 5; // PANIC/GRIEF (separation distress)
pub const PLAY: usize = 6;

/// Hijack fires when threat arousal >= this (before Reactivity modulation). M-B.
pub const HIJACK_AROUSAL_THRESHOLD: f32 = 0.6;

/// Per-system leaky-integrator retention (how long an activation lingers).
pub const LAMBDA_DEFAULT: f32 = 0.8;

/// SEEKING forage-bias gain: how hard a hungry agent steers toward `plant_direction`.
pub const K_SEEK_FORAGE: f32 = 0.6;
/// SEEKING wander gain: intensification of the program's own heading when no
/// food is sensed. A deterministic, RNG-free exploratory proxy — the read-side
/// hook has neither position nor RNG to synthesize a fresh wander vector, so it
/// amplifies whatever direction the evolved program already chose.
pub const K_SEEK_WANDER: f32 = 0.3;
/// Movement-speed gain from SEEKING (+ arousal, which is 0 in M-A).
pub const K_AFFECT_SPEED: f32 = 0.5;

/// Per-agent subcortical activations, one per Panksepp system, each in [0,1].
/// Persistent (serialized). Neutral default = all zero.
pub type AffectState = [f32; AFFECT_SYSTEMS];

/// Layer-0 homeostatic drive: normalized energy deficit in [0,1]. 0 = sated
/// (energy >= SPAWN_ENERGY), → 1 as energy → 0. Setpoint is `SPAWN_ENERGY`.
#[inline]
pub fn homeostatic_drive(energy: f32) -> f32 {
    let setpoint = crate::agent::SPAWN_ENERGY;
    ((setpoint - energy) / setpoint).clamp(0.0, 1.0)
}

/// Aggregate threat arousal from the defensive activations (FEAR, RAGE, PANIC).
/// M-A: those stay 0.0, so this is a 0.0 baseline; M-B finalizes it with the
/// hijack.
#[inline]
pub fn arousal(affect: &AffectState) -> f32 {
    affect[FEAR].max(affect[RAGE]).max(affect[PANIC])
}

/// Movement-speed multiplier from SEEKING + arousal. Exactly `1.0` at neutral
/// (all-zero) affect. Consumed in integrate.rs alongside personality_speed_factor.
#[inline]
pub fn affect_speed_factor(affect: &AffectState) -> f32 {
    (1.0 + K_AFFECT_SPEED * affect[SEEK]).max(0.0)
}

/// Reproduction-threshold multiplier from LUST. Exactly `1.0` at neutral.
/// M-A ships this identity stub; M-C implements the LUST effect and wires it
/// into reproduce.rs.
#[inline]
pub fn affect_reproduction_factor(_affect: &AffectState) -> f32 {
    1.0
}
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core affect::tests`.

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/affect.rs crates/anabios-core/src/lib.rs` + `git commit -m "affect: Layer-0 drive + affect constants/factors (M-A skeleton)"`.

---

## Task 2 — Temperament genes: rename reserved slots + signed accessors

**Files:**
- Modify `crates/anabios-core/src/genome.rs`: enum variants (lines 68-70, 110-111), `SLOT_NAMES` (indices 17/18/19/34/35 at lines 165-167, 182-183), accessors (after `cognitive_potential()` ~line 340), tests (SLOT_NAMES test ~443-462; new distance/accessor tests).
- Test: `crates/anabios-core/src/genome.rs` `mod tests`.

**Interfaces:**
- Produces: `GenomeSlot::{Boldness=17, Aggressiveness=18, Nurturance=19, Sociality=34, Reactivity=35}`; `fn boldness(&self)->f32`, `fn aggressiveness(&self)->f32`, `fn nurturance(&self)->f32`, `fn sociality(&self)->f32`, `fn reactivity(&self)->f32` (each `2·slot − 1`, neutral `0.5`→`0.0`).
- Speciation decision (recorded here): temperament genes **count** toward `Genome::distance` — they are NOT added to `PERSONALITY_MASK`. Slots 17/18/19/34/35 are not personality slots, so `distance()` already includes them; the new test locks that in.

Steps:

- [ ] **Step 1 — Write failing test.** Add to `genome.rs` `mod tests`:
```rust
    #[test]
    fn temperament_slot_names_align_with_the_enum() {
        assert_eq!(SLOT_NAMES[GenomeSlot::Boldness.idx()], "Boldness");
        assert_eq!(SLOT_NAMES[GenomeSlot::Aggressiveness.idx()], "Aggressiveness");
        assert_eq!(SLOT_NAMES[GenomeSlot::Nurturance.idx()], "Nurturance");
        assert_eq!(SLOT_NAMES[GenomeSlot::Sociality.idx()], "Sociality");
        assert_eq!(SLOT_NAMES[GenomeSlot::Reactivity.idx()], "Reactivity");
    }

    #[test]
    fn temperament_accessors_are_signed_minus1_to_plus1() {
        let mut g = Genome::neutral(); // all 0.5 → 0.0 signed
        assert!(g.boldness().abs() < 1e-6);
        assert!(g.reactivity().abs() < 1e-6);
        g.set(GenomeSlot::Reactivity, 1.0);
        assert!((g.reactivity() - 1.0).abs() < 1e-6);
        g.set(GenomeSlot::Aggressiveness, 0.0);
        assert!((g.aggressiveness() + 1.0).abs() < 1e-6);
    }

    #[test]
    fn temperament_counts_toward_speciation_distance() {
        // Recorded M-A decision: temperament is adaptive → it counts toward
        // distance (unlike the OCEAN personality slots, which are masked out).
        let a = Genome::neutral();
        let mut b = Genome::neutral();
        b.set(GenomeSlot::Boldness, 1.0);
        assert!(a.distance(&b) > 0.0, "temperament (Boldness) must count toward distance");
        // A pure personality change still contributes nothing.
        let mut c = Genome::neutral();
        c.set(GenomeSlot::Openness, 1.0);
        assert_eq!(a.distance(&c), 0.0, "personality (Openness) stays excluded from distance");
    }
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core genome::tests::temperament` — fails: `no variant named Boldness`/`no method named boldness`.

- [ ] **Step 3 — Minimal impl.** In `genome.rs`:
  - Replace the enum lines 68-70:
```rust
    /// Boldness (temperament, signed via accessor): +1 bold (low FEAR setpoint /
    /// high freeze threshold), −1 timid. Read by the affect layer. Counts toward
    /// speciation distance. Inert when `World::affect_enabled` is false.
    Boldness = 17,
    /// Aggressiveness (temperament): +1 high RAGE gain, −1 placid.
    Aggressiveness = 18,
    /// Nurturance (temperament): +1 high CARE gain, −1 neglectful.
    Nurturance = 19,
```
  - Replace the enum lines 110-111:
```rust
    /// Sociality (temperament): +1 strongly bonded (PANIC/PLAY/CARE bond weight),
    /// −1 solitary. Read by the affect layer; counts toward speciation distance.
    Sociality = 34,
    /// Reactivity (temperament): +1 high arousal gain / low hijack threshold /
    /// slow decay, −1 phlegmatic. Read by the affect layer.
    Reactivity = 35,
```
  - In `SLOT_NAMES` set index 17→`"Boldness"`, 18→`"Aggressiveness"`, 19→`"Nurturance"`, 34→`"Sociality"`, 35→`"Reactivity"` (replacing the `reserved_*` strings at those positions).
  - Add accessors right after `cognitive_potential()`:
```rust
    /// Boldness in `[-1,+1]` (`2·slot − 1`). +1 bold, −1 timid. Neutral `0.5`→`0.0`.
    pub fn boldness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Boldness) - 1.0
    }
    /// Aggressiveness in `[-1,+1]`. +1 aggressive (high RAGE gain), −1 placid.
    pub fn aggressiveness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Aggressiveness) - 1.0
    }
    /// Nurturance in `[-1,+1]`. +1 nurturing (high CARE gain), −1 neglectful.
    pub fn nurturance(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Nurturance) - 1.0
    }
    /// Sociality in `[-1,+1]`. +1 bonded, −1 solitary.
    pub fn sociality(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Sociality) - 1.0
    }
    /// Reactivity in `[-1,+1]`. +1 reactive (high arousal gain), −1 phlegmatic.
    pub fn reactivity(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Reactivity) - 1.0
    }
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core genome::tests` (the new tests plus the pre-existing `slot_names_align_with_the_enum` must all pass; the rename does not change any slot value or index, so serialization is byte-identical).

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/genome.rs` + `git commit -m "genome: temperament genes (Boldness/Aggressiveness/Nurturance/Sociality/Reactivity) in reserved slots"`.

---

## Task 3 — Serialized per-agent `affect` column in `AgentBuffers`

**Files:**
- Modify `crates/anabios-core/src/agent.rs`: struct field (after `iq_enrich_ticks`, line 70), reuse-branch init (after line 168), push-branch init (after line 192).
- Test: `crates/anabios-core/src/agent.rs` `mod tests`.

**Interfaces:**
- Produces: `AgentBuffers.affect: Vec<crate::affect::AffectState>`, initialized to `[0.0; AFFECT_SYSTEMS]` in both spawn branches. Serialized (the struct derives Serialize/Deserialize; the field is NOT `#[serde(skip)]`).
- Reset semantics mirror `iq` exactly: the dead-slot reset is the spawn *reuse* branch (`kill` does not zero serialized columns; readers guard on `alive`).

Steps:

- [ ] **Step 1 — Write failing test.** Add to `agent.rs` `mod tests`:
```rust
    #[test]
    fn spawn_zeroes_affect() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(
            Vec2::ZERO, neutral(), 1, [LINEAGE_NONE; 2], 0,
            crate::module::starter_kit(), Program::empty(), false,
        );
        assert_eq!(a.affect[id as usize], [0.0; crate::affect::AFFECT_SYSTEMS]);
        assert_eq!(a.affect.len(), a.capacity(), "affect grows in lockstep with capacity");
    }

    #[test]
    fn reused_slot_resets_affect() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(
            Vec2::ZERO, neutral(), 1, [LINEAGE_NONE; 2], 0,
            crate::module::starter_kit(), Program::empty(), false,
        );
        a.affect[id0 as usize][crate::affect::SEEK] = 0.9;
        a.kill(id0);
        let id1 = a.spawn(
            Vec2::ZERO, neutral(), 2, [LINEAGE_NONE; 2], 0,
            crate::module::starter_kit(), Program::empty(), false,
        );
        assert_eq!(id1, id0, "LIFO free list reuses slot 0");
        assert_eq!(
            a.affect[id1 as usize], [0.0; crate::affect::AFFECT_SYSTEMS],
            "reused (dead) slot resets affect to neutral"
        );
    }
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core agent::tests::spawn_zeroes_affect agent::tests::reused_slot_resets_affect` — fails: `no field affect on type AgentBuffers`.

- [ ] **Step 3 — Minimal impl.** In `agent.rs`:
  - Add the field to `AgentBuffers` right after `pub iq_enrich_ticks: Vec<u32>,` (line 70):
```rust
    /// Per-agent subcortical affect activations (7 Panksepp systems, each in
    /// `[0,1]`), developed each tick by `affect::develop_all` and read by the
    /// affect bias hooks. Neutral default `[0.0; AFFECT_SYSTEMS]`; stays all-zero
    /// when `World::affect_enabled` is false, so it is cost/identity-neutral
    /// there. Serialized (persistent) — a path-dependent accumulator feeding
    /// hashed movement, so it must NOT be `#[serde(skip)]` (still-ticks v13 footgun).
    pub affect: Vec<crate::affect::AffectState>,
```
  - In the reuse branch, after `self.iq_enrich_ticks[i] = 0;` (line 168): `self.affect[i] = [0.0; crate::affect::AFFECT_SYSTEMS];`
  - In the push branch, after `self.iq_enrich_ticks.push(0);` (line 192): `self.affect.push([0.0; crate::affect::AFFECT_SYSTEMS]);`

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core agent::tests` (new tests green; the whole crate still compiles). Note: the three golden tests (`determinism.rs`, `cognition.rs`, `inventions.rs`) go RED here from layout growth — expected, refreshed in Task 9. Do not touch them.

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/agent.rs` + `git commit -m "agent: serialized per-agent affect column (init both spawn branches)"`.

---

## Task 4 — `World::affect_enabled` flag

**Files:**
- Modify `crates/anabios-core/src/world.rs`: field (after the `cognition_enabled` block, ~line 92), `World::new` default (after `cognition_enabled: false,`, line 329).
- Test: `crates/anabios-core/src/world.rs` `mod tests` (line 530).

**Interfaces:**
- Produces: `World.affect_enabled: bool`, `#[serde(default)]`, default `false` in `World::new`.

Steps:

- [ ] **Step 1 — Write failing test.** Add to `world.rs` `mod tests`:
```rust
    #[test]
    fn affect_enabled_defaults_off() {
        let w = World::new(1);
        assert!(!w.affect_enabled, "affect layer is opt-in; off by default");
    }
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core world::tests::affect_enabled_defaults_off` — fails: `no field affect_enabled on type World`.

- [ ] **Step 3 — Minimal impl.** In `world.rs`:
  - Add the field immediately after the `cognition_enabled` field block (after line 92):
```rust
    /// When true, the subcortical affect layer is active: `affect::develop_all`
    /// updates per-agent Panksepp activations and the affect bias hooks steer
    /// behavior. Off by default; opt-in per scenario. When false the affect stage
    /// is a strict no-op (zero RNG) and every read-side hook is exact identity, so
    /// a flag-off world is byte-identical. Same bincode/`FORMAT_VERSION` caveat as
    /// `env_period`.
    #[serde(default)]
    pub affect_enabled: bool,
```
  - Add `affect_enabled: false,` to `World::new` immediately after `cognition_enabled: false,` (line 329).

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core world::tests::affect_enabled_defaults_off`.

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/world.rs` + `git commit -m "world: affect_enabled opt-in flag (default off)"`.

---

## Task 5 — Scenario threading + flag-ON scenario TOML

**Files:**
- Modify `crates/anabios-core/src/scenario.rs`: `Scenario` field (after `cognition_enabled`, line 55), `instantiate()` copy (after `w.cognition_enabled = self.cognition_enabled;`, line 433), test in `mod tests`.
- Create `scenarios/affect-seeking.toml`.
- Test: `crates/anabios-core/src/scenario.rs` `mod tests`.

**Interfaces:**
- Produces: `Scenario.affect_enabled: bool` (`#[serde(default)]`); `instantiate()` sets `w.affect_enabled = self.affect_enabled;`.
- Consumes: `World.affect_enabled` (Task 4).

Steps:

- [ ] **Step 1 — Write failing test.** Add to `scenario.rs` `mod tests`:
```rust
    #[test]
    fn affect_enabled_defaults_off_and_scenario_applies() {
        // Omitting the field leaves it off (serde default) for baseline identity.
        let base = "name = \"base\"\nseed = 1\n\n[[agents]]\ncount = 5\n[agents.traits]\n";
        let s0 = Scenario::parse_toml(base).expect("parse");
        assert!(!s0.affect_enabled);
        assert!(!s0.instantiate().affect_enabled);
        // Setting it propagates into the instantiated world.
        let on = "name = \"on\"\nseed = 1\naffect_enabled = true\n\n[[agents]]\ncount = 5\n[agents.traits]\n";
        let s1 = Scenario::parse_toml(on).expect("parse");
        assert!(s1.affect_enabled);
        assert!(s1.instantiate().affect_enabled);
    }
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core scenario::tests::affect_enabled_defaults_off_and_scenario_applies` — fails: `no field affect_enabled on type Scenario` (and `deny_unknown_fields` would reject `affect_enabled = true` until the field exists).

- [ ] **Step 3 — Minimal impl.** In `scenario.rs`:
  - Add the field after the `cognition_enabled` field (after line 55):
```rust
    /// Opt-in: enable the subcortical affect layer (per-agent Panksepp activations
    /// developed each tick; SEEKING biases foraging in M-A). `false` (default)
    /// leaves behavior byte-identical.
    #[serde(default)]
    pub affect_enabled: bool,
```
  - Add to `instantiate()` immediately after `w.cognition_enabled = self.cognition_enabled;` (line 433): `w.affect_enabled = self.affect_enabled;`
  - Create `scenarios/affect-seeking.toml`:
```toml
name = "affect-seeking"
seed = 12345
# Flag-ON anchor for the M-A affect layer: identical world to minimal.toml, but
# with SEEKING active so hungry agents forage toward food. The golden test in
# tests/affect.rs pins the resulting flag-ON trajectory.
affect_enabled = true
# Pin the cap so the flag-ON golden test stays fast and stable.
max_population = 2000

[[agents]]
count = 200
placement = { kind = "uniform" }

[agents.traits]
perception_radius = 0.5
size = 0.4
basal_metabolism = 0.4
lifespan_bias = 0.6
reproduction_threshold = 0.5
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core scenario::tests::affect_enabled_defaults_off_and_scenario_applies`.

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/scenario.rs scenarios/affect-seeking.toml` + `git commit -m "scenario: affect_enabled threading + affect-seeking.toml flag-on anchor"`.

---

## Task 6 — `affect::develop_all` compute stage + tick wiring

**Files:**
- Modify `crates/anabios-core/src/affect.rs`: add `use crate::world::World;` + `develop_all` + tests.
- Modify `crates/anabios-core/src/tick.rs`: insert the stage between sense (line 33) and decide (line 36).
- Test: `crates/anabios-core/src/affect.rs` `mod tests`.

**Interfaces:**
- Produces: `fn develop_all(world: &mut World)` — strict no-op when `!world.affect_enabled`; else index-disjoint `par_iter` over the `affect` column updating SEEK as a leaky integrator of `homeostatic_drive(energy)`; ZERO RNG; other six systems untouched (stay 0.0).
- Consumes: `World.affect_enabled`, `AgentBuffers.{affect, energy, alive}`, `homeostatic_drive`, `LAMBDA_DEFAULT`, `SEEK`.

Steps:

- [ ] **Step 1 — Write failing test.** Add to `affect.rs` `mod tests` (add `use crate::genome::Genome; use crate::prelude::Vec2; use crate::world::World;` inside the test module as needed):
```rust
    #[test]
    fn develop_is_noop_when_flag_off() {
        let mut w = World::new(2);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = 0.0; // would build SEEK if the layer ran
        develop_all(&mut w);
        assert_eq!(
            w.agents.affect[id as usize], [0.0; AFFECT_SYSTEMS],
            "flag off ⇒ affect untouched, zero work"
        );
    }

    #[test]
    fn seeking_builds_from_energy_deficit_when_on() {
        let mut w = World::new(2);
        w.affect_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = 0.0; // max drive = 1.0
        develop_all(&mut w);
        // One tick from neutral: seek = λ·0 + (1−λ)·1.0 = (1−λ).
        let seek = w.agents.affect[id as usize][SEEK];
        assert!((seek - (1.0 - LAMBDA_DEFAULT)).abs() < 1e-6);
    }

    #[test]
    fn sated_agent_builds_no_seek() {
        let mut w = World::new(2);
        w.affect_enabled = true;
        // Spawn energy == SPAWN_ENERGY ⇒ drive 0 ⇒ SEEK stays 0.
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        develop_all(&mut w);
        assert_eq!(w.agents.affect[id as usize][SEEK], 0.0, "sated ⇒ no SEEKING");
    }
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core affect::tests::seeking_builds_from_energy_deficit_when_on` — fails: `function develop_all not found in module affect`.

- [ ] **Step 3 — Minimal impl.** In `affect.rs`, add `use crate::world::World;` near the top (below the module doc), then append:
```rust
/// Compute stage (Layer 0 → Layer 1). Update each alive agent's affect column
/// from this tick's physiology as a leaky integrator. M-A drives SEEK from the
/// homeostatic energy deficit; the other six systems stay 0.0 (later milestones
/// fill their triggers — FEAR reads sensors in M-B, etc.). STRICT no-op when
/// `!world.affect_enabled`. ZERO RNG. Index-disjoint `par_iter` (iq::develop_all
/// template): each agent writes only its own slot and reads only shared columns
/// by `&`, so the parallel loop is bit-identical to a serial ascending-id loop.
/// Runs post-sense / pre-decide so THIS tick's decision reads fresh affect.
pub fn develop_all(world: &mut World) {
    if !world.affect_enabled {
        return;
    }
    use rayon::prelude::*;
    let cap = world.agents.capacity();
    let crate::agent::AgentBuffers { affect, energy, alive, .. } = &mut world.agents;
    let (energy, alive) = (&*energy, &*alive);
    affect[..cap].par_iter_mut().enumerate().for_each(|(i, a)| {
        if !alive[i] {
            return;
        }
        // Layer 0: homeostatic drive (energy deficit) powers SEEKING.
        let drive = homeostatic_drive(energy[i]);
        // Layer 1: leaky-integrator update of the SEEK activation.
        let seek = LAMBDA_DEFAULT * a[SEEK] + (1.0 - LAMBDA_DEFAULT) * drive;
        a[SEEK] = seek.clamp(0.0, 1.0);
    });
}
```
In `tick.rs`, insert between the `sense_all(...)` call (ends line 33) and the `// Stage 3: decide.` comment (line 35):
```rust

    // Stage 2b: subcortical affect — update per-agent Panksepp activations from
    // this tick's physiology (SEEKING from the energy deficit in M-A). Runs
    // AFTER sense and BEFORE decide so the decision reads fresh affect. Strict
    // no-op + zero RNG when `affect_enabled` is false.
    crate::affect::develop_all(world);
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core affect::tests`.

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/affect.rs crates/anabios-core/src/tick.rs` + `git commit -m "affect: develop_all SEEKING compute stage wired post-sense/pre-decide"`.

---

## Task 7 — `affect::apply_affect` SEEKING bias hook + decide wiring

**Files:**
- Modify `crates/anabios-core/src/affect.rs`: add `use crate::{genome::Genome, prelude::Vec2, program::ActionRegister, sense::SensorRegister};` + `apply_affect` + tests.
- Modify `crates/anabios-core/src/tick.rs`: call `apply_affect` in `decide_all` right after `apply_personality` (after line 192).
- Test: `crates/anabios-core/src/affect.rs` `mod tests`.

**Interfaces:**
- Produces: `fn apply_affect(action: &mut ActionRegister, affect: &AffectState, genome: &Genome, sensors: &SensorRegister, energy: f32)` — EXACT identity at neutral (all-zero) affect (guard `if seek != 0.0`). SEEKING steers `move_x/move_y` toward `sensors.plant_direction` when food is sensed, else intensifies the program's own heading. Writes only live channels. No RNG.
- Consumes: `AffectState`, `SEEK`, `K_SEEK_FORAGE`, `K_SEEK_WANDER`, `sensors.plant_direction`.
- Note: `genome` and `energy` params are part of the fixed contract signature but unused in M-A (later milestones read temperament + energy) → bind as `_genome`/`_energy` to stay clippy-clean; the public type signature is unchanged.

Steps:

- [ ] **Step 1 — Write failing test.** Add to `affect.rs` `mod tests` (add `use crate::agent::SPAWN_ENERGY; use crate::program::ActionRegister; use crate::sense::SensorRegister;` as needed):
```rust
    #[test]
    fn apply_affect_is_identity_at_neutral() {
        let mut action = ActionRegister { move_x: 0.3, move_y: -0.2, ..Default::default() };
        let before = action; // ActionRegister is Copy
        let affect = [0.0; AFFECT_SYSTEMS];
        let s = SensorRegister { plant_direction: Vec2::new(1.0, 0.0), ..Default::default() };
        apply_affect(&mut action, &affect, &Genome::neutral(), &s, SPAWN_ENERGY);
        assert_eq!(action.move_x, before.move_x);
        assert_eq!(action.move_y, before.move_y);
    }

    #[test]
    fn seeking_biases_toward_sensed_food() {
        let mut action = ActionRegister::default();
        let mut affect = [0.0; AFFECT_SYSTEMS];
        affect[SEEK] = 1.0;
        let s = SensorRegister { plant_direction: Vec2::new(1.0, 0.0), ..Default::default() };
        apply_affect(&mut action, &affect, &Genome::neutral(), &s, 0.0);
        assert!(action.move_x > 0.0, "high SEEKING steers toward food (+x)");
    }

    #[test]
    fn seeking_intensifies_heading_when_no_food() {
        let mut action = ActionRegister { move_x: 0.5, move_y: 0.0, ..Default::default() };
        let mut affect = [0.0; AFFECT_SYSTEMS];
        affect[SEEK] = 1.0;
        let s = SensorRegister::default(); // plant_direction == Vec2::ZERO
        apply_affect(&mut action, &affect, &Genome::neutral(), &s, 0.0);
        assert!(action.move_x > 0.5, "no-food SEEKING intensifies the program's heading");
    }
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core affect::tests::seeking_biases_toward_sensed_food` — fails: `function apply_affect not found`.

- [ ] **Step 3 — Minimal impl.** In `affect.rs`, extend the imports to `use crate::{genome::Genome, prelude::Vec2, program::ActionRegister, sense::SensorRegister, world::World};`, then append:
```rust
/// Read-side bias hook. Modulate `action` from current affect + percepts +
/// temperament. EXACT IDENTITY at neutral (all-zero) affect — the SEEKING block
/// is guarded `if seek != 0.0` (personality.rs idiom), so a neutral agent's
/// action is left bit-for-bit unchanged. Called in decide_all right AFTER
/// `apply_personality`. Writes only live channels; no RNG. M-A implements
/// SEEKING; later milestones add their systems (they will read `genome`/`energy`).
pub fn apply_affect(
    action: &mut ActionRegister,
    affect: &AffectState,
    _genome: &Genome,
    sensors: &SensorRegister,
    _energy: f32,
) {
    // SEEKING: steer toward food when a plant direction is sensed; otherwise
    // intensify the program's own heading as a deterministic exploratory wander
    // (no RNG/position is available in the read-side hook).
    let seek = affect[SEEK];
    if seek != 0.0 {
        let pd = sensors.plant_direction;
        if pd != Vec2::ZERO {
            action.move_x += K_SEEK_FORAGE * seek * pd.x;
            action.move_y += K_SEEK_FORAGE * seek * pd.y;
        } else {
            let gain = 1.0 + K_SEEK_WANDER * seek;
            action.move_x *= gain;
            action.move_y *= gain;
        }
    }
}
```
In `tick.rs` `decide_all`, immediately after the `crate::personality::apply_personality(...)` call (ends line 192), add:
```rust
            // Subcortical affect bias (SEEKING in M-A): steer/intensify movement
            // from the agent's current affect. Exact identity at neutral affect.
            crate::affect::apply_affect(
                &mut action,
                &agents.affect[i],
                &agents.genome[i],
                &sensors[i],
                agents.energy[i],
            );
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core affect::tests`.

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/affect.rs crates/anabios-core/src/tick.rs` + `git commit -m "affect: apply_affect SEEKING bias hook wired after apply_personality"`.

---

## Task 8 — `affect_speed_factor` consumed in `integrate.rs`

**Files:**
- Modify `crates/anabios-core/src/integrate.rs`: add `affect` to the `AgentBuffers` destructure (lines 43-56); multiply the speed by `affect::affect_speed_factor` (line 97).
- Test: `crates/anabios-core/src/integrate.rs` `mod tests`.

**Interfaces:**
- Consumes: `affect::affect_speed_factor(&affect[i])` — exactly `1.0` at neutral affect, so a flag-off world (affect all-zero) is byte-identical (multiplying by exactly `1.0` is IEEE-exact).

Steps:

- [ ] **Step 1 — Write failing test.** Add to `integrate.rs` `mod tests`:
```rust
    #[test]
    fn seeking_raises_effective_speed() {
        // Two identical max-speed agents; the one with a SEEKING activation must
        // travel farther under the same unit direction.
        let displacement = |seek: f32| -> f32 {
            let mut w = World::new(1);
            let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
            for m in w.agents.modules[id as usize].iter_mut() {
                if let crate::module::Module::Locomotor { max_speed, .. } = m {
                    *max_speed = 1.0;
                }
            }
            w.agents.affect[id as usize][crate::affect::SEEK] = seek;
            let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
            desired[id as usize] = Vec2::new(1.0, 0.0);
            let before = w.agents.position[id as usize];
            integrate_all(&mut w.agents, &desired, w.world_size, false, false);
            (w.agents.position[id as usize] - before).length()
        };
        let neutral = displacement(0.0);
        let seeking = displacement(1.0);
        assert!(seeking > neutral, "SEEKING boosts movement speed: {neutral} -> {seeking}");
    }
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core integrate::tests::seeking_raises_effective_speed` — fails: with `affect` not in the destructure and the factor not applied, `neutral == seeking` (assert fails), or a compile error referencing `affect`.

- [ ] **Step 3 — Minimal impl.** In `integrate.rs`:
  - Add `affect` to the destructure (line 43-54) and the shared-ref tuple (line 55-56):
```rust
    let AgentBuffers {
        position,
        velocity,
        energy,
        modules,
        genome,
        meme_vector,
        iq,
        affect,
        sex,
        alive,
        ..
    } = agents;
    let (modules, genome, meme_vector, iq, affect, sex, alive) =
        (&*modules, &*genome, &*meme_vector, &*iq, &*affect, &*sex, &*alive);
```
  - After `let speed_factor = crate::personality::personality_speed_factor(&genome[i]);` (line 90) add:
```rust
            // Affect movement-speed factor (SEEKING + arousal). Exactly 1.0 at
            // neutral affect, so a flag-off world stays byte-identical.
            let affect_speed = crate::affect::affect_speed_factor(&affect[i]);
```
  - Change the velocity line (line 97) to append `* affect_speed` at the END (multiplying by exactly `1.0` at neutral is IEEE-exact, preserving byte-identity):
```rust
            let v = direction
                * (SPEED_MAX_CAP * module_speed * speed_factor * inv_speed * affect_speed);
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core integrate::tests`.

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/integrate.rs` + `git commit -m "integrate: apply affect_speed_factor (SEEKING) beside personality speed"`.

---

## Task 9 — `FORMAT_VERSION` bump + one-time flag-off golden refresh

**Files:**
- Modify `crates/anabios-core/src/snapshot.rs`: bump `FORMAT_VERSION` 23→24 (line 102) + a `///` changelog entry (before line 102).
- Modify `crates/anabios-core/tests/determinism.rs`: refresh `GOLDEN` (line 162) + dated changelog note.
- Modify `crates/anabios-core/tests/cognition.rs`: refresh `COGNITIVE_GOLDEN` (line 93) + dated changelog note.
- Modify `crates/anabios-core/tests/inventions.rs`: refresh `INVENTIONS_GOLDEN` (line 1045) + dated changelog note.
- Test: the three golden tests themselves (they become the gate).

**Interfaces:**
- Consumes: the serialized layout as of Task 8 (affect column + `affect_enabled` flag). Flag-off behavior is byte-identical; only the payload grew — so all three goldens move exactly once and for layout reasons only.

Steps:

- [ ] **Step 1 — Add the version changelog + bump.** In `snapshot.rs`, add above `pub const FORMAT_VERSION` (line 102):
```rust
/// v24: M-A subcortical affect layer — AgentBuffers.affect column (7 serialized
///      f32 per agent), World.affect_enabled flag, and genome temperament slots
///      17/18/19/34/35 renamed in place (Boldness/Aggressiveness/Nurturance/
///      Sociality/Reactivity — values/indices unchanged, so genome bytes are
///      identical). affect_enabled off in every existing golden scenario ⇒
///      develop_all no-ops (zero RNG), read-side hooks are identity, and the
///      speed factor is exactly 1.0 — behavior byte-identical; only the
///      serialized layout grew.
pub const FORMAT_VERSION: u32 = 24;
```
(Change `23` → `24`.)

- [ ] **Step 2 — Run the goldens, expect FAIL (layout drift).** `cargo test -p anabios-core --test determinism --test cognition --test inventions` — the three `*_matches_golden_hashes` tests fail with hash drift (expected: the new `affect` column grew every payload).

- [ ] **Step 3 — Regenerate + paste.** Controller runs each with `UPDATE_HASHES=1` and pastes the printed tuples back into the corresponding `const`:
  - `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes -- --nocapture` → paste into `GOLDEN` (determinism.rs:162).
  - `UPDATE_HASHES=1 cargo test -p anabios-core --test cognition cognitive_scenario_matches_golden_hashes -- --nocapture` → paste into `COGNITIVE_GOLDEN` (cognition.rs:93).
  - `UPDATE_HASHES=1 cargo test -p anabios-core --test inventions <inventions_golden_test_name> -- --nocapture` → paste into `INVENTIONS_GOLDEN` (inventions.rs:1045).
  Add one dated changelog line above each `const`, e.g.:
```rust
// Refreshed 2026-08-02 (M-A affect layer, FORMAT_VERSION 23→24): AgentBuffers
// gained the serialized `affect` column and World gained `affect_enabled`.
// affect_enabled is off here, so develop_all no-ops, the read-side hooks are
// identity, and the speed factor is exactly 1.0 — trajectory byte-identical,
// only the serialized layout grew, so all pinned hashes moved once.
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core --test determinism --test cognition --test inventions`. If any hash moved for a NON-layout reason, that is a determinism bug — stop and investigate (do not paste through a real drift).

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/src/snapshot.rs crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/cognition.rs crates/anabios-core/tests/inventions.rs` + `git commit -m "snapshot: FORMAT_VERSION 24 + one-time golden refresh for affect column (layout growth, flag off byte-identical)"`.

---

## Task 10 — Flag-ON golden + save→load→step + parallel coverage (`tests/affect.rs`)

**Files:**
- Create `crates/anabios-core/tests/affect.rs`.
- Modify `crates/anabios-core/tests/determinism.rs`: add `scenarios/affect-seeking.toml` to the `parallel_matches_serial_across_thread_counts` scenario array (lines 179-182).
- Test: the new integration tests.

**Interfaces:**
- Consumes: `scenarios/affect-seeking.toml` (Task 5), the fully-wired affect layer (Tasks 6-8), snapshot save/load (Task 9's `FORMAT_VERSION`).
- Produces: a pinned flag-ON golden (`AFFECT_GOLDEN`) that locks SEEKING behavior; a save→load→step equality test proving the serialized `affect` column survives a round-trip; parallel-vs-serial coverage of the flag-on paths.

Steps:

- [ ] **Step 1 — Write failing test.** Create `crates/anabios-core/tests/affect.rs`:
```rust
//! End-to-end determinism for the flag-ON affect scenario. `determinism.rs`
//! locks the flag-OFF minimal scenario; this pins the affect layer's real
//! behavior (SEEKING-biased foraging) so it cannot drift silently, and proves
//! the serialized `affect` column survives a save→load→step round-trip.

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/affect-seeking.toml");

#[test]
fn affect_scenario_parses_with_flag_on() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    assert!(s.affect_enabled, "scenario must enable the affect layer");
}

#[test]
fn affect_scenario_is_self_consistent() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse");
    let run = |ticks: u64| {
        let mut w = s.instantiate();
        for _ in 0..ticks {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(300), run(300), "same seed + flag on → bit-identical");
}

#[test]
fn affect_scenario_survives_save_load_step() {
    let mut world = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    assert!(world.affect_enabled);
    // Warm the world so SEEKING activations accumulate before the snapshot.
    for _ in 0..300 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(
        state_hash(&world), state_hash(&reloaded),
        "load must restore identical state (affect column persisted)"
    );
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world), state_hash(&reloaded),
        "affect world diverged after save→load→step (non-serialized affect state?)"
    );
}

/// Pinned flag-ON golden. Generated once with `UPDATE_HASHES=1` after the
/// SEEKING layer was wired; regenerate deliberately whenever an affect change
/// is intentional.
const AFFECT_GOLDEN: &[(u64, u64)] = &[(0, 0x0), (100, 0x0), (300, 0x0)];

#[test]
fn affect_scenario_matches_golden_hashes() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    let mut w = s.instantiate();
    let max_tick = AFFECT_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < AFFECT_GOLDEN.len() && AFFECT_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated affect hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }
    for ((exp_tick, exp_hash), (got_tick, got_hash)) in AFFECT_GOLDEN.iter().zip(&observed) {
        assert_eq!(exp_tick, got_tick, "tick mismatch");
        assert_eq!(
            *exp_hash, *got_hash,
            "affect hash drift at tick {exp_tick}: expected 0x{exp_hash:016x}, got 0x{got_hash:016x}.\n\
             If intentional, rerun with UPDATE_HASHES=1 and copy the printed values.",
        );
    }
}
```
Also add the affect scenario to the parallel test's array in `determinism.rs` (lines 179-182):
```rust
    for scenario_src in [
        include_str!("../../../scenarios/minimal.toml"),
        include_str!("../../../scenarios/tech-gene-coupling.toml"),
        include_str!("../../../scenarios/affect-seeking.toml"),
    ] {
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p anabios-core --test affect` — `affect_scenario_matches_golden_hashes` fails: the placeholder `0x0` hashes do not match the real run.

- [ ] **Step 3 — Regenerate the golden (controller).** `UPDATE_HASHES=1 cargo test -p anabios-core --test affect affect_scenario_matches_golden_hashes -- --nocapture` and paste the three printed tuples into `AFFECT_GOLDEN`, replacing the `0x0` sentinels.

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p anabios-core --test affect --test determinism` (the whole affect suite green, and the parallel test still passes with the new flag-on scenario exercising `develop_all` under 1/2/8 threads).

- [ ] **Step 5 — Commit.** `git add crates/anabios-core/tests/affect.rs crates/anabios-core/tests/determinism.rs` + `git commit -m "tests: flag-on affect golden + save→load→step + parallel coverage"`.

---

## Self-review notes

**Spec coverage (design §, contract):**
- §3.1 tick plug-in points → Task 6 (develop_all post-sense/pre-decide) + Task 7 (apply_affect after apply_personality) + Task 8 (speed factor in integrate). The hijack (§3.1 read-side override) and stage-level reproduction factor consumption are M-B/M-C; M-A ships the `arousal` baseline (Task 1) and the `affect_reproduction_factor` identity stub (Task 1).
- §3.2 Layer-0 homeostatic drive (`e* = SPAWN_ENERGY`, linear deficit) → Task 1 `homeostatic_drive`.
- §3.3 Layer-1 leaky integrators (`a[k] ← λ·a[k] + (1−λ)·trigger`), arousal → Task 6 (SEEK trigger) + Task 1 (`arousal`, `LAMBDA_DEFAULT`). Lateral inhibition is deferred (no antagonist systems active in M-A).
- §4.1 SEEKING trigger (drive) + output (forage bias toward `plant_direction` / wander + speed factor) → Task 6 trigger, Task 7 bias, Task 8 speed. (SEEK trigger is drive-only in M-A; food cues shape the *output* direction, documented in Task 6.)
- §5.1 serialized `affect` column, neutral `[0.0;7]`, both spawn branches + dead-slot (reuse) reset mirroring `iq` → Task 3. (`affect_prev_crowding` is M-D, per the contract.)
- §5.2 temperament genes in reserved slots 17/18/19/34/35, signed accessors, `SLOT_NAMES`, count toward `distance` (NOT in `PERSONALITY_MASK`) → Task 2, with the speciation decision recorded and locked by `temperament_counts_toward_speciation_distance`.
- §5.3 `affect_enabled` world flag + scenario threading (not in `minimal.toml`; new flag-on TOML) → Tasks 4, 5.
- §6 determinism playbook: (1) flag-off byte-identity + neutral identity → Tasks 6/7/8 + Global Constraints; (2) zero RNG in `develop_all` → Task 6; (3) serialized never-skip + save→load→step → Tasks 3, 10; (4) index-disjoint `par_iter` + parallel-vs-serial coverage → Tasks 6, 10; (5) no new enum variants (temperament reuses reserved slots) → Task 2; (6) one-time golden refresh + `FORMAT_VERSION` bump → Task 9; (7) flag-on golden → Task 10.
- §8 M-A "done when": flag-off byte-identical (Task 9), flag-on golden pins SEEKING (Task 10), save→load→step passes (Task 10), speciation-distance decision recorded (Task 2).

**Type consistency vs the contract:** all signatures (`homeostatic_drive`, `develop_all`, `arousal`, `apply_affect`, `affect_speed_factor`, `affect_reproduction_factor`, temperament accessors, `AffectState`, `AFFECT_SYSTEMS`, indices, `HIJACK_AROUSAL_THRESHOLD`, `LAMBDA_DEFAULT`, `affect_enabled` fields) match the interface contract verbatim. The contract's shorthand `local_biomass`/`plant_dir` maps to the actual sensor fields `local_plant_biomass`/`plant_direction` (verified in `sense.rs`); `apply_affect`'s unused-in-M-A `genome`/`energy` params are bound as `_genome`/`_energy` without changing the public signature.

**Placeholder scan:** no "TBD"/"similar to Task N"/"add error handling". The only `0x0` values are golden-hash sentinels populated by `UPDATE_HASHES=1` in Tasks 9-10 — the established anabios pattern, not prose placeholders.
