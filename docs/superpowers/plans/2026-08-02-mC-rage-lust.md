# M-C: Agonistic + Reproductive — RAGE + LUST — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the RAGE (agonistic) and LUST (reproductive) Panksepp systems to the affect layer: a derived-frustration RAGE trigger + attack-driven RAGE impulse, a mate-readiness LUST trigger, FEAR⊣RAGE lateral inhibition, read-side biases (fire/approach + mate-approach), and the real `affect_reproduction_factor` that lowers the mating energy gate under high LUST.

**Architecture:** M-C consumes the M-A framework (`affect.rs` module, serialized `affect: Vec<AffectState>` column, `develop_all` leaky-integrator stage, `apply_affect` bias hook, `homeostatic_drive`, temperament gene accessors) and the M-B threat circuit (FEAR activation + `arousal`). It writes RAGE/LUST triggers into `develop_all`'s per-system dispatch, adds one lateral-inhibition edge (FEAR suppresses RAGE) after the raw updates, extends `apply_affect` with RAGE/LUST bias blocks (each exact-identity-at-neutral), implements the M-A `affect_reproduction_factor` stub and consumes it in `reproduce.rs` beside `personality_reproduction_factor`, and adds a determinism-safe attack→RAGE impulse that `combat_pass` folds into the **existing serialized affect column** (no new column). No new serialized columns, no `FORMAT_VERSION` bump, no new RNG.

**Tech Stack:** Rust, anabios-core, rayon, serde/bincode, cargo test.

## Global Constraints

- Single Xoshiro256++ RNG; the affect stage draws ZERO RNG (even flag-on); flag-off byte-identical.
- Affect writes only LIVE channels (`fire_intent`, `move_x/move_y`, `affect_reproduction_factor`→reproduce) — NEVER `feed_intent`/`mate_intent` (latent, out of scope).
- Every read-side block is neutral-identity, guarded `if x != 0.0`, using `1.0 + k·x` / `1.0 − k·x` / `+ k·x` forms (personality.rs idiom).
- Append-only enums (no new `Node`/`EventType` in M-C).
- Golden refresh only via `UPDATE_HASHES=1`; the flag-OFF goldens (minimal/cognition/inventions) MUST NOT move in M-C — assert byte-identity.
- The controller (not the implementer subagent) runs all `cargo`/`git`/golden-refresh/commit steps. Implementer subagents Edit/Read only.

## Dependencies

- Requires **M-A** (affect framework: `affect.rs`, serialized `affect` column, `develop_all` stage post-sense/pre-decide, `apply_affect` SEEK bias, `homeostatic_drive`, `affect_reproduction_factor` identity stub, temperament genes incl. `aggressiveness()`, `affect_enabled` flag + a flag-ON affect scenario/golden) merged.
- Requires **M-B** (FEAR activation folded in `develop_all`, `arousal`, the hijack) merged — M-C's FEAR⊣RAGE inhibition reads the FEAR activation M-B produces.
- Interface signatures are normative from `docs/superpowers/plans/2026-08-02-affect-layer-interface-contract.md`. Indices: `SEEK=0, FEAR=1, RAGE=2, LUST=3, CARE=4, PANIC=5, PLAY=6`.

---

## File Structure

- **Modify `crates/anabios-core/src/affect.rs`** — M-C constants; `trigger_rage`/`trigger_lust` pure helpers; RAGE+LUST dispatch inside `develop_all`; FEAR⊣RAGE inhibition; RAGE+LUST blocks in `apply_affect`; real `affect_reproduction_factor` body; the `RAGE_ATTACK_IMPULSE` constant + `RAGE` index reused by `interact.rs`. This is where nearly all M-C logic lives (one focused module, mirroring how `personality.rs` owns all OCEAN modulation).
- **Modify `crates/anabios-core/src/reproduce.rs`** — factor the `1.5` literal into a shared `REPRO_ENERGY_MULT` const; multiply the mating threshold in `is_eligible` (`reproduce.rs:289-293`) by `affect::affect_reproduction_factor(&agents.affect[i])`.
- **Modify `crates/anabios-core/src/interact.rs`** — at the combat damage site (`combat_pass`, `interact.rs:206-209`), add a flag-gated RAGE impulse into the target's serialized `affect[RAGE]` (reuse the affect column; no new state).
- **Tests** — unit tests live inline in `affect.rs`/`reproduce.rs` `#[cfg(test)]` modules. A flag-on behavior integration test goes in the existing M-A affect integration test file (`crates/anabios-core/tests/affect.rs`); if M-A named it differently, add the test there. The flag-ON affect golden (M-A/M-B) is refreshed once for the new trajectory.

Verified anchors (do not trust from memory — re-open before editing): `reproduce.rs` threshold `reproduce.rs:289-293`; combat damage `interact.rs:206-209`; `tick.rs` decide_all `apply_personality` call `tick.rs:187-192`, movement normalize `tick.rs:258-261`; `program/mod.rs` `ActionRegister` `:121-153` (`fire_intent`, `move_x/move_y`, `target_id`, `NO_TARGET`); `sense.rs` `SensorRegister` (`crowding`, `nearest_same_id`/`nearest_same_dir`, `nearest_other_id`/`nearest_other_dir`, `NO_NEIGHBOR_ID`); `genome.rs` `ReproductionThreshold = 30`, `aggressiveness()` accessor (M-A, next to `cognitive_potential()` ~`:338`); `agent::SPAWN_ENERGY`.

---

### Task 1: M-C constants + `trigger_rage`/`trigger_lust` pure helpers

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (add constants + two pure helpers near the other trigger helpers)
- Modify: `crates/anabios-core/src/reproduce.rs` (add shared `REPRO_ENERGY_MULT` const, no behavior change)
- Test: `crates/anabios-core/src/affect.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes (from M-A): `pub const RAGE: usize = 2; pub const LUST: usize = 3; pub const FEAR: usize = 1;` `pub type AffectState = [f32; AFFECT_SYSTEMS];` `pub fn homeostatic_drive(energy: f32) -> f32;` `Genome::aggressiveness(&self) -> f32` (signed `[-1,+1]`, neutral genome ⇒ `0.0`). From `sense.rs`: `SensorRegister { crowding: u32, nearest_same_id: u32, .. }`, `NO_NEIGHBOR_ID`. From `agent`: `SPAWN_ENERGY: f32`. From `genome`: `GenomeSlot::ReproductionThreshold`.
- Produces (for later M-C tasks):
  - `pub const RAGE_CROWD_REF: f32` — neighbor count that saturates crowd-pressure.
  - `pub const RAGE_ATTACK_IMPULSE: f32` — per-hit RAGE bump written by `interact.rs`.
  - `pub const K_RAGE_FIRE: f32`, `pub const K_RAGE_APPROACH: f32` — RAGE read-side gains.
  - `pub const FEAR_INHIBITS_RAGE: f32` — FEAR⊣RAGE suppression strength.
  - `pub const K_LUST_REPRO: f32`, `pub const K_LUST_APPROACH: f32` — LUST read-side gains.
  - `pub fn trigger_rage(drive: f32, genome: &Genome, sensors: &SensorRegister) -> f32` — RAGE trigger in `[0,1]`.
  - `pub fn trigger_lust(energy: f32, genome: &Genome, sensors: &SensorRegister) -> f32` — LUST trigger in `[0,1]`.
  - `reproduce::REPRO_ENERGY_MULT: f32 = 1.5` — shared so `trigger_lust` and `is_eligible` agree on the mating-energy reference.

- [ ] **Step 1: Write the failing tests**

In the `affect.rs` `#[cfg(test)]` module:

```rust
#[test]
fn trigger_rage_scales_with_drive_and_crowding() {
    use crate::genome::Genome;
    use crate::sense::SensorRegister;
    let g = Genome::neutral(); // aggressiveness() == 0.0 → gain 0.5
    // No crowding → no frustration regardless of drive.
    let alone = SensorRegister { crowding: 0, ..Default::default() };
    assert_eq!(trigger_rage(1.0, &g, &alone), 0.0);
    // High drive + crowded → positive frustration.
    let crowded = SensorRegister { crowding: RAGE_CROWD_REF as u32, ..Default::default() };
    let r = trigger_rage(1.0, &g, &crowded);
    assert!(r > 0.0 && r <= 1.0, "frustrated agent has positive RAGE trigger: {r}");
    // Well-fed (drive 0) → no frustration even when crowded.
    assert_eq!(trigger_rage(0.0, &g, &crowded), 0.0);
}

#[test]
fn trigger_rage_gain_rises_with_aggressiveness() {
    use crate::genome::{Genome, GenomeSlot};
    use crate::sense::SensorRegister;
    let crowded = SensorRegister { crowding: RAGE_CROWD_REF as u32, ..Default::default() };
    let mut calm = Genome::neutral();
    calm.set(GenomeSlot::Aggressiveness, 0.0); // aggressiveness() == -1.0 → gain 0.0
    let mut fierce = Genome::neutral();
    fierce.set(GenomeSlot::Aggressiveness, 1.0); // aggressiveness() == +1.0 → gain 1.0
    assert!(trigger_rage(1.0, &fierce, &crowded) > trigger_rage(1.0, &calm, &crowded));
}

#[test]
fn trigger_lust_needs_mate_and_energy() {
    use crate::genome::{Genome, GenomeSlot};
    use crate::sense::{SensorRegister, NO_NEIGHBOR_ID};
    use crate::agent::SPAWN_ENERGY;
    let mut g = Genome::neutral();
    g.set(GenomeSlot::ReproductionThreshold, 0.4);
    let repro_energy = SPAWN_ENERGY * 0.4 * crate::reproduce::REPRO_ENERGY_MULT;
    let with_mate = SensorRegister { nearest_same_id: 3, ..Default::default() };
    let no_mate = SensorRegister { nearest_same_id: NO_NEIGHBOR_ID, ..Default::default() };
    // Ready + mate present → lust.
    assert!(trigger_lust(repro_energy + 1.0, &g, &with_mate) > 0.0);
    // Ready but nobody around → no lust.
    assert_eq!(trigger_lust(repro_energy + 1.0, &g, &no_mate), 0.0);
    // Mate present but below the mating energy gate → no lust.
    assert_eq!(trigger_lust(repro_energy - 1.0, &g, &with_mate), 0.0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core affect::tests::trigger_ -- --nocapture`
Expected: FAIL — `trigger_rage`/`trigger_lust`/constants/`REPRO_ENERGY_MULT`/`GenomeSlot::Aggressiveness` not found (compile error is an acceptable failure here).

- [ ] **Step 3: Add the shared reproduce const (no behavior change)**

In `reproduce.rs`, next to `PARENT_ENERGY_COST_FRAC`:

```rust
/// Multiplier on `ReproductionThreshold × SPAWN_ENERGY` that sets the mating
/// energy gate (see `is_eligible`). Named so the affect layer's LUST trigger
/// (`affect::trigger_lust`) reads the same reference the gate uses.
pub const REPRO_ENERGY_MULT: f32 = 1.5;
```

Then in `is_eligible` (`reproduce.rs:289-293`) replace the literal `* 1.5` with `* REPRO_ENERGY_MULT` (numerically identical — `REPRO_ENERGY_MULT == 1.5`, so the multiply is byte-for-byte unchanged):

```rust
    let threshold = SPAWN_ENERGY
        * agents.genome[i].get(GenomeSlot::ReproductionThreshold)
        * REPRO_ENERGY_MULT
        * crate::personality::personality_reproduction_factor(&agents.genome[i]);
```

- [ ] **Step 4: Add M-C constants + trigger helpers to `affect.rs`**

Near the other affect constants:

```rust
// --- M-C: RAGE (agonistic) ---
/// Neighbour count at which crowd-pressure (the "blocked from resources" half of
/// frustration) saturates. Higher crowding + hunger ⇒ more RAGE.
pub const RAGE_CROWD_REF: f32 = 6.0;
/// RAGE added to a target's activation each time it takes combat damage
/// (written into the serialized `affect` column by `interact::combat_pass`, so
/// no new column and no serde-skip replay hazard). Clamped to 1.0.
pub const RAGE_ATTACK_IMPULSE: f32 = 0.5;
/// RAGE → `fire_intent` gain (approach-and-attack the combat target).
pub const K_RAGE_FIRE: f32 = 1.0;
/// RAGE → approach-target movement gain.
pub const K_RAGE_APPROACH: f32 = 0.5;
/// FEAR⊣RAGE lateral-inhibition strength (flee before fight): at FEAR = 1 and
/// strength 1, RAGE is fully suppressed.
pub const FEAR_INHIBITS_RAGE: f32 = 1.0;

// --- M-C: LUST (reproductive) ---
/// LUST → reproduction-threshold *reduction* (lowers the mating energy gate).
/// At LUST = 1 the gate drops by this fraction (30%).
pub const K_LUST_REPRO: f32 = 0.3;
/// LUST → approach-mate movement gain.
pub const K_LUST_APPROACH: f32 = 0.4;
```

Then the two pure helpers (place beside M-A's SEEK / M-B's FEAR trigger functions):

```rust
/// RAGE trigger — *derived frustration*. anabios has no native "frustration"
/// field, so we derive it: an agent is frustrated when it is **hungry while
/// blocked**, i.e. high homeostatic `drive` (energy deficit, a proxy for low
/// recent intake) AND high `crowding` (competitors in the way of the resource).
/// The product means BOTH must hold — a well-fed crowded agent, or a starving
/// solitary one, is not frustrated. Scaled by an `aggressiveness`-derived gain
/// (RAGE gain gene, mapped `[-1,+1] → [0,1]`). Result in `[0,1]`.
///
/// Note: the "having-been-attacked this tick" half of the spec's heuristic is
/// applied as a separate impulse in `interact::combat_pass` (see
/// `RAGE_ATTACK_IMPULSE`), NOT here — combat runs after the affect stage, and
/// its `#[serde(skip)]` `combat_damaged` scratch cannot be read into serialized
/// affect across a tick boundary without reintroducing the serde-skip replay
/// footgun. Writing the impulse into the serialized `affect` column at combat
/// time keeps it determinism-safe.
pub fn trigger_rage(drive: f32, genome: &Genome, sensors: &SensorRegister) -> f32 {
    let crowd_pressure = (sensors.crowding as f32 / RAGE_CROWD_REF).clamp(0.0, 1.0);
    let frustration = drive * crowd_pressure;
    // aggressiveness() ∈ [-1,+1]; map to a [0,1] gain (neutral genome ⇒ 0.5).
    let gain = (0.5 + 0.5 * genome.aggressiveness()).clamp(0.0, 1.0);
    (frustration * gain).clamp(0.0, 1.0)
}

/// LUST trigger — mate-readiness. Rises to 1.0 when the agent's energy is at or
/// above the mating gate (`SPAWN_ENERGY × ReproductionThreshold × REPRO_ENERGY_MULT`,
/// the same reference `reproduce::is_eligible` uses) AND a same-species neighbour
/// is in perception (`nearest_same_id`). Zero otherwise. Deterministic function
/// of serialized/​recomputed state only (energy + genome + this tick's sensors).
pub fn trigger_lust(energy: f32, genome: &Genome, sensors: &SensorRegister) -> f32 {
    if sensors.nearest_same_id == crate::sense::NO_NEIGHBOR_ID {
        return 0.0;
    }
    let repro_energy = crate::agent::SPAWN_ENERGY
        * genome.get(crate::genome::GenomeSlot::ReproductionThreshold)
        * crate::reproduce::REPRO_ENERGY_MULT;
    if repro_energy <= 0.0 || energy < repro_energy {
        return 0.0;
    }
    1.0
}
```

Ensure `use` items cover `Genome`, `GenomeSlot`, `SensorRegister` (they are already imported by M-A's `affect.rs`; add only what the compiler reports missing).

> **Note on `GenomeSlot::Aggressiveness`:** M-A renamed reserved slot 18 to `Aggressiveness` and added the `aggressiveness()` accessor. If M-A's rename used a different variant name, use that name here and in the tests — the accessor `Genome::aggressiveness()` is normative per the contract; the slot enum name is whatever M-A chose. Confirm by reading `genome.rs` before editing.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p anabios-core affect::tests::trigger_`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/affect.rs crates/anabios-core/src/reproduce.rs
git commit -m "feat(affect): RAGE/LUST trigger helpers + shared REPRO_ENERGY_MULT"
```

---

### Task 2: Wire RAGE + LUST into `develop_all`

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (`develop_all` per-agent body)
- Test: `crates/anabios-core/src/affect.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `trigger_rage`, `trigger_lust` (Task 1); `homeostatic_drive` (M-A); `LAMBDA_DEFAULT`, `RAGE`, `LUST` (M-A); the existing `develop_all` leaky-integrator body (M-A/M-B) which already, per agent, computes `drive` and updates `a[SEEK]`/`a[FEAR]` from this tick's `sensors[i]` + `energy[i]` + `genome[i]` inside a bounds guard for the sensors buffer.
- Produces: `develop_all` now also updates `a[RAGE]` and `a[LUST]` (raw, pre-inhibition). Flag-off remains a strict no-op (M-A's `if !world.affect_enabled { return; }` guard is unchanged).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn develop_all_raises_rage_when_frustrated_and_lust_when_mate_ready() {
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;
    use crate::agent::SPAWN_ENERGY;
    use crate::world::World;

    let mut w = World::new(1);
    w.affect_enabled = true;
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral()) as usize;
    // Hungry (high drive) and crowded → frustration; energy above the mating
    // gate with a same-species neighbour present → mate-ready.
    let mut g = Genome::neutral();
    g.set(GenomeSlot::ReproductionThreshold, 0.4);
    w.agents.genome[id] = g;
    w.agents.energy[id] = 0.05 * SPAWN_ENERGY; // deep energy deficit → drive ≈ 1
    w.sensors.resize(w.agents.capacity(), Default::default());
    w.sensors[id].crowding = RAGE_CROWD_REF as u32;
    w.sensors[id].nearest_same_id = 999; // a same-species neighbour exists

    // Mate-ready branch needs energy ≥ gate; run once frustrated (low energy)
    // to check RAGE, then again well-fed to check LUST.
    develop_all(&mut w);
    assert!(w.agents.affect[id][RAGE] > 0.0, "frustrated agent accrues RAGE");

    w.agents.energy[id] = SPAWN_ENERGY; // above the mating gate
    for _ in 0..5 { develop_all(&mut w); } // let the leaky integrator climb
    assert!(w.agents.affect[id][LUST] > 0.0, "mate-ready agent accrues LUST");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core affect::tests::develop_all_raises_rage_when_frustrated`
Expected: FAIL — RAGE/LUST stay `0.0` (not yet dispatched in `develop_all`).

- [ ] **Step 3: Add the RAGE + LUST dispatch inside `develop_all`**

Inside the existing per-agent body of `develop_all`, where M-A/M-B already have `drive` and the guarded `sensors[i]` in scope and update `a[SEEK]`/`a[FEAR]`, add (using the same `LAMBDA_DEFAULT` leaky form):

```rust
            // M-C RAGE: derived frustration (hungry + blocked), gated by the
            // aggressiveness gain gene. Zero RNG.
            let t_rage = trigger_rage(drive, &genome[i], &sensors[i]);
            a[RAGE] = LAMBDA_DEFAULT * a[RAGE] + (1.0 - LAMBDA_DEFAULT) * t_rage;
            // M-C LUST: mate-readiness (energy ≥ mating gate + same-species
            // neighbour present).
            let t_lust = trigger_lust(energy[i], &genome[i], &sensors[i]);
            a[LUST] = LAMBDA_DEFAULT * a[LUST] + (1.0 - LAMBDA_DEFAULT) * t_lust;
```

Match the exact local binding names M-A used (e.g. `genome`, `energy`, `sensors`, and the per-agent activation binding `a`); if M-A destructured `AgentBuffers` columns like `iq::develop_all` does, RAGE/LUST slot into the same destructured loop. Keep the writes inside M-A's sensors-length bounds guard (the `if i < sensors.len()` pattern from `iq.rs:96`) so a growth-tick short sensors buffer is safe.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core affect::tests::develop_all_raises_rage_when_frustrated`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): fold RAGE + LUST triggers into develop_all"
```

---

### Task 3: FEAR⊣RAGE lateral inhibition

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (`develop_all`, after all raw activation updates)
- Test: `crates/anabios-core/src/affect.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `FEAR_INHIBITS_RAGE`, `FEAR`, `RAGE` (Task 1 / M-A/M-B); the raw `a[RAGE]`/`a[FEAR]` written earlier in `develop_all`.
- Produces: post-inhibition `a[RAGE]` (flee-before-fight). Applied after ALL raw per-system updates so it composes with M-D/M-E edges later.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn fear_suppresses_rage() {
    // Two identical frustrated agents; the one with high FEAR ends with less RAGE.
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;
    use crate::agent::SPAWN_ENERGY;
    use crate::world::World;

    let setup = |fear: f32| -> f32 {
        let mut w = World::new(1);
        w.affect_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral()) as usize;
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.4);
        w.agents.genome[id] = g;
        w.agents.energy[id] = 0.05 * SPAWN_ENERGY;
        w.sensors.resize(w.agents.capacity(), Default::default());
        w.sensors[id].crowding = RAGE_CROWD_REF as u32;
        // Preload FEAR before this tick's update so inhibition has something to
        // gate against (M-B's FEAR update this tick blends toward its own trigger;
        // with no threat sensed the trigger is ~0, so the preloaded value decays
        // but stays positive for the high-fear case).
        w.agents.affect[id][FEAR] = fear;
        develop_all(&mut w);
        w.agents.affect[id][RAGE]
    };
    let calm = setup(0.0);
    let afraid = setup(1.0);
    assert!(afraid < calm, "FEAR must suppress RAGE: afraid={afraid} calm={calm}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core affect::tests::fear_suppresses_rage`
Expected: FAIL — without inhibition, both agents finish with equal RAGE.

- [ ] **Step 3: Add the inhibition edge**

In `develop_all`, **after** every raw per-system update for the agent (RAGE, FEAR, LUST, SEEK, …) and before the final clamp, add:

```rust
            // M-C lateral inhibition — flee before fight: FEAR gates down RAGE,
            // fully suppressing it at FEAR = 1 (with FEAR_INHIBITS_RAGE = 1).
            a[RAGE] = (a[RAGE] * (1.0 - FEAR_INHIBITS_RAGE * a[FEAR])).clamp(0.0, 1.0);
```

If M-A/M-B already have a "`// lateral inhibition`" section (the contract calls for "a short, documented set of pairwise suppressions"), append this edge there instead of adding a second block. Ensure a final `a[k].clamp(0.0, 1.0)` still covers RAGE (the line above already clamps RAGE; leave M-A's per-system clamp intact).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core affect::tests::fear_suppresses_rage`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): FEAR-inhibits-RAGE lateral edge (flee before fight)"
```

---

### Task 4: RAGE bias in `apply_affect` (fire + approach)

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (`apply_affect`)
- Test: `crates/anabios-core/src/affect.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `apply_affect(action: &mut ActionRegister, affect: &AffectState, genome: &Genome, sensors: &SensorRegister, energy: f32)` (M-A, already implements SEEK); `K_RAGE_FIRE`, `K_RAGE_APPROACH`, `RAGE` (Task 1); `ActionRegister { fire_intent, move_x, move_y }`, `SensorRegister { nearest_other_id, nearest_other_dir }`, `NO_NEIGHBOR_ID`.
- Produces: RAGE block in `apply_affect` — exact identity at `affect[RAGE] == 0.0`. Approaches and raises fire toward the nearest OTHER-species neighbour, which is exactly the target `combat_pass` resolves (`interact.rs:174`, `tgt = sensors.nearest_other_id`), so a raised `fire_intent` drives real combat.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn apply_affect_rage_raises_fire_and_approaches_target() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::program::ActionRegister;
    use crate::sense::SensorRegister;

    let mut a = ActionRegister { fire_intent: 0.1, ..Default::default() };
    let mut affect: AffectState = [0.0; AFFECT_SYSTEMS];
    affect[RAGE] = 0.8;
    let s = SensorRegister {
        nearest_other_id: 5,
        nearest_other_dir: Vec2::new(1.0, 0.0),
        ..Default::default()
    };
    apply_affect(&mut a, &affect, &Genome::neutral(), &s, crate::agent::SPAWN_ENERGY);
    assert!(a.fire_intent > 0.1, "RAGE raises fire_intent: {}", a.fire_intent);
    assert!(a.move_x > 0.0, "RAGE approaches the target: {}", a.move_x);
}

#[test]
fn apply_affect_rage_is_identity_at_neutral() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::program::ActionRegister;
    use crate::sense::SensorRegister;

    let mut a = ActionRegister { fire_intent: 0.3, move_x: 0.2, ..Default::default() };
    let before = a;
    let affect: AffectState = [0.0; AFFECT_SYSTEMS]; // RAGE == 0
    let s = SensorRegister {
        nearest_other_id: 5,
        nearest_other_dir: Vec2::new(1.0, 0.0),
        ..Default::default()
    };
    apply_affect(&mut a, &affect, &Genome::neutral(), &s, crate::agent::SPAWN_ENERGY);
    assert_eq!(a.fire_intent, before.fire_intent, "neutral RAGE: fire unchanged");
    assert_eq!(a.move_x, before.move_x, "neutral RAGE: move unchanged");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core affect::tests::apply_affect_rage`
Expected: `apply_affect_rage_raises_fire_and_approaches_target` FAILs (RAGE block absent); `apply_affect_rage_is_identity_at_neutral` PASSes vacuously (still run it to confirm you did not regress identity when adding the block).

- [ ] **Step 3: Add the RAGE block to `apply_affect`**

Inside `apply_affect`, after M-A's SEEK block:

```rust
    // M-C RAGE: approach and attack the nearest other-species neighbour (the
    // target combat_pass resolves). Guarded on RAGE ≠ 0 → exact identity at
    // neutral affect.
    let rage = affect[RAGE];
    if rage != 0.0 && sensors.nearest_other_id != crate::sense::NO_NEIGHBOR_ID {
        action.fire_intent += K_RAGE_FIRE * rage;
        action.move_x += K_RAGE_APPROACH * rage * sensors.nearest_other_dir.x;
        action.move_y += K_RAGE_APPROACH * rage * sensors.nearest_other_dir.y;
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p anabios-core affect::tests::apply_affect_rage`
Expected: PASS (both).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): RAGE bias in apply_affect (raise fire + approach target)"
```

---

### Task 5: LUST bias in `apply_affect` (approach mate)

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (`apply_affect`)
- Test: `crates/anabios-core/src/affect.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `apply_affect` (M-A + Task 4); `K_LUST_APPROACH`, `LUST` (Task 1); `SensorRegister { nearest_same_id, nearest_same_dir }`, `NO_NEIGHBOR_ID`.
- Produces: LUST approach block in `apply_affect` — exact identity at `affect[LUST] == 0.0`. Does NOT touch `mate_intent` (latent).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn apply_affect_lust_approaches_same_species() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::program::ActionRegister;
    use crate::sense::SensorRegister;

    let mut a = ActionRegister::default();
    let before_mate = a.mate_intent;
    let mut affect: AffectState = [0.0; AFFECT_SYSTEMS];
    affect[LUST] = 0.9;
    let s = SensorRegister {
        nearest_same_id: 4,
        nearest_same_dir: Vec2::new(0.0, 1.0),
        ..Default::default()
    };
    apply_affect(&mut a, &affect, &Genome::neutral(), &s, crate::agent::SPAWN_ENERGY);
    assert!(a.move_y > 0.0, "LUST approaches the mate: {}", a.move_y);
    assert_eq!(a.mate_intent, before_mate, "LUST must not touch latent mate_intent");
}

#[test]
fn apply_affect_lust_is_identity_at_neutral() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::program::ActionRegister;
    use crate::sense::SensorRegister;

    let mut a = ActionRegister { move_y: 0.2, ..Default::default() };
    let before = a;
    let affect: AffectState = [0.0; AFFECT_SYSTEMS]; // LUST == 0
    let s = SensorRegister {
        nearest_same_id: 4,
        nearest_same_dir: Vec2::new(0.0, 1.0),
        ..Default::default()
    };
    apply_affect(&mut a, &affect, &Genome::neutral(), &s, crate::agent::SPAWN_ENERGY);
    assert_eq!(a.move_y, before.move_y, "neutral LUST: move unchanged");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core affect::tests::apply_affect_lust`
Expected: `apply_affect_lust_approaches_same_species` FAILs; identity test passes vacuously.

- [ ] **Step 3: Add the LUST block to `apply_affect`**

After the RAGE block:

```rust
    // M-C LUST: approach the nearest same-species neighbour (a potential mate).
    // Guarded on LUST ≠ 0 → exact identity at neutral affect. mate_intent stays
    // latent; the reproduction gate is lowered via affect_reproduction_factor.
    let lust = affect[LUST];
    if lust != 0.0 && sensors.nearest_same_id != crate::sense::NO_NEIGHBOR_ID {
        action.move_x += K_LUST_APPROACH * lust * sensors.nearest_same_dir.x;
        action.move_y += K_LUST_APPROACH * lust * sensors.nearest_same_dir.y;
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p anabios-core affect::tests::apply_affect_lust`
Expected: PASS (both).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): LUST bias in apply_affect (approach nearest same-species)"
```

---

### Task 6: Implement `affect_reproduction_factor` + consume it in `reproduce.rs`

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (replace M-A's identity stub body)
- Modify: `crates/anabios-core/src/reproduce.rs` (`is_eligible`, `reproduce.rs:289-293`)
- Test: `crates/anabios-core/src/affect.rs` and `crates/anabios-core/src/reproduce.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `affect_reproduction_factor(affect: &AffectState) -> f32` (M-A stub, currently `1.0`); `K_LUST_REPRO`, `LUST` (Task 1); `is_eligible(agents: &AgentBuffers, id: u32) -> bool` (`reproduce.rs:279`), which reads `agents.affect[i]` (M-A serialized column).
- Produces: real `affect_reproduction_factor` — `1.0` at neutral, `1.0 − K_LUST_REPRO·LUST` (floored at 0) when LUST > 0. Consumed as an extra factor on the mating threshold, mirroring `personality_reproduction_factor`. Flag-off: `affect` all-zero ⇒ factor exactly `1.0` ⇒ `× 1.0` byte-identical ⇒ no golden move.

- [ ] **Step 1: Write the failing tests**

In `affect.rs`:

```rust
#[test]
fn affect_reproduction_factor_lowers_gate_with_lust() {
    let neutral: AffectState = [0.0; AFFECT_SYSTEMS];
    assert_eq!(affect_reproduction_factor(&neutral), 1.0, "identity at neutral");
    let mut lusty: AffectState = [0.0; AFFECT_SYSTEMS];
    lusty[LUST] = 1.0;
    let f = affect_reproduction_factor(&lusty);
    assert!(f < 1.0 && f >= 0.0, "high LUST lowers the reproduction gate: {f}");
    assert!((f - (1.0 - K_LUST_REPRO)).abs() < 1e-6);
}
```

In `reproduce.rs` `#[cfg(test)]` (proves the consumption changes real mating behavior):

```rust
#[test]
fn high_lust_lets_a_below_threshold_pair_mate() {
    // Energy set between the LUST-lowered gate and the neutral gate: they mate
    // only when LUST is high.
    let mates = |lust: f32| -> bool {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        // fertile_genome: ReproductionThreshold 0.4 → neutral gate =
        // SPAWN_ENERGY*0.4*1.5 = 0.6*SPAWN_ENERGY. LUST=1 gate = 0.7×that.
        let gate = SPAWN_ENERGY * 0.4 * crate::reproduce::REPRO_ENERGY_MULT;
        let e = gate * 0.85; // below neutral gate, above the LUST-lowered gate
        w.agents.energy[id0 as usize] = e;
        w.agents.energy[id1 as usize] = e;
        w.agents.affect[id0 as usize][crate::affect::LUST] = lust;
        w.agents.affect[id1 as usize][crate::affect::LUST] = lust;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let before = w.agents.live_count();
        reproduce_all(&mut w);
        w.agents.live_count() == before + 1
    };
    assert!(!mates(0.0), "neutral LUST: below-gate pair must not mate");
    assert!(mates(1.0), "high LUST lowers the gate enough to mate");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core affect_reproduction_factor_lowers_gate_with_lust high_lust_lets_a_below_threshold_pair_mate`
Expected: FAIL — stub returns `1.0` (factor test), and `is_eligible` doesn't consult affect yet (mating test: both `mates(0.0)` and `mates(1.0)` return false).

- [ ] **Step 3: Implement the factor body**

Replace M-A's `affect_reproduction_factor` stub:

```rust
/// Reproduction-threshold multiplier from LUST. Exactly `1.0` at neutral affect;
/// LUST lowers the mating energy gate by up to `K_LUST_REPRO` (30%). Consumed in
/// `reproduce::is_eligible` alongside `personality_reproduction_factor`.
pub fn affect_reproduction_factor(affect: &AffectState) -> f32 {
    let lust = affect[LUST];
    if lust != 0.0 {
        (1.0 - K_LUST_REPRO * lust).max(0.0)
    } else {
        1.0
    }
}
```

- [ ] **Step 4: Consume it in `is_eligible`**

In `reproduce.rs:289-293`, append the affect factor:

```rust
    let threshold = SPAWN_ENERGY
        * agents.genome[i].get(GenomeSlot::ReproductionThreshold)
        * REPRO_ENERGY_MULT
        * crate::personality::personality_reproduction_factor(&agents.genome[i])
        * crate::affect::affect_reproduction_factor(&agents.affect[i]);
```

(Flag-off: `agents.affect[i]` is all-zero because `develop_all` no-ops, so the new factor is exactly `1.0` and the multiply is byte-identical — the minimal/cognition/inventions goldens do not move.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p anabios-core affect_reproduction_factor_lowers_gate_with_lust high_lust_lets_a_below_threshold_pair_mate`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/affect.rs crates/anabios-core/src/reproduce.rs
git commit -m "feat(affect): affect_reproduction_factor lowers mating gate under LUST"
```

---

### Task 7: Attack→RAGE impulse in `combat_pass` (reuse the affect column)

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (`combat_pass`, `interact.rs:206-209`)
- Test: `crates/anabios-core/src/interact.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `RAGE_ATTACK_IMPULSE`, `RAGE` (Task 1); `world.affect_enabled` (M-A); `world.agents.affect` (M-A serialized column); the combat damage site where `world.combat_damaged[t] = true` is set.
- Produces: the spec's "having-been-attacked this tick" RAGE contribution, written into the **serialized** `affect` column at combat time (same tick as the hit). Determinism-safe: no new column, no RNG, gated on `affect_enabled` so flag-off writes nothing. The next tick's `develop_all` decays it via the leaky integrator, and that tick's `decide` reads the raised RAGE — a one-tick retaliation memory.

- [ ] **Step 1: Write the failing test**

Add to the `interact.rs` test module (follow the module's existing combat-test setup: a weapon-bearing attacker adjacent to an other-species target, `fire_intent` above `FIRE_THRESHOLD`, spatial hash + sensors built):

```rust
#[test]
fn combat_damage_bumps_target_rage_when_affect_on() {
    let mut w = World::new(13);
    w.affect_enabled = true;
    // Build the standard adjacent attacker/target combat setup used by the
    // other combat tests in this module (weapon on the attacker, other-species
    // target in range, fire_intent high, sensors + spatial populated).
    let (attacker, target) = setup_adjacent_combat_pair(&mut w); // helper in this module
    let before = w.agents.affect[target as usize][crate::affect::RAGE];
    interact_all(&mut w);
    let after = w.agents.affect[target as usize][crate::affect::RAGE];
    assert!(after > before, "a struck agent accrues RAGE: {before} -> {after}");
    assert!(after <= 1.0, "RAGE stays clamped: {after}");

    // Flag-off: no write.
    let mut w2 = World::new(13);
    // affect_enabled defaults false
    let (_a2, t2) = setup_adjacent_combat_pair(&mut w2);
    interact_all(&mut w2);
    assert_eq!(
        w2.agents.affect[t2 as usize][crate::affect::RAGE], 0.0,
        "flag off: combat must not touch affect"
    );
}
```

If no shared `setup_adjacent_combat_pair` helper exists in the module, inline the setup from the nearest existing combat test (search the module for `combat` / `FireWeapon` / `effective_weapon`) — reuse that exact fixture rather than inventing a new one.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core combat_damage_bumps_target_rage_when_affect_on`
Expected: FAIL — `affect[target][RAGE]` stays `0.0` (no bump yet).

- [ ] **Step 3: Add the impulse at the damage site**

In `combat_pass`, right after `world.combat_damaged[t] = true;` / `world.combat_attacker[t] = ...` (`interact.rs:208-209`):

```rust
        // M-C: being struck feeds RAGE. Written into the SERIALIZED affect
        // column here (same tick as the hit), so it round-trips through save/
        // load — unlike the #[serde(skip)] combat_damaged scratch, which the
        // affect stage must not read across a tick boundary. Gated + zero RNG.
        if world.affect_enabled {
            let r = &mut world.agents.affect[t][crate::affect::RAGE];
            *r = (*r + crate::affect::RAGE_ATTACK_IMPULSE).min(1.0);
        }
```

`t` is an alive agent slot and `agents.affect` is a full-capacity serialized column (resized on spawn like the other agent columns), so the index is in bounds.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core combat_damage_bumps_target_rage_when_affect_on`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/interact.rs
git commit -m "feat(affect): combat damage feeds target RAGE via serialized affect column"
```

---

### Task 8: Flag-on behavior test + determinism gates + golden refresh

**Files:**
- Modify/Create: `crates/anabios-core/tests/affect.rs` (M-A's flag-on affect integration test file; add the behavior + save/load/step tests there — create the file with `mod`-level scaffolding only if M-A did not already add it)
- Modify: whichever test holds the **flag-ON affect golden** (M-A/M-B) — refresh its pinned hashes for the new RAGE/LUST trajectory, with a dated changelog note.
- Controller-run: `determinism.rs`, `cognition.rs`, `inventions.rs` flag-OFF goldens must be unchanged (assert, do not refresh).

**Interfaces:**
- Consumes: `Scenario`/`World` step API; the flag-ON affect scenario TOML (M-A); `state_hash` (`snapshot.rs`); `save_to_bytes`/`load_from_bytes` (`snapshot.rs`); `RAGE`, `LUST` indices.
- Produces: an end-to-end guard that (a) frustration raises `fire_intent` and mate-readiness lowers the reproduction gate + biases approach, (b) the affect state survives save→load→step, (c) flag-off is byte-identical.

- [ ] **Step 1: Write the failing behavior + replay tests**

In `crates/anabios-core/tests/affect.rs`:

```rust
use anabios_core::affect::{LUST, RAGE};
use anabios_core::agent::SPAWN_ENERGY;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude::Vec2;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;
use anabios_core::world::World;

/// Frustration (hungry + crowded) with an other-species neighbour present
/// drives RAGE up over a few ticks, which raises the agent's fire_intent.
#[test]
fn frustration_raises_fire_intent() {
    let mut w = World::new(7);
    w.affect_enabled = true;
    // A frustrated agent surrounded by an other-species crowd.
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Aggressiveness, 1.0);
    let me = w.spawn_agent(Vec2::new(500.0, 500.0), g) as usize;
    w.agents.energy[me] = 0.05 * SPAWN_ENERGY; // deep deficit → high drive
    // Ring of other-species neighbours to create crowding + an other target.
    for k in 0..6 {
        let other = w.spawn_agent(Vec2::new(500.5 + k as f32 * 0.3, 500.0), Genome::neutral());
        w.agents.species_id[other as usize] = 1;
    }
    let mut prev_rage = 0.0;
    for _ in 0..10 {
        step(&mut w);
        prev_rage = w.agents.affect[me][RAGE];
        if !w.agents.is_alive(me as u32) { break; }
    }
    assert!(prev_rage > 0.0, "frustrated, crowded agent accrues RAGE: {prev_rage}");
    // With RAGE up and an other-species target sensed, its action fires.
    assert!(w.actions[me].fire_intent > 0.0, "RAGE drives fire_intent");
}

/// The affect column (RAGE/LUST) round-trips through save→load→step.
#[test]
fn affect_survives_save_load_step() {
    let mut w = World::new(11);
    w.affect_enabled = true;
    // Seed a couple of agents and warm up so affect is non-zero.
    for k in 0..8 {
        let id = w.spawn_agent(Vec2::new(480.0 + k as f32, 500.0), Genome::neutral());
        if k % 2 == 1 { w.agents.species_id[id as usize] = 1; }
    }
    for _ in 0..50 { step(&mut w); }
    let bytes = save_to_bytes(&w).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&w), state_hash(&reloaded), "load restores identical state");
    step(&mut w);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&w), state_hash(&reloaded),
        "affect world diverged after save→load→step (hidden non-serialized affect state?)",
    );
}
```

- [ ] **Step 2: Run to verify the behavior test drives the new paths (and replay passes)**

Run: `cargo test -p anabios-core --test affect frustration_raises_fire_intent affect_survives_save_load_step`
Expected: `frustration_raises_fire_intent` PASS (RAGE + fire wired in Tasks 2/4/7); `affect_survives_save_load_step` PASS (affect column is serialized; the RAGE impulse writes serialized state). If `affect_survives_save_load_step` FAILS, a non-serialized read is feeding affect — stop and audit (this is the serde-skip footgun guard).

- [ ] **Step 3: Confirm flag-OFF goldens are byte-identical (controller)**

Run: `cargo test -p anabios-core --test determinism --test cognition --test inventions`
Expected: PASS with **no hash changes**. M-C adds no serialized column and every read-side effect is neutral-identity / flag-gated, so these must not move. If any moves, a neutral-identity guard or a flag gate is missing — fix before refreshing anything.

- [ ] **Step 4: Refresh the flag-ON affect golden (controller, deliberate)**

The flag-ON affect scenario's trajectory changes (RAGE/LUST now act), so its golden moves once. Regenerate:

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test affect <affect_golden_test_name> -- --nocapture`
Paste the printed `(tick, 0x…)` values back into the golden array, and add a dated note:

```rust
// Refreshed 2026-08-02 (M-C RAGE+LUST): agonistic + reproductive systems now
// act flag-on — RAGE bias (fire/approach), LUST bias (approach) + lowered
// reproduction gate, FEAR⊣RAGE inhibition, and the combat→RAGE impulse. A
// genuine flag-on behavior change (NOT layout): no serialized column added,
// FORMAT_VERSION unchanged; flag-off (minimal/cognition/inventions) byte-identical.
```

- [ ] **Step 5: Confirm parallel determinism (controller)**

Run: `cargo test -p anabios-core --test determinism parallel_matches_serial_across_thread_counts`
Expected: PASS — `develop_all` writes only slot `i` (index-disjoint), and the combat→RAGE impulse runs in the serial `combat_pass` loop, so thread count cannot change results.

- [ ] **Step 6: Full crate test + fmt/clippy (controller)**

Run: `cargo test -p anabios-core && cargo fmt --check && cargo clippy -p anabios-core -- -D warnings`
Expected: PASS (matches the CI gate — committed tree must be `cargo fmt`-clean).

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/tests/affect.rs crates/anabios-core/src
git commit -m "test(affect): flag-on RAGE/LUST behavior + save/load/step; refresh affect golden"
```

---

## Self-review notes

- **Spec coverage.** §4.3 RAGE trigger (derived frustration = drive×crowding, aggressiveness gain) → Task 1/2; RAGE bias (raise `fire_intent` + approach target) → Task 4; "having-been-attacked this tick" → Task 7 (determinism-safe impulse into the serialized affect column). §4.4 LUST trigger (mate-readiness + same-species neighbour) → Task 1/2; LUST approach bias → Task 5; `affect_reproduction_factor` lowering the gate, consumed beside `personality_reproduction_factor` → Task 6; `mate_intent` left latent (asserted in Task 5). §3.3 FEAR⊣RAGE inhibition after raw updates → Task 3. Determinism/opt-in (§6): no new serialized column, no `FORMAT_VERSION` bump, zero RNG, flag-off byte-identical (Task 8 Step 3), flag-on golden refreshed once (Task 8 Step 4), save→load→step (Task 8 Step 2), parallel-matches-serial (Task 8 Step 5).

- **RAGE "attacked" heuristic — footgun resolved.** The spec offers RAGE from frustration OR "having-been-attacked this tick." The naive read — have `develop_all` read `world.combat_damaged` — is unsafe: `combat_damaged` is `#[serde(skip)]` scratch, and the affect stage runs before `interact`, so it would read the *previous* tick's value across a save/load boundary, exactly the serde-skip replay footgun in the project memory. Resolution (Task 7): the attack contribution is written into the **serialized** `affect[RAGE]` column at combat time (same tick as the hit, gated, no RNG), honoring the "reuse affect column" constraint and keeping save/load/step green. The frustration branch (Task 1/2) uses only serialized energy + this-tick sensors, so it is deterministic without any new state.

- **Flag-off byte-identity, verified path by path.** `develop_all` no-ops flag-off (M-A guard) → RAGE/LUST triggers + inhibition never run. `apply_affect` RAGE/LUST blocks are guarded `if rage != 0.0` / `if lust != 0.0`, and the affect column is all-zero flag-off → no arithmetic. `affect_reproduction_factor(all-zero) == 1.0` exactly → the new `× factor` in `is_eligible` is `× 1.0` (IEEE identity) → mating threshold unchanged. `REPRO_ENERGY_MULT == 1.5` → the `1.5 → REPRO_ENERGY_MULT` swap is byte-identical. Combat impulse is gated on `affect_enabled`. Net: minimal/cognition/inventions goldens must not move (Task 8 Step 3 asserts it).

- **Type consistency vs contract.** `affect_reproduction_factor(&AffectState) -> f32`, `apply_affect(&mut ActionRegister, &AffectState, &Genome, &SensorRegister, f32)`, indices `RAGE=2`/`LUST=3`/`FEAR=1`, and `LAMBDA_DEFAULT` leaky form all match `2026-08-02-affect-layer-interface-contract.md`. New public items (`trigger_rage`, `trigger_lust`, `RAGE_ATTACK_IMPULSE`, the `K_RAGE_*`/`K_LUST_*`/`FEAR_INHIBITS_RAGE`/`RAGE_CROWD_REF` consts, `reproduce::REPRO_ENERGY_MULT`) are additive and not named in the contract, so they do not conflict with M-A/M-B/M-D.

- **Assumptions to confirm at execution time (flagged, not guessed).** (1) M-A's slot-18 enum variant / accessor names — the accessor `Genome::aggressiveness()` is normative, but re-read `genome.rs` for the exact `GenomeSlot` variant used in tests. (2) The exact local binding names inside M-A/M-B's `develop_all` loop (`a`, `genome`, `energy`, `sensors`, the sensors-length bounds guard) — match them rather than assume. (3) The flag-on affect golden test's file/const name (Task 8 Step 4) — locate M-A/M-B's flag-on golden and refresh that one. (4) `combat_pass`'s test fixture — reuse the module's existing adjacent-combat setup rather than a new helper. None of these change the design; they are name/anchor confirmations for the implementer.

- **No placeholders.** Every code and test step contains real Rust. No "TBD"/"handle edge cases"/"similar to Task N".
