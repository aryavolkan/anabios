# M12 — Competition I: Combat & Predation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the inert `FireWeapon` node into real combat, close the trophic loop with carcass scavenging, and add the `Predation`/`CombatRaid`/`ArmsRace` detectors — the first emergent predator/prey behavior in anabios.

**Architecture:** Combat and predation run inside the existing serial `interact()` stage (tick stage 5), which runs *before* `age_and_starve()` (stage 7) — so combat that drives a target's energy ≤ 0 dies through the unchanged starvation death path. Killed agents leave a **carcass** (a new `World.carcasses: Vec<Carcass>` pool) whose flesh energy is proportional to body size; carnivore `Mouth` modules scavenge carcasses for energy. Death attribution (was this death combat-caused?) is recorded via per-tick World scratch and read by the codex detectors in stage 9. A scenario **archetype** extension lets `scenarios/*.toml` seed predators (Weapon + carnivore Mouth + `starter_stalker`) as a distinct species, enabling the multi-seed emergence test.

**Tech Stack:** Rust (`anabios-core` pure-sim crate, `anabios-headless` CLI crate), `glam::Vec2`, `serde`/`bincode` snapshots, `smallvec` module lists, `bitvec`.

## Global Constraints

- **Determinism (design §7.2):** all tick-path iteration is id-ordered; collect `iter_alive()` into a `Vec<u32>` before mutating; **no `HashMap` iteration** in the tick path (detectors use `BTreeMap`/`BTreeSet`/`VecDeque` only); no unordered float reductions; ties broken by ascending id (attackers) or ascending index (carcasses, via strict `<` comparison).
- **`EventType` variants are appended at the END of the enum, in this order:** `Predation = 6`, `CombatRaid = 7`, `ArmsRace = 8`. bincode encodes enum variants by **positional index** — never insert mid-enum. (`crates/anabios-core/src/codex.rs:38`.)
- **Combat targeting:** combat targets `SensorRegister.nearest_other_id` (nearest *other-species* agent), **never** `ActionRegister.target_id` (which is the overall-nearest neighbor and may be kin). Skip when `nearest_other_id == NO_NEIGHBOR_ID`. (Carried from the M11 review; spec §M12 targeting note.)
- **Module gating (design §3.5):** an action has effect only if the enabling module is present. No `Weapon` → no combat. No `Mouth` or herbivore `Mouth` (`diet_affinity == 0`) → no scavenging.
- **Snapshot / golden-tick:** this milestone changes the serialized snapshot layout (`World.carcasses`, new `CodexState` fields, new `EventType` variants). Per spec §2.3 the committed golden-tick hashes in `crates/anabios-core/tests/determinism.rs` are **refreshed** (not preserved). The controller regenerates them (subagents cannot run cargo).
- **World scratch that must NOT be serialized** (`#[serde(skip)]`): `combat_damaged`, `combat_attacker`. These reset every tick. `ActionRegister` is already `#[serde(skip)]`.
- **Sentinels:** `NO_NEIGHBOR_ID = u32::MAX` (`sense.rs:20`), `NO_TARGET = u32::MAX` (`program.rs:38`).

---

## File Structure

- `crates/anabios-core/src/module.rs` — add combat trait helpers (`effective_weapon`, `effective_armor_protection`) and a `predator_kit()` module constructor.
- `crates/anabios-core/src/interact.rs` — change `interact_all` to take `&mut World`; add combat pass and scavenge pass alongside the existing feed pass.
- `crates/anabios-core/src/carcass.rs` — **new**: `Carcass` struct, carcass constants, `carcass_step`.
- `crates/anabios-core/src/world.rs` — add `carcasses`, `combat_damaged`, `combat_attacker` fields; init + resize.
- `crates/anabios-core/src/age.rs` — form a carcass on death; record combat-attributed deaths.
- `crates/anabios-core/src/tick.rs` — call `interact_all(world)`; add `carcass_step` stage.
- `crates/anabios-core/src/codex.rs` — new `EventType` variants, `CombatDeath`, `CodexState` fields, `detect_predation`/`detect_combat_raid`/`detect_arms_race` + a pure `arms_race_signal` helper; wire into `observe_all`.
- `crates/anabios-core/src/scenario.rs` — `archetype` field on `AgentSpec`; per-archetype species + module + program seeding.
- `crates/anabios-core/src/lib.rs` — `pub mod carcass;`.
- `crates/anabios-headless/src/sweep.rs` — extend `event_name` + `write_summary_csv` for the 3 new events.
- `crates/anabios-core/tests/combat_predation.rs` — **new**: mechanism tests.
- `crates/anabios-core/tests/predator_prey_emergence.rs` — **new**: multi-seed emergence test.
- `scenarios/predator-prey.toml` — **new**: emergence scenario.

---

## Task 1: Combat helpers + `FireWeapon` wiring in `interact`

**Files:**
- Modify: `crates/anabios-core/src/module.rs` (add helpers near `effective_diet_carnivory:276`)
- Modify: `crates/anabios-core/src/world.rs` (add `combat_damaged`, `combat_attacker` fields; `new`; `resize_scratch`)
- Modify: `crates/anabios-core/src/interact.rs` (change signature to `&mut World`; add combat pass)
- Modify: `crates/anabios-core/src/tick.rs:32` (call site → `interact_all(world)`)
- Test: `crates/anabios-core/tests/combat_predation.rs` (new)

**Interfaces:**
- Produces: `module::effective_weapon(&ModuleList) -> Option<(f32, f32)>` returns `(damage, energy_cost)` of the highest-damage `Weapon`, `None` if no `Weapon`. `module::effective_armor_protection(&ModuleList) -> f32` (max `protection`, `0.0` if none).
- Produces: `interact::interact_all(world: &mut World)` (was `(&mut AgentBuffers, &mut BiomeField)`).
- Produces: `interact::FIRE_THRESHOLD: f32 = 0.5`, `interact::COMBAT_RANGE: f32 = 2.0`.
- Produces: `World.combat_damaged: Vec<bool>` (serde-skip), `World.combat_attacker: Vec<u32>` (serde-skip). After a combat pass, `combat_damaged[t]` is true iff slot `t` took combat damage this tick, and `combat_attacker[t]` holds the attacker's species id.
- Consumes: `SensorRegister.nearest_other_id`, `.nearest_other_dist` (`sense.rs:50-54`); `ActionRegister.fire_intent` (`program.rs:104`); `AgentBuffers.{is_alive,energy,modules,species_id,position,genome}`.

- [ ] **Step 1: Write the failing test** — append to a new file `crates/anabios-core/tests/combat_predation.rs`:

```rust
//! M12 mechanism tests: combat, carcasses, predation, and their detectors.

use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::module::{Module, SensorType};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program};
use anabios_core::tick::step;
use anabios_core::world::World;

/// Give slot `i` a predator kit: Locomotor + Vision Sensor + carnivore Mouth +
/// Weapon(damage, cost). Returns nothing; mutates the world in place.
fn arm_predator(w: &mut World, i: usize, damage: f32, cost: f32) {
    w.agents.modules[i] = smallvec_kit(damage, cost, /*armor=*/ 0.0);
}

/// Build a module kit inline (test-local so the test is self-contained).
fn smallvec_kit(weapon_damage: f32, weapon_cost: f32, armor: f32) -> anabios_core::module::ModuleList {
    let mut m = anabios_core::module::ModuleList::new();
    m.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
    m.push(Module::Sensor { sensor_type: SensorType::Vision, radius: 0.6, acuity: 0.6 });
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 1.0 });
    if weapon_damage > 0.0 {
        m.push(Module::Weapon { damage: weapon_damage, energy_cost: weapon_cost });
    }
    if armor > 0.0 {
        m.push(Module::Armor { protection: armor, mass_penalty: 0.1 });
    }
    m
}

/// Move an agent into a fresh second species, keeping species bookkeeping
/// tables consistent (mirrors the helper in social_substrate.rs).
fn reassign_to_new_species(w: &mut World, agent: u32) -> u32 {
    let sid = w.species_centroids.len() as u32;
    w.species_centroids.push(Genome::neutral());
    w.species_parents.push(Some(0));
    w.species_member_counts.push(0);
    w.next_species_id = sid + 1;
    w.remove_from_species(w.agents.species_id[agent as usize]);
    w.agents.species_id[agent as usize] = sid;
    w.add_to_species(sid);
    sid
}

/// A program that always fires the weapon (fire_intent = 1.0 > FIRE_THRESHOLD).
fn always_fire() -> Program {
    Program::from_slice(&[Node::Const(1.0), Node::FireWeapon])
}

#[test]
fn combat_deals_damage_minus_armor_and_spends_cost() {
    let mut w = World::new(7);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral()); // 1.0 apart < COMBAT_RANGE
    reassign_to_new_species(&mut w, prey);
    arm_predator(&mut w, pred as usize, /*damage=*/ 10.0, /*cost=*/ 2.0);
    // Give the prey armor 3.0 so net damage = 10 - 3 = 7.
    w.agents.modules[prey as usize] =
        smallvec_kit(/*weapon=*/ 0.0, /*cost=*/ 0.0, /*armor=*/ 3.0);
    w.agents.program[pred as usize] = always_fire();

    let pred_e0 = w.agents.energy[pred as usize];
    let prey_e0 = w.agents.energy[prey as usize];
    step(&mut w);

    // Prey lost exactly (damage - armor) = 7.0 to combat. Its own metabolism +
    // any grazing also move energy, so compare the combat delta directly by
    // asserting at least 7.0 was removed relative to a no-combat control below.
    assert!(w.agents.energy[prey as usize] <= prey_e0 - 7.0 + 1e-3);
    // Attacker paid the weapon energy_cost (2.0) on top of metabolism.
    assert!(w.agents.energy[pred as usize] <= pred_e0 - 2.0 + 1e-3);
    // Attribution recorded for the detectors.
    assert!(w.combat_damaged[prey as usize]);
    assert_eq!(w.combat_attacker[prey as usize], w.agents.species_id[pred as usize]);
}

#[test]
fn no_weapon_module_means_no_combat_damage() {
    let mut w = World::new(7);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    reassign_to_new_species(&mut w, prey);
    // Predator has a carnivore kit but NO weapon.
    w.agents.modules[pred as usize] = smallvec_kit(0.0, 0.0, 0.0);
    w.agents.program[pred as usize] = always_fire();
    step(&mut w);
    assert!(!w.combat_damaged[prey as usize], "no Weapon module → gating → no damage");
}

#[test]
fn combat_out_of_range_does_nothing() {
    let mut w = World::new(7);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(600.0, 500.0), Genome::neutral()); // 100 apart >> COMBAT_RANGE
    reassign_to_new_species(&mut w, prey);
    arm_predator(&mut w, pred as usize, 10.0, 2.0);
    w.agents.program[pred as usize] = always_fire();
    step(&mut w);
    assert!(!w.combat_damaged[prey as usize], "target out of COMBAT_RANGE → no combat");
}

#[test]
fn combat_targets_other_species_not_nearer_kin() {
    let mut w = World::new(7);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let kin = w.spawn_agent(Vec2::new(500.5, 500.0), Genome::neutral()); // same species, nearer
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral()); // other species, farther
    reassign_to_new_species(&mut w, prey);
    arm_predator(&mut w, pred as usize, 10.0, 2.0);
    w.agents.program[pred as usize] = always_fire();
    step(&mut w);
    assert!(!w.combat_damaged[kin as usize], "must not fire at nearer same-species kin");
    assert!(w.combat_damaged[prey as usize], "must fire at the other-species target");
}
```

- [ ] **Step 2: Run the tests to verify they fail** — the file references `w.combat_damaged`/`w.combat_attacker` (don't exist yet) and `Module`/`SensorType`/`ModuleList` re-exports.

Run: `cargo test -p anabios-core --test combat_predation`
Expected: FAIL to compile (`no field combat_damaged on World`, etc.).

- [ ] **Step 3: Add the module helpers** — in `crates/anabios-core/src/module.rs`, after `effective_diet_carnivory` (ends ~line 286):

```rust
/// Damage + energy_cost of the highest-damage `Weapon`, or `None` if the
/// agent has no `Weapon` module (combat gating, design §3.5).
#[inline]
pub fn effective_weapon(modules: &ModuleList) -> Option<(f32, f32)> {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Weapon { damage, energy_cost } => Some((*damage, *energy_cost)),
            _ => None,
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
}

/// Max `Armor.protection`, or `0.0` if the agent has no `Armor` module.
#[inline]
pub fn effective_armor_protection(modules: &ModuleList) -> f32 {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Armor { protection, .. } => Some(*protection),
            _ => None,
        })
        .fold(0.0_f32, f32::max)
}
```

Confirm `ModuleList`, `Module`, `SensorType` are `pub` and re-exported for tests. `module.rs:210` defines `pub type ModuleList`. If `ModuleList::new()`/`push` aren't usable from tests, they are (it's a `SmallVec` type alias). Ensure `SensorType` is `pub` in `module.rs` (it is — used by `starter_kit`).

- [ ] **Step 4: Add World scratch fields** — in `crates/anabios-core/src/world.rs`, in the `World` struct near `actions` (`world.rs:52`):

```rust
    /// Per-tick combat attribution scratch (reset each tick in `interact_all`).
    /// `combat_damaged[t]` is set when slot `t` takes combat damage; read by
    /// `age_and_starve` / the codex detectors to attribute deaths.
    #[serde(skip)]
    pub combat_damaged: Vec<bool>,
    /// Attacker species id for each combat-damaged slot (valid only where
    /// `combat_damaged[t]` is true this tick).
    #[serde(skip)]
    pub combat_attacker: Vec<u32>,
```

In `World::new` (`world.rs:66`) initialize both: `combat_damaged: Vec::new(), combat_attacker: Vec::new(),`.

In `resize_scratch` (`world.rs:165`), after the `actions` resize block:

```rust
        if self.combat_damaged.len() < cap {
            self.combat_damaged.resize(cap, false);
        }
        if self.combat_attacker.len() < cap {
            self.combat_attacker.resize(cap, crate::sense::NO_NEIGHBOR_SPECIES);
        }
```

- [ ] **Step 5: Rewrite `interact_all` to take `&mut World` and add the combat pass** — replace the body of `crates/anabios-core/src/interact.rs` (keep the feeding logic verbatim, just move it under a `&mut World` signature). Full new file body:

```rust
//! Interaction stage: feeding (grazing), combat, and predation (scavenging).

use crate::genome::GenomeSlot;
use crate::module::{self, ModuleType};
use crate::world::World;

/// Max biomass an agent can bite from the biome in one tick (before scaling).
pub const BITE_MAX: f32 = 0.5;
/// Energy yielded per unit of plant biomass eaten.
pub const FOOD_ENERGY_PER_BIOMASS: f32 = 4.0;
/// `fire_intent` above this threshold triggers a weapon attack.
pub const FIRE_THRESHOLD: f32 = 0.5;
/// Contact range (world units) within which combat can land. Mirrors
/// `reproduce::MATING_RANGE`.
pub const COMBAT_RANGE: f32 = 2.0;

/// Run all interaction rules for one tick: feed, then combat, then scavenge.
/// Each pass iterates alive agents in ascending id order (determinism).
pub fn interact_all(world: &mut World) {
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    // Reset combat attribution scratch for this tick.
    for b in world.combat_damaged.iter_mut() {
        *b = false;
    }

    feed_pass(world, &alive_ids);
    combat_pass(world, &alive_ids);
    scavenge_pass(world, &alive_ids);
}

/// Grazing: a herbivore-capable Mouth bites plant biomass at its cell.
fn feed_pass(world: &mut World, alive_ids: &[u32]) {
    for &id in alive_ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Mouth) {
            continue;
        }
        let bite_cap = module::effective_bite_size(&world.agents.modules[i]);
        let diet_carn = module::effective_diet_carnivory(&world.agents.modules[i]);
        let herbivory = (1.0 - diet_carn).clamp(0.0, 1.0);
        if herbivory <= 0.0 || bite_cap <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let size = world.agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let desired_bite = BITE_MAX * size * bite_cap * herbivory;
        let taken = world.biome.graze(pos, desired_bite);
        if taken > 0.0 {
            world.agents.energy[i] += taken * FOOD_ENERGY_PER_BIOMASS;
        }
    }
}

/// Combat: a Weapon-bearing agent that fires deals `damage - target_armor`
/// energy damage to the nearest *other-species* agent within `COMBAT_RANGE`,
/// spending its own weapon `energy_cost`.
fn combat_pass(world: &mut World, alive_ids: &[u32]) {
    for &id in alive_ids {
        let i = id as usize;
        if world.actions[i].fire_intent <= FIRE_THRESHOLD {
            continue;
        }
        let Some((damage, cost)) = module::effective_weapon(&world.agents.modules[i]) else {
            continue; // no Weapon module → gated out
        };
        let tgt = world.sensors[i].nearest_other_id;
        if tgt == crate::sense::NO_NEIGHBOR_ID {
            continue;
        }
        if world.sensors[i].nearest_other_dist >= COMBAT_RANGE {
            continue;
        }
        let t = tgt as usize;
        if t == i || !world.agents.is_alive(tgt) {
            continue;
        }
        let armor = module::effective_armor_protection(&world.agents.modules[t]);
        let net = (damage - armor).max(0.0);
        world.agents.energy[t] -= net;
        world.agents.energy[i] -= cost;
        world.combat_damaged[t] = true;
        world.combat_attacker[t] = world.agents.species_id[i];
    }
}

/// Predation: filled in by Task 3 (carnivore Mouth scavenges carcasses).
fn scavenge_pass(_world: &mut World, _alive_ids: &[u32]) {}
```

Note: `graze` signature — confirm `BiomeField::graze(&mut self, pos, amount) -> f32` matches the original call. The original `interact.rs:33` used `biome.graze(pos, desired_bite)`; preserved.

- [ ] **Step 6: Update the tick call site** — in `crates/anabios-core/src/tick.rs:32`, replace:

```rust
    // Stage 5: interact (feeding, combat, predation).
    interact_all(world);
```

(The `use crate::interact::interact_all;` import at `tick.rs:6` stays.)

- [ ] **Step 7: Run the combat tests to verify they pass**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: PASS (4 tests: damage/armor/cost, no-weapon gating, out-of-range, other-species targeting).
Note (controller): the determinism golden test will now FAIL because the scratch fields are serde-skip but the `interact_all` refactor must not change baseline behavior — run `cargo test -p anabios-core --test determinism`; it should still PASS here (no serialized layout change yet). If it fails, the refactor changed feeding behavior — fix before committing.

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/module.rs crates/anabios-core/src/world.rs \
        crates/anabios-core/src/interact.rs crates/anabios-core/src/tick.rs \
        crates/anabios-core/tests/combat_predation.rs
git commit -m "feat(core): M12 combat — wire FireWeapon to energy damage in interact

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Carcass substrate (formation + decay)

**Files:**
- Create: `crates/anabios-core/src/carcass.rs`
- Modify: `crates/anabios-core/src/lib.rs` (`pub mod carcass;`)
- Modify: `crates/anabios-core/src/world.rs` (`carcasses` field; `new`)
- Modify: `crates/anabios-core/src/age.rs` (form carcass on death)
- Modify: `crates/anabios-core/src/tick.rs` (add `carcass_step` stage)
- Test: `crates/anabios-core/tests/combat_predation.rs` (append)

**Interfaces:**
- Produces: `carcass::Carcass { pos: Vec2, flesh: f32, age: u32, species_id: u32 }` (derives `Debug, Clone, Copy, Serialize, Deserialize`).
- Produces: `carcass::CARCASS_FLESH_PER_SIZE: f32 = 20.0`, `carcass::CARCASS_DECAY_TICKS: u32 = 100`.
- Produces: `carcass::carcass_step(world: &mut World)` — ages all carcasses, removes those with `flesh <= 0.0 || age >= CARCASS_DECAY_TICKS`.
- Produces: `World.carcasses: Vec<Carcass>` (serialized — snapshot layout change).
- Consumes: `AgentBuffers.{genome,position,species_id}`, `GenomeSlot::Size`.

- [ ] **Step 1: Write the failing test** — append to `crates/anabios-core/tests/combat_predation.rs`:

```rust
#[test]
fn death_forms_carcass_with_flesh_proportional_to_size() {
    use anabios_core::carcass::CARCASS_FLESH_PER_SIZE;
    let mut w = World::new(3);
    // Barren spot so the agent starves quickly; strip Locomotor so it can't move.
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Size, 0.5);
    let id = w.spawn_agent(Vec2::new(300.0, 300.0), g);
    w.agents.modules[id as usize].retain(|m| !matches!(m, Module::Locomotor { .. }));
    w.agents.energy[id as usize] = 0.3; // dies next age_and_starve
    // Run until it dies (energy <= 0).
    for _ in 0..50 {
        step(&mut w);
        if !w.agents.is_alive(id) {
            break;
        }
    }
    assert!(!w.agents.is_alive(id), "agent should have starved");
    assert_eq!(w.carcasses.len(), 1, "one carcass formed on death");
    let c = w.carcasses[0];
    // size clamps to >= 0.1; here size = 0.5 → flesh = 0.5 * CARCASS_FLESH_PER_SIZE.
    assert!((c.flesh - 0.5 * CARCASS_FLESH_PER_SIZE).abs() < 1e-3);
    assert_eq!(c.species_id, 0);
}

#[test]
fn carcass_decays_and_is_removed_after_decay_ticks() {
    use anabios_core::carcass::{carcass_step, Carcass, CARCASS_DECAY_TICKS};
    let mut w = World::new(1);
    w.carcasses.push(Carcass {
        pos: Vec2::new(10.0, 10.0),
        flesh: 5.0,
        age: 0,
        species_id: 0,
    });
    for _ in 0..CARCASS_DECAY_TICKS {
        carcass_step(&mut w);
    }
    assert!(w.carcasses.is_empty(), "carcass removed once age reaches CARCASS_DECAY_TICKS");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: FAIL to compile (`no module carcass`, `no field carcasses`).

- [ ] **Step 3: Create the carcass module** — `crates/anabios-core/src/carcass.rs`:

```rust
//! Carcasses: dead-but-edible flesh left by killed/starved agents. Carnivore
//! Mouth modules scavenge them (see `interact::scavenge_pass`). Flesh energy is
//! proportional to body size, not the (depleted) metabolic energy at death —
//! agents die at energy ≤ 0, so flesh must come from body mass to close the
//! trophic loop.

use serde::{Deserialize, Serialize};

use crate::prelude::Vec2;
use crate::world::World;

/// Flesh energy per unit of `GenomeSlot::Size` a fresh carcass carries.
/// (Balance value; tuning deferred to M16.)
pub const CARCASS_FLESH_PER_SIZE: f32 = 20.0;
/// Ticks after which a carcass is removed even if not fully scavenged.
pub const CARCASS_DECAY_TICKS: u32 = 100;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Carcass {
    pub pos: Vec2,
    pub flesh: f32,
    pub age: u32,
    pub species_id: u32,
}

/// Age every carcass by one tick and drop the depleted/expired ones.
/// `retain` preserves order → deterministic.
pub fn carcass_step(world: &mut World) {
    for c in world.carcasses.iter_mut() {
        c.age = c.age.saturating_add(1);
    }
    world.carcasses.retain(|c| c.flesh > 0.0 && c.age < CARCASS_DECAY_TICKS);
}
```

Add `pub mod carcass;` to `crates/anabios-core/src/lib.rs` (alongside the other `pub mod` lines). Confirm `crate::prelude::Vec2` is the right path (`world.rs` uses `crate::prelude::Vec2`; `prelude::Vec2` is `pub(crate)` so within-crate use is fine).

- [ ] **Step 4: Add the `carcasses` field to World** — in `crates/anabios-core/src/world.rs`, in the struct (near the other serialized buffers, NOT the serde-skip scratch):

```rust
    /// Dead-but-edible flesh left by deaths this run; scavenged by carnivores.
    pub carcasses: Vec<crate::carcass::Carcass>,
```

In `World::new`, initialize `carcasses: Vec::new(),`.

- [ ] **Step 5: Form a carcass on death** — in `crates/anabios-core/src/age.rs`, inside the `if died {` block (`age.rs:22-25`), BEFORE `world.agents.kill(id)`:

```rust
        if died {
            let sid = world.agents.species_id[i];
            let size = world.agents.genome[i].get(crate::genome::GenomeSlot::Size).max(0.1);
            let pos = world.agents.position[i];
            world.carcasses.push(crate::carcass::Carcass {
                pos,
                flesh: crate::carcass::CARCASS_FLESH_PER_SIZE * size,
                age: 0,
                species_id: sid,
            });
            world.agents.kill(id);
            world.remove_from_species(sid);
        }
```

(The combat-death attribution block is added here in Task 4.)

- [ ] **Step 6: Add the `carcass_step` stage to the tick** — in `crates/anabios-core/src/tick.rs`, after Stage 7 (`age_and_starve(world);` at `tick.rs:42`):

```rust
    // Stage 7b: carcass aging + removal (design step 9 analogue).
    crate::carcass::carcass_step(world);
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: PASS (both new carcass tests + the 4 Task 1 tests).
Note (controller): the golden-tick determinism test will now FAIL because `World.carcasses` changes the serialized snapshot (and carcasses form from starvation deaths in `minimal.toml`). **Refresh the golden hashes now:** run `cargo test -p anabios-core --test determinism`, read the three actual `(tick, hash)` values from the failure output, update the `GOLDEN` constant at `crates/anabios-core/tests/determinism.rs:15-16`, and re-run to confirm PASS (determinism = same hashes on a second run). This is the anticipated §2.3 refresh.

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/carcass.rs crates/anabios-core/src/lib.rs \
        crates/anabios-core/src/world.rs crates/anabios-core/src/age.rs \
        crates/anabios-core/src/tick.rs crates/anabios-core/tests/combat_predation.rs \
        crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M12 carcass substrate — deaths leave scavengeable flesh

Refresh golden-tick hashes for the new World.carcasses snapshot field (§2.3).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Predation — carnivore Mouth scavenges carcasses

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (fill in `scavenge_pass`)
- Modify: `crates/anabios-core/src/carcass.rs` (scavenge constants)
- Test: `crates/anabios-core/tests/combat_predation.rs` (append)

**Interfaces:**
- Produces: `carcass::SCAVENGE_RANGE: f32 = 2.0`, `carcass::SCAVENGE_MAX: f32 = 0.5`, `carcass::FLESH_ENERGY_PER_UNIT: f32 = 4.0`.
- Consumes: `module::{has, effective_diet_carnivory, effective_bite_size}`, `spatial::torus_distance`, `World.carcasses`.

- [ ] **Step 1: Write the failing test** — append to `crates/anabios-core/tests/combat_predation.rs`:

```rust
#[test]
fn carnivore_scavenges_carcass_gaining_energy_and_depleting_flesh() {
    use anabios_core::carcass::Carcass;
    let mut w = World::new(2);
    let eater = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
    // Carnivore Mouth (diet_affinity = 1.0), no weapon needed to scavenge.
    w.agents.modules[eater as usize] = smallvec_kit(0.0, 0.0, 0.0);
    w.carcasses.push(Carcass {
        pos: Vec2::new(400.5, 400.0), // within SCAVENGE_RANGE
        flesh: 10.0,
        age: 0,
        species_id: 1,
    });
    let e0 = w.agents.energy[eater as usize];
    step(&mut w);
    assert!(w.agents.energy[eater as usize] > e0, "carnivore gained energy from flesh");
    assert!(w.carcasses[0].flesh < 10.0, "carcass flesh depleted by scavenging");
}

#[test]
fn herbivore_does_not_scavenge_flesh() {
    use anabios_core::carcass::Carcass;
    let mut w = World::new(2);
    let eater = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
    // Default starter_kit Mouth has diet_affinity = 0.0 (pure herbivore).
    w.carcasses.push(Carcass {
        pos: Vec2::new(400.5, 400.0),
        flesh: 10.0,
        age: 0,
        species_id: 1,
    });
    step(&mut w);
    assert_eq!(w.carcasses[0].flesh, 10.0, "herbivore Mouth does not eat flesh (gating)");
}

#[test]
fn carcass_out_of_scavenge_range_is_not_eaten() {
    use anabios_core::carcass::Carcass;
    let mut w = World::new(2);
    let eater = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
    w.agents.modules[eater as usize] = smallvec_kit(0.0, 0.0, 0.0); // carnivore
    w.carcasses.push(Carcass {
        pos: Vec2::new(500.0, 400.0), // 100 units away
        flesh: 10.0,
        age: 0,
        species_id: 1,
    });
    step(&mut w);
    assert_eq!(w.carcasses[0].flesh, 10.0, "carcass out of range is untouched");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: FAIL (scavenging is a no-op; `carnivore_scavenges_*` asserts energy gain that doesn't happen).

- [ ] **Step 3: Add scavenge constants** — in `crates/anabios-core/src/carcass.rs`, after `CARCASS_DECAY_TICKS`:

```rust
/// Max distance (world units) a carnivore can reach a carcass. Mirrors
/// `interact::COMBAT_RANGE`.
pub const SCAVENGE_RANGE: f32 = 2.0;
/// Max flesh a Mouth can take from a carcass in one tick (before scaling).
pub const SCAVENGE_MAX: f32 = 0.5;
/// Energy yielded per unit of flesh scavenged (mirrors FOOD_ENERGY_PER_BIOMASS).
pub const FLESH_ENERGY_PER_UNIT: f32 = 4.0;
```

- [ ] **Step 4: Implement `scavenge_pass`** — in `crates/anabios-core/src/interact.rs`, replace the stub:

```rust
/// Predation: a carnivore-capable Mouth bites the nearest carcass within
/// `SCAVENGE_RANGE`, converting its flesh into energy. Ties on distance break
/// toward the lower carcass index (strict `<`), keeping this deterministic.
fn scavenge_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::carcass::{FLESH_ENERGY_PER_UNIT, SCAVENGE_MAX, SCAVENGE_RANGE};
    for &id in alive_ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Mouth) {
            continue;
        }
        let carn = module::effective_diet_carnivory(&world.agents.modules[i]);
        let bite_cap = module::effective_bite_size(&world.agents.modules[i]);
        if carn <= 0.0 || bite_cap <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let mut best: Option<usize> = None;
        let mut best_d = SCAVENGE_RANGE;
        for (ci, c) in world.carcasses.iter().enumerate() {
            if c.flesh <= 0.0 {
                continue;
            }
            let d = crate::spatial::torus_distance(pos, c.pos);
            if d < best_d {
                best_d = d;
                best = Some(ci);
            }
        }
        if let Some(ci) = best {
            let size = world.agents.genome[i].get(GenomeSlot::Size).max(0.1);
            let desired = SCAVENGE_MAX * size * bite_cap * carn;
            let taken = desired.min(world.carcasses[ci].flesh);
            if taken > 0.0 {
                world.carcasses[ci].flesh -= taken;
                world.agents.energy[i] += taken * FLESH_ENERGY_PER_UNIT;
            }
        }
    }
}
```

Confirm `crate::spatial::torus_distance` is `pub` (`spatial.rs:127`). It is.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: PASS (3 new scavenge tests + all prior).
Note (controller): `minimal.toml` has no carnivores, so scavenging never triggers there — the golden hashes from Task 2 stay valid. Run `cargo test -p anabios-core --test determinism` to confirm PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/src/carcass.rs \
        crates/anabios-core/tests/combat_predation.rs
git commit -m "feat(core): M12 predation — carnivore Mouth scavenges carcasses

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Death attribution + `Predation` & `CombatRaid` detectors

**Files:**
- Modify: `crates/anabios-core/src/codex.rs` (EventType variants; `CombatDeath`; `CodexState` fields; `record_combat_death`; two detectors; wire into `observe_all`)
- Modify: `crates/anabios-core/src/age.rs` (record combat-attributed deaths)
- Test: `crates/anabios-core/tests/combat_predation.rs` (append)

**Interfaces:**
- Produces: `EventType::Predation = 6`, `EventType::CombatRaid = 7` (appended; `ArmsRace` is added at index 8 in Task 5).
- Produces: `codex::CombatDeath { tick: u64, victim_species: u32, attacker_species: u32, loc_x: f32, loc_y: f32 }`.
- Produces: `CodexState.combat_deaths: VecDeque<CombatDeath>`, `.predation_emitted: bool`, `.raid_active: bool`.
- Produces: `CodexState::record_combat_death(&mut self, tick, victim_species, attacker_species, x, y)`.
- Produces: `codex::COMBAT_RAID_WINDOW: u64 = 100`, `codex::COMBAT_RAID_THRESHOLD: usize = 3`.
- Consumes: `World.{combat_damaged, combat_attacker}` (Task 1), carcass formation block (Task 2).

- [ ] **Step 1: Write the failing test** — append to `crates/anabios-core/tests/combat_predation.rs`:

```rust
use anabios_core::codex::EventType;

/// Count events of a given type currently in the codex ring buffer.
fn count_events(w: &World, t: EventType) -> usize {
    w.codex.events.iter().filter(|e| e.event_type == t).count()
}

/// Build a lethal predator (huge damage) that always fires, adjacent to prey.
fn spawn_lethal_duel(seed: u64) -> (World, u32, u32) {
    let mut w = World::new(seed);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    reassign_to_new_species(&mut w, prey);
    arm_predator(&mut w, pred as usize, /*damage=*/ 1000.0, /*cost=*/ 1.0);
    w.agents.program[pred as usize] = always_fire();
    (w, pred, prey)
}

#[test]
fn predation_event_fires_once_on_a_combat_kill() {
    let (mut w, _pred, prey) = spawn_lethal_duel(11);
    // Step until the prey dies from combat.
    for _ in 0..10 {
        step(&mut w);
        if !w.agents.is_alive(prey) {
            break;
        }
    }
    assert!(!w.agents.is_alive(prey), "prey should be killed by combat");
    assert_eq!(count_events(&w, EventType::Predation), 1, "Predation fires exactly once");
    // Keep stepping — it must not fire again (latched).
    for _ in 0..20 {
        step(&mut w);
    }
    assert_eq!(count_events(&w, EventType::Predation), 1, "Predation stays latched");
}

#[test]
fn starvation_death_does_not_fire_predation() {
    let mut w = World::new(5);
    let mut g = Genome::neutral();
    let id = w.spawn_agent(Vec2::new(300.0, 300.0), g.clone());
    w.agents.modules[id as usize].retain(|m| !matches!(m, Module::Locomotor { .. }));
    w.agents.energy[id as usize] = 0.2;
    let _ = &mut g;
    for _ in 0..50 {
        step(&mut w);
        if !w.agents.is_alive(id) {
            break;
        }
    }
    assert!(!w.agents.is_alive(id), "agent starved");
    assert_eq!(count_events(&w, EventType::Predation), 0, "starvation is not predation");
}

#[test]
fn combat_raid_fires_on_sustained_conflict_not_a_single_kill() {
    use anabios_core::codex::{CombatDeath, COMBAT_RAID_THRESHOLD};
    // Drive the detector directly via recorded combat deaths, then observe.
    let mut w = World::new(9);
    // A single death: below threshold → no raid.
    w.codex.record_combat_death(w.tick, 1, 0, 10.0, 10.0);
    anabios_core::codex::observe_all(&mut w);
    assert_eq!(count_events(&w, EventType::CombatRaid), 0, "one kill is not a raid");
    // Push up to threshold within the window.
    for _ in 1..COMBAT_RAID_THRESHOLD {
        w.codex.record_combat_death(w.tick, 1, 0, 10.0, 10.0);
    }
    anabios_core::codex::observe_all(&mut w);
    assert_eq!(count_events(&w, EventType::CombatRaid), 1, "sustained conflict → one CombatRaid");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: FAIL to compile (`EventType::Predation` etc. don't exist).

- [ ] **Step 3: Append the EventType variants** — in `crates/anabios-core/src/codex.rs`, at the END of the enum (`codex.rs:44`, after `NovelBehaviorPattern = 5`):

```rust
    NovelBehaviorPattern = 5,
    /// First agent death caused by another agent's weapon (vs starvation/age).
    Predation = 6,
    /// Sustained combat deaths crossing a rolling window threshold.
    CombatRaid = 7,
```

- [ ] **Step 4: Add `CombatDeath`, `CodexState` fields, and `record_combat_death`** — in `crates/anabios-core/src/codex.rs`. Add constants near the other window constants (`codex.rs:23-33`):

```rust
/// Window (ticks) over which combat deaths accumulate for CombatRaid.
pub const COMBAT_RAID_WINDOW: u64 = 100;
/// Combat deaths within the window needed to declare a CombatRaid.
pub const COMBAT_RAID_THRESHOLD: usize = 3;
```

Add the record struct (near `CodexEvent`):

```rust
/// A death attributed to another agent's weapon. Fuel for the Predation /
/// CombatRaid detectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatDeath {
    pub tick: u64,
    pub victim_species: u32,
    pub attacker_species: u32,
    pub loc_x: f32,
    pub loc_y: f32,
}
```

Add fields to `CodexState` (`codex.rs:66-77`), before `events`:

```rust
    /// Rolling window of combat-attributed deaths (pruned to COMBAT_RAID_WINDOW).
    pub combat_deaths: VecDeque<CombatDeath>,
    /// Latch: the first Predation event has been emitted.
    pub predation_emitted: bool,
    /// Edge-trigger state for CombatRaid (armed while below threshold).
    pub raid_active: bool,
```

`CodexState` derives `Default` — `VecDeque`/`bool` default fine. Add the recorder method in the `impl CodexState` block:

```rust
    /// Record a combat-attributed death for the Predation/CombatRaid detectors.
    pub fn record_combat_death(
        &mut self,
        tick: u64,
        victim_species: u32,
        attacker_species: u32,
        x: f32,
        y: f32,
    ) {
        self.combat_deaths.push_back(CombatDeath {
            tick,
            victim_species,
            attacker_species,
            loc_x: x,
            loc_y: y,
        });
    }
```

- [ ] **Step 5: Record combat deaths in `age_and_starve`** — in `crates/anabios-core/src/age.rs`, extend the `if died {` block (from Task 2) to attribute combat kills BEFORE `world.agents.kill(id)`:

```rust
        if died {
            let sid = world.agents.species_id[i];
            let size = world.agents.genome[i].get(crate::genome::GenomeSlot::Size).max(0.1);
            let pos = world.agents.position[i];
            world.carcasses.push(crate::carcass::Carcass {
                pos,
                flesh: crate::carcass::CARCASS_FLESH_PER_SIZE * size,
                age: 0,
                species_id: sid,
            });
            if world.combat_damaged.get(i).copied().unwrap_or(false) {
                let attacker = world.combat_attacker[i];
                world.codex.record_combat_death(world.tick, sid, attacker, pos.x, pos.y);
            }
            world.agents.kill(id);
            world.remove_from_species(sid);
        }
```

- [ ] **Step 6: Add the two detectors and wire them in** — in `crates/anabios-core/src/codex.rs`, add after `detect_novel_behavior`:

```rust
/// Predation: emit once, the first tick a combat-attributed death is recorded.
/// Payload species = the attacker (predator) species.
fn detect_predation(world: &mut World) {
    if world.codex.predation_emitted {
        return;
    }
    let tick = world.tick;
    if let Some(cd) = world.codex.combat_deaths.iter().find(|d| d.tick == tick) {
        let ev = CodexEvent {
            event_type: EventType::Predation,
            tick,
            species_id: cd.attacker_species,
            value: 1.0,
            loc_x: cd.loc_x,
            loc_y: cd.loc_y,
        };
        world.codex.push_event(ev);
        world.codex.predation_emitted = true;
    }
}

/// CombatRaid: prune the combat-death window, then edge-trigger when the count
/// reaches COMBAT_RAID_THRESHOLD. Re-arms when it drops back below threshold.
fn detect_combat_raid(world: &mut World) {
    let tick = world.tick;
    let cutoff = tick.saturating_sub(COMBAT_RAID_WINDOW);
    while let Some(front) = world.codex.combat_deaths.front() {
        if front.tick < cutoff {
            world.codex.combat_deaths.pop_front();
        } else {
            break;
        }
    }
    let count = world.codex.combat_deaths.len();
    let raiding = count >= COMBAT_RAID_THRESHOLD;
    if raiding && !world.codex.raid_active {
        let last = world.codex.combat_deaths.back().expect("non-empty when raiding");
        let ev = CodexEvent {
            event_type: EventType::CombatRaid,
            tick,
            species_id: last.attacker_species,
            value: count as f32,
            loc_x: last.loc_x,
            loc_y: last.loc_y,
        };
        world.codex.push_event(ev);
        world.codex.raid_active = true;
    } else if !raiding {
        world.codex.raid_active = false;
    }
}
```

Wire into `observe_all` (`codex.rs:105`, after `detect_novel_behavior`):

```rust
    detect_novel_behavior(world, &centroids);
    detect_predation(world);
    detect_combat_raid(world);
```

- [ ] **Step 7: Run to verify pass**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: PASS (3 new detector tests + all prior).
Note (controller): `CodexState` gained serialized fields → refresh golden hashes again. Run `cargo test -p anabios-core --test determinism`, update `GOLDEN` at `determinism.rs:15-16` with the new values, re-run to confirm PASS. (`minimal.toml` emits no combat events, so the change is purely the empty `VecDeque`/`bool` layout addition.)

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/src/age.rs \
        crates/anabios-core/tests/combat_predation.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M12 Predation + CombatRaid detectors with death attribution

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: `ArmsRace` detector (co-rising weapon/armor trend)

**Files:**
- Modify: `crates/anabios-core/src/codex.rs` (EventType variant; trait-history fields; pure `arms_race_signal`; `detect_arms_race`; wire in)
- Test: `crates/anabios-core/tests/combat_predation.rs` (append)

**Interfaces:**
- Produces: `EventType::ArmsRace = 8` (appended at end, after `CombatRaid = 7`).
- Produces: `CodexState.weapon_history: BTreeMap<u32, VecDeque<f32>>`, `.armor_history: BTreeMap<u32, VecDeque<f32>>`, `.arms_race_active: bool`.
- Produces: `codex::ARMS_WINDOW: usize = 20`, `codex::ARMS_MIN_DELTA: f32 = 0.5`.
- Produces (pure, testable): `codex::arms_race_signal(weapon_history, armor_history) -> Option<(u32, f32)>` — returns `(weaponized_species, weapon_rise)` when some species' mean weapon damage rose by ≥ `ARMS_MIN_DELTA` across a full window AND a *different* species' mean armor rose by ≥ `ARMS_MIN_DELTA`.
- Consumes: `module::{effective_weapon, effective_armor_protection}` (Task 1).

- [ ] **Step 1: Write the failing test** — append to `crates/anabios-core/tests/combat_predation.rs`:

```rust
#[test]
fn arms_race_signal_detects_co_rising_trend() {
    use anabios_core::codex::{arms_race_signal, ARMS_WINDOW};
    use std::collections::{BTreeMap, VecDeque};
    let mut weapon: BTreeMap<u32, VecDeque<f32>> = BTreeMap::new();
    let mut armor: BTreeMap<u32, VecDeque<f32>> = BTreeMap::new();
    // Species 0: weapon damage rises 0→10 over the window.
    // Species 1: armor rises 0→10 over the window.
    let rising: VecDeque<f32> = (0..ARMS_WINDOW).map(|k| k as f32 * 0.6).collect();
    let flat: VecDeque<f32> = (0..ARMS_WINDOW).map(|_| 1.0).collect();
    weapon.insert(0, rising.clone());
    weapon.insert(1, flat.clone());
    armor.insert(0, flat.clone());
    armor.insert(1, rising.clone());
    let sig = arms_race_signal(&weapon, &armor);
    assert!(matches!(sig, Some((0, _))), "species 0 weapons + species 1 armor both rise");
}

#[test]
fn arms_race_signal_silent_on_flat_traits() {
    use anabios_core::codex::{arms_race_signal, ARMS_WINDOW};
    use std::collections::{BTreeMap, VecDeque};
    let flat: VecDeque<f32> = (0..ARMS_WINDOW).map(|_| 1.0).collect();
    let mut weapon = BTreeMap::new();
    let mut armor = BTreeMap::new();
    weapon.insert(0, flat.clone());
    armor.insert(1, flat.clone());
    assert!(arms_race_signal(&weapon, &armor).is_none(), "flat traits → no arms race");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: FAIL to compile (`arms_race_signal`, `ARMS_WINDOW` don't exist).

- [ ] **Step 3: Add the variant, constants, fields, pure signal, and detector** — in `crates/anabios-core/src/codex.rs`:

Append the variant at the enum end (after `CombatRaid = 7`):

```rust
    /// One species' mean weapon damage and another's mean armor both trend up.
    ArmsRace = 8,
```

Constants:

```rust
/// Samples retained per species for the weapon/armor trend windows.
pub const ARMS_WINDOW: usize = 20;
/// Minimum rise (window back − front) in a trait mean to count as "trending up".
pub const ARMS_MIN_DELTA: f32 = 0.5;
```

`CodexState` fields (before `events`):

```rust
    /// Rolling per-species mean weapon damage (window ARMS_WINDOW).
    pub weapon_history: BTreeMap<u32, VecDeque<f32>>,
    /// Rolling per-species mean armor protection (window ARMS_WINDOW).
    pub armor_history: BTreeMap<u32, VecDeque<f32>>,
    /// Edge-trigger state for ArmsRace.
    pub arms_race_active: bool,
```

Pure signal helper (module-level `pub fn`):

```rust
/// Pure ArmsRace test: is there a species whose weapon-damage mean rose across
/// a full window while a *different* species' armor mean also rose? Returns
/// `(weaponized_species, weapon_rise)`.
pub fn arms_race_signal(
    weapon_history: &BTreeMap<u32, VecDeque<f32>>,
    armor_history: &BTreeMap<u32, VecDeque<f32>>,
) -> Option<(u32, f32)> {
    let rise = |buf: &VecDeque<f32>| -> Option<f32> {
        if buf.len() < ARMS_WINDOW {
            return None;
        }
        let delta = buf.back()? - buf.front()?;
        (delta >= ARMS_MIN_DELTA).then_some(delta)
    };
    for (wsid, wbuf) in weapon_history.iter() {
        let Some(wrise) = rise(wbuf) else { continue };
        for (asid, abuf) in armor_history.iter() {
            if asid == wsid {
                continue;
            }
            if rise(abuf).is_some() {
                return Some((*wsid, wrise));
            }
        }
    }
    None
}

/// Update per-species weapon/armor trend windows from the current population,
/// then edge-trigger ArmsRace when a co-rising trend appears.
fn detect_arms_race(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    // Accumulate per-species means (BTreeMap → deterministic).
    let mut wsum: BTreeMap<u32, (f64, u32)> = BTreeMap::new();
    let mut asum: BTreeMap<u32, (f64, u32)> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let wd = crate::module::effective_weapon(&world.agents.modules[i])
            .map(|(d, _)| d)
            .unwrap_or(0.0);
        let ap = crate::module::effective_armor_protection(&world.agents.modules[i]);
        let w = wsum.entry(sid).or_insert((0.0, 0));
        w.0 += wd as f64;
        w.1 += 1;
        let a = asum.entry(sid).or_insert((0.0, 0));
        a.0 += ap as f64;
        a.1 += 1;
    }
    let push = |hist: &mut BTreeMap<u32, VecDeque<f32>>, sid: u32, mean: f32| {
        let buf = hist.entry(sid).or_default();
        if buf.len() == ARMS_WINDOW {
            buf.pop_front();
        }
        buf.push_back(mean);
    };
    for (sid, (sum, n)) in wsum.iter() {
        push(&mut world.codex.weapon_history, *sid, (*sum / *n as f64) as f32);
    }
    for (sid, (sum, n)) in asum.iter() {
        push(&mut world.codex.armor_history, *sid, (*sum / *n as f64) as f32);
    }

    let signal = arms_race_signal(&world.codex.weapon_history, &world.codex.armor_history);
    match signal {
        Some((sid, rise)) if !world.codex.arms_race_active => {
            let (lx, ly) = centroid_of(centroids, sid);
            world.codex.push_event(CodexEvent {
                event_type: EventType::ArmsRace,
                tick: world.tick,
                species_id: sid,
                value: rise,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.arms_race_active = true;
        }
        None => world.codex.arms_race_active = false,
        _ => {}
    }
}
```

Wire into `observe_all` after `detect_combat_raid`:

```rust
    detect_combat_raid(world);
    detect_arms_race(world, &centroids);
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p anabios-core --test combat_predation`
Expected: PASS (2 new arms-race signal tests + all prior).
Note (controller): `CodexState` gained two `BTreeMap` fields + a bool → refresh golden hashes. Run `cargo test -p anabios-core --test determinism`; update `GOLDEN`; re-run to confirm.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/tests/combat_predation.rs \
        crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M12 ArmsRace detector — co-rising weapon/armor trend

Detector ships in M12; emergence confirmation deferred to M16 (spec §5).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: Sweep integration (event names + CSV columns)

**Files:**
- Modify: `crates/anabios-headless/src/sweep.rs` (`event_name` match; `write_summary_csv` header + `g()` calls; top comment)
- Test: `crates/anabios-headless/src/sweep.rs` (add a `#[test]` for `event_name`, or a small CSV-header assertion)

**Interfaces:**
- Consumes: `EventType::{Predation, CombatRaid, ArmsRace}` (Tasks 4–5).

- [ ] **Step 1: Write the failing test** — add to the `#[cfg(test)] mod tests` in `crates/anabios-headless/src/sweep.rs` (create the module if absent):

```rust
#[test]
fn event_name_covers_m12_events() {
    use anabios_core::codex::EventType;
    assert_eq!(super::event_name(EventType::Predation), "predation");
    assert_eq!(super::event_name(EventType::CombatRaid), "combat_raid");
    assert_eq!(super::event_name(EventType::ArmsRace), "arms_race");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-headless event_name_covers_m12_events`
Expected: FAIL to compile — the `match` in `event_name` is non-exhaustive (missing the 3 new variants).

- [ ] **Step 3: Extend `event_name`** — in `crates/anabios-headless/src/sweep.rs:93-102`, add the three arms:

```rust
        EventType::NovelBehaviorPattern => "novel_behavior",
        EventType::Predation => "predation",
        EventType::CombatRaid => "combat_raid",
        EventType::ArmsRace => "arms_race",
```

- [ ] **Step 4: Extend the CSV header + rows** — in `write_summary_csv` (`sweep.rs:107-111`), change the header string to append the three columns:

```rust
        "seed,ticks,final_alive,final_biomass,state_hash,\
         extinction,pop_crash,speciation,migration,novel_module,novel_behavior,\
         predation,combat_raid,arms_race"
```

Add three format placeholders and three `g()` calls to the per-row `writeln!` (extend the format string with `,{},{},{}` and append `g("predation"), g("combat_raid"), g("arms_race"),`). Update the top-of-file comment from "6 events" to "9 events".

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p anabios-headless`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-headless/src/sweep.rs
git commit -m "feat(headless): M12 sweep — predation/combat_raid/arms_race columns

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Determinism lock + snapshot round-trip verification

**Files:**
- Verify: `crates/anabios-core/tests/determinism.rs` (GOLDEN already refreshed across Tasks 2/4/5)
- Verify: `crates/anabios-core/src/snapshot.rs` round-trip tests (if present)

This task is a controller-run verification gate (no new production code). It confirms the accumulated snapshot-layout changes are internally consistent and stable.

- [ ] **Step 1: Confirm determinism is green and stable**

Run: `cargo test -p anabios-core --test determinism`
Expected: PASS. If it fails, the GOLDEN constant was not refreshed after the last snapshot-affecting task — regenerate from the failure output and re-run.

- [ ] **Step 2: Confirm snapshot round-trip covers the new state**

Run: `cargo test -p anabios-core snapshot`
Expected: PASS. If a snapshot round-trip test enumerates `World` fields explicitly, ensure `carcasses` and the new `CodexState` fields survive a save/load cycle (they derive `Serialize`/`Deserialize`, so a generic bincode round-trip already covers them). If the test builds a world, steps it, serializes, deserializes, and re-hashes, confirm carcasses/codex state match.

- [ ] **Step 3: Full workspace gate**

Run: `cargo test --workspace` then `cargo clippy --workspace --all-targets -- -D warnings` then `cargo fmt --check`
Expected: all PASS/clean.

- [ ] **Step 4: Commit (only if GOLDEN or any file changed here)**

```bash
git add -A
git commit -m "test(core): M12 determinism + snapshot round-trip verification

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

(If nothing changed, skip the commit — the determinism refreshes were committed with their originating tasks.)

---

## Task 8: Scenario archetype extension (seed programs + modules + species)

**Files:**
- Modify: `crates/anabios-core/src/module.rs` (`predator_kit()` constructor)
- Modify: `crates/anabios-core/src/scenario.rs` (`archetype` field; per-archetype seeding + species grouping)
- Modify: `crates/anabios-core/src/world.rs` (a `spawn_seeded` path taking species + modules + program)
- Test: `crates/anabios-core/src/scenario.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `module::predator_kit() -> ModuleList` — Locomotor + Vision Sensor + carnivore Mouth (`diet_affinity: 1.0`) + `Weapon { damage: 8.0, energy_cost: 1.0 }`.
- Produces: `World::spawn_seeded(&mut self, position, genome, species_id, modules, program) -> AgentId` — like `spawn_agent` but with explicit species/modules/program (registers the species via `add_to_species`).
- Produces: `AgentSpec.archetype: Option<String>` — one of `"grazer"`, `"stalker"`, `"pack_hunter"`, `"sentinel"`, `"herd"`. When set, each archetype group instantiates into its own species id (assigned in scenario-declaration order, starting at 0) with the matching starter program + module kit.
- Consumes: `program::{starter_grazer, starter_stalker, starter_pack_hunter, starter_sentinel, starter_herd}`, `module::{starter_kit, predator_kit}`.

- [ ] **Step 1: Write the failing test** — add to `crates/anabios-core/src/scenario.rs` `mod tests`:

```rust
    #[test]
    fn archetype_seeds_distinct_species_with_kits() {
        let text = r#"
name = "pp"
seed = 3

[[agents]]
count = 4
archetype = "grazer"
placement = { kind = "uniform" }

[[agents]]
count = 2
archetype = "stalker"
placement = { kind = "uniform" }
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let w = s.instantiate();
        assert_eq!(w.agents.live_count(), 6);
        // Two distinct species: 0 (grazers) and 1 (stalkers).
        let stalkers: Vec<u32> = w
            .agents
            .iter_alive()
            .filter(|&id| w.agents.species_id[id as usize] == 1)
            .collect();
        assert_eq!(stalkers.len(), 2, "stalker archetype forms species 1");
        // Stalkers carry a Weapon module (predator kit).
        for id in stalkers {
            assert!(
                crate::module::effective_weapon(&w.agents.modules[id as usize]).is_some(),
                "stalker has a Weapon"
            );
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core scenario::tests::archetype_seeds_distinct_species_with_kits`
Expected: FAIL to compile (`archetype` field unknown).

- [ ] **Step 3: Add `predator_kit`** — in `crates/anabios-core/src/module.rs`, after `starter_kit` (`module.rs:222`):

```rust
/// A carnivore starter kit: mobile, sighted, meat-eating, and armed. Used by
/// the `stalker`/`pack_hunter` scenario archetypes.
pub fn predator_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.7, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.8, acuity: 0.7 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 1.0 },
        Module::Weapon { damage: 8.0, energy_cost: 1.0 },
    ]
}
```

- [ ] **Step 4: Add `spawn_seeded`** — in `crates/anabios-core/src/world.rs`, after `spawn_agent` (`world.rs:117`):

```rust
    /// Spawn an agent with an explicit species, module kit, and program.
    /// Used by scenario archetypes (`spawn_agent` always uses species 0 +
    /// grazer defaults).
    pub fn spawn_seeded(
        &mut self,
        position: Vec2,
        genome: Genome,
        species_id: crate::agent::SpeciesId,
        modules: crate::module::ModuleList,
        program: crate::program::Program,
    ) -> AgentId {
        let lineage = self.next_lineage();
        let id = self.agents.spawn(
            position,
            genome,
            lineage,
            [LINEAGE_NONE; 2],
            species_id,
            modules,
            program,
        );
        self.add_to_species(species_id);
        id
    }
```

Confirm `crate::agent::SpeciesId` is the right alias (`agent.rs:49` uses `SpeciesId`). Ensure the species table (`species_centroids`/`species_parents`) is grown for non-zero species — `add_to_species` only grows `species_member_counts`. See Step 6 for centroid/parent seeding in `instantiate`.

- [ ] **Step 5: Add the `archetype` field** — in `crates/anabios-core/src/scenario.rs`, `AgentSpec` (`scenario.rs:22-28`):

```rust
    #[serde(default)]
    pub archetype: Option<String>,
```

Add a resolver mapping name → (species program builder, module kit):

```rust
/// Resolve an archetype name to its starter program + module kit. Unknown
/// names fall back to the grazer defaults.
fn archetype_kit(name: &str) -> (crate::module::ModuleList, crate::program::Program) {
    use crate::module::{predator_kit, starter_kit};
    use crate::program::{
        starter_grazer, starter_herd, starter_pack_hunter, starter_sentinel, starter_stalker,
    };
    match name {
        "stalker" => (predator_kit(), starter_stalker()),
        "pack_hunter" => (predator_kit(), starter_pack_hunter()),
        "sentinel" => (starter_kit(), starter_sentinel()),
        "herd" => (starter_kit(), starter_herd()),
        _ => (starter_kit(), starter_grazer()),
    }
}
```

- [ ] **Step 6: Seed archetypes in `instantiate`** — in `crates/anabios-core/src/scenario.rs`, rewrite the per-spec loop so that each spec with an `archetype` gets its own species id (assigned by spec index). Replace the body of the `for spec in &self.agents` loop:

```rust
        for (spec_idx, spec) in self.agents.iter().enumerate() {
            // Each archetype spec is its own species; specs without an
            // archetype stay in species 0 (legacy trait-only behavior).
            let (species_id, kit) = match &spec.archetype {
                Some(name) => {
                    let sid = spec_idx as u32;
                    // Grow the species tables for this id (spawn_seeded's
                    // add_to_species only grows the member-count vec).
                    while w.species_centroids.len() <= sid as usize {
                        w.species_centroids.push(Genome::neutral());
                        w.species_parents.push(Some(0));
                        w.species_member_counts.push(0);
                    }
                    if w.next_species_id <= sid {
                        w.next_species_id = sid + 1;
                    }
                    (sid, Some(archetype_kit(name)))
                }
                None => (0u32, None),
            };
            for _ in 0..spec.count {
                let position = match spec.placement {
                    Placement::Uniform => {
                        let x = w.rng.f32_range(0.0, WORLD_SIZE);
                        let y = w.rng.f32_range(0.0, WORLD_SIZE);
                        Vec2::new(x, y)
                    }
                    Placement::Cluster { center_x, center_y, radius } => {
                        let theta = w.rng.f32_range(0.0, std::f32::consts::TAU);
                        let r = w.rng.f32_range(0.0, radius);
                        Vec2::new(
                            center_x + r * crate::mathf::cosf(theta),
                            center_y + r * crate::mathf::sinf(theta),
                        )
                    }
                };
                let mut g = Genome::neutral();
                spec.traits.apply(&mut g);
                match &kit {
                    Some((modules, program)) => {
                        w.spawn_seeded(position, g, species_id, modules.clone(), program.clone());
                    }
                    None => {
                        w.spawn_agent(position, g);
                    }
                }
            }
        }
```

Confirm `World` exposes `species_centroids`, `species_parents`, `species_member_counts`, `next_species_id` as `pub` (they are — used by `social_substrate.rs` test helper). Confirm `Genome` is imported in `scenario.rs` (it is, `scenario.rs:7`).

- [ ] **Step 7: Run to verify pass**

Run: `cargo test -p anabios-core scenario`
Expected: PASS (new archetype test + the 3 existing scenario tests, which use no `archetype` and stay species 0).

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/module.rs crates/anabios-core/src/world.rs \
        crates/anabios-core/src/scenario.rs
git commit -m "feat(core): M12 scenario archetypes — seed predator/prey species + kits

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 9: Emergence scenario + multi-seed test

**Files:**
- Create: `scenarios/predator-prey.toml`
- Create: `crates/anabios-core/tests/predator_prey_emergence.rs`

**Interfaces:**
- Consumes: `Scenario::{parse_toml, instantiate}` (Task 8), `tick::step`, `EventType::Predation`.

**Emergence-test discipline (spec §2.2):** the test is deterministic (fixed seeds, fixed tick budget). The controller measures the ACTUAL pass rate first, then sets the asserted floor comfortably below it. Gate behind release so debug CI stays fast.

- [ ] **Step 1: Write the scenario** — `scenarios/predator-prey.toml`:

```toml
name = "predator-prey"
seed = 0

# Prey: a herd of grazers clustered in the middle of the world.
[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 200.0 }
[agents.traits]
size = 0.5
lifespan_bias = 1.0

# Predators: a handful of stalkers seeded into the same region.
[[agents]]
count = 8
archetype = "stalker"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 200.0 }
[agents.traits]
size = 0.7
diet_carnivory = 1.0
lifespan_bias = 1.0
```

- [ ] **Step 2: Write the (initially failing) emergence test** — `crates/anabios-core/tests/predator_prey_emergence.rs`:

```rust
//! M12 emergence: seeded stalkers predate grazers across many seeds.
//! Release-gated (ignored in debug builds) per spec §2.2.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/predator-prey.toml");
const SEEDS: u64 = 16;
const TICKS: u32 = 800;
/// Floor set below the measured pass rate (Step 4 records the real number).
const PREDATION_FLOOR: u64 = 10;

#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn predation_emerges_across_seeds() {
    let mut with_predation = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse predator-prey");
        s.seed = seed;
        let mut w = s.instantiate();
        for _ in 0..TICKS {
            step(&mut w);
        }
        let predated = w.codex.events.iter().any(|e| e.event_type == EventType::Predation);
        if predated {
            with_predation += 1;
        }
    }
    assert!(
        with_predation >= PREDATION_FLOOR,
        "Predation emerged in only {with_predation}/{SEEDS} seeds (floor {PREDATION_FLOOR})"
    );
}
```

Confirm the `include_str!` path: this test file is at `crates/anabios-core/tests/`, so `../../../scenarios/` reaches the repo-root `scenarios/` (matches `determinism.rs:11` which uses `../../../scenarios/minimal.toml`).

- [ ] **Step 3: Run in release to measure the real rate** (controller)

Run: `cargo test -p anabios-core --release --test predator_prey_emergence -- --ignored --nocapture`
Expected: it runs 16 seeds. Read the actual `with_predation` count.

- [ ] **Step 4: Tune the floor and (if needed) the scenario** (controller judgment)

- If predation is robust (e.g. 15/16), set `PREDATION_FLOOR` a few below the observed value (e.g. 12) so tuning drift won't flake it.
- If predation is too rare (predators starve before reaching prey, or prey wiped instantly), adjust the scenario: raise predator `count`, tighten the shared `radius` so encounters happen sooner, or extend `TICKS`. Re-measure until predation is reliable AND both species persist past the initial ticks (spec: "both populations persist past a crash-only baseline"). Optionally add a second assertion that ≥1 grazer and ≥1 stalker remain alive at `TICKS` in a majority of seeds — add it only if it holds with margin.
- Record the observed rate in a code comment above `PREDATION_FLOOR`.

- [ ] **Step 5: Confirm the test passes in release and is ignored in debug**

Run: `cargo test -p anabios-core --release --test predator_prey_emergence -- --ignored`
Expected: PASS.
Run: `cargo test -p anabios-core --test predator_prey_emergence`
Expected: 0 run / 1 ignored (debug gate works).

- [ ] **Step 6: Commit**

```bash
git add scenarios/predator-prey.toml crates/anabios-core/tests/predator_prey_emergence.rs
git commit -m "test(core): M12 predator-prey emergence — Predation across seeds

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review (author checklist — completed)

**1. Spec coverage (spec §M12):**
- Combat wiring (FireWeapon + Weapon gating, damage−armor, energy_cost, nearest_other_id targeting, lethal via normal death path) → Task 1. ✅
- Predation (carnivore Mouth on flesh, closes trophic loop) → Tasks 2 (carcass) + 3 (scavenge). ✅
- `Predation` detector (first kill, not starvation; predator species payload) → Task 4. ✅
- `CombatRaid` detector (sustained vs one-off) → Task 4. ✅
- `ArmsRace` detector (ships M12, emergence deferred M16) → Task 5. ✅
- Determinism (id-ordered interact, attacker-id ties) → Tasks 1/3 (strict `<` tie-breaks) + Task 7. ✅
- Mechanism tests (damage/armor/cost, no-weapon gating, out-of-range, carnivore vs herbivore, Predation once, CombatRaid sustained) → Tasks 1/3/4. ✅
- Emergence scenario + multi-seed test → Tasks 8 (archetype substrate) + 9. ✅
- Sweep integration (event_name + CSV, "6"→"9") → Task 6. ✅
- Golden-tick refresh (§2.3) → Tasks 2/4/5 (controller) + Task 7 lock. ✅

**2. Placeholder scan:** every code step contains full code; test steps contain full assertions; the only judgment step (Task 9 Step 4 floor tuning) is explicitly a measure-then-set controller action, not a placeholder. ✅

**3. Type consistency:** `effective_weapon -> Option<(f32,f32)>` (Task 1) reused in Task 5. `Carcass` fields (`pos/flesh/age/species_id`) consistent across Tasks 2/3/4. `combat_damaged: Vec<bool>` / `combat_attacker: Vec<u32>` set in Task 1, read in Task 4. `CombatDeath` fields consistent Task 4↔tests. `spawn_seeded` signature consistent Task 8↔9 (via scenario). `EventType` appended in order 6/7/8 across Tasks 4/5. ✅

## Deviation notes (for reviewers)

- **Carcass flesh comes from body size, not "remaining energy."** The spec §M12 says predation converts "a fraction of the victim's remaining energy." But agents die at energy ≤ 0 (starvation *and* combat both drive energy down), so remaining energy at death is ~0 — a literal reading yields no food and no trophic loop. Flesh is therefore `CARCASS_FLESH_PER_SIZE * size` (body mass), which is the coherent food source and is what the spec's own mechanism test ("carcass depletes, eater gains") requires. Chosen model confirmed with the user (carcass scavenging).
- **CombatRaid "regional" rate is approximated by a global rolling window** for M12 (`COMBAT_RAID_WINDOW`/`THRESHOLD`). Finer spatial regionality is a balancing concern deferred to M16; the detector still distinguishes sustained conflict (≥3 deaths/window) from one-off predation (1 death).
```
