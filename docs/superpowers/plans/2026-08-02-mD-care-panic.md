# M-D: Social Bonding — CARE + PANIC/GRIEF — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the last two social Panksepp systems into the affect layer — CARE (kin proximity → provision/protect) and PANIC/GRIEF (social isolation / kin-loss → distress signal + reunion) — behind the existing `affect_enabled` flag, plus the one-tick-memory serialized column PANIC needs.

**Architecture:** M-D extends the M-A framework, consuming its `affect` column, `develop_all` compute stage, and `apply_affect` bias hook. It adds a second serialized per-agent column `affect_prev_crowding` (one-tick crowding memory), two new leaky-integrator triggers (`care_trigger`, `panic_trigger`) computed in `develop_all`, the PANIC⊣SEEK lateral-inhibition edge, and CARE/PANIC read-side bias on already-live channels (`share_intent`, `emit_intent`, `broadcast_intent`, `move_x/move_y`, `target_id`). All affect math is a pure deterministic function of already-drawn sensors + genome + physiology; zero RNG; flag-off byte-identical.

**Tech Stack:** Rust, anabios-core, rayon, serde/bincode, cargo test.

## Global Constraints

- Single Xoshiro256++ RNG; the affect stage draws ZERO RNG, flag-on or flag-off.
- Flag-off (`affect_enabled == false`) MUST be byte-identical: `develop_all` early-returns as a strict no-op, the affect columns stay all-zero, and `apply_affect`'s per-block `if x != 0.0` guards make it an exact identity.
- The new `affect_prev_crowding` column MUST be serialized — never `#[serde(skip)]`. It is a path-dependent accumulator that feeds hashed state (movement/share/pheromone), the still_ticks/prev_desired_direction v13 footgun precedent.
- Affect writes ONLY live channels: `share_intent`, `emit_intent[ch]`, `broadcast_intent[ch]`, `move_x/move_y`, `target_id`, and the stage factors. NEVER `feed_intent` / `mate_intent` (latent, out of scope).
- Neutral-identity guarded: every read-side block wrapped `if activation != 0.0`, `+ k·x` / `1.0 + k·x` forms (personality.rs idiom).
- Append-only enums: no new `Node` / `EventType` in M-D (codex detectors are M-F). Genome temperament genes are the M-A reserved-slot renames, already present.
- Golden refresh once (layout growth only): regenerate the three layout goldens with `UPDATE_HASHES=1`, add a dated "flag off ⇒ byte-identical" note, bump `FORMAT_VERSION` in `snapshot.rs`.
- Controller runs cargo/git/golden/commit. Implementer subagents Edit/Read only.

## Dependencies

- Requires M-A..M-C merged: the `affect.rs` module, the `affect: Vec<AffectState>` column, `affect_enabled` world/scenario flag, the ~5 temperament genome accessors (`nurturance()`, `sociality()`, …), `affect::develop_all` (post-sense/pre-decide, zero-RNG, per-agent leaky-integrator closure with trigger + inhibition + clamp sections), `affect::apply_affect` (called in `decide_all` right after `apply_personality`), and the constants/indices (`SEEK`, `CARE`, `PANIC`, `AFFECT_SYSTEMS`, `LAMBDA_DEFAULT`). M-D inserts into these; it must NOT delete M-A..M-C's SEEK/FEAR/RAGE/LUST blocks or the FEAR⊣RAGE edge.

---

## File Structure

- `crates/anabios-core/src/agent.rs` — add the `affect_prev_crowding: Vec<f32>` column beside `affect`; init in both spawn branches and the dead-slot reset in `kill`. (Modify)
- `crates/anabios-core/src/affect.rs` — add M-D constants; the pure `care_trigger` / `panic_trigger` helpers; the CARE + PANIC trigger blocks, the PANIC⊣SEEK inhibition edge, and the end-of-closure `affect_prev_crowding` write inside `develop_all`; the CARE + PANIC bias blocks inside `apply_affect`. (Modify)
- `scenarios/affect-social.toml` — new flag-on scenario (`affect_enabled = true`) tuned to elicit CARE (kin cluster) and PANIC (isolation), used by the flag-on behavior + golden + save/load tests. (Create)
- `crates/anabios-core/tests/determinism.rs` — add a save→load→step test for the affect-social scenario (covers `affect_prev_crowding`); add the scenario to `parallel_matches_serial_across_thread_counts`; refresh the `GOLDEN` layout hashes. (Modify)
- `crates/anabios-core/tests/cognition.rs` — refresh `COGNITIVE_GOLDEN` layout hashes. (Modify)
- `crates/anabios-core/tests/inventions.rs` — refresh its golden layout hashes. (Modify)
- `crates/anabios-core/tests/affect_social.rs` — new flag-on behavior + golden test for M-D. (Create)
- `crates/anabios-core/src/snapshot.rs` — bump `FORMAT_VERSION`, add the changelog `///` entry. (Modify)

---

### Task 1: Serialized `affect_prev_crowding` column on `AgentBuffers`

**Files:**
- Modify: `crates/anabios-core/src/agent.rs` (column decl ~`:66` next to `affect`; spawn reuse branch `:152-175`; spawn push branch `:176-200`; dead-slot reset in `kill` `:215-232`)
- Test: `crates/anabios-core/src/agent.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: M-A's `affect: Vec<AffectState>` column and its init sites.
- Produces: `pub affect_prev_crowding: Vec<f32>` — serialized, default `0.0`, one entry per slot, kept in lockstep with every other agent column.

- [ ] **Step 1: Write the failing test**

Add to the inline `tests` module in `agent.rs`:

```rust
    #[test]
    fn spawn_zeroes_affect_prev_crowding_and_kill_resets() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
        );
        // Present, zeroed, and sized to capacity on spawn.
        assert_eq!(a.affect_prev_crowding[id as usize], 0.0);
        assert_eq!(a.affect_prev_crowding.len(), a.capacity());
        // A stale value is cleared on death (dead-slot reset).
        a.affect_prev_crowding[id as usize] = 5.0;
        a.kill(id);
        assert_eq!(a.affect_prev_crowding[id as usize], 0.0, "dead slot reset");
        // Reused slot re-initializes to 0.0.
        a.affect_prev_crowding[id as usize] = 7.0;
        let id2 = a.spawn(
            Vec2::ZERO,
            neutral(),
            2,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
        );
        assert_eq!(id2, id, "slot reused");
        assert_eq!(a.affect_prev_crowding[id as usize], 0.0, "reuse re-init");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib agent::tests::spawn_zeroes_affect_prev_crowding_and_kill_resets`
Expected: FAIL — `no field affect_prev_crowding on type AgentBuffers`.

- [ ] **Step 3: Add the column and wire every init site**

In the `AgentBuffers` struct, immediately after the M-A `affect` column:

```rust
    /// Previous-tick neighbour count (`SensorRegister.crowding` as `f32`),
    /// written at the end of `affect::develop_all`. One-tick memory that lets
    /// PANIC/GRIEF detect a *drop* in social contact (kin left / died), not
    /// just an absolute-low crowding. Serialized (NOT `#[serde(skip)]`): it is
    /// a path-dependent accumulator feeding hashed state, so dropping it on load
    /// would make restore-and-continue diverge (still_ticks v13 precedent).
    /// Stays `0.0` for every agent when `affect_enabled` is off.
    pub affect_prev_crowding: Vec<f32>,
```

In the spawn **reuse** branch, next to `self.affect[i] = [0.0; AFFECT_SYSTEMS];` (M-A):

```rust
            self.affect_prev_crowding[i] = 0.0;
```

In the spawn **push** branch, next to `self.affect.push([0.0; AFFECT_SYSTEMS]);` (M-A):

```rust
            self.affect_prev_crowding.push(0.0);
```

In `kill`, next to the M-A dead-slot `self.affect[i] = [0.0; AFFECT_SYSTEMS];` reset (add it there; if M-A left the reset only in the reuse branch, add this line in `kill` after `self.energy[i] = 0.0;`):

```rust
        self.affect_prev_crowding[i] = 0.0;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core --lib agent::tests::spawn_zeroes_affect_prev_crowding_and_kill_resets`
Expected: PASS.

- [ ] **Step 5: Run the full agent unit suite to confirm no column-length drift**

Run: `cargo test -p anabios-core --lib agent::`
Expected: PASS (all spawn/kill/reuse invariants hold with the extra column).

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/agent.rs
git commit -m "feat(affect): add serialized affect_prev_crowding column (M-D)"
```

---

### Task 2: Pure `care_trigger` helper + constants

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (M-D constants block; `care_trigger`; inline tests)

**Interfaces:**
- Consumes: nothing new (pure scalar function).
- Produces: `pub fn care_trigger(nearest_kinship: f32, nearest_same_dist: f32, nurturance: f32) -> f32` returning a CARE activation target in `[0,1]`. Zero when no close kin.

- [ ] **Step 1: Write the failing test**

Add to the inline `tests` module in `affect.rs`:

```rust
    #[test]
    fn care_trigger_rises_with_close_kin_and_zero_when_absent() {
        // Close, highly-related kin, neutral nurturance → positive CARE.
        let close = care_trigger(0.8, 4.0, 0.0);
        assert!(close > 0.0, "close kin should elicit CARE, got {close}");
        // No kinship → no CARE regardless of distance.
        assert_eq!(care_trigger(0.0, 4.0, 0.0), 0.0);
        // Kin out of range → no CARE.
        assert_eq!(care_trigger(0.8, CARE_RANGE, 0.0), 0.0);
        // Nurturance gain: more nurturant → stronger CARE at the same percept.
        let nurt = care_trigger(0.8, 4.0, 1.0);
        assert!(nurt > close, "nurturance should raise CARE: {nurt} > {close}");
        // Bounded to [0,1].
        assert!((0.0..=1.0).contains(&nurt));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib affect::tests::care_trigger_rises_with_close_kin_and_zero_when_absent`
Expected: FAIL — `cannot find function care_trigger` / `cannot find value CARE_RANGE`.

- [ ] **Step 3: Add the constants and the helper**

Add the M-D constants alongside the M-A ones in `affect.rs`:

```rust
// --- M-D: CARE (kin provision/protect) ---
/// Below this overall-nearest kinship, CARE does not engage.
pub const CARE_KINSHIP_MIN: f32 = 0.25;
/// Same-species neighbour must be within this distance to elicit CARE.
pub const CARE_RANGE: f32 = 40.0;
/// CARE leaky-integrator retention.
pub const LAMBDA_CARE: f32 = 0.8;
/// CARE → `share_intent` gain (read side).
pub const K_CARE_SHARE: f32 = 0.8;
/// CARE → stay-near-kin movement gain (read side).
pub const K_CARE_APPROACH: f32 = 0.3;
```

Add the helper (near the other `*_trigger` fns M-A/M-B/M-C added):

```rust
/// CARE activation target in `[0,1]` from kin proximity + Nurturance gain.
/// Zero unless a same-species neighbour is both close (`< CARE_RANGE`) and
/// sufficiently related (`> CARE_KINSHIP_MIN`). `nurturance` is the signed
/// `[-1,+1]` temperament gene (neutral `0.0`). Pure; no RNG.
#[inline]
pub fn care_trigger(nearest_kinship: f32, nearest_same_dist: f32, nurturance: f32) -> f32 {
    if nearest_kinship <= CARE_KINSHIP_MIN || nearest_same_dist >= CARE_RANGE {
        return 0.0;
    }
    // Closer kin → stronger drive to stay and provision.
    let proximity = (1.0 - nearest_same_dist / CARE_RANGE).clamp(0.0, 1.0);
    // Neutral nurturance (0.0) → gain 0.5; caring temperament scales up.
    let gain = (0.5 + 0.5 * nurturance).clamp(0.0, 1.0);
    (nearest_kinship * proximity * gain).clamp(0.0, 1.0)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core --lib affect::tests::care_trigger_rises_with_close_kin_and_zero_when_absent`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): CARE trigger + constants (M-D)"
```

---

### Task 3: Pure `panic_trigger` helper + constants

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (M-D constants block; `panic_trigger`; inline tests)

**Interfaces:**
- Consumes: nothing new (pure scalar function).
- Produces: `pub fn panic_trigger(crowding: f32, prev_crowding: f32, sociality: f32) -> f32` returning a PANIC activation target in `[0,1]`. Combines absolute isolation with a one-tick crowding *drop*, gated on social temperament.

- [ ] **Step 1: Write the failing test**

Add to the inline `tests` module in `affect.rs`:

```rust
    #[test]
    fn panic_trigger_fires_on_isolation_and_loss_only_for_social() {
        // Social agent, alone this tick → isolation panic.
        let isolated = panic_trigger(0.0, 0.0, 0.0);
        assert!(isolated > 0.0, "isolated social agent should panic, got {isolated}");
        // Well-crowded, no drop → no panic.
        assert_eq!(panic_trigger(6.0, 6.0, 0.0), 0.0);
        // Sudden loss of kin (crowding fell) even while not fully isolated → panic.
        let loss = panic_trigger(2.0, 6.0, 0.0);
        assert!(loss > 0.0, "kin-loss drop should panic, got {loss}");
        // Asocial temperament (sociality = -1) never panics from isolation.
        assert_eq!(panic_trigger(0.0, 0.0, -1.0), 0.0);
        // More social → stronger panic at the same isolation.
        let very_social = panic_trigger(0.0, 0.0, 1.0);
        assert!(very_social > isolated, "sociality should raise panic: {very_social} > {isolated}");
        assert!((0.0..=1.0).contains(&very_social));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib affect::tests::panic_trigger_fires_on_isolation_and_loss_only_for_social`
Expected: FAIL — `cannot find function panic_trigger` / `cannot find value PANIC_CROWDING_LOW`.

- [ ] **Step 3: Add the constants and the helper**

Add the M-D PANIC constants alongside the CARE ones:

```rust
// --- M-D: PANIC/GRIEF (separation distress) ---
/// Crowding below this counts as social isolation.
pub const PANIC_CROWDING_LOW: f32 = 2.0;
/// Crowding drop (prev − now) that saturates the kin-loss term.
pub const PANIC_LOSS_SCALE: f32 = 3.0;
/// PANIC leaky-integrator retention (lingers slightly longer than CARE).
pub const LAMBDA_PANIC: f32 = 0.85;
/// Distress-pheromone channel PANIC emits on (read side).
pub const PANIC_PHEROMONE_CHANNEL: usize = 0;
/// PANIC → distress-pheromone `emit_intent` gain (read side).
pub const K_PANIC_EMIT: f32 = 1.0;
/// PANIC → alarm `broadcast_intent` gain (read side).
pub const K_PANIC_BROADCAST: f32 = 1.0;
/// PANIC → reunion (toward nearest same-species) movement gain (read side).
pub const K_PANIC_REUNION: f32 = 0.4;
/// PANIC⊣SEEK lateral-inhibition strength (withdrawal).
pub const PANIC_SEEK_INHIBITION: f32 = 0.5;
```

Add the helper:

```rust
/// PANIC/GRIEF activation target in `[0,1]` from social isolation and/or a
/// one-tick drop in crowding, gated on social temperament. `crowding` /
/// `prev_crowding` are this-tick and last-tick neighbour counts (as `f32`).
/// `sociality` is the signed `[-1,+1]` temperament gene (neutral `0.0`);
/// asocial agents (`sociality <= -1`) never panic from isolation. Pure; no RNG.
#[inline]
pub fn panic_trigger(crowding: f32, prev_crowding: f32, sociality: f32) -> f32 {
    // Neutral sociality (0.0) → weight 0.5; asocial temperament → 0.0.
    let social = (0.5 + 0.5 * sociality).clamp(0.0, 1.0);
    if social <= 0.0 {
        return 0.0;
    }
    // Absolute isolation: how far below the "not alone" reference we are.
    let isolation = if crowding < PANIC_CROWDING_LOW {
        (1.0 - crowding / PANIC_CROWDING_LOW).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Kin-loss: a drop in crowding since last tick (someone left / died).
    let loss = ((prev_crowding - crowding) / PANIC_LOSS_SCALE).clamp(0.0, 1.0);
    (social * isolation.max(loss)).clamp(0.0, 1.0)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core --lib affect::tests::panic_trigger_fires_on_isolation_and_loss_only_for_social`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): PANIC/GRIEF trigger + constants (M-D)"
```

---

### Task 4: Wire CARE + PANIC into `develop_all` (triggers, PANIC⊣SEEK, prev-crowding write)

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (`develop_all`)
- Test: `crates/anabios-core/src/affect.rs` (inline tests, world-level)

**Interfaces:**
- Consumes: M-A's `develop_all(world: &mut World)` closure (per-agent leaky-integrator over `world.agents.affect`), `world.agents.affect_prev_crowding` (Task 1), `care_trigger`/`panic_trigger` (Tasks 2/3), `Genome::nurturance()` / `Genome::sociality()` (M-A), `SensorRegister.{crowding, nearest_kinship, nearest_same_dist}`, constants `SEEK`/`CARE`/`PANIC`/`LAMBDA_CARE`/`LAMBDA_PANIC`/`PANIC_SEEK_INHIBITION`.
- Produces: after a flag-on tick, `world.agents.affect[i][CARE]` and `[PANIC]` reflect this tick's kin/isolation percepts; `world.agents.affect[i][SEEK]` is suppressed by PANIC; `world.agents.affect_prev_crowding[i]` holds this tick's crowding.

**IMPORTANT:** This modifies the SHARED `develop_all`. INSERT the M-D blocks into the existing per-agent closure — do NOT delete the M-A SEEK block, the M-B FEAR block, the M-C RAGE/LUST blocks, or the FEAR⊣RAGE inhibition edge.

- [ ] **Step 1: Write the failing test**

Add to the inline `tests` module in `affect.rs` (uses `develop_all` end-to-end via a flag-on world):

```rust
    #[test]
    fn develop_all_writes_care_panic_and_prev_crowding() {
        use crate::genome::{Genome, GenomeSlot};
        use crate::prelude::Vec2;
        use crate::world::World;

        // Isolated social agent → PANIC accrues, prev_crowding recorded.
        let mut w = World::new(1);
        w.affect_enabled = true;
        let lone = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        crate::tick::step(&mut w);
        assert!(
            w.agents.affect[lone as usize][PANIC] > 0.0,
            "isolated social agent should accrue PANIC"
        );
        assert_eq!(
            w.agents.affect_prev_crowding[lone as usize], 0.0,
            "lone agent saw zero neighbours this tick"
        );

        // A tight same-species kin cluster → CARE accrues for a member.
        let mut w2 = World::new(2);
        w2.affect_enabled = true;
        let a = w2.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let _b = w2.spawn_agent(Vec2::new(303.0, 300.0), Genome::neutral());
        let _c = w2.spawn_agent(Vec2::new(300.0, 303.0), Genome::neutral());
        crate::tick::step(&mut w2);
        assert!(
            w2.agents.affect[a as usize][CARE] > 0.0,
            "kin-clustered agent should accrue CARE"
        );
        assert!(
            w2.agents.affect_prev_crowding[a as usize] >= 1.0,
            "clustered agent recorded neighbours in prev_crowding"
        );

        // Asocial isolated agent → no PANIC (temperament gate).
        let mut w3 = World::new(3);
        w3.affect_enabled = true;
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Sociality, 0.0); // signed sociality() = -1 → no panic
        let asoc = w3.spawn_agent(Vec2::new(500.0, 500.0), g);
        crate::tick::step(&mut w3);
        assert_eq!(
            w3.agents.affect[asoc as usize][PANIC], 0.0,
            "asocial agent must not panic from isolation"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib affect::tests::develop_all_writes_care_panic_and_prev_crowding`
Expected: FAIL — asserts on CARE/PANIC/prev_crowding that `develop_all` does not yet write (values are all `0.0`).

- [ ] **Step 3: Extend the `develop_all` destructure and closure**

Add `affect_prev_crowding` to the M-A destructure of `world.agents` and zip its `par_iter_mut` beside `affect` (mirrors the `iq` stage's disjoint-column zip). The shared reads (`sensors`, `genome`, `alive`) stay `&`:

```rust
    let sensors = &world.sensors;
    let crate::agent::AgentBuffers { affect, affect_prev_crowding, genome, alive, .. } =
        &mut world.agents;
    let (genome, alive) = (&*genome, &*alive);
    affect[..cap]
        .par_iter_mut()
        .zip(affect_prev_crowding[..cap].par_iter_mut())
        .enumerate()
        .for_each(|(i, (aff, prev))| {
            if !alive[i] {
                return;
            }
            let s = &sensors[i];
            let g = &genome[i];
            // Snapshot last tick's crowding BEFORE overwriting it below.
            let prev_crowding = *prev;

            // --- M-A SEEK block stays here (unchanged) ---
            // --- M-B FEAR block stays here (unchanged) ---
            // --- M-C RAGE / LUST blocks stay here (unchanged) ---

            // --- M-D: CARE trigger (leaky integrator) ---
            let care_raw = care_trigger(s.nearest_kinship, s.nearest_same_dist, g.nurturance());
            aff[CARE] = LAMBDA_CARE * aff[CARE] + (1.0 - LAMBDA_CARE) * care_raw;

            // --- M-D: PANIC/GRIEF trigger (leaky integrator) ---
            let panic_raw = panic_trigger(s.crowding as f32, prev_crowding, g.sociality());
            aff[PANIC] = LAMBDA_PANIC * aff[PANIC] + (1.0 - LAMBDA_PANIC) * panic_raw;

            // --- Lateral inhibition (M-C FEAR⊣RAGE stays; M-D adds PANIC⊣SEEK) ---
            // PANIC withdraws the appetitive SEEKING engine.
            aff[SEEK] = (aff[SEEK] - PANIC_SEEK_INHIBITION * aff[PANIC]).max(0.0);

            // --- Clamp all activations to [0,1] (M-A owns this loop) ---
            for v in aff.iter_mut() {
                *v = v.clamp(0.0, 1.0);
            }

            // --- M-D: record this tick's crowding for next tick's PANIC ---
            *prev = s.crowding as f32;
        });
```

Notes for the implementer:
- If M-A already has the `if !alive[i]` guard, the clamp loop, and the `let s`/`let g` bindings, reuse them — do not duplicate. Only ADD the `affect_prev_crowding` zip, `let prev_crowding = *prev;`, the CARE + PANIC trigger lines, the PANIC⊣SEEK line (place it in the existing inhibition section, after all triggers), and the final `*prev = s.crowding as f32;`.
- `nearest_kinship` is `#[serde(skip)]` scratch but is recomputed every tick in `sense_one`, so it is valid here (affect runs post-sense).
- `develop_all` still early-returns when `!world.affect_enabled` (M-A) — keep that guard.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core --lib affect::tests::develop_all_writes_care_panic_and_prev_crowding`
Expected: PASS.

- [ ] **Step 5: Run the whole affect unit suite (M-A..M-D triggers still hold)**

Run: `cargo test -p anabios-core --lib affect::`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): CARE/PANIC triggers + PANIC-SEEK inhibition in develop_all (M-D)"
```

---

### Task 5: CARE read-side bias in `apply_affect` (provision + protect)

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (`apply_affect`)
- Test: `crates/anabios-core/src/affect.rs` (inline tests)

**Interfaces:**
- Consumes: M-A's `apply_affect(action, affect, genome, sensors, energy)`; `SensorRegister.{nearest_same_id, nearest_same_dir}`; `NO_NEIGHBOR_ID` (sense.rs); `NO_TARGET` (program); `CARE`, `K_CARE_SHARE`, `K_CARE_APPROACH`.
- Produces: when `affect[CARE] != 0.0`, `action.share_intent` is raised toward kin, movement biased to stay near kin, and `target_id` filled to the kin when otherwise unset. Exact identity when `affect[CARE] == 0.0`.

- [ ] **Step 1: Write the failing test**

Add to the inline `tests` module in `affect.rs`:

```rust
    #[test]
    fn apply_affect_care_shares_and_stays_near_kin() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::{ActionRegister, NO_TARGET};
        use crate::sense::SensorRegister;

        let g = Genome::neutral();
        let mut sensors = SensorRegister::default();
        sensors.nearest_same_id = 7;
        sensors.nearest_same_dir = Vec2::new(1.0, 0.0);

        // Neutral affect → exact identity.
        let mut base = ActionRegister::default();
        let affect_zero = [0.0; AFFECT_SYSTEMS];
        apply_affect(&mut base, &affect_zero, &g, &sensors, 30.0);
        assert_eq!(base.share_intent, 0.0);
        assert_eq!(base.move_x, 0.0);
        assert_eq!(base.target_id, NO_TARGET);

        // Active CARE → sharing raised, movement toward kin, target filled.
        let mut act = ActionRegister::default();
        let mut affect = [0.0; AFFECT_SYSTEMS];
        affect[CARE] = 1.0;
        apply_affect(&mut act, &affect, &g, &sensors, 30.0);
        assert!(act.share_intent > 0.0, "CARE should raise share_intent");
        assert!(act.move_x > 0.0, "CARE should bias movement toward kin");
        assert_eq!(act.target_id, 7, "CARE directs the share at the kin");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib affect::tests::apply_affect_care_shares_and_stays_near_kin`
Expected: FAIL — CARE branch not implemented; `share_intent`/`move_x`/`target_id` unchanged.

- [ ] **Step 3: Add the CARE block to `apply_affect`**

Inside `apply_affect`, after the M-A/M-B/M-C blocks, add (import `NO_NEIGHBOR_ID` / `NO_TARGET` at the top of `affect.rs` if not already present):

```rust
    // --- M-D: CARE — provision + protect kin (identity at neutral CARE) ---
    let care = affect[CARE];
    if care != 0.0 && sensors.nearest_same_id != crate::sense::NO_NEIGHBOR_ID {
        // Protect: bias movement to stay near the kin.
        action.move_x += K_CARE_APPROACH * care * sensors.nearest_same_dir.x;
        action.move_y += K_CARE_APPROACH * care * sensors.nearest_same_dir.y;
        // Provision: raise sharing. `share_pass` (interact.rs) transfers to
        // `target_id` when it clears SHARE_THRESHOLD and Altruism > 0; direct
        // the share at the kin when the program left no target.
        action.share_intent += K_CARE_SHARE * care;
        if action.target_id == crate::program::NO_TARGET {
            action.target_id = sensors.nearest_same_id;
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core --lib affect::tests::apply_affect_care_shares_and_stays_near_kin`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): CARE read-side bias (share + protect) in apply_affect (M-D)"
```

---

### Task 6: PANIC read-side bias in `apply_affect` (distress signal + reunion)

**Files:**
- Modify: `crates/anabios-core/src/affect.rs` (`apply_affect`)
- Test: `crates/anabios-core/src/affect.rs` (inline tests)

**Interfaces:**
- Consumes: M-A's `apply_affect`; `culture::ALARM_MEME_CHANNEL`; `PANIC`, `PANIC_PHEROMONE_CHANNEL`, `K_PANIC_EMIT`, `K_PANIC_BROADCAST`, `K_PANIC_REUNION`; `SensorRegister.{nearest_same_id, nearest_same_dir}`.
- Produces: when `affect[PANIC] != 0.0`, `action.emit_intent[PANIC_PHEROMONE_CHANNEL]` and `action.broadcast_intent[ALARM_MEME_CHANNEL]` are raised and movement is biased toward the nearest same-species neighbour. Exact identity when `affect[PANIC] == 0.0`.

- [ ] **Step 1: Write the failing test**

Add to the inline `tests` module in `affect.rs`:

```rust
    #[test]
    fn apply_affect_panic_signals_and_seeks_reunion() {
        use crate::culture::ALARM_MEME_CHANNEL;
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::ActionRegister;
        use crate::sense::SensorRegister;

        let g = Genome::neutral();
        let mut sensors = SensorRegister::default();
        sensors.nearest_same_id = 3;
        sensors.nearest_same_dir = Vec2::new(0.0, 1.0);

        // Neutral affect → exact identity.
        let mut base = ActionRegister::default();
        apply_affect(&mut base, &[0.0; AFFECT_SYSTEMS], &g, &sensors, 30.0);
        assert_eq!(base.emit_intent[PANIC_PHEROMONE_CHANNEL], 0.0);
        assert_eq!(base.broadcast_intent[ALARM_MEME_CHANNEL], 0.0);
        assert_eq!(base.move_y, 0.0);

        // Active PANIC → distress pheromone + alarm broadcast + reunion move.
        let mut act = ActionRegister::default();
        let mut affect = [0.0; AFFECT_SYSTEMS];
        affect[PANIC] = 1.0;
        apply_affect(&mut act, &affect, &g, &sensors, 30.0);
        assert!(act.emit_intent[PANIC_PHEROMONE_CHANNEL] > 0.0, "PANIC emits distress pheromone");
        assert!(act.broadcast_intent[ALARM_MEME_CHANNEL] > 0.0, "PANIC broadcasts alarm");
        assert!(act.move_y > 0.0, "PANIC biases movement toward nearest same-species (reunion)");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib affect::tests::apply_affect_panic_signals_and_seeks_reunion`
Expected: FAIL — PANIC branch not implemented.

- [ ] **Step 3: Add the PANIC block to `apply_affect`**

Inside `apply_affect`, after the CARE block:

```rust
    // --- M-D: PANIC/GRIEF — distress signal + reunion (identity at neutral) ---
    let panic = affect[PANIC];
    if panic != 0.0 {
        // Distress signal on both live broadcast channels. Consumption is
        // module-gated downstream: `deposit_pass` (interact.rs) needs a
        // Pheromone module; the AlarmCall detector (M-F) reads the alarm
        // broadcast from Communicators. Writing the intent is affect's job.
        action.emit_intent[PANIC_PHEROMONE_CHANNEL] += K_PANIC_EMIT * panic;
        action.broadcast_intent[crate::culture::ALARM_MEME_CHANNEL] += K_PANIC_BROADCAST * panic;
        // Reunion: bias movement toward the nearest same-species neighbour.
        if sensors.nearest_same_id != crate::sense::NO_NEIGHBOR_ID {
            action.move_x += K_PANIC_REUNION * panic * sensors.nearest_same_dir.x;
            action.move_y += K_PANIC_REUNION * panic * sensors.nearest_same_dir.y;
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core --lib affect::tests::apply_affect_panic_signals_and_seeks_reunion`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/affect.rs
git commit -m "feat(affect): PANIC read-side bias (distress + reunion) in apply_affect (M-D)"
```

---

### Task 7: Flag-on scenario + end-to-end behavior test

**Files:**
- Create: `scenarios/affect-social.toml`
- Create: `crates/anabios-core/tests/affect_social.rs`

**Interfaces:**
- Consumes: `Scenario::parse_toml` / `instantiate` (M-A threads `affect_enabled`); `tick::step`; `culture::ALARM_MEME_CHANNEL`; `culture::MEME_BROADCAST_THRESHOLD`.
- Produces: an integration proof that a flag-on world produces CARE sharing pressure and a PANIC alarm broadcast; the scenario file the golden/save-load tests reuse.

- [ ] **Step 1: Write the scenario file**

Create `scenarios/affect-social.toml`. Mirror `scenarios/minimal.toml`'s structure (copy its fields), then set `affect_enabled = true` at top level and seed a modest population so clusters and isolates both occur. (Read `scenarios/minimal.toml` first and copy its exact key set; only the two changes below are M-D-specific.)

```toml
# Flag-on affect scenario for M-D (CARE + PANIC/GRIEF). Same base as minimal.toml
# but with the primitive-brain affect layer enabled.
name = "affect-social"
seed = 42
affect_enabled = true
# ... copy every other field verbatim from scenarios/minimal.toml ...
```

- [ ] **Step 2: Write the failing behavior test**

Create `crates/anabios-core/tests/affect_social.rs`:

```rust
//! M-D flag-on behavior: CARE (kin provision) + PANIC/GRIEF (isolation distress).

use anabios_core::affect::{CARE, PANIC};
use anabios_core::culture::{ALARM_MEME_CHANNEL, MEME_BROADCAST_THRESHOLD};
use anabios_core::genome::Genome;
use anabios_core::prelude::Vec2;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::world::World;

const AFFECT_SOCIAL: &str = include_str!("../../../scenarios/affect-social.toml");

#[test]
fn affect_social_scenario_enables_the_layer() {
    let w = Scenario::parse_toml(AFFECT_SOCIAL).expect("parse affect-social").instantiate();
    assert!(w.affect_enabled, "scenario must turn the affect layer on");
}

#[test]
fn isolated_social_agent_broadcasts_distress() {
    // A lone social agent, given a Communicator, should raise an alarm broadcast
    // once PANIC has accrued over a few ticks.
    let mut w = World::new(11);
    w.affect_enabled = true;
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    // Ensure it can broadcast (Communicator present in the starter kit or add one).
    for _ in 0..10 {
        step(&mut w);
    }
    assert!(w.agents.affect[id as usize][PANIC] > 0.0, "PANIC should have accrued");
    assert!(
        w.actions[id as usize].broadcast_intent[ALARM_MEME_CHANNEL] > MEME_BROADCAST_THRESHOLD,
        "isolated social agent should broadcast alarm above threshold"
    );
}

#[test]
fn kin_cluster_raises_care_and_sharing() {
    let mut w = World::new(12);
    w.affect_enabled = true;
    let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
    let _b = w.spawn_agent(Vec2::new(303.0, 300.0), Genome::neutral());
    let _c = w.spawn_agent(Vec2::new(300.0, 303.0), Genome::neutral());
    for _ in 0..5 {
        step(&mut w);
    }
    assert!(w.agents.affect[a as usize][CARE] > 0.0, "kin cluster should raise CARE");
    assert!(
        w.actions[a as usize].share_intent > 0.0,
        "CARE should push share_intent above zero for a clustered agent"
    );
}
```

- [ ] **Step 3: Run the tests to verify the expected failure mode**

Run: `cargo test -p anabios-core --test affect_social`
Expected: the scenario/behavior asserts drive the run. If `broadcast_intent` stays `0.0` because the starter kit lacks a Communicator, add a Communicator module in the test (`w.agents.modules[id as usize].push(...)`) rather than weakening the assert — the affect layer writes the intent regardless, but this test verifies an end-to-end above-threshold signal.

- [ ] **Step 4: Adjust modules/tuning so the behavior asserts pass**

If needed, give the lone agent a Communicator in `isolated_social_agent_broadcasts_distress`, and confirm `K_PANIC_BROADCAST * PANIC > MEME_BROADCAST_THRESHOLD` holds after ~10 ticks (PANIC saturates toward `social * isolation` ≈ 0.5 with neutral sociality; `LAMBDA_PANIC = 0.85` reaches ≈ 0.5·(1−0.85^10) ≈ 0.4 by tick 10, so lengthen the loop to ~20 ticks if the assert is marginal). Prefer lengthening the warm-up over lowering the threshold.

Run: `cargo test -p anabios-core --test affect_social`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add scenarios/affect-social.toml crates/anabios-core/tests/affect_social.rs
git commit -m "test(affect): flag-on CARE/PANIC behavior + affect-social scenario (M-D)"
```

---

### Task 8: save → load → step determinism test (covers `affect_prev_crowding`)

**Files:**
- Modify: `crates/anabios-core/tests/determinism.rs` (add one test, model `determinism.rs:16-36`)

**Interfaces:**
- Consumes: `save_to_bytes` / `load_from_bytes` / `state_hash` (snapshot.rs); `Scenario::parse_toml`; the `affect-social.toml` scenario (Task 7).
- Produces: a replay gate proving `affect` + `affect_prev_crowding` round-trip through serialization — a stale/dropped `affect_prev_crowding` would diverge the extra step.

- [ ] **Step 1: Write the failing test**

Add to `crates/anabios-core/tests/determinism.rs`:

```rust
/// The affect layer must survive a save→load→step round-trip: PANIC/GRIEF reads
/// last tick's crowding from the serialized `affect_prev_crowding` column, so a
/// dropped (or `#[serde(skip)]`) column would make a reloaded world panic-detect
/// differently on the next tick. Guards both affect columns against the
/// serde-skip replay footgun (still_ticks v13 precedent).
#[test]
fn affect_layer_survives_save_load_step() {
    const AFFECT: &str = include_str!("../../../scenarios/affect-social.toml");
    let mut world = Scenario::parse_toml(AFFECT).expect("parse affect scenario").instantiate();
    assert!(world.affect_enabled, "scenario must enable the affect layer");
    // Warm up so affect activations + prev_crowding are non-trivial.
    for _ in 0..300 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load must restore identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "affect world diverged after save→load→step (affect_prev_crowding not serialized?)",
    );
}
```

- [ ] **Step 2: Run test to verify it passes (or catches a bug)**

Run: `cargo test -p anabios-core --test determinism affect_layer_survives_save_load_step`
Expected: PASS. If it FAILS with a post-step divergence, `affect_prev_crowding` (or `affect`) is `#[serde(skip)]` or missing from an init site — fix Task 1 before proceeding (do not silence the test).

- [ ] **Step 3: Commit**

```bash
git add crates/anabios-core/tests/determinism.rs
git commit -m "test(affect): save-load-step covers affect_prev_crowding (M-D)"
```

---

### Task 9: FORMAT_VERSION bump + changelog + refresh the three layout goldens + parallel-scenario add

**Files:**
- Modify: `crates/anabios-core/src/snapshot.rs` (`FORMAT_VERSION` + changelog)
- Modify: `crates/anabios-core/tests/determinism.rs` (`GOLDEN`; add scenario to `parallel_matches_serial_across_thread_counts`)
- Modify: `crates/anabios-core/tests/cognition.rs` (`COGNITIVE_GOLDEN`)
- Modify: `crates/anabios-core/tests/inventions.rs` (its golden hash array)

**Interfaces:**
- Consumes: everything above; `UPDATE_HASHES=1` regeneration path.
- Produces: goldens re-pinned for the new serialized layout, with a dated flag-off byte-identical note; `FORMAT_VERSION` incremented; the affect par-path exercised under 1/2/8 threads.

**This is a controller task** (runs `cargo` + `UPDATE_HASHES`, edits committed hashes).

- [ ] **Step 1: Bump `FORMAT_VERSION` and add the changelog entry**

In `snapshot.rs`, increment `FORMAT_VERSION` by 1 above its current value (read the current line — M-A..M-C precede M-D; do NOT hard-code a number, increment whatever is there). Add the `///` changelog line above the const:

```rust
/// vNN: affect layer M-D — AgentBuffers.affect_prev_crowding (new serialized
///      f32 column: one-tick crowding memory for PANIC/GRIEF separation
///      detection). affect_enabled is off in every golden scenario ⇒
///      develop_all early-returns, the column stays 0.0, and behaviour is
///      byte-identical; only the serialized layout grew.
```

- [ ] **Step 2: Add the affect-social scenario to the parallel test**

In `parallel_matches_serial_across_thread_counts` (`determinism.rs:176`), add the new scenario to the `for scenario_src in [...]` list so the affect `par_iter` (index-disjoint `affect` + `affect_prev_crowding` writes) is checked across thread counts:

```rust
        include_str!("../../../scenarios/affect-social.toml"),
```

- [ ] **Step 3: Run the parallel test (must already pass — no refresh)**

Run: `cargo test -p anabios-core --test determinism parallel_matches_serial_across_thread_counts`
Expected: PASS. A failure here means a real cross-thread ordering bug in the M-D `develop_all` zip — fix it, do not refresh anything.

- [ ] **Step 4: Regenerate the three layout goldens**

The new serialized column grows the bincode payload, so the minimal / cognitive / inventions goldens each move ONCE (layout growth; flags off ⇒ behaviour byte-identical). Regenerate:

```bash
UPDATE_HASHES=1 cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes
UPDATE_HASHES=1 cargo test -p anabios-core --test cognition cognitive_scenario_matches_golden_hashes
UPDATE_HASHES=1 cargo test -p anabios-core --test inventions
```

Paste the printed `(tick, hash)` triples into `GOLDEN` (`determinism.rs:162`), `COGNITIVE_GOLDEN` (`cognition.rs:93`), and the inventions golden array. Add a dated note above each, e.g.:

```rust
    // Refreshed 2026-08-02 (affect M-D, FORMAT_VERSION NN): added
    // AgentBuffers.affect_prev_crowding serialized column. affect_enabled off in
    // every golden scenario ⇒ develop_all no-op, column stays 0.0 — trajectory
    // byte-identical, only the serialized layout grew, so all hashes moved once.
```

- [ ] **Step 5: Verify the goldens are now pinned**

Run: `cargo test -p anabios-core --test determinism && cargo test -p anabios-core --test cognition && cargo test -p anabios-core --test inventions`
Expected: PASS with the refreshed hashes (run WITHOUT `UPDATE_HASHES`).

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/snapshot.rs crates/anabios-core/tests/determinism.rs \
        crates/anabios-core/tests/cognition.rs crates/anabios-core/tests/inventions.rs
git commit -m "chore(affect): bump FORMAT_VERSION + refresh layout goldens (M-D)"
```

---

### Task 10: Flag-on golden pin for the affect-social scenario

**Files:**
- Modify: `crates/anabios-core/tests/affect_social.rs` (add a golden-hash test)

**Interfaces:**
- Consumes: `state_hash`; `Scenario::parse_toml` / `instantiate`; `affect-social.toml`.
- Produces: a byte-level pin of the M-D flag-ON trajectory, so future affect changes that alter real behaviour are caught deliberately (model: `cognition.rs`).

**Golden regeneration is a controller step** (`UPDATE_HASHES`).

- [ ] **Step 1: Write the golden test with placeholder hashes**

Add to `crates/anabios-core/tests/affect_social.rs`:

```rust
use anabios_core::snapshot::state_hash;

/// Flag-ON trajectory pin for the affect layer (CARE + PANIC live). Regenerate
/// with `UPDATE_HASHES=1` when the affect behaviour changes deliberately.
const AFFECT_GOLDEN: &[(u64, u64)] = &[(0, 0), (100, 0), (300, 0)];

#[test]
fn affect_social_matches_golden_hashes() {
    let mut w = Scenario::parse_toml(AFFECT_SOCIAL).expect("parse affect-social").instantiate();
    let max_tick = AFFECT_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < AFFECT_GOLDEN.len() && AFFECT_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        for (t, h) in &observed {
            println!("({t}, {h:#018x}),");
        }
    }
    assert_eq!(observed, AFFECT_GOLDEN.to_vec(), "affect flag-on trajectory changed");
}
```

- [ ] **Step 2: Regenerate the flag-on golden**

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test affect_social affect_social_matches_golden_hashes -- --nocapture`
Paste the printed `(tick, hash)` triples into `AFFECT_GOLDEN`.

- [ ] **Step 3: Verify the pin holds**

Run: `cargo test -p anabios-core --test affect_social affect_social_matches_golden_hashes`
Expected: PASS with the real hashes.

- [ ] **Step 4: Full-suite regression sweep**

Run: `cargo test -p anabios-core`
Expected: PASS (unit + determinism + cognition + inventions + affect_social).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/tests/affect_social.rs
git commit -m "test(affect): flag-on golden pin for affect-social (M-D)"
```

---

## Self-review notes

- **Spec coverage (§4.5 CARE, §4.6 PANIC/GRIEF, §5.1 column, contract M-D row).**
  - CARE trigger (kin present via `nearest_kinship` + `nearest_same_dist`, Nurturance gain) → Task 2; wired in `develop_all` → Task 4. CARE bias (share toward kin `target_id`, protect movement) → Task 5. ✓
  - `affect_prev_crowding` new serialized column, default 0.0, init in both spawn branches + dead-slot reset, serialized (not skip), updated at end of `develop_all` → Tasks 1 & 4. ✓
  - PANIC/GRIEF trigger (isolation ×/or crowding-drop × Sociality) → Task 3; wired + prev-crowding write → Task 4. PANIC bias (distress `emit_intent` + alarm `broadcast_intent`, reunion movement) → Task 6. PANIC⊣SEEK inhibition → Task 4. ✓
  - Determinism: FORMAT_VERSION bump + 3 layout goldens refreshed once with dated note + flag-on golden + save→load→step + parallel-still-passes → Tasks 8, 9, 10. ✓
- **Live-channels-only invariant:** CARE writes `share_intent`, `move_x/move_y`, `target_id` (fill-when-unset only); PANIC writes `emit_intent`, `broadcast_intent`, `move_x/move_y`. No `feed_intent`/`mate_intent`. ✓
- **Neutral identity:** every read-side block guarded `if care != 0.0` / `if panic != 0.0`; when `affect_enabled` is off, `develop_all` no-ops so the affect columns stay all-zero and `apply_affect` is exact identity — flag-off byte-identical. ✓
- **Zero RNG:** `care_trigger`/`panic_trigger` and the `develop_all` closure are pure over sensors + genome + prev-crowding; no `rng` touched. ✓
- **Index-disjoint parallelism:** `develop_all` zips `affect[i]` + `affect_prev_crowding[i]` (same `i`), reads shared fields by `&` — mirrors the `iq` stage; Task 9 adds the scenario to the 1/2/8-thread gate. ✓
- **Type consistency:** `care_trigger(f32,f32,f32)->f32`, `panic_trigger(f32,f32,f32)->f32`, `apply_affect(action, affect, genome, sensors, energy)` (contract), `affect_prev_crowding: Vec<f32>`, indices `CARE`/`PANIC`/`SEEK` and constants used identically across Tasks 2–6. `nurturance()`/`sociality()` and `GenomeSlot::Sociality` are M-A accessors (Dependencies). ✓
- **Deferred to M-F (documented, not a gap):** the spec's "grief/separation *event*" bar is codex-detector territory (M-F owns EventTypes/detectors, spec §7.1 + contract "new EventType append at END only"). M-D proves the same behaviour at the action level — a PANIC agent's above-threshold alarm `broadcast_intent[ALARM_MEME_CHANNEL]` (Task 7) is exactly what the M-F `AlarmCall`/mass-grief detector will consume — without adding an enum variant here. No `Node`/`EventType` added in M-D, honouring the append-only constraint. ✓
- **Consumption caveats surfaced for the implementer:** `share_pass` needs `Altruism > 0` + target within `SHARE_RANGE`; `deposit_pass` needs a Pheromone module; `AlarmCall` needs a Communicator. Affect writes the intent regardless; Task 7 adds the module where an end-to-end above-threshold signal is asserted, rather than weakening asserts. ✓
- **Merge discipline:** Task 4 explicitly INSERTS into the shared `develop_all` and Tasks 5/6 into the shared `apply_affect` — do not delete M-A..M-C blocks or the FEAR⊣RAGE edge. Called out in-task. ✓
