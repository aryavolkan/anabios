# M-E: PLAY (Minimal) + Enrichment Coupling — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the **lowest-fidelity** Panksepp system — PLAY — into the affect
layer, and couple it into the *existing* IQ social-enrichment accumulator. A
juvenile that is safe and has a peer nearby accrues a small PLAY activation that
(a) biases its movement toward the peer (social approach) and (b) nudges its
`iq_enrich_acc` upward, so a played-with juvenile develops slightly higher
realized IQ than an isolated one. This is explicitly the minimal, cuttable
milestone (spec §4.7): anabios has **no play substrate**, so PLAY produces no
consummatory or combat output — only a movement approach-bias plus a bounded
enrichment contribution.

**Architecture:** M-E adds **zero new serialized state**. PLAY lives in slot
`PLAY` (index 6) of the already-serialized `affect: Vec<[f32; 7]>` column created
by M-A, and the enrichment contribution flows through the already-serialized
`iq_enrich_acc`. Three touchpoints, all behind existing flags:

1. **Trigger** — extend `affect::develop_all` (runs post-sense/pre-decide,
   zero-RNG, index-disjoint `par_iter`) with a PLAY leaky-integrator update:
   `juvenile (age < IQ_MATURATION_AGE) ∧ peer-near (nearest_same_dist small) ∧
   safe (low hostility)`, weighted by the `sociality()` gene, then a documented
   `FEAR ⊣ PLAY` lateral-inhibition suppression.
2. **Movement bias** — extend `affect::apply_affect` (called in `decide_all`
   right after `apply_personality`) with a PLAY block that biases `move_x/move_y`
   toward `nearest_same_dir`, guarded `if play != 0.0` (personality idiom →
   exact identity at neutral affect).
3. **Enrichment coupling** — inside `iq::develop_all` (stage 5b, late), add a
   small affect-gated term to the juvenile's `social` enrichment signal, guarded
   by `if world.affect_enabled` **and** `if play != 0.0`. Because `develop_all`
   already early-returns when `!cognition_enabled`, the coupling fires **only
   when BOTH `cognition_enabled` && `affect_enabled`** are on. When affect is off
   the added ops never execute, so the cognition golden (affect off) stays
   **byte-identical** and no FORMAT_VERSION bump / golden layout-growth is needed.

**Ordering rationale (documented, no double-count):** `affect::develop_all`
runs **early** in tick T (between `sense_all` and `decide_all`), so
`affect[i][PLAY]` for tick T is already written when `iq::develop_all` runs
**late** in the same tick T (stage 5b, `tick.rs:66`). The coupling therefore
reads a fresh, this-tick PLAY value; it is a *separate additive* enrichment term,
never a re-read of crowding, so it does not double-count the existing
crowding→`iq_enrich_acc` path.

**Tech Stack:** Rust, anabios-core, rayon, serde/bincode, cargo test.

## Global Constraints
- LOWEST-FIDELITY minimal system (spec §4.7) — explicitly cuttable / foldable
  into M-D; no play substrate, so output is approach-bias + enrichment only.
- Single Xoshiro256++ RNG; **ZERO RNG** added on any path (`develop_all` /
  `apply_affect` / `iq::develop_all` all stay pure functions of already-drawn
  state, following the `iq::develop_all` precedent).
- **Flag-off byte-identical** — assert BOTH the minimal golden
  (`determinism.rs`, affect+cognition off) AND the cognition golden
  (`cognition.rs`, cognition on / affect off) are unchanged **without any
  refresh**. M-E adds no serialized columns → no layout growth → no
  FORMAT_VERSION bump.
- PLAY enrichment is gated on `affect_enabled && cognition_enabled`.
- Affect writes only LIVE channels: `move_x/move_y` (approach bias) and the
  existing `iq_enrich_acc`. No `feed_intent`/`mate_intent` (latent). No
  consummatory/combat output.
- Neutral-identity guarded — every added read-side block is guarded
  `if x != 0.0` (personality.rs:52-102 idiom).
- Golden refresh (flag-ON only) via `UPDATE_HASHES=1`; the **controller** runs
  all cargo/git/golden/commit gates. Implementer subagents Edit/Read only.

## Dependencies
- Requires **M-A..M-D merged**. M-E consumes, from the interface contract
  (`docs/superpowers/plans/2026-08-02-affect-layer-interface-contract.md`):
  - `affect.rs`: constants `AFFECT_SYSTEMS = 7`, `PLAY = 6`, `FEAR = 1`,
    `LAMBDA_DEFAULT = 0.8`; the `affect: Vec<AffectState>` column
    (`agent.rs`, serialized); `affect::develop_all(world)` (M-A);
    `affect::apply_affect(action, affect, genome, sensors, energy)` (M-A) and
    its call site in `decide_all` after `apply_personality` (`tick.rs:187-192`).
  - `genome.rs`: `sociality()` accessor (slot 34, `[-1,+1]`, neutral `0.0`).
  - `world.rs` / `scenario.rs`: `affect_enabled` flag + scenario threading (M-A).
- **Note: this milestone is CUTTABLE / foldable into M-D per the spec (§8, §9.4).**

---

## File Structure

Files touched (all existing except the two new flag-ON test fixtures):

```
crates/anabios-core/src/affect.rs        # + PLAY constants, PLAY trigger in develop_all, PLAY bias in apply_affect
crates/anabios-core/src/iq.rs            # + affect-gated enrichment term in develop_all (guarded)
crates/anabios-core/tests/cognition.rs   # assert UNCHANGED (byte-identical, no refresh)
crates/anabios-core/tests/determinism.rs # assert minimal golden UNCHANGED (byte-identical)
crates/anabios-core/tests/affect_play.rs # NEW flag-ON golden + save→load→step (model cognition.rs)
scenarios/affect-play.toml               # NEW flag-ON scenario: affect_enabled + cognition_enabled + juvenile cluster
```

Verified source anchors (do not drift): `IQ_MATURATION_AGE = 100`
(`iq.rs:21`); `IQ_SOCIAL_REF = 8.0` (`iq.rs:31`); `iq::develop_all` destructure
`AgentBuffers { iq_enrich_acc, iq_enrich_ticks, iq, age, position, genome,
alive, .. }` (`iq.rs:58-67`), guard `if !world.cognition_enabled { return; }`
(`iq.rs:46-49`), `social` + `*acc += 0.5*nutrition + 0.5*social` (`iq.rs:96-101`);
`SensorRegister { nearest_same_dist (INFINITY if none), nearest_same_dir,
nearest_same_id (NO_NEIGHBOR_ID), crowding, hostility }` (`sense.rs:46-75`);
`ActionRegister.move_x/move_y` (`program/mod.rs:124-125`); `apply_personality`
identity idiom (`personality.rs:52-102`).

---

## Task 1 — PLAY constants + trigger in `affect::develop_all`

- [ ] **Files:** `crates/anabios-core/src/affect.rs`
- [ ] **Interfaces:** consumes `PLAY`, `FEAR`, `LAMBDA_DEFAULT`, `AFFECT_SYSTEMS`
      (M-A consts), `Genome::sociality()`, `SensorRegister.{nearest_same_dist,
      hostility}`, `IQ_MATURATION_AGE` (re-exported from `iq`). Adds
      `PLAY_PEER_RADIUS`.

### Failing test first

Add to `affect.rs` `#[cfg(test)] mod tests`. Drive one `develop_all` tick with
`affect_enabled = true` on a juvenile standing next to a same-species peer, and
assert `affect[i][PLAY] > 0.0`; then assert it is `0.0` for an adult, for an
isolated agent (`nearest_same_dist = INFINITY`), and for a juvenile under high
hostility.

```rust
#[test]
fn play_activates_for_safe_juvenile_near_peer() {
    let mut w = World::new(2);
    w.affect_enabled = true;
    let a = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let _b = w.spawn_agent(Vec2::new(505.0, 500.0), Genome::neutral());
    let i = a as usize;
    w.agents.age[i] = 0; // juvenile
    w.sensors.resize(w.agents.capacity(), Default::default());
    w.sensors[i].nearest_same_dist = 5.0; // peer near
    w.sensors[i].hostility = 0.0; // safe
    crate::affect::develop_all(&mut w);
    assert!(w.agents.affect[i][crate::affect::PLAY] > 0.0, "safe juvenile near a peer should play");
}

#[test]
fn play_is_zero_for_adult_isolated_or_threatened() {
    let base = |setup: &dyn Fn(&mut World, usize)| -> f32 {
        let mut w = World::new(1);
        w.affect_enabled = true;
        let a = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let i = a as usize;
        w.sensors.resize(w.agents.capacity(), Default::default());
        w.sensors[i].nearest_same_dist = 5.0;
        w.sensors[i].hostility = 0.0;
        w.agents.age[i] = 0;
        setup(&mut w, i);
        crate::affect::develop_all(&mut w);
        w.agents.affect[i][crate::affect::PLAY]
    };
    assert_eq!(base(&|w, i| w.agents.age[i] = crate::iq::IQ_MATURATION_AGE), 0.0, "adult");
    assert_eq!(base(&|w, i| w.sensors[i].nearest_same_dist = f32::INFINITY), 0.0, "isolated");
    assert_eq!(base(&|w, i| w.sensors[i].hostility = 1.0), 0.0, "threatened");
}
```

- [ ] Run the two tests → **FAIL** (PLAY stays 0 / no trigger yet).

### Minimal implementation

Add the constant near the other tuned affect constants:

```rust
/// A same-species peer within this torus distance counts as "nearby" for PLAY.
pub const PLAY_PEER_RADIUS: f32 = 40.0;
/// PLAY target when a safe juvenile has a peer within `PLAY_PEER_RADIUS`,
/// weighted by the safety of the surroundings and the `sociality` temperament.
```

Inside `develop_all`'s per-agent body (the same index-disjoint closure that M-A
already writes all activations from), append the PLAY leaky-integrator update.
`age` and `sensors` are already in scope (M-A reads them for other systems);
`IQ_MATURATION_AGE` is `iq`'s public const:

```rust
// --- PLAY (M-E, lowest-fidelity: juvenile + safe + peer nearby) ---
// Raw trigger: a juvenile with a same-species peer in range, scaled by how
// safe it feels (low hostility) and its heritable sociality. No RNG.
let play_target = if age[i] < crate::iq::IQ_MATURATION_AGE
    && sensors[i].nearest_same_dist < PLAY_PEER_RADIUS
{
    let safe = (1.0 - sensors[i].hostility).clamp(0.0, 1.0);
    // sociality() is [-1,+1]; map to a [0,1] bond weight (neutral genome → 0.5).
    let social_w = (0.5 + 0.5 * genome[i].sociality()).clamp(0.0, 1.0);
    safe * social_w
} else {
    0.0
};
let play = &mut affect[i][PLAY];
*play = LAMBDA_DEFAULT * *play + (1.0 - LAMBDA_DEFAULT) * play_target;
// Lateral inhibition (spec §3.3): FEAR suppresses PLAY (no play under threat).
*play *= (1.0 - affect[i][FEAR]).clamp(0.0, 1.0);
*play = play.clamp(0.0, 1.0);
```

> **Integration note for the implementer:** M-A's `develop_all` destructures the
> disjoint `affect`/`age`/`sensors`/`genome` columns into the `par_iter` closure.
> Add the block above alongside the other systems' updates in that same closure —
> do NOT introduce a second pass. If M-A applied lateral inhibition in a dedicated
> post-raw sub-block, place the `FEAR ⊣ PLAY` line there instead, keeping the raw
> integrator update with the other raw updates. `affect[i][FEAR]` is this-tick
> FEAR (M-B); reading it after its own raw update is the documented inhibition
> order and is index-local (same `i`), so `parallel_matches_serial` is preserved.

- [ ] Run the two tests → **PASS**.
- [ ] **Commit:** `feat(affect): PLAY trigger for safe juveniles near a peer (M-E)`

---

## Task 2 — PLAY movement bias in `affect::apply_affect`

- [ ] **Files:** `crates/anabios-core/src/affect.rs`
- [ ] **Interfaces:** extends `apply_affect(action, affect, genome, sensors,
      energy)` (M-A). Consumes `SensorRegister.{nearest_same_dir, nearest_same_id}`
      (`NO_NEIGHBOR_ID` sentinel), `ActionRegister.move_x/move_y`. Adds
      `PLAY_APPROACH_GAIN`.

### Failing test first

```rust
#[test]
fn play_biases_movement_toward_peer() {
    use crate::program::ActionRegister;
    use crate::sense::NO_NEIGHBOR_ID;
    let g = Genome::neutral();
    let mut sensors = SensorRegister::default();
    sensors.nearest_same_id = 0; // a peer exists
    sensors.nearest_same_dir = Vec2::new(1.0, 0.0);
    let mut affect = [0.0f32; crate::affect::AFFECT_SYSTEMS];

    // Neutral (PLAY == 0): action is left bit-for-bit unchanged.
    let mut a0 = ActionRegister::default();
    let before = a0.move_x;
    crate::affect::apply_affect(&mut a0, &affect, &g, &sensors, 50.0);
    assert_eq!(a0.move_x, before, "PLAY==0 must be exact identity");

    // PLAY > 0: movement biases toward the peer (+x).
    affect[crate::affect::PLAY] = 0.5;
    let mut a1 = ActionRegister::default();
    crate::affect::apply_affect(&mut a1, &affect, &g, &sensors, 50.0);
    assert!(a1.move_x > 0.0, "playful juvenile should approach its peer");
}
```

- [ ] Run → **FAIL** (no PLAY block in `apply_affect` yet; identity assert may
      pass but the approach assert fails).

### Minimal implementation

Append to `apply_affect`, mirroring the extraversion approach block
(`personality.rs:59-62`):

```rust
/// Social-approach movement gain per unit PLAY activation (toward the peer).
pub const PLAY_APPROACH_GAIN: f32 = 0.3;
```

```rust
// PLAY (M-E): a playful juvenile drifts toward its nearest same-species peer.
// Guarded on non-zero PLAY → exact identity at neutral affect (personality idiom).
let play = affect[PLAY];
if play != 0.0 && sensors.nearest_same_id != crate::sense::NO_NEIGHBOR_ID {
    action.move_x += PLAY_APPROACH_GAIN * play * sensors.nearest_same_dir.x;
    action.move_y += PLAY_APPROACH_GAIN * play * sensors.nearest_same_dir.y;
}
```

- [ ] Run → **PASS**.
- [ ] **Commit:** `feat(affect): PLAY social-approach movement bias (M-E)`

---

## Task 3 — Enrichment coupling in `iq::develop_all`

- [ ] **Files:** `crates/anabios-core/src/iq.rs`
- [ ] **Interfaces:** reads `world.affect_enabled` + the `affect` column
      (`crate::affect::PLAY`); writes the existing `iq_enrich_acc` via the
      juvenile `social` term. Adds `PLAY_ENRICH_WEIGHT`.

### Failing test first

Extend `iq.rs` tests. Prove that, with **both flags on**, a juvenile whose
`affect[PLAY] > 0` develops a strictly higher IQ than the same juvenile with
`affect[PLAY] == 0` — holding `crowding` (the existing social signal) **equal**
so the delta is attributable to PLAY alone; and prove flag-off is unchanged.

```rust
/// Develop one juvenile for a single tick with a fixed PLAY activation and the
/// affect flag toggled; crowding is held constant so only PLAY varies.
fn develop_with_play(play: f32, affect_on: bool) -> f32 {
    let mut w = World::new(1);
    w.cognition_enabled = true;
    w.affect_enabled = affect_on;
    let (col, row) = grass_cell(&w);
    let cap = w.biome.at(col, row).terrain.carrying_capacity();
    let idx = w.biome.cell_index(col, row);
    w.biome.cells[idx].plant_biomass = 0.5 * cap; // fixed nutrition
    let spot = Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
    let id = w.spawn_agent(spot, Genome::neutral());
    let i = id as usize;
    w.agents.age[i] = 0;
    w.sensors.resize(w.agents.capacity(), Default::default());
    w.sensors[i].crowding = 4; // identical base social signal in every case
    w.agents.affect[i][crate::affect::PLAY] = play;
    develop_all(&mut w);
    w.agents.iq[i]
}

#[test]
fn play_lifts_realized_iq_via_enrichment() {
    let played = develop_with_play(0.8, true);
    let idle = develop_with_play(0.0, true);
    assert!(played > idle, "PLAY should raise enrichment → higher IQ: {played} vs {idle}");
}

#[test]
fn play_enrichment_is_inert_when_affect_off() {
    // Flag off: PLAY column is present but the coupling must not read it — the
    // developed IQ is identical whether PLAY is 0.8 or 0.0.
    let hi = develop_with_play(0.8, false);
    let lo = develop_with_play(0.0, false);
    assert_eq!(hi, lo, "affect off ⇒ PLAY column ignored, IQ byte-identical");
}
```

- [ ] Run → **FAIL** (`play_lifts_realized_iq_via_enrichment` fails; PLAY not
      yet coupled).

### Minimal implementation

Add the constant near the other IQ constants:

```rust
/// Bounded PLAY contribution to a juvenile's social-enrichment signal, per unit
/// PLAY activation. Small: PLAY is a nudge on top of sensed crowding, and the
/// combined social term is re-clamped to `[0,1]` so realized IQ stays bounded.
pub const PLAY_ENRICH_WEIGHT: f32 = 0.25;
```

Capture the flag **before** the `&mut world.agents` destructure (the destructure
borrows `world.agents`, so `world.affect_enabled` must be read first), and add
`affect` to the destructured columns as a shared read:

```rust
    if !world.cognition_enabled {
        return;
    }
    use rayon::prelude::*;
    let cap = world.agents.capacity();
    let affect_enabled = world.affect_enabled; // read before the &mut borrow below
    let biome = &world.biome;
    let sensors = &world.sensors;
    let crate::agent::AgentBuffers {
        iq_enrich_acc,
        iq_enrich_ticks,
        iq,
        age,
        position,
        genome,
        alive,
        affect, // M-E: read-only PLAY source
        ..
    } = &mut world.agents;
    let (age, position, genome, alive, affect) =
        (&*age, &*position, &*genome, &*alive, &*affect);
```

Then, inside the closure, fold PLAY into the existing `social` term — keeping the
**flag-off and PLAY-zero paths bit-for-bit unchanged** via the personality idiom.
Rewrite the current `let social = …;` (`iq.rs:96-100`) so the base value is
computed exactly as today, then optionally nudged:

```rust
            let mut social = if i < sensors.len() {
                (sensors[i].crowding as f32 / IQ_SOCIAL_REF).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // PLAY enrichment (M-E): a juvenile who plays banks slightly richer
            // social enrichment. Gated on affect_enabled AND cognition_enabled
            // (this whole fn early-returns when cognition is off), and guarded on
            // non-zero PLAY, so the cognition golden (affect OFF) stays
            // byte-identical. Re-clamped to [0,1] to keep realized IQ bounded.
            if affect_enabled {
                let play = affect[i][crate::affect::PLAY];
                if play != 0.0 {
                    social = (social + PLAY_ENRICH_WEIGHT * play).clamp(0.0, 1.0);
                }
            }
```

The subsequent `*acc += 0.5 * nutrition + 0.5 * social;` line is unchanged. When
`affect_enabled` is false the `if` body never runs and `social` is the exact
prior value → the cognition golden does not move.

- [ ] Run both new tests + the existing `iq` tests → **PASS**.
- [ ] **Commit:** `feat(iq): couple PLAY activation into juvenile social enrichment (M-E)`

---

## Task 4 — Assert flag-OFF goldens are byte-identical (no refresh)

- [ ] **Files:** none edited — this is a **controller gate**. Confirms M-E added
      no serialized layout and did not perturb any flag-off trajectory.
- [ ] **Interfaces:** existing goldens `determinism.rs:162` (minimal) and
      `cognition.rs:93` (cognition on / affect off).

### Steps (controller runs)

- [ ] `cargo test -p anabios-core --test determinism` → **PASS** with the minimal
      `GOLDEN` **unchanged** (affect+cognition both off; PLAY paths never taken).
- [ ] `cargo test -p anabios-core --test cognition` → **PASS** with
      `COGNITIVE_GOLDEN` **unchanged**. The cognition scenario has
      `affect_enabled` absent (⇒ false via serde default), so `develop_all`'s
      enrichment guard `if affect_enabled` is dead, and `affect::develop_all`
      early-returns — the trajectory is byte-identical.
- [ ] Confirm **no FORMAT_VERSION bump** in `snapshot.rs`: M-E adds no serialized
      column, so the bincode payload does not grow. (If either golden moved, that
      is a determinism bug — stop and fix, do **not** refresh.)
- [ ] **Commit (if any doc/changelog note added):**
      `test(affect): confirm M-E flag-off goldens byte-identical (no layout growth)`

---

## Task 5 — Flag-ON scenario + golden pinning PLAY behavior

- [ ] **Files:** `scenarios/affect-play.toml` (new),
      `crates/anabios-core/tests/affect_play.rs` (new)
- [ ] **Interfaces:** `Scenario::parse_toml` / `instantiate`, `state_hash`,
      `step` (model `cognition.rs`). Scenario sets `affect_enabled = true` +
      `cognition_enabled = true` so BOTH the movement bias and the enrichment
      coupling are exercised, with a same-species juvenile cluster so PLAY fires.

### Scenario

Create `scenarios/affect-play.toml` — a small clustered juvenile population
(fresh agents spawn at `age = 0`, juvenile for the first `IQ_MATURATION_AGE = 100`
ticks) so PLAY is active early. Mirror the flag/format of
`cognitive-coevolution.toml`:

```toml
name = "affect-play"
seed = 0
affect_enabled = true
cognition_enabled = true
max_population = 200

# A tight cluster of same-species foragers: peers stay within PLAY_PEER_RADIUS,
# safe (no war), so juveniles play — biasing toward each other and banking extra
# social enrichment. Pins the M-E PLAY + enrichment coupling end-to-end.
[[agents]]
count = 40
archetype = "forager"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 60.0 }
```

> Adjust `archetype`/trait keys to whatever the scenario schema exposes (grep
> other scenario TOMLs); the load-bearing requirements are `affect_enabled = true`,
> `cognition_enabled = true`, and a same-species cluster of juveniles.

### Failing test first

```rust
//! End-to-end flag-ON golden for the M-E PLAY + enrichment coupling.
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/affect-play.toml");

#[test]
fn affect_play_scenario_parses_with_both_flags() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect-play scenario");
    assert!(s.affect_enabled);
    assert!(s.cognition_enabled);
}

#[test]
fn affect_play_scenario_is_self_consistent() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse");
    let run = |ticks: u64| {
        let mut w = s.instantiate();
        for _ in 0..ticks {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(200), run(200), "same seed + flags on → bit-identical");
}

// Flag-ON golden. Regenerate deliberately with UPDATE_HASHES=1.
const PLAY_GOLDEN: &[(u64, u64)] = &[(0, 0x0), (50, 0x0), (150, 0x0)]; // controller fills via UPDATE_HASHES=1

#[test]
fn affect_play_scenario_matches_golden_hashes() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut w = s.instantiate();
    let max_tick = PLAY_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < PLAY_GOLDEN.len() && PLAY_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated affect-play hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }
    for ((et, eh), (gt, gh)) in PLAY_GOLDEN.iter().zip(&observed) {
        assert_eq!(et, gt, "tick mismatch");
        assert_eq!(*eh, *gh, "affect-play hash drift at tick {et}: expected 0x{eh:016x}, got 0x{gh:016x}");
    }
}
```

- [ ] Run → **FAIL** (placeholder `0x0` hashes).

### Golden generation (controller)

- [ ] `UPDATE_HASHES=1 cargo test -p anabios-core --test affect_play affect_play_scenario_matches_golden_hashes -- --nocapture`
- [ ] Paste the printed triples into `PLAY_GOLDEN`, with a dated changelog line:
      `// Golden for M-E PLAY + enrichment (affect_enabled + cognition_enabled).`
- [ ] Re-run the whole file → **PASS**.
- [ ] **Commit:** `test(affect): flag-on golden pins M-E PLAY + enrichment (M-E)`

---

## Task 6 — save→load→step equality for the flag-ON world

- [ ] **Files:** `crates/anabios-core/tests/affect_play.rs`
- [ ] **Interfaces:** `save_to_bytes` / `load_from_bytes` / `state_hash`
      (model `determinism.rs:16-36`). Guards that the PLAY path relies on **no
      hidden non-serialized state** (the serde-skip replay footgun). M-E adds no
      new serialized column, but PLAY reads the serialized `affect` column and
      writes the serialized `iq_enrich_acc`, so the round-trip must hold.

### Test

```rust
#[test]
fn affect_play_survives_save_load_step() {
    let mut world = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    for _ in 0..80 {
        step(&mut world); // warm juveniles so PLAY activations accumulate
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load must restore identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "affect-play world diverged after save→load→step (hidden non-serialized PLAY state?)",
    );
}
```

- [ ] Run → **PASS** (should pass immediately since `affect` + `iq_enrich_acc`
      are both serialized; if it fails, a PLAY input was left `#[serde(skip)]` —
      investigate before proceeding).
- [ ] **Commit:** `test(affect): save→load→step for the flag-on PLAY world (M-E)`

---

## Task 7 — Refresh any pre-existing affect-ON golden that PLAY moves

- [ ] **Files:** whichever affect flag-ON golden M-A/M-B established (e.g. an
      `affect`-scenario golden in a prior test module), if present.
- [ ] **Interfaces:** the affect-on golden's `UPDATE_HASHES=1` path.

### Steps (controller)

- [ ] Search for an existing affect flag-ON golden test
      (`grep -rn "affect_enabled" crates/anabios-core/tests`). If one exists and
      its scenario contains juveniles with peers, the **new PLAY trigger +
      movement bias changes its trajectory** — this is an intended **behavior**
      change, not layout growth.
- [ ] Regenerate that golden with `UPDATE_HASHES=1` and add a dated note:
      `// Refreshed 2026-08-02 (M-E): PLAY trigger + social-approach bias added;
      behavior change (NOT layout — no new serialized column, no FORMAT_VERSION
      bump). Flag-off (minimal / cognition) goldens unaffected.`
- [ ] If **no** such golden exists yet (M-E is the first affect-on golden),
      Task 5 is the canonical flag-ON pin and this task is a no-op — record that.
- [ ] `cargo test -p anabios-core` full suite green;
      `parallel_matches_serial_across_thread_counts` passes (PLAY writes are
      index-local: `affect[i]`, `iq_enrich_acc[i]`).
- [ ] **Commit:** `test(affect): refresh affect-on golden for PLAY behavior (M-E)`

---

## Self-review notes

- **No new serialized state — the defining property of this milestone.** PLAY
  reuses `affect[i][PLAY]` (slot 6 of M-A's 7-wide column) and the existing
  `iq_enrich_acc`. Therefore: no FORMAT_VERSION bump, no minimal/cognition golden
  layout-growth refresh, and the only golden that moves is a flag-ON one (Task 5,
  and Task 7 if a prior affect-on golden exists). Verified against `agent.rs`
  (no column added) and `snapshot.rs` (no version change).
- **Double-count / ordering checked.** PLAY's enrichment is a *separate additive*
  term folded into `social`, not a re-read of crowding. It reads this-tick
  `affect[i][PLAY]` (written early in the tick by `affect::develop_all`) from the
  late `iq::develop_all` (stage 5b) — one consistent value per tick, no feedback
  loop within a tick.
- **Flag-off identity is triple-guarded:** (1) `iq::develop_all` early-returns
  when `!cognition_enabled`; (2) the enrichment block is wrapped `if
  affect_enabled`; (3) inner `if play != 0.0`. Any one being false leaves `social`
  and thus the golden byte-identical. `apply_affect`'s PLAY block and
  `develop_all`'s PLAY update only execute meaningfully under `affect_enabled`
  (the whole affect stage no-ops when off, per M-A). Confirmed the `iq.rs`
  destructure adds `affect` as a shared borrow and reads `world.affect_enabled`
  **before** the `&mut world.agents` borrow (borrowck).
- **Zero RNG** across all three touchpoints — pure functions of genome + sensors
  + physiology + already-written affect, matching the `iq::develop_all` precedent.
- **Confound control in the behavior test:** `play_lifts_realized_iq_via_enrichment`
  holds `crowding` (the existing social signal) fixed and varies only
  `affect[PLAY]`, so the IQ delta is attributable to PLAY alone;
  `play_enrichment_is_inert_when_affect_off` pins the flag-off equality.
- **FEAR ⊣ PLAY** lateral inhibition uses this-tick FEAR (index-local, same `i`),
  preserving `parallel_matches_serial`. If M-B has not merged FEAR, drop that line
  and gate "safe" purely on `sensors.hostility` (documented fallback).
- **This milestone is optional / cuttable — it can be folded into M-D or dropped
  entirely per spec §8/§9.4 without affecting the rest of the affect arc.**
