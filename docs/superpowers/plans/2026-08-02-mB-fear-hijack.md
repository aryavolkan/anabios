# M-B: Threat/Survival Circuit — FEAR + Hijack — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Panksepp FEAR survival circuit on top of M-A's affect framework: FEAR activation (leaky integrator, Boldness-gated setpoint), a finalized threat `arousal`, a soft flee/dampen bias, and the hard Bracha reflex `apply_hijack` (Freeze→Flight→Fight→Fright/Faint) that overwrites live action channels under high threat-arousal. Add a `MassFright` codex event + detector that fires in a flag-on predator/prey scenario. Flag-off stays byte-identical to the M-A baseline (M-B adds **zero** new serialized state).

**Architecture:** Consume M-A's `affect.rs` module, `AffectState` column, `develop_all`, `apply_affect`, `arousal` stub, `affect_enabled` flag, and the temperament genome accessors (`boldness()`, `reactivity()`). Extend `develop_all` to write the FEAR slot from **fresh, replay-safe** sensor signals (`hostility`, threatening `nearest_other_dir` with large `nearest_rel_size`/`nearest_rel_energy`), finalize `arousal` over the defensive activations, extend `apply_affect` with the FEAR flee/dampen bias, and add `apply_hijack`. Wire the bias + hijack into `decide_all` (`tick.rs`) gated on `affect_enabled`. Add `EventType::MassFright` (END of enum) + a **latch-free** detector (deduped against the already-serialized `codex.events` log, so no new serialized field) into `observe_all`. New flag-on scenario `scenarios/affect-threat.toml` + a golden/behavior test pins the real behavior.

**Tech Stack:** Rust, anabios-core, rayon, serde/bincode, cargo test.

## Global Constraints
- single Xoshiro256++ RNG; affect/hijack draw ZERO RNG; flag-off byte-identical; hijack overwrites only LIVE channels; neutral-identity guarded; new EventType appends at END of enum; append-only; golden refresh via UPDATE_HASHES=1; controller runs all cargo/git/golden/commit gates.

## Dependencies
- Requires M-A (affect module, AffectState column, develop_all, apply_affect, world.affect_enabled) already merged.

---

## Deviations from the spec (verified against source — read before implementing)

Two spec/contract items are intentionally scoped in a replay-safe way for M-B. Both are documented here and in the Self-review notes:

1. **Combat-damage FEAR term is DEFERRED (not omitted by accident).** Spec §4.2 lists "damage taken this tick (combat scratch)" as a FEAR trigger. Source check: `World.combat_damaged` (`world.rs:236`) is `#[serde(skip)]` per-tick scratch, reset at the top of `interact_all` (`interact.rs:35`) and set in `combat_pass` (`interact.rs:208`) — i.e. **stage 5**, after `decide` (stage 3). `develop_all` runs post-sense/pre-decide (**stage ~2.5**), so it can only ever read the *previous* tick's `combat_damaged`. Folding a serde-skip cross-tick scratch value into the **serialized** `affect[FEAR]` accumulator is exactly the serde-skip replay footgun (spec §6.3; the v13 still_ticks precedent): a `save→load→step` loses `combat_damaged` (resets to `false`) and the reloaded world's next FEAR update diverges from a continuous run. A correct combat term needs a *serialized* per-agent "recently-damaged" signal, which is deliberate layout growth and out of M-B's no-new-serialized-state scope. **M-B triggers FEAR from `hostility` + threatening neighbor only** — both come from `world.sensors`, which `sense_all` recomputes fresh **every tick before** `develop_all` reads it (so even the `#[serde(skip)] hostility` field is replay-safe at read time). This is sufficient to drive the whole circuit: a large, close, hostile other-species neighbor raises FEAR → arousal → hijack → mass fright. The combat term is left for a follow-up that adds a serialized damage bit.

2. **`MassFright` detector is LATCH-FREE (no new serialized `CodexState` field).** Every existing edge-triggered detector stores a serialized `BTreeSet`/`bool` latch (e.g. `herd_active`, `predation_emitted`) — and every such addition moved all three determinism goldens (see the `snapshot.rs` v9–v21 changelog). Because the task requires flag-off to stay byte-identical to the M-A baseline (no further golden move, no `FORMAT_VERSION` bump), M-B must add **zero** serialized state. Resolution: dedup the event against the **already-serialized** `codex.events` ring (`mod.rs:273`, not `#[serde(skip)]`) with a cooldown scan — replay-safe (events survive save→load), deterministic, and adds no field. Flag-off pushes no events, so `events` stays empty ⇒ byte-identical.

## File Structure

- **Edit** `crates/anabios-core/src/affect.rs` — add M-B constants; `fear_trigger()`; finalize `arousal()`; extend `develop_all` (FEAR slot); extend `apply_affect` (FEAR flee/dampen); add `apply_hijack()`.
- **Edit** `crates/anabios-core/src/tick.rs` — call `apply_hijack` in `decide_all` after the movement biases, before normalization (~`tick.rs:252`→`253`), gated on `affect_enabled`. (M-A already wired `apply_affect` after `apply_personality`; M-B's FEAR bias rides that existing call.)
- **Edit** `crates/anabios-core/src/codex/event.rs` — append `EventType::MassFright = 53`; bump `EVENT_TYPE_COUNT`.
- **New** `crates/anabios-core/src/codex/affect.rs` — `detect_mass_fright()`. Register `mod affect;` in `codex/mod.rs` and call it in `observe_all`.
- **New** `scenarios/affect-threat.toml` — flag-on (`affect_enabled = true`) predator/prey scenario.
- **New** `crates/anabios-core/tests/affect.rs` — flag-on self-consistency golden, `save→load→step`, and the `MassFright`-fires behavior test.
- **No edits** to `agent.rs`, `world.rs`, `scenario.rs`, `genome.rs`, `snapshot.rs` (`FORMAT_VERSION` unchanged), or the `determinism.rs`/`cognition.rs`/`inventions.rs` goldens — M-B adds no serialized layout.

Interfaces this milestone **consumes** from M-A: `AffectState`, `AFFECT_SYSTEMS`, `SEEK/FEAR/RAGE/LUST/CARE/PANIC/PLAY` indices, `HIJACK_AROUSAL_THRESHOLD`, `LAMBDA_DEFAULT`, `homeostatic_drive`, `develop_all`, `apply_affect`, `arousal`, `affect_speed_factor`, `world.affect_enabled`, `agents.affect`, `Genome::{boldness, reactivity}`. Interfaces this milestone **produces**: `apply_hijack`, `fear_trigger`, finalized `arousal`, `EventType::MassFright`, `codex::affect::detect_mass_fright`.

---

## Task 1 — FEAR trigger (`fear_trigger`) + M-B constants

**Files:** `crates/anabios-core/src/affect.rs`
**Interfaces:** Produces `pub(crate) fn fear_trigger(&SensorRegister, &Genome) -> f32`; consumes M-A constants + `Genome::boldness`.

- [ ] Add a failing unit test to `affect.rs`'s `#[cfg(test)] mod tests`:

```rust
#[test]
fn fear_trigger_rises_with_close_big_hostile_threat_and_falls_with_boldness() {
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;
    use crate::sense::{SensorRegister, NO_NEIGHBOR_ID};

    // No neighbor ⇒ no fear.
    let mut s = SensorRegister::default();
    assert_eq!(fear_trigger(&s, &Genome::neutral()), 0.0);

    // A large, close, hostile other-species neighbor ⇒ strong fear.
    s.nearest_other_id = 7;
    s.nearest_neighbor_id = 7;
    s.nearest_other_dist = 20.0;
    s.nearest_other_dir = Vec2::new(1.0, 0.0);
    s.nearest_rel_size = 2.0; // predator twice our size
    s.nearest_rel_energy = 1.5;
    s.hostility = 0.5;
    let neutral = fear_trigger(&s, &Genome::neutral());
    assert!(neutral > 0.4, "expected strong fear, got {neutral}");
    assert!(neutral <= 1.0);

    // A bold genome (Boldness slot = 1.0 ⇒ boldness() = +1.0) feels LESS fear.
    let mut bold = Genome::neutral();
    bold.set(GenomeSlot::Boldness, 1.0);
    let brave = fear_trigger(&s, &bold);
    assert!(brave < neutral, "boldness must lower the FEAR setpoint: {brave} !< {neutral}");

    // A distant neighbor is barely threatening.
    let mut far = s;
    far.nearest_other_dist = FEAR_RANGE * 2.0;
    assert!(fear_trigger(&far, &Genome::neutral()) < neutral);
}
```

- [ ] Run `cargo test -p anabios-core fear_trigger_rises` → **expect FAIL** (`fear_trigger`, `FEAR_RANGE` unresolved).
- [ ] Add the constants + function (near M-A's constants; keep M-A's block intact):

```rust
// --- M-B: FEAR / threat circuit ---
/// Perception distance (world units) beyond which a threatening neighbor stops
/// contributing to FEAR. Below it, proximity scales the threat linearly.
pub const FEAR_RANGE: f32 = 200.0;
/// Weight of the size/proximity threat term in the FEAR trigger.
pub const K_FEAR_THREAT: f32 = 1.0;
/// Weight of the war-hostility term in the FEAR trigger.
pub const K_FEAR_HOSTILITY: f32 = 0.8;
/// How strongly Boldness lowers the effective threat (raises the FEAR setpoint).
pub const K_FEAR_BOLDNESS: f32 = 0.3;
/// FEAR leaky-integrator retention (how long fear lingers). Reuses the default.
pub const LAMBDA_FEAR: f32 = LAMBDA_DEFAULT;

/// Instantaneous FEAR drive from THIS tick's fresh sensors + temperament.
/// Pure function of `world.sensors` (recomputed every tick before `develop_all`
/// reads it) + genome ⇒ replay-safe, ZERO RNG. Returns `[0,1]`. `0.0` when there
/// is no locatable other-species neighbor and no hostility.
pub(crate) fn fear_trigger(sensors: &SensorRegister, genome: &Genome) -> f32 {
    let mut threat = 0.0f32;
    if sensors.nearest_other_id != crate::sense::NO_NEIGHBOR_ID {
        // Closer + bigger + more energetic other-species neighbor ⇒ scarier.
        // rel_size/rel_energy are of the overall-nearest neighbor; when the
        // nearest is the other-species predator (the case that matters) they
        // describe it. Approximation documented in the spec-deviation note.
        let prox = (1.0 - sensors.nearest_other_dist / FEAR_RANGE).clamp(0.0, 1.0);
        let size = sensors.nearest_rel_size.clamp(0.0, 2.0) * 0.5; // rel 2.0 ⇒ 1.0
        let ener = (sensors.nearest_rel_energy - 1.0).clamp(0.0, 1.0); // stronger prey feels safe
        threat += K_FEAR_THREAT * prox * (size + 0.5 * ener).min(1.0);
    }
    threat += K_FEAR_HOSTILITY * sensors.hostility;
    // Boldness raises the setpoint: a bold agent needs a bigger raw threat to
    // register the same fear. boldness() is signed [-1,+1], neutral 0.0.
    threat = (threat - K_FEAR_BOLDNESS * genome.boldness()).clamp(0.0, 1.0);
    threat
}
```

- [ ] Ensure `use` items exist at the top of `affect.rs`: `crate::genome::Genome`, `crate::sense::SensorRegister` (M-A likely already imports these; add only if missing).
- [ ] Run `cargo test -p anabios-core fear_trigger_rises` → **expect PASS**.
- [ ] Controller commits: `M-B: FEAR trigger from fresh threat sensors + Boldness setpoint`.

## Task 2 — Fold FEAR into `develop_all`'s leaky integrator

**Files:** `crates/anabios-core/src/affect.rs`
**Interfaces:** Extends M-A's `develop_all`; consumes `fear_trigger`, `LAMBDA_FEAR`, `FEAR`.

- [ ] Add a failing integration test to `affect.rs` tests (drives the real stage through a `World`):

```rust
#[test]
fn develop_all_raises_fear_when_a_predator_looms() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::world::World;

    let mut w = World::new(3);
    w.affect_enabled = true;
    // Small prey next to a big predator of a different species.
    let prey = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let pred = w.spawn_agent(Vec2::new(516.0, 500.0), Genome::neutral());
    crate::prelude_test::reassign_to_new_species(&mut w, pred);
    w.agents.genome[pred as usize].set(crate::genome::GenomeSlot::Size, 1.0);

    // Advance a few ticks so sense→develop_all runs and FEAR integrates up.
    for _ in 0..20 {
        crate::tick::step(&mut w);
    }
    let fear = w.agents.affect[prey as usize][FEAR];
    assert!(fear > 0.1, "prey next to a looming predator should build FEAR, got {fear}");
}
```
> Adjust the "size" genome slot name (`Size`) to the real one in `genome.rs`; the test only needs the predator to read as larger. If `prelude_test::reassign_to_new_species` is not exported, spawn the predator far, mutate its species via the existing test helper used by `codex/mod.rs` tests.

- [ ] Run `cargo test -p anabios-core develop_all_raises_fear` → **expect FAIL** (FEAR stays 0 — M-A left it at 0.0).
- [ ] In `develop_all`'s per-agent update, set the FEAR slot's trigger to `fear_trigger(&sensors[i], &genome[i])` inside the existing leaky-integrator update, e.g. where M-A dispatches per-system triggers:

```rust
// FEAR (M-B): threat/survival drive from fresh sensors + Boldness.
let fear_in = fear_trigger(s, g);
a[FEAR] = (LAMBDA_FEAR * a[FEAR] + (1.0 - LAMBDA_FEAR) * fear_in).clamp(0.0, 1.0);
```
> Match M-A's local names for the sensor (`s`), genome (`g`), and activation array (`a`). If M-A already writes `a[FEAR]` as a `0.0` placeholder, replace that line. Keep the write inside the same index-disjoint `par_iter` body (writes only slot `i`; ZERO RNG).

- [ ] Run `cargo test -p anabios-core develop_all_raises_fear` → **expect PASS**.
- [ ] Controller commits: `M-B: wire FEAR trigger into develop_all leaky integrator`.

## Task 3 — Finalize `arousal` over defensive activations

**Files:** `crates/anabios-core/src/affect.rs`
**Interfaces:** Finalizes M-A's `pub fn arousal(&AffectState) -> f32`.

> Open decision §9.5 (max vs softmax) resolved here: **max** — cheapest, deterministic, monotone, and adequate for the single-defensive-system M-B.

- [ ] Add a failing test:

```rust
#[test]
fn arousal_is_max_of_defensive_activations() {
    let mut a: AffectState = [0.0; AFFECT_SYSTEMS];
    assert_eq!(arousal(&a), 0.0); // neutral
    a[SEEK] = 0.9; // appetitive, not defensive
    assert_eq!(arousal(&a), 0.0, "SEEKING must not raise threat arousal");
    a[FEAR] = 0.7;
    assert_eq!(arousal(&a), 0.7);
    a[PANIC] = 0.8;
    assert_eq!(arousal(&a), 0.8, "PANIC dominates");
    a[RAGE] = 0.85;
    assert_eq!(arousal(&a), 0.85, "RAGE dominates");
}
```

- [ ] Run `cargo test -p anabios-core arousal_is_max` → **expect FAIL** (M-A stub returns 0.0/SEEK-free baseline).
- [ ] Replace the `arousal` body:

```rust
/// Aggregate threat arousal from the defensive activations (FEAR, RAGE, PANIC).
/// `max` of the three — SEEKING and the affiliative systems do not raise threat.
/// Exactly `0.0` at neutral. ZERO RNG.
pub fn arousal(affect: &AffectState) -> f32 {
    affect[FEAR].max(affect[RAGE]).max(affect[PANIC])
}
```

- [ ] Run `cargo test -p anabios-core arousal_is_max` → **expect PASS**. Re-run Task 2's `develop_all_raises_fear` (arousal now feeds `affect_speed_factor`; still passes).
- [ ] Controller commits: `M-B: finalize threat arousal as max(FEAR,RAGE,PANIC)`.

## Task 4 — FEAR flee/dampen bias in `apply_affect`

**Files:** `crates/anabios-core/src/affect.rs`
**Interfaces:** Extends M-A's `apply_affect(action, affect, genome, sensors, energy)`.

- [ ] Add a failing test:

```rust
#[test]
fn apply_affect_fear_flees_and_dampens_but_is_identity_at_neutral() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::program::ActionRegister;
    use crate::sense::SensorRegister;

    let g = Genome::neutral();
    let mut s = SensorRegister::default();
    s.nearest_other_id = 4;
    s.nearest_other_dir = Vec2::new(1.0, 0.0); // threat to the +x

    // Neutral affect ⇒ exact identity (no arithmetic).
    let mut neutral_act = ActionRegister::default();
    neutral_act.move_x = 0.3;
    neutral_act.share_intent = 0.5;
    let before = neutral_act;
    apply_affect(&mut neutral_act, &[0.0; AFFECT_SYSTEMS], &g, &s, 100.0);
    assert_eq!(neutral_act.move_x, before.move_x);
    assert_eq!(neutral_act.share_intent, before.share_intent);

    // High FEAR ⇒ movement biased AWAY from the threat, share dampened.
    let mut a: AffectState = [0.0; AFFECT_SYSTEMS];
    a[FEAR] = 0.8;
    let mut act = ActionRegister::default();
    act.move_x = 0.0;
    act.share_intent = 0.5;
    act.broadcast_intent[0] = 0.4;
    apply_affect(&mut act, &a, &g, &s, 100.0);
    assert!(act.move_x < 0.0, "FEAR should push away from +x threat, got {}", act.move_x);
    assert!(act.share_intent < 0.5, "FEAR should dampen sharing");
    assert!(act.broadcast_intent[0] < 0.4, "FEAR should dampen broadcasts");
}
```

- [ ] Run `cargo test -p anabios-core apply_affect_fear_flees` → **expect FAIL**.
- [ ] Add M-B bias constants and the FEAR block inside `apply_affect` (append to M-A's SEEK logic; every effect guarded on `fear != 0.0` for neutral identity):

```rust
/// Flee-bias gain: FEAR pushes movement away from the threat direction.
pub const K_FLEE: f32 = 0.6;
/// Non-defensive-intent damping gain under FEAR (share/broadcast/emit).
pub const K_FEAR_DAMP: f32 = 0.5;
```
```rust
// FEAR (M-B): flee the nearest other-species neighbor and dampen non-defensive
// LIVE intents (share/broadcast/emit). Guarded so neutral affect is identity.
let fear = affect[FEAR];
if fear != 0.0 && sensors.nearest_other_id != crate::sense::NO_NEIGHBOR_ID {
    action.move_x -= K_FLEE * fear * sensors.nearest_other_dir.x;
    action.move_y -= K_FLEE * fear * sensors.nearest_other_dir.y;
    let damp = (1.0 - K_FEAR_DAMP * fear).max(0.0);
    action.share_intent *= damp;
    for c in action.broadcast_intent.iter_mut() {
        *c *= damp;
    }
    for c in action.emit_intent.iter_mut() {
        *c *= damp;
    }
}
```

- [ ] Run `cargo test -p anabios-core apply_affect_fear_flees` → **expect PASS**.
- [ ] Controller commits: `M-B: FEAR flee + non-defensive-intent dampen in apply_affect`.

## Task 5 — `apply_hijack`: gate + Freeze + Flight

**Files:** `crates/anabios-core/src/affect.rs`
**Interfaces:** Produces `pub fn apply_hijack(action, affect, genome, sensors, energy) -> bool` (contract signature).

- [ ] Add failing tests:

```rust
#[test]
fn apply_hijack_gate_and_freeze_flight() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::program::ActionRegister;
    use crate::sense::SensorRegister;

    let g = Genome::neutral();

    // Below-threshold arousal ⇒ no override, action untouched, returns false.
    let mut low: AffectState = [0.0; AFFECT_SYSTEMS];
    low[FEAR] = 0.3; // < HIJACK_AROUSAL_THRESHOLD (0.6)
    let mut s = SensorRegister::default();
    s.nearest_other_id = 2;
    s.nearest_other_dist = 100.0;
    s.nearest_other_dir = Vec2::new(1.0, 0.0);
    let mut act = ActionRegister::default();
    act.move_x = 0.9;
    assert!(!apply_hijack(&mut act, &low, &g, &s, 100.0));
    assert_eq!(act.move_x, 0.9, "no hijack below threshold");

    // High arousal, DISTANT threat ⇒ Freeze (zero movement), returns true.
    let mut hi: AffectState = [0.0; AFFECT_SYSTEMS];
    hi[FEAR] = 0.9;
    let mut freeze_s = s;
    freeze_s.nearest_other_dist = FREEZE_DIST + 10.0;
    let mut freeze_act = ActionRegister::default();
    freeze_act.move_x = 0.9;
    freeze_act.move_y = -0.4;
    assert!(apply_hijack(&mut freeze_act, &hi, &g, &freeze_s, 100.0));
    assert_eq!((freeze_act.move_x, freeze_act.move_y), (0.0, 0.0), "distant threat ⇒ Freeze");

    // High arousal, MID-RANGE threat ⇒ Flight (flee away from +x), returns true.
    let mut flight_s = s;
    flight_s.nearest_other_dist = (FREEZE_DIST + CORNER_DIST) * 0.5;
    let mut flight_act = ActionRegister::default();
    flight_act.move_x = 0.9; // was charging toward threat
    assert!(apply_hijack(&mut flight_act, &hi, &g, &flight_s, 100.0));
    assert!(flight_act.move_x < 0.0, "mid-range threat ⇒ flee -x, got {}", flight_act.move_x);
}
```

- [ ] Run `cargo test -p anabios-core apply_hijack_gate_and_freeze_flight` → **expect FAIL**.
- [ ] Add hijack constants + the gate/Freeze/Flight portion of `apply_hijack` (Fight/Faint filled in Task 6):

```rust
/// Reactivity raises hijack sensitivity; Boldness lowers it. Both signed [-1,1].
pub const K_HIJACK_REACT: f32 = 0.2;
pub const K_HIJACK_BOLD: f32 = 0.2;
/// Threat farther than this ⇒ Freeze (orient, don't be seen).
pub const FREEZE_DIST: f32 = 140.0;
/// Threat closer than this ⇒ cornered (Fight or Fright/Faint).
pub const CORNER_DIST: f32 = 30.0;

/// Survival-reflex override (Bracha: Freeze→Flight→Fight→Fright/Faint). When
/// threat arousal, scaled by Reactivity/Boldness, reaches
/// `HIJACK_AROUSAL_THRESHOLD`, OVERWRITE the LIVE action channels with the
/// reflex chosen by threat proximity/escapability and return `true`. Otherwise
/// leave `action` untouched and return `false`. ZERO RNG. Exact identity at
/// neutral affect (arousal 0 ⇒ returns false before touching `action`).
pub fn apply_hijack(
    action: &mut ActionRegister,
    affect: &AffectState,
    genome: &Genome,
    sensors: &SensorRegister,
    energy: f32,
) -> bool {
    let threat = arousal(affect);
    if threat <= 0.0 {
        return false;
    }
    // "low road" cancel path: bold/steady agents keep cortical control longer.
    let effective = threat + K_HIJACK_REACT * genome.reactivity() - K_HIJACK_BOLD * genome.boldness();
    if effective < HIJACK_AROUSAL_THRESHOLD {
        return false;
    }

    // No locatable threat ⇒ Freeze in place.
    if sensors.nearest_other_id == crate::sense::NO_NEIGHBOR_ID {
        action.move_x = 0.0;
        action.move_y = 0.0;
        return true;
    }
    let d = sensors.nearest_other_dist;
    let toward = sensors.nearest_other_dir;
    if d >= FREEZE_DIST {
        // Freeze — distant/ambiguous.
        action.move_x = 0.0;
        action.move_y = 0.0;
    } else if d > CORNER_DIST {
        // Flight — flee directly away; affect_speed_factor (arousal-driven)
        // supplies the speed boost in integrate.rs.
        action.move_x = -toward.x;
        action.move_y = -toward.y;
    } else {
        // Cornered — Fight vs Fright/Faint resolved in Task 6.
        return hijack_cornered(action, affect, sensors, energy);
    }
    let _ = energy;
    true
}
```
> Add a temporary `fn hijack_cornered(...) -> bool { action.move_x = 0.0; action.move_y = 0.0; true }` stub so this task compiles; Task 6 replaces it. (Keeps each task's build green.)

- [ ] Run `cargo test -p anabios-core apply_hijack_gate_and_freeze_flight` → **expect PASS**.
- [ ] Controller commits: `M-B: apply_hijack gate + Freeze + Flight reflexes`.

## Task 6 — `apply_hijack`: Fight + Fright/Faint (cornered)

**Files:** `crates/anabios-core/src/affect.rs`
**Interfaces:** Completes `apply_hijack` via `hijack_cornered`.

- [ ] Add failing tests:

```rust
#[test]
fn apply_hijack_cornered_fights_or_faints() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::program::{ActionRegister, NO_TARGET};
    use crate::sense::SensorRegister;

    let g = Genome::neutral();
    let mut hi: AffectState = [0.0; AFFECT_SYSTEMS];
    hi[FEAR] = 0.9;
    let mut s = SensorRegister::default();
    s.nearest_other_id = 11;
    s.nearest_other_dir = Vec2::new(1.0, 0.0);
    s.nearest_other_dist = CORNER_DIST * 0.5; // cornered

    // Cornered but able (energy high) ⇒ Fight: approach + fire + target set.
    let mut fight = ActionRegister::default();
    assert!(apply_hijack(&mut fight, &hi, &g, &s, 100.0));
    assert!(fight.move_x > 0.0, "Fight approaches the threat (+x)");
    assert!(fight.fire_intent > 0.0, "Fight fires");
    assert_eq!(fight.target_id, 11);
    assert_ne!(fight.target_id, NO_TARGET);

    // Cornered, extreme arousal, and too weak to fight ⇒ Fright/Faint:
    // tonic immobility, intents suppressed.
    let mut faint_aff: AffectState = [0.0; AFFECT_SYSTEMS];
    faint_aff[FEAR] = FAINT_AROUSAL + 0.01;
    let mut faint = ActionRegister::default();
    faint.move_x = 0.7;
    faint.fire_intent = 0.5;
    faint.share_intent = 0.5;
    faint.broadcast_intent[0] = 0.5;
    assert!(apply_hijack(&mut faint, &faint_aff, &g, &s, FIGHT_ENERGY_MIN - 1.0));
    assert_eq!((faint.move_x, faint.move_y), (0.0, 0.0), "Faint ⇒ tonic immobility");
    assert_eq!(faint.fire_intent, 0.0);
    assert_eq!(faint.share_intent, 0.0);
    assert_eq!(faint.broadcast_intent[0], 0.0);
}
```

- [ ] Run `cargo test -p anabios-core apply_hijack_cornered` → **expect FAIL** (stub freezes).
- [ ] Add constants + replace the `hijack_cornered` stub:

```rust
/// Cornered arousal at/above which a too-weak agent tips into Fright/Faint.
pub const FAINT_AROUSAL: f32 = 0.95;
/// Minimum energy to choose Fight (turn-and-attack) when cornered.
pub const FIGHT_ENERGY_MIN: f32 = 0.35 * crate::agent::SPAWN_ENERGY;
/// fire_intent asserted when the hijack chooses Fight.
pub const FIRE_HIJACK: f32 = 1.0;

/// Cornered branch: Fight when able, else Fright/Faint. Writes only LIVE
/// channels. Always overrides ⇒ returns true.
fn hijack_cornered(
    action: &mut ActionRegister,
    affect: &AffectState,
    sensors: &SensorRegister,
    energy: f32,
) -> bool {
    let extreme = arousal(affect) >= FAINT_AROUSAL;
    let can_fight = energy >= FIGHT_ENERGY_MIN;
    if extreme && !can_fight {
        // Fright/Faint — tonic immobility, suppress LIVE intents.
        action.move_x = 0.0;
        action.move_y = 0.0;
        action.fire_intent = 0.0;
        action.share_intent = 0.0;
        for c in action.emit_intent.iter_mut() {
            *c = 0.0;
        }
        for c in action.broadcast_intent.iter_mut() {
            *c = 0.0;
        }
    } else {
        // Fight — turn to the threat and attack.
        action.move_x = sensors.nearest_other_dir.x;
        action.move_y = sensors.nearest_other_dir.y;
        action.fire_intent = action.fire_intent.max(FIRE_HIJACK);
        action.target_id = sensors.nearest_other_id;
    }
    true
}
```
> Remove the `let _ = energy;` line from Task 5's `apply_hijack` now that `hijack_cornered` consumes it.

- [ ] Run `cargo test -p anabios-core apply_hijack` → **expect PASS** (all hijack tests).
- [ ] Controller commits: `M-B: apply_hijack Fight + Fright/Faint cornered reflexes`.

## Task 7 — Wire `apply_hijack` into `decide_all`

**Files:** `crates/anabios-core/src/tick.rs`
**Interfaces:** Consumes `apply_hijack`, `world.affect_enabled`, `agents.affect`.

- [ ] Add a failing behavior test to `tick.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn hijack_overrides_program_movement_under_threat() {
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::world::World;

    // Same seed/population, flag off vs on. With the flag on, a terrified prey's
    // desired_direction must diverge from the un-hijacked run.
    let build = |affect_on: bool| -> Vec2 {
        let mut w = World::new(9);
        w.affect_enabled = affect_on;
        let prey = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let pred = w.spawn_agent(Vec2::new(515.0, 500.0), Genome::neutral());
        crate::prelude_test::reassign_to_new_species(&mut w, pred);
        w.agents.genome[pred as usize].set(crate::genome::GenomeSlot::Size, 1.0);
        for _ in 0..25 {
            crate::tick::step(&mut w);
        }
        w.desired_direction[prey as usize]
    };
    let off = build(false);
    let on = build(true);
    assert_ne!(on, off, "affect_enabled must change the frightened prey's heading");
}
```
> Confirm the `reassign_to_new_species` import to match Task 2. If a heading equality is too strict, assert the prey's net displacement away from the predator instead.

- [ ] Run `cargo test -p anabios-core hijack_overrides_program_movement` → **expect FAIL**.
- [ ] In `decide_all`, capture the flag next to the other locals (`tick.rs:148-153`):

```rust
let affect_enabled = world.affect_enabled;
```
> M-A already borrows `agents` (`let agents = &world.agents;`) — `agents.affect[i]` is in scope inside the closure.

- [ ] Insert the hijack call **after** the livestock-pen override block and **before** the normalization (between `tick.rs:252` and `tick.rs:253`), so it overwrites the final movement biases (spec §3.1 ordering):

```rust
// Survival-reflex hijack (M-B, opt-in): under high threat-arousal, the Bracha
// reflex overwrites the movement intent + defensive intents chosen so far.
// Gated so flag-off stays byte-identical; zero RNG. Runs after every movement
// bias and before desired_direction normalization.
if affect_enabled {
    crate::affect::apply_hijack(
        &mut action,
        &agents.affect[i],
        &agents.genome[i],
        &sensors[i],
        agents.energy[i],
    );
}
```
> Verify M-A already added the `apply_affect` call after `apply_personality` (`tick.rs:192`) with the FEAR bias now active. If M-A did **not** gate/insert `apply_affect`, add it here too, gated on `affect_enabled`, immediately after `apply_personality`:
> ```rust
> if affect_enabled {
>     crate::affect::apply_affect(&mut action, &agents.affect[i], &agents.genome[i], &sensors[i], agents.energy[i]);
> }
> ```

- [ ] Run `cargo test -p anabios-core hijack_overrides_program_movement` → **expect PASS**.
- [ ] Controller runs the full flag-off gate now: `cargo test -p anabios-core --test determinism` → **the minimal golden + parallel test must still PASS UNCHANGED** (M-B added no serialized state, and every affect path is gated on `affect_enabled` which is off in `minimal.toml`). If it fails, a gate leaked — fix before continuing; do **not** edit `GOLDEN`.
- [ ] Controller commits: `M-B: call apply_hijack in decide_all (gated, pre-normalization)`.

## Task 8 — `EventType::MassFright` + latch-free detector

**Files:** `crates/anabios-core/src/codex/event.rs`, `crates/anabios-core/src/codex/affect.rs` (new), `crates/anabios-core/src/codex/mod.rs`
**Interfaces:** Produces `EventType::MassFright`, `codex::affect::detect_mass_fright`.

- [ ] Add a failing unit test to the new `codex/affect.rs` (or a `#[cfg(test)]` block there):

```rust
#[cfg(test)]
mod tests {
    use crate::codex::EventType;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::world::World;

    #[test]
    fn mass_fright_fires_once_per_cooldown_when_a_cluster_panics() {
        use crate::affect::FEAR;
        let mut w = World::new(5);
        w.affect_enabled = true;
        // Five same-species agents; drive their FEAR over the detector threshold.
        let mut ids = Vec::new();
        for k in 0..5 {
            ids.push(w.spawn_agent(Vec2::new(500.0 + k as f32 * 5.0, 500.0), Genome::neutral()));
        }
        w.resize_scratch();
        for &id in &ids {
            w.agents.affect[id as usize][FEAR] = 0.9;
        }
        // Build the per-species aggregate and run the detector directly.
        let mut agg = std::mem::take(&mut w.codex_agg);
        agg.build(&w);
        super::detect_mass_fright(&mut w, &agg);
        w.codex_agg = agg;
        let n = w.codex.events.iter().filter(|e| e.event_type == EventType::MassFright).count();
        assert_eq!(n, 1, "one MassFright for the frightened cluster");

        // Immediately re-running within the cooldown must NOT double-fire.
        let mut agg2 = std::mem::take(&mut w.codex_agg);
        agg2.build(&w);
        super::detect_mass_fright(&mut w, &agg2);
        w.codex_agg = agg2;
        let n2 = w.codex.events.iter().filter(|e| e.event_type == EventType::MassFright).count();
        assert_eq!(n2, 1, "cooldown dedup prevents a re-fire");
    }
}
```

- [ ] Run `cargo test -p anabios-core mass_fright_fires_once` → **expect FAIL** (`EventType::MassFright`, `detect_mass_fright` unresolved).
- [ ] In `event.rs`, append the variant at the **END** of `EventType` (after `LivestockHerd = 52`):

```rust
    /// A cluster of same-species agents simultaneously in high FEAR — the first
    /// mass fright / panic response (Bracha rout). `value` = frightened member
    /// count; loc = species centroid.
    MassFright = 53,
```

- [ ] Update the count constant just below the enum:

```rust
pub const EVENT_TYPE_COUNT: usize = EventType::MassFright as usize + 1;
```

- [ ] Create `crates/anabios-core/src/codex/affect.rs`:

```rust
//! Affect-layer codex detector (M-B): mass fright / panic rout.
//!
//! Latch-free by design — deduped against the already-serialized `codex.events`
//! ring rather than a new `CodexState` latch, so the affect layer adds ZERO
//! serialized state and flag-off stays byte-identical to the pre-affect
//! baseline. Reads only serialized `affect[FEAR]` ⇒ replay-safe.

use super::agg::SpeciesAggTable;
use super::event::{CodexEvent, EventType};
use crate::affect::FEAR;
use crate::world::World;

/// A member counts as frightened at/above this FEAR activation.
pub const FRIGHT_FEAR_MIN: f32 = 0.6;
/// Minimum simultaneously-frightened same-species members for a "mass" event.
pub const FRIGHT_MIN_MEMBERS: usize = 4;
/// Ticks a species is suppressed from re-firing after a MassFright.
pub const FRIGHT_COOLDOWN: u64 = 50;

pub(super) fn detect_mass_fright(world: &mut World, agg: &SpeciesAggTable) {
    // Cheap, provable no-op when the layer is off (affect is all-zero anyway).
    if !world.affect_enabled {
        return;
    }
    // Affect column is only sized after resize_scratch (mirror detect_herd_cohesion).
    if world.agents.affect.len() < world.agents.capacity() {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        let frightened = entry
            .member_idx
            .iter()
            .filter(|&&i| world.agents.affect[i][FEAR] >= FRIGHT_FEAR_MIN)
            .count();
        if frightened < FRIGHT_MIN_MEMBERS {
            continue;
        }
        // Dedup against the serialized event log (no new serialized latch).
        let recently = world.codex.events.iter().any(|e| {
            e.event_type == EventType::MassFright
                && e.species_id == sid
                && tick.saturating_sub(e.tick) < FRIGHT_COOLDOWN
        });
        if recently {
            continue;
        }
        let (lx, ly) = super::centroid_of(agg, sid);
        to_push.push(CodexEvent {
            event_type: EventType::MassFright,
            tick,
            species_id: sid,
            value: frightened as f32,
            loc_x: lx,
            loc_y: ly,
        });
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}
```
> Confirm the exact `pub`/path of `SpeciesAggTable`, `agg.active()`, `agg.get()`, `entry.member_idx`, and `centroid_of` against `codex/agg.rs` + `codex/mod.rs` (all used verbatim by `detect_herd_cohesion`). Match visibility.

- [ ] Register + wire in `codex/mod.rs`: add `mod affect;` next to the other detector modules (`mod agg;` … block, ~`mod.rs:21-40`), and add the call in `observe_all` (after `spatial::detect_herd_cohesion`, ~`mod.rs:387`):

```rust
    affect::detect_mass_fright(world, &agg);
```

- [ ] Run `cargo test -p anabios-core mass_fright_fires_once` → **expect PASS**.
- [ ] If a Rust-side event name/color table asserts `EVENT_TYPE_COUNT` length, extend it by one entry; the GDScript viewer's parallel array is a separate viewer follow-up (M-F), not this Rust milestone.
- [ ] Controller commits: `M-B: MassFright EventType + latch-free codex detector`.

## Task 9 — Flag-on scenario

**Files:** `scenarios/affect-threat.toml`
**Interfaces:** none (data).

- [ ] Create `scenarios/affect-threat.toml` — prey herd + oversized predators, affect on, so FEAR/hijack drive a rout. Model on `predator-prey.toml`:

```toml
name = "affect-threat"
seed = 0
affect_enabled = true
# Combat pressure so the predators actually loom; keep population modest so the
# story is the panic response, not Malthusian collapse.
max_population = 500

# Prey: a tight grazer herd at world center — the population that panics.
[[agents]]
count = 40
archetype = "grazer"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 90.0 }
[agents.traits]
size = 0.4
lifespan_bias = 1.0

# Predators: a handful of large stalkers seeded inside the herd — immediate,
# close, big other-species threats that spike prey FEAR.
[[agents]]
count = 8
archetype = "stalker"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 90.0 }
[agents.traits]
size = 0.9
lifespan_bias = 1.0
```
> Verify `affect_enabled` is accepted at the scenario top level (M-A added the `#[serde(default)] pub affect_enabled: bool` field + `w.affect_enabled = self.affect_enabled;` in `instantiate`). Verify `archetype`/`traits.size` names against `scenario.rs`; reuse whatever `predator-prey.toml` uses.

- [ ] Sanity check: `cargo test -p anabios-core --test all_scenarios` (or the scenario-parse test) parses the new file. **Expect PASS.**
- [ ] Controller commits: `M-B: affect-threat flag-on scenario`.

## Task 10 — Flag-on behavior + determinism tests

**Files:** `crates/anabios-core/tests/affect.rs` (new)
**Interfaces:** consumes the public API only.

- [ ] Create `crates/anabios-core/tests/affect.rs` with (a) MassFright-fires, (b) self-consistency, (c) save→load→step, and (d) a pinned golden. Start with the behavior + self-consistency + round-trip tests:

```rust
//! Flag-ON determinism + behavior for the affect (FEAR/hijack) layer.
//! Flag-OFF byte-identity is guarded by `determinism.rs` (unchanged minimal
//! golden) — M-B adds no serialized state.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/affect-threat.toml");

#[test]
fn affect_scenario_parses_with_flag_on() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    assert!(s.affect_enabled, "scenario must enable the affect layer");
}

#[test]
fn affect_scenario_is_self_consistent() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    let run = || {
        let mut w = s.instantiate();
        for _ in 0..300 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "same seed + flag on ⇒ bit-identical");
}

#[test]
fn affect_scenario_survives_save_load_step() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    let mut world = s.instantiate();
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
        "affect world diverged after save→load→step (hidden non-serialized affect state?)",
    );
}

#[test]
fn affect_scenario_emits_mass_fright() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    let mut w = s.instantiate();
    let mut saw = false;
    for _ in 0..1500 {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::MassFright) {
            saw = true;
            break;
        }
    }
    assert!(saw, "predator-driven FEAR must produce a MassFright within 1500 ticks");
}
```

- [ ] Run `cargo test -p anabios-core --test affect` → **expect the round-trip + self-consistency + MassFright tests PASS.** If `affect_scenario_survives_save_load_step` FAILS, a serde-skip value is leaking into serialized affect state (recheck Task 1's deviation note — FEAR must read only fresh sensors, never `combat_damaged`); fix before pinning the golden.
- [ ] Add the pinned golden (model `cognition.rs`): the controller runs `UPDATE_HASHES=1 cargo test -p anabios-core --test affect affect_scenario_matches_golden_hashes` to print values, then pastes them in:

```rust
/// Pinned golden for the flag-ON affect scenario. Regenerate deliberately with
/// `UPDATE_HASHES=1` whenever an affect-behavior change is intentional.
// Created 2026-08-03 (M-B): first pin of the FEAR/hijack layer's real behavior.
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
    for ((et, eh), (gt, gh)) in AFFECT_GOLDEN.iter().zip(&observed) {
        assert_eq!(et, gt, "tick mismatch");
        assert_eq!(*eh, *gh, "affect hash drift at tick {et}: expected 0x{eh:016x}, got 0x{gh:016x}");
    }
}
```

- [ ] Controller: `UPDATE_HASHES=1 cargo test -p anabios-core --test affect affect_scenario_matches_golden_hashes`, paste the three printed values over the `0x0` placeholders, then re-run **without** the env var → **expect PASS**.
- [ ] Controller commits: `M-B: flag-on affect golden + save/load/step + MassFright behavior tests`.

## Task 11 — Full determinism gate + flag-off byte-identity confirmation

**Files:** none (gate only) — plus, if the parallel test needs the flag-on path, one line in `determinism.rs`.
**Interfaces:** none.

- [ ] Controller runs the whole determinism gate and confirms **no golden moved** and **no `FORMAT_VERSION` bump**:
  - `cargo test -p anabios-core --test determinism` → minimal golden + `parallel_matches_serial_across_thread_counts` PASS unchanged.
  - `cargo test -p anabios-core --test cognition` and `--test inventions` → PASS unchanged (M-B adds no serialized layout).
- [ ] (Optional, recommended) Extend `parallel_matches_serial_across_thread_counts` to also exercise the flag-on affect scenario, proving the affect `par_iter` + hijack are thread-order-independent:

```rust
    for scenario_src in [
        include_str!("../../../scenarios/minimal.toml"),
        include_str!("../../../scenarios/tech-gene-coupling.toml"),
        include_str!("../../../scenarios/affect-threat.toml"),
    ] {
```
> Only add this if M-A did not already add the affect scenario there. Run `--test determinism` → **expect PASS** (1 vs 2 vs 8 threads identical).
- [ ] Confirm `git grep -n "FORMAT_VERSION" crates/anabios-core/src/snapshot.rs` still reads `23` — M-B must NOT bump it.
- [ ] Controller runs `cargo fmt --check` + `cargo clippy -p anabios-core -- -D warnings` (CI-gate parity) and the full `cargo test -p anabios-core` once → **expect PASS**.
- [ ] Controller commits: `M-B: confirm flag-off byte-identity (no golden move, no FORMAT_VERSION bump)`.

---

## Self-review notes

**Spec coverage (§4.2, §5.4, §3.3):**
- FEAR trigger from `hostility` + threatening neighbor (`nearest_other_dir` + large `nearest_rel_size`/`nearest_rel_energy`) — Task 1/2. Boldness lowers the setpoint — Task 1 (`K_FEAR_BOLDNESS`). Leaky integrator — Task 2 (`LAMBDA_FEAR`).
- **Combat-damage term (§4.2) DEFERRED** — verified `World.combat_damaged` is `#[serde(skip)]` per-tick scratch set post-decide; folding a cross-tick serde-skip value into the serialized `affect[FEAR]` accumulator would break `save→load→step` (spec §6.3 footgun). M-B uses only fresh, replay-safe sensor signals; a serialized damage bit is a deliberate follow-up (documented in the deviations section + Task 10's failure hint).
- `arousal` finalized as `max(FEAR,RAGE,PANIC)` — Task 3 (resolves open decision §9.5 → max).
- FEAR bias: flee away from threat + dampen non-defensive LIVE intents (share/broadcast/emit), identity at neutral — Task 4. Only live channels per spec §2.2 (never `feed_intent`/`mate_intent`).
- Hijack Freeze→Flight→Fight→Fright/Faint, ordered by proximity/escapability, no RNG, overwrites only live channels, Reactivity/Boldness-scaled threshold vs `HIJACK_AROUSAL_THRESHOLD` — Tasks 5/6. Call site after movement biases, before normalization (`tick.rs:252`→`253`) — Task 7 (livestock override at `:241-252` is the precedent for overwriting `move_x/move_y`).
- Codex: `EventType::MassFright` appended at END, detector wired into `observe_all` (§7.1 panic-cascade first-fire), flag-on scenario fires it — Tasks 8/9/10.

**Determinism (spec §6 / contract checklist):**
- (1) Flag-off byte-identical: every path gated on `affect_enabled` (off in `minimal.toml`); FEAR bias/hijack guarded on non-neutral affect; determinism/cognition/inventions goldens **unchanged** (Task 11). (2) Zero RNG in `develop_all`/`apply_affect`/`apply_hijack`/detector — all pure functions of serialized/fresh state. (3) **No new serialized columns** — FEAR reuses M-A's `affect` column; detector is latch-free (deduped via serialized `codex.events`); no `FORMAT_VERSION` bump. (4) `save→load→step` test added — Task 10. (5) `parallel_matches_serial` extended to the affect scenario — Task 11 (index-disjoint `par_iter`, writes slot `i` only). (6) `MassFright` appended at END — Task 8. (7) Flag-on golden pins behavior — Task 10.

**Type-consistency vs contract:** `apply_hijack(action, affect, genome, sensors, energy) -> bool` and `apply_affect(action, affect, genome, sensors, energy)` match the contract exactly (arg order `action, affect, genome, sensors, energy`). Consumes `Genome::{boldness, reactivity}` (signed `[-1,+1]`, neutral 0.0), `HIJACK_AROUSAL_THRESHOLD` (0.6), `LAMBDA_DEFAULT`, `FEAR/RAGE/PANIC/SEEK` indices, `AffectState`/`AFFECT_SYSTEMS` — all M-A-owned per the interface contract.

**Placeholder scan:** the only intentional placeholders are the `AFFECT_GOLDEN` `0x0` hashes (Task 10) and, if needed, temperament/size genome slot names + `reassign_to_new_species` test-helper imports (flagged inline in Tasks 2/7/9) — all resolved by the controller against live source during execution. No `todo!()`/`unimplemented!()`; the Task 5 `hijack_cornered` stub is replaced in Task 6 within the same milestone.
