# Weapons Branch + Demographic Payoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Append four military inventions (Hafted Spears, Archery, Fortifications, Steel Arms) to the tech tree with combat spoils, target-side defense, weapon range, and a birth-ledger subsidy, so invention-holding lineages gain a demographic edge.

**Architecture:** Everything rides the existing invention substrate: appended `INVENTIONS` entries own new meme channels (`MEME_CHANNELS` 20→24), and four new identity-at-mask-0 multipliers hook into `combat_pass` (spoils/defense/range) and `reproduce::is_eligible` (birth threshold). No new flag, no new codex events, no new RNG draws. `FORMAT_VERSION` 39→40; all goldens regenerate once at the end.

**Tech Stack:** Rust (`crates/anabios-core`), GDScript viewer table (`game/scripts/building_sprites.gd`), TOML scenario.

**Spec:** `docs/superpowers/specs/2026-09-07-weapons-branch-design.md`

## Global Constraints

- **Byte-identity when inert:** with `inventions_enabled = false`, or with none of the four techs held, every new code path must be arithmetically exact identity (`× 1.0` multipliers, spoils guarded by `if spoils > 0.0` so no `+0.0` is ever added to energy — `-0.0 + 0.0` flips the sign bit and changes state hashes). No new RNG draws anywhere.
- **Append-only ids:** new inventions take ids 10–13; ids 0–9, their channels, and their keys are untouched.
- **`FORMAT_VERSION` 39→40** in `crates/anabios-core/src/snapshot.rs:185`, with a changelog line in the comment block above it.
- **Golden discipline:** golden-hash tests (determinism / inventions / cognition families) WILL fail from Task 1 until the single regen in Task 7. Do not regen per-task; per-task verification runs only the named test targets. (Repo convention: fast checks locally, heavy suite on PR.)
- **Local gate before push (Task 7):** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo doc --no-deps` clean (CI runs rustdoc `-D warnings`), full `cargo test -p anabios-core`.
- **Staging:** always `git add <explicit paths>` — never `git add -A` or `git add .`.
- **Comment style:** match the dense doc-comment idiom of `invention/mod.rs` (constraints and rationale, not narration).

## File Structure

- `crates/anabios-core/src/invention/params.rs` — new effect-magnitude constants (Task 1).
- `crates/anabios-core/src/invention/mod.rs` — id consts, 4 table entries, new multiplier fns, extensions to existing multipliers, unit tests (Tasks 1–2).
- `crates/anabios-core/src/program/mod.rs:42` — `MEME_CHANNELS` 20→24 (Task 1).
- `crates/anabios-core/src/snapshot.rs:185` — `FORMAT_VERSION` 40 (Task 1). (`practice.rs` needs no edit: `PRACTICE_CHANNEL_BASE` is derived from `INVENTION_CHANNEL_BASE + INVENTION_COUNT`; its compile-time assert keeps holding.)
- `crates/anabios-core/src/interact.rs:170-258` — combat hooks (Task 3).
- `crates/anabios-core/src/reproduce.rs:461-467` — birth subsidy (Task 4).
- `crates/anabios-core/tests/inventions.rs` — integration tests (Tasks 3–4).
- `scenarios/weapons.toml` — probe scenario (Task 5).
- `game/scripts/building_sprites.gd` + `game/scripts/test_building_sprites.gd` — viewer key→building mappings (Task 6).
- Golden tables inside `crates/anabios-core/tests/*` — regenerated via `UPDATE_HASHES=1` (Task 7).

---

### Task 1: Table extension + layout/format bump

**Files:**
- Modify: `crates/anabios-core/src/invention/params.rs` (append constants)
- Modify: `crates/anabios-core/src/invention/mod.rs:35` (`INVENTION_COUNT`), `:46-55` (id consts), `:145-290` (table), `:375` (`candidates` doc comment), tests `:908-914` (`id_from_name...`), `:1141-1156` (`candidates_respect_prereqs`)
- Modify: `crates/anabios-core/src/program/mod.rs:42`
- Modify: `crates/anabios-core/src/snapshot.rs:177-185`

**Interfaces:**
- Produces: ids `HAFTED_SPEARS = 10`, `ARCHERY = 11`, `FORTIFICATIONS = 12`, `STEEL_ARMS = 13`; constants `SPEARS_DAMAGE`, `SPEARS_SPOILS`, `ARCHERY_RANGE`, `ARCHERY_DAMAGE`, `ARCHERY_UPKEEP`, `FORT_DEFENSE`, `FORT_BIRTH_SUBSIDY`, `FORT_SPEED_PENALTY`, `STEEL_DAMAGE`, `STEEL_SPOILS`, `STEEL_UPKEEP` (all `f32`, reachable as `crate::invention::<NAME>`). Later tasks consume these names verbatim.

- [ ] **Step 1: Extend the failing table tests first**

In `crates/anabios-core/src/invention/mod.rs` tests module, update `id_from_name_resolves_keys_case_insensitively` (add before the `wheel` line):

```rust
        assert_eq!(id_from_name("hafted_spears"), Some(HAFTED_SPEARS));
        assert_eq!(id_from_name("Steel_Arms"), Some(STEEL_ARMS));
```

Replace the body of `candidates_respect_prereqs` (Hafted Spears joins the frontier once Stone Tools is held):

```rust
    #[test]
    fn candidates_respect_prereqs() {
        let mut got = Vec::new();
        candidates(0, |k| got.push(k));
        assert_eq!(got, vec![STONE_TOOLS]);
        got.clear();
        candidates(bit(STONE_TOOLS), |k| got.push(k));
        assert_eq!(got, vec![FIRE, HAFTED_SPEARS]);
        got.clear();
        candidates(bit(STONE_TOOLS) | bit(FIRE), |k| got.push(k));
        assert_eq!(got, vec![FARMING, METALWORKING, HAFTED_SPEARS]);
        got.clear();
        // Machinery needs BOTH metalworking and writing; Archery needs Spears.
        candidates(bit(STONE_TOOLS) | bit(FIRE) | bit(METALWORKING) | bit(HAFTED_SPEARS), |k| {
            got.push(k)
        });
        assert_eq!(got, vec![FARMING, ARCHERY]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p anabios-core --lib invention -- --nocapture 2>&1 | tail -20`
Expected: compile FAILURE — `HAFTED_SPEARS` not found (the constants don't exist yet).

- [ ] **Step 3: Append the params constants**

At the end of the `--- Effect magnitudes ---` section of `crates/anabios-core/src/invention/params.rs` (after line 45, before the pollution block):

```rust
/// Hafted Spears: weapon-damage bonus; fraction of the final net damage the
/// attacker recovers as energy (hunt spoils — a transfer, never creation).
pub const SPEARS_DAMAGE: f32 = 0.25;
pub const SPEARS_SPOILS: f32 = 0.30;
/// Archery: weapon-reach multiplier bonus; weapon-damage bonus; small flat
/// per-tick upkeep (fletching and staves).
pub const ARCHERY_RANGE: f32 = 0.50;
pub const ARCHERY_DAMAGE: f32 = 0.15;
pub const ARCHERY_UPKEEP: f32 = 0.003;
/// Fortifications: incoming net-damage reduction; effective breeding-threshold
/// reduction (the birth-ledger subsidy — practices tax births, walls subsidize
/// them); locomotor speed penalty (sedentary).
pub const FORT_DEFENSE: f32 = 0.25;
pub const FORT_BIRTH_SUBSIDY: f32 = 0.15;
pub const FORT_SPEED_PENALTY: f32 = 0.10;
/// Steel Arms: weapon-damage bonus (stacks additively with Metalworking's
/// inside the same multiplier); added spoils fraction; extra module upkeep.
pub const STEEL_DAMAGE: f32 = 0.60;
pub const STEEL_SPOILS: f32 = 0.20;
pub const STEEL_UPKEEP: f32 = 0.10;
```

- [ ] **Step 4: Bump the layout and append ids + table entries**

`crates/anabios-core/src/program/mod.rs:42`: `pub const MEME_CHANNELS: usize = 20;` → `pub const MEME_CHANNELS: usize = 24;` (update the layout comment at `:37-41`: invention tree now channels 8..22, practices 22..24).

`crates/anabios-core/src/invention/mod.rs:35`: `pub const INVENTION_COUNT: usize = 10;` → `14`.

After `pub const NUCLEAR_POWER: usize = 9;` (`:55`):

```rust
// The military branch (appended 2026-09; ids are append-only, so the branch
// sits after Nuclear even though its eras are 1-3).
pub const HAFTED_SPEARS: usize = 10;
pub const ARCHERY: usize = 11;
pub const FORTIFICATIONS: usize = 12;
pub const STEEL_ARMS: usize = 13;
```

Append to the `INVENTIONS` array (after the Nuclear Power entry):

```rust
    Invention {
        name: "Hafted Spears",
        key: "hafted_spears",
        era: 1,
        prereqs: bit(STONE_TOOLS),
        // A knapped point lashed to a shaft with resin.
        materials: [0.0, 1.0, 1.0, 0.0],
        buff: "+25% weapon damage, hunt spoils",
        debuff: "none",
        // Hunting weapons pay off for lineages that hold and contest ground:
        // shares Metalworking's Territoriality slot.
        affinity: Some(GeneAffinity { slot: GenomeSlot::Territoriality, coeff: 0.8 }),
        // Era-1 entry tech stays genetically free (matches Stone Tools).
        gene_req: None,
    },
    Invention {
        name: "Archery",
        key: "archery",
        era: 2,
        prereqs: bit(HAFTED_SPEARS),
        // Bow stave + string sinew and fletching resin.
        materials: [0.0, 1.0, 2.0, 0.0],
        buff: "+50% weapon range, +15% damage",
        debuff: "small upkeep",
        // Band hunting is social coordination: wires the previously
        // tree-unused Extraversion slot into the coevolution loop.
        affinity: Some(GeneAffinity { slot: GenomeSlot::Extraversion, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::Extraversion, min: 0.40 }),
    },
    Invention {
        name: "Fortifications",
        key: "fortifications",
        era: 3,
        prereqs: bit(FARMING) | bit(HAFTED_SPEARS),
        // Rampart timber, quarry stone, and provisioning salt.
        materials: [1.0, 1.0, 0.0, 1.0],
        buff: "-25% incoming damage, easier births",
        debuff: "-10% speed",
        // Walls reward prudent planners: shares Farming's Conscientiousness
        // slot. The birth subsidy is the branch's demographic payoff — the
        // O3-measured margin lives on the birth ledger, not energy.
        affinity: Some(GeneAffinity { slot: GenomeSlot::Conscientiousness, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::Conscientiousness, min: 0.45 }),
    },
    Invention {
        name: "Steel Arms",
        key: "steel_arms",
        era: 3,
        prereqs: bit(METALWORKING) | bit(ARCHERY),
        // Ore, forge fuel, and quench media — the Metalworking line escalated.
        materials: [1.0, 2.0, 1.0, 0.0],
        buff: "+60% weapon damage, richer spoils",
        debuff: "+10% module upkeep",
        affinity: Some(GeneAffinity { slot: GenomeSlot::Territoriality, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::Territoriality, min: 0.50 }),
    },
```

Update the `candidates` doc comment (`:375`): change "Visits ids ascending (era order)." to "Visits ids ascending (ids 0-9 are era-ordered; the appended military branch sits after them)."

`crates/anabios-core/src/snapshot.rs`: `FORMAT_VERSION` 39 → 40, and add to the changelog comment block above it:

```rust
//  40: meme vector widened 20->24; military invention branch appended
//      (hafted_spears/archery/fortifications/steel_arms, ids 10-13).
```

- [ ] **Step 5: Run the invention unit tests**

Run: `cargo test -p anabios-core --lib invention`
Expected: PASS — including the pre-existing table-shape tests (basket bounds/era monotonicity: per-era basket averages become 2.33 / 3.33 / 3.8 / 5.33, still non-decreasing; gene-gate era maxima stay 0.30 / 0.40 / 0.50 / 0.65; every entry has an affinity with |coeff| < 2; prereqs reference only earlier ids). Also run `cargo test -p anabios-core --lib practice` — the derived `PRACTICE_CHANNEL_BASE` (now 22) must still pass its tests.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/invention/params.rs crates/anabios-core/src/invention/mod.rs crates/anabios-core/src/program/mod.rs crates/anabios-core/src/snapshot.rs
git commit -m "core: append the military invention branch (ids 10-13), FORMAT_VERSION 40"
```

---

### Task 2: New multipliers + extensions to existing ones

**Files:**
- Modify: `crates/anabios-core/src/invention/mod.rs` — extend `weapon_multiplier_coupled` (`:527-537`), `speed_multiplier_coupled` (`:555-565`), `module_upkeep_multiplier` (`:575-577`), `flat_upkeep`/`flat_upkeep_coupled` (`:672-691`), and their `#[cfg(test)]` oracles; add four new fns + tests.

**Interfaces:**
- Consumes: Task 1's ids and constants.
- Produces: `pub fn spoils_fraction_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32`, `pub fn defense_multiplier(mask: u32) -> f32`, `pub fn range_multiplier(mask: u32) -> f32`, `pub fn repro_threshold_multiplier(mask: u32) -> f32`. Tasks 3–4 call these exact names.

- [ ] **Step 1: Write the failing tests**

Add to the tests module of `invention/mod.rs`:

```rust
    #[test]
    fn military_multipliers_are_identity_at_mask_zero() {
        let neutral = Genome::neutral();
        assert_eq!(spoils_fraction_coupled(0, &neutral, false), 0.0);
        assert_eq!(spoils_fraction_coupled(0, &neutral, true), 0.0);
        assert_eq!(defense_multiplier(0), 1.0);
        assert_eq!(range_multiplier(0), 1.0);
        assert_eq!(repro_threshold_multiplier(0), 1.0);
        // Held values move each one the right direction.
        let spears = bit(HAFTED_SPEARS);
        assert!((spoils_fraction_coupled(spears, &neutral, false) - SPEARS_SPOILS).abs() < 1e-6);
        let both = spears | bit(STEEL_ARMS);
        assert!(
            (spoils_fraction_coupled(both, &neutral, false) - (SPEARS_SPOILS + STEEL_SPOILS)).abs()
                < 1e-6
        );
        assert!((defense_multiplier(bit(FORTIFICATIONS)) - (1.0 - FORT_DEFENSE)).abs() < 1e-6);
        assert!((range_multiplier(bit(ARCHERY)) - (1.0 + ARCHERY_RANGE)).abs() < 1e-6);
        assert!(
            (repro_threshold_multiplier(bit(FORTIFICATIONS)) - (1.0 - FORT_BIRTH_SUBSIDY)).abs()
                < 1e-6
        );
        // Spoils coupling scales with Territoriality; neutral genome = base.
        let mut territorial = Genome::neutral();
        territorial.set(GenomeSlot::Territoriality, 1.0);
        assert!(
            spoils_fraction_coupled(spears, &territorial, true)
                > spoils_fraction_coupled(spears, &neutral, true)
        );
        assert!(
            (spoils_fraction_coupled(spears, &neutral, true)
                - spoils_fraction_coupled(spears, &neutral, false))
            .abs()
                < 1e-6
        );
        // Spoils can never exceed 1.0 (transfer, not creation) for any mask.
        for mask in [both, u32::MAX & ((1u32 << INVENTION_COUNT) - 1)] {
            assert!(spoils_fraction_coupled(mask, &territorial, true) <= 1.0);
        }
    }

    #[test]
    fn military_terms_extend_the_existing_multipliers() {
        let neutral = Genome::neutral();
        // Weapon damage stacks additively: Metalworking + Spears + Archery + Steel.
        let all = bit(METALWORKING) | bit(HAFTED_SPEARS) | bit(ARCHERY) | bit(STEEL_ARMS);
        let expect = 1.0 + METALWORKING_DAMAGE + SPEARS_DAMAGE + ARCHERY_DAMAGE + STEEL_DAMAGE;
        assert!((weapon_multiplier(all) - expect).abs() < 1e-6);
        assert_eq!(weapon_multiplier_coupled(all, &neutral, false), weapon_multiplier(all));
        // Fortifications slow their holder (uncoupled debuff, like metabolism).
        let fort = bit(FORTIFICATIONS);
        assert!((speed_multiplier(fort) - (1.0 - FORT_SPEED_PENALTY)).abs() < 1e-6);
        assert!(speed_multiplier(fort) > 0.0);
        // Steel Arms upkeep stacks with Metalworking's.
        let heavy = bit(METALWORKING) | bit(STEEL_ARMS);
        assert!(
            (module_upkeep_multiplier(heavy) - (1.0 + METALWORKING_UPKEEP + STEEL_UPKEEP)).abs()
                < 1e-6
        );
        // Archery pays flat upkeep.
        assert!((flat_upkeep(bit(ARCHERY)) - ARCHERY_UPKEEP).abs() < 1e-6);
        assert_eq!(flat_upkeep_coupled(bit(ARCHERY), &neutral, false), flat_upkeep(bit(ARCHERY)));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --lib invention::tests::military -- --nocapture 2>&1 | tail -5`
Expected: compile FAILURE — `spoils_fraction_coupled` etc. not found.

- [ ] **Step 3: Implement**

In `invention/mod.rs`, extend the existing fns (each keeps its FMA shape):

`weapon_multiplier_coupled` body becomes:

```rust
    1.0 + METALWORKING_DAMAGE * coupled_held_genome(mask, METALWORKING, genome, coupling)
        + SPEARS_DAMAGE * coupled_held_genome(mask, HAFTED_SPEARS, genome, coupling)
        + ARCHERY_DAMAGE * coupled_held_genome(mask, ARCHERY, genome, coupling)
        + STEEL_DAMAGE * coupled_held_genome(mask, STEEL_ARMS, genome, coupling)
```

`weapon_multiplier` (test oracle) body becomes the same with `held_f32(mask, ..)` terms.

`speed_multiplier_coupled` gains `- FORT_SPEED_PENALTY * held_f32(mask, FORTIFICATIONS)` as its last term (uncoupled debuff, like `metabolism_multiplier` — update its doc comment to say "(Machinery buff, Fortifications penalty)"); mirror in the `speed_multiplier` oracle.

`module_upkeep_multiplier` gains `+ STEEL_UPKEEP * held_f32(mask, STEEL_ARMS)`.

`flat_upkeep` and `flat_upkeep_coupled` each gain `cost += ARCHERY_UPKEEP * held_f32(mask, ARCHERY);` (uncoupled, alongside the Writing/Medicine lines).

New fns, placed after `weapon_multiplier` with the same doc style:

```rust
/// Fraction of the FINAL net combat damage (post-armor, post-defense) the
/// attacker recovers as energy (Hafted Spears, Steel Arms) —
/// `interact::combat_pass`. A transfer bounded by what the target lost, never
/// creation; `0.0` with neither tech held. With `coupling` on, each term
/// scales with its invention's affinity gene.
#[inline]
pub fn spoils_fraction_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    (SPEARS_SPOILS * coupled_held_genome(mask, HAFTED_SPEARS, genome, coupling)
        + STEEL_SPOILS * coupled_held_genome(mask, STEEL_ARMS, genome, coupling))
    .min(1.0)
}

/// Incoming net-damage multiplier read on the TARGET side of
/// `interact::combat_pass` (Fortifications) — the tree's first defensive read.
#[inline]
pub fn defense_multiplier(mask: u32) -> f32 {
    1.0 - FORT_DEFENSE * held_f32(mask, FORTIFICATIONS)
}

/// Weapon-reach multiplier (Archery) — `interact::combat_pass` range check
/// only; sense radii and the anthro-race threat register are untouched.
#[inline]
pub fn range_multiplier(mask: u32) -> f32 {
    1.0 + ARCHERY_RANGE * held_f32(mask, ARCHERY)
}

/// Effective breeding-threshold multiplier (Fortifications) —
/// `reproduce::is_eligible`. Below 1.0 the holder breeds earlier and more
/// often: the branch's demographic payoff, symmetric with the practices'
/// birth tax. Exactly 1.0 at mask 0, so inventions-off worlds need no flag
/// plumbing. Uncoupled in v1 (one moving part for the O3 measurement).
#[inline]
pub fn repro_threshold_multiplier(mask: u32) -> f32 {
    1.0 - FORT_BIRTH_SUBSIDY * held_f32(mask, FORTIFICATIONS)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p anabios-core --lib invention`
Expected: PASS (new tests and all existing oracle/coupling tests).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/invention/mod.rs
git commit -m "core: military-branch multipliers (spoils, defense, range, birth subsidy)"
```

---

### Task 3: Combat hooks (range, defense, spoils)

**Files:**
- Modify: `crates/anabios-core/src/interact.rs:170-258` (`combat_pass`)
- Test: `crates/anabios-core/tests/inventions.rs`

**Interfaces:**
- Consumes: `invention::{held_mask, range_multiplier, defense_multiplier, spoils_fraction_coupled, weapon_multiplier_coupled}` (Tasks 1–2).
- Produces: combat behavior later tasks and scenarios observe; no new API.

- [ ] **Step 1: Write the failing integration tests**

Append to `crates/anabios-core/tests/inventions.rs`. The file already provides `comm_kit()`, `set_held`, `size_scratch`; combat needs an armed kit and a second species — bring the pattern over from `tests/combat_predation.rs` (test-local helpers; the diet is 0.5 so `enforce_ape_only` never strips the held channels mid-tick):

```rust
// --- Military branch: combat hooks ------------------------------------------

/// Armed omnivore kit (diet 0.5 keeps the holder inside the ape band so
/// enforce_ape_only never strips the seeded invention channels).
fn armed_kit(weapon_damage: f32, weapon_cost: f32, armor: f32) -> anabios_core::module::ModuleList {
    let mut m = anabios_core::module::ModuleList::new();
    m.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
    m.push(Module::Sensor {
        sensor_type: anabios_core::module::SensorType::Vision,
        radius: 0.6,
        acuity: 0.6,
    });
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.5 });
    if weapon_damage > 0.0 {
        m.push(Module::Weapon { damage: weapon_damage, energy_cost: weapon_cost });
    }
    if armor > 0.0 {
        m.push(Module::Armor { protection: armor, mass_penalty: 0.1 });
    }
    m
}

/// A program that always fires (fire_intent = 1.0 > FIRE_THRESHOLD).
fn always_fire() -> Program {
    Program::from_slice(&[Node::Const(1.0), Node::FireWeapon])
}

/// Ape-sized genome (Size >= APE_SIZE_MIN) so is_ape holds for armed kits.
fn ape_genome() -> Genome {
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Size, 0.5);
    g
}

#[test]
fn spears_add_damage_and_recover_spoils() {
    use anabios_core::prelude_test::reassign_to_new_species;
    let mut w = World::new(7);
    w.inventions_enabled = true;
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), ape_genome());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), ape_genome());
    reassign_to_new_species(&mut w, prey);
    w.agents.modules[pred as usize] = armed_kit(10.0, 2.0, 0.0);
    w.agents.modules[prey as usize] = armed_kit(0.0, 0.0, 3.0);
    w.agents.program[pred as usize] = always_fire();
    set_held(&mut w, pred, invention::HAFTED_SPEARS);
    let pred_e0 = w.agents.energy[pred as usize];
    let prey_e0 = w.agents.energy[prey as usize];
    step(&mut w);
    // net = damage*(1+SPEARS_DAMAGE) - armor = 10*1.25 - 3 = 9.5.
    let net = 10.0 * (1.0 + invention::SPEARS_DAMAGE) - 3.0;
    assert!(w.agents.energy[prey as usize] <= prey_e0 - net + 1e-3);
    // Spoils: attacker recovered SPEARS_SPOILS * net = 2.85, net of the 2.0
    // weapon cost and metabolism the tick cost strictly less than the
    // no-spoils baseline below.
    let mut w2 = World::new(7);
    w2.inventions_enabled = true;
    let pred2 = w2.spawn_agent(Vec2::new(500.0, 500.0), ape_genome());
    let prey2 = w2.spawn_agent(Vec2::new(501.0, 500.0), ape_genome());
    reassign_to_new_species(&mut w2, prey2);
    w2.agents.modules[pred2 as usize] = armed_kit(10.0, 2.0, 0.0);
    w2.agents.modules[prey2 as usize] = armed_kit(0.0, 0.0, 3.0);
    w2.agents.program[pred2 as usize] = always_fire();
    // No spears in w2: same seed, same layout -> spoils are the only delta
    // on the attacker beyond the (larger) damage they dealt.
    step(&mut w2);
    let gain = w.agents.energy[pred as usize] - pred_e0;
    let gain_baseline = w2.agents.energy[pred2 as usize] - pred_e0;
    assert!(
        gain > gain_baseline + invention::SPEARS_SPOILS * net - 1.0,
        "spoils must lift the attacker's energy over the no-spears baseline"
    );
}

#[test]
fn spoils_never_exceed_target_loss() {
    use anabios_core::prelude_test::reassign_to_new_species;
    let mut w = World::new(11);
    w.inventions_enabled = true;
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), ape_genome());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), ape_genome());
    reassign_to_new_species(&mut w, prey);
    w.agents.modules[pred as usize] = armed_kit(16.0, 0.0, 0.0);
    w.agents.modules[prey as usize] = armed_kit(0.0, 0.0, 0.0);
    w.agents.program[pred as usize] = always_fire();
    set_held(&mut w, pred, invention::HAFTED_SPEARS);
    set_held(&mut w, pred, invention::STEEL_ARMS);
    let pred_e0 = w.agents.energy[pred as usize];
    let prey_e0 = w.agents.energy[prey as usize];
    step(&mut w);
    let target_loss = prey_e0 - w.agents.energy[prey as usize];
    let attacker_gain = w.agents.energy[pred as usize] - pred_e0;
    assert!(attacker_gain <= target_loss + 1e-3, "spoils are a transfer, not creation");
}

#[test]
fn fortifications_blunt_incoming_damage() {
    use anabios_core::prelude_test::reassign_to_new_species;
    let mut w = World::new(13);
    w.inventions_enabled = true;
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), ape_genome());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), ape_genome());
    reassign_to_new_species(&mut w, prey);
    w.agents.modules[pred as usize] = armed_kit(10.0, 2.0, 0.0);
    w.agents.modules[prey as usize] = armed_kit(0.0, 0.0, 3.0);
    w.agents.program[pred as usize] = always_fire();
    // Fortifications need a Communicator to be held legitimately, but the
    // combat read only consults the meme channel; seed it directly. The prey
    // is ape-band (omnivore + large) so enforce_ape_only keeps it.
    set_held(&mut w, prey, invention::FORTIFICATIONS);
    let prey_e0 = w.agents.energy[prey as usize];
    step(&mut w);
    // net = (10 - 3) * (1 - FORT_DEFENSE) = 5.25 (< the unfortified 7.0).
    let expected = (10.0 - 3.0) * (1.0 - invention::FORT_DEFENSE);
    let loss = prey_e0 - w.agents.energy[prey as usize];
    assert!(loss >= expected - 1e-3, "combat still lands");
    assert!(loss < 7.0, "fortifications must reduce the unfortified 7.0 net");
}

#[test]
fn archery_extends_weapon_reach() {
    use anabios_core::prelude_test::reassign_to_new_species;
    // 2.5 apart: outside the Weapon module's 2.0 reach, inside 2.0 * 1.5.
    let mut w = World::new(17);
    w.inventions_enabled = true;
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), ape_genome());
    let prey = w.spawn_agent(Vec2::new(502.5, 500.0), ape_genome());
    reassign_to_new_species(&mut w, prey);
    w.agents.modules[pred as usize] = armed_kit(10.0, 2.0, 0.0);
    w.agents.modules[prey as usize] = armed_kit(0.0, 0.0, 0.0);
    w.agents.program[pred as usize] = always_fire();
    step(&mut w);
    assert!(!w.combat_damaged[prey as usize], "out of bare-module range");

    let mut w2 = World::new(17);
    w2.inventions_enabled = true;
    let pred2 = w2.spawn_agent(Vec2::new(500.0, 500.0), ape_genome());
    let prey2 = w2.spawn_agent(Vec2::new(502.5, 500.0), ape_genome());
    reassign_to_new_species(&mut w2, prey2);
    w2.agents.modules[pred2 as usize] = armed_kit(10.0, 2.0, 0.0);
    w2.agents.modules[prey2 as usize] = armed_kit(0.0, 0.0, 0.0);
    w2.agents.program[pred2 as usize] = always_fire();
    set_held(&mut w2, pred2, invention::ARCHERY);
    step(&mut w2);
    assert!(w2.combat_damaged[prey2 as usize], "archery reach covers 2.5");
}
```

Note the imports this block needs at the top of the file (extend the existing `use` lines): `Module` is already imported; add `SensorType` usage via the fully qualified path shown, and `Program`/`Node`/`step` are already imported.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --test inventions spears_add -- --nocapture 2>&1 | tail -10`
Expected: FAIL — spears add no damage yet (loss assertion under 12.5-net fails) or spoils assertion fails. (`archery_extends_weapon_reach` and `fortifications_blunt_incoming_damage` fail on their behavior assertions.)

- [ ] **Step 3: Implement the combat hooks**

In `combat_pass` (`crates/anabios-core/src/interact.rs`), hoist the attacker's mask and add the three reads. The block from `:183` becomes:

```rust
        // Archery buff: reach extends past the bare module range (identity
        // multiplier when unheld, so pre-branch behavior is bit-identical).
        let mask_i = crate::invention::held_mask(&world.agents.meme_vector[i]);
        if world.sensors[i].nearest_other_dist
            >= weapon.range * crate::invention::range_multiplier(mask_i)
        {
            continue;
        }
```

and the damage block (`:195-212`) becomes:

```rust
        // Metalworking / military-branch buffs: better weapons deal more damage.
        let inv_weapon_mult = crate::invention::weapon_multiplier_coupled(
            mask_i,
            &world.agents.genome[i],
            world.gene_tech_coupling,
        );
        // E12: males of dimorphic species hit harder (identity when the flag
        // is off).
        let dimorph_mult = crate::dimorphism::damage_factor(
            &world.agents.genome[i],
            world.agents.sex.get(i).map(|b| *b).unwrap_or(false),
            world.sexual_dimorphism_enabled,
        );
        let damage = weapon.damage * inv_weapon_mult * dimorph_mult;
        let armor = module::effective_armor_protection(&world.agents.modules[t]);
        // Fortifications blunt the blow on the TARGET side (×1.0 when unheld).
        let mask_t = crate::invention::held_mask(&world.agents.meme_vector[t]);
        let net = (damage - armor).max(0.0) * crate::invention::defense_multiplier(mask_t);
        world.agents.energy[t] -= net;
        // Hunt spoils: the attacker recovers a fraction of the FINAL net (what
        // the target actually lost — a transfer, never creation). Guarded so
        // spoils-free combat never adds +0.0 to energy (-0.0 + 0.0 flips the
        // sign bit and would break flag-off byte-identity).
        let spoils = crate::invention::spoils_fraction_coupled(
            mask_i,
            &world.agents.genome[i],
            world.gene_tech_coupling,
        );
        if spoils > 0.0 {
            world.agents.energy[i] += spoils * net;
        }
        world.agents.energy[i] -= weapon.energy_cost;
```

(The old inline `held_mask` call inside the `weapon_multiplier_coupled` argument list at `:197` is replaced by `mask_i`.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p anabios-core --test inventions` and `cargo test -p anabios-core --test combat_predation`
Expected: PASS — all four new tests plus every pre-existing combat/invention integration test (goldens inside `tests/inventions.rs`, if hit, are Task 7's regen; only behavior tests must pass here — if a golden test in this file fails, note it and move on).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/tests/inventions.rs
git commit -m "core: combat hooks for the military branch (range, defense, spoils)"
```

---

### Task 4: Birth subsidy hook

**Files:**
- Modify: `crates/anabios-core/src/reproduce.rs:461-467` (`is_eligible`)
- Test: `crates/anabios-core/tests/inventions.rs`

**Interfaces:**
- Consumes: `invention::{held_mask, repro_threshold_multiplier}` (Tasks 1–2).

- [ ] **Step 1: Write the failing test**

Append to `crates/anabios-core/tests/inventions.rs`:

```rust
// --- Military branch: birth-ledger subsidy ----------------------------------

#[test]
fn fortifications_lower_the_breeding_threshold() {
    use anabios_core::reproduce::{REPRO_ENERGY_MULT, SPAWN_ENERGY};
    // Base threshold for a neutral genome (ReproductionThreshold = 0.5,
    // neutral personality/affect factors are exactly 1.0).
    let base = SPAWN_ENERGY * 0.5 * REPRO_ENERGY_MULT;
    let subsidized = base * (1.0 - invention::FORT_BIRTH_SUBSIDY);
    // Energy between the two thresholds: eligible ONLY with Fortifications.
    let energy = (base + subsidized) / 2.0;

    let mut count_births = |hold_fort: bool| -> usize {
        let mut w = World::new(23);
        w.inventions_enabled = true;
        let mut ids = Vec::new();
        for n in 0..2 {
            let id = w.spawn_agent(Vec2::new(500.0 + n as f32, 500.0), ape_genome());
            let mut m = comm_kit();
            m.push(Module::Reproductive { fecundity: 1.0 });
            w.agents.modules[id as usize] = m;
            w.agents.energy[id as usize] = energy;
            if hold_fort {
                set_held(&mut w, id, invention::FORTIFICATIONS);
            }
            ids.push(id);
        }
        let pop0 = w.agents.iter_alive().count();
        for _ in 0..5 {
            step(&mut w);
        }
        w.agents.iter_alive().count().saturating_sub(pop0)
    };

    assert_eq!(count_births(false), 0, "below the unsubsidized threshold: no births");
    assert!(count_births(true) > 0, "the subsidy makes the same energy eligible");
}
```

If `SPAWN_ENERGY`/`REPRO_ENERGY_MULT` or `Module::Reproductive`'s field name differ from the above, read `crates/anabios-core/src/reproduce.rs:1-50` and `crates/anabios-core/src/module/mod.rs` and use the real names — the test's structure (energy pinned between the two thresholds, births counted with/without the held channel) is the contract. Metabolism drains energy during the 5 ticks, so if the no-fort case ever births, widen the gap by using `base * 0.99` as the energy instead.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --test inventions fortifications_lower -- --nocapture 2>&1 | tail -5`
Expected: FAIL — `count_births(true)` is 0 (no subsidy exists yet).

- [ ] **Step 3: Implement**

In `crates/anabios-core/src/reproduce.rs:461-467`, extend the threshold product and the comment above it:

```rust
    // Conscientiousness raises the effective breeding threshold;
    // Fortifications lower it (the invention tree's birth-ledger subsidy —
    // exactly ×1.0 when unheld, so inventions-off worlds are byte-identical).
    let threshold = SPAWN_ENERGY
        * agents.genome[i].get(GenomeSlot::ReproductionThreshold)
        * REPRO_ENERGY_MULT
        * crate::personality::personality_reproduction_factor(&agents.genome[i])
        * crate::affect::affect_reproduction_factor(&agents.affect[i])
        * crate::invention::repro_threshold_multiplier(crate::invention::held_mask(
            &agents.meme_vector[i],
        ));
    agents.energy[i] >= threshold
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p anabios-core --test inventions fortifications_lower` and `cargo test -p anabios-core --test save_load_roundtrip`
Expected: PASS (round-trip exercises the widened meme vector at FORMAT_VERSION 40).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/reproduce.rs crates/anabios-core/tests/inventions.rs
git commit -m "core: Fortifications birth-ledger subsidy in is_eligible"
```

---

### Task 5: Probe scenario

**Files:**
- Create: `scenarios/weapons.toml`

**Interfaces:**
- Consumes: invention keys `hafted_spears`/`archery` (`starting_inventions` resolves via `invention::id_from_name`, which Task 1's test covers).

- [ ] **Step 1: Write the scenario**

Create `scenarios/weapons.toml` (modeled on `scenarios/inventions.toml`; the armed lineage starts holding the entry weapons so the branch's combat + birth payoffs are observable without waiting on discovery):

```toml
name = "weapons"
seed = 0
inventions_enabled = true
# Keep the world below saturation so the armed-vs-plain contest — not
# Malthusian collapse — dominates the dynamics.
max_population = 400

# Military-branch probe: an armed culture-bearing lineage (seeded with the
# spear line) shares a range with an unarmed cultural lineage and an acultural
# control. Watch whether the armed lineage's spoils + (once Fortifications
# arrive) birth subsidy convert tech-holding into a demographic edge — the O3
# "make inventions pay" question in scenario form.

# Armed innovators: seeded with the entry weapons line.
[[agents]]
count = 24
archetype = "innovator"
starting_inventions = ["stone_tools", "hafted_spears"]
placement = { kind = "cluster", center_x = 300.0, center_y = 512.0, radius = 80.0 }
[agents.traits]
altruism = 0.3
basal_metabolism = 0.6
lifespan_bias = 1.0

# Unarmed cultural lineage: same temperament, no head start.
[[agents]]
count = 24
archetype = "traditionalist"
placement = { kind = "cluster", center_x = 724.0, center_y = 512.0, radius = 80.0 }
[agents.traits]
altruism = 0.3
basal_metabolism = 0.6
lifespan_bias = 1.0

# Acultural control: no Communicator, era 0 forever.
[[agents]]
count = 16
archetype = "asocial_forager"
placement = { kind = "cluster", center_x = 512.0, center_y = 280.0, radius = 60.0 }
[agents.traits]
altruism = 0.0
basal_metabolism = 0.6
lifespan_bias = 1.0
```

- [ ] **Step 2: Verify it parses and runs**

Run: `cargo test -p anabios-core --test all_scenarios 2>&1 | tail -5`
Expected: PASS — `all_scenarios` sweeps every `scenarios/*.toml`, so the new file is picked up automatically; a parse error (unknown invention key, missing field) fails here. If `all_scenarios` also asserts a pinned scenario count, update that count.

- [ ] **Step 3: Commit**

```bash
git add scenarios/weapons.toml
git commit -m "scenarios: weapons.toml military-branch probe"
```

---

### Task 6: Viewer key→building mappings

**Files:**
- Modify: `game/scripts/building_sprites.gd` (the `INVENTION_BUILDING` dict, ~line 43)
- Modify: `game/scripts/test_building_sprites.gd` (its key list, ~line 47)

**Interfaces:**
- Consumes: the four new invention keys (Task 1).

- [ ] **Step 1: Add the mappings**

In `game/scripts/building_sprites.gd`, extend `INVENTION_BUILDING` (new keys map to the nearest existing sprite kind — distinct pixel art is a deliberate follow-up, and `building_for_invention` already returns -1 gracefully for unknown keys, so this is a display upgrade, not a crash fix):

```gdscript
const INVENTION_BUILDING := {
	"stone_tools": STONE_TOOLS,
	"fire": FIRE,
	"farming": FARMING,
	"metalworking": METALWORKING,
	"writing": WRITING,
	"medicine": MEDICINE,
	"husbandry": HUSBANDRY,
	"machinery": MACHINERY,
	"electricity": ELECTRICITY,
	"nuclear_power": NUCLEAR,
	# Military branch (2026-09): mapped to the nearest existing sprite kind
	# until the branch gets its own pixel art.
	"hafted_spears": STONE_TOOLS,
	"archery": STONE_TOOLS,
	"fortifications": FARMING,
	"steel_arms": METALWORKING,
}
```

- [ ] **Step 2: Extend the sprite test's key coverage**

In `game/scripts/test_building_sprites.gd`, find the list of keys iterated near line 47 (`var kind: int = B.building_for_invention(key)`) and add the four new keys to it, so every catalog key is asserted to resolve to a valid kind.

- [ ] **Step 3: Run the sprite test the way CI does**

Find the exact invocation: `grep -rn "test_building_sprites" .github/workflows/ game/` and run that command (repo convention: headless Godot script tests; per `verifying-godot-headless`, check for `SCRIPT ERROR` lines in the output, not just the exit code).
Expected: PASS, no `SCRIPT ERROR`/parse-error lines.

- [ ] **Step 4: Lint and commit**

Run: `gdformat game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd && gdlint game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd`
(Do NOT run `gdlint --dump-default-config` — it writes a stray `gdlintrc` that shadows `.gdlintrc`.)

```bash
git add game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
git commit -m "viewer: map military-branch invention keys to building sprites"
```

---

### Task 7: Golden regen + full local gate

**Files:**
- Modify: whichever test sources hold golden tables (rewritten by the regen run — expect `crates/anabios-core/tests/determinism.rs`, `tests/inventions.rs`, `tests/cognition.rs`; `tests/common/mod.rs::assert_golden` rewrites its `src` file when `UPDATE_HASHES` is set)

- [ ] **Step 1: Regenerate every golden**

Run: `UPDATE_HASHES=1 cargo test -p anabios-core 2>&1 | tail -20`
Expected: the run completes; `assert_golden` sites rewrite their tables in place (the mechanism prints/writes the observed values — see `tests/common/mod.rs:58-97`). Every golden legitimately changes: the meme vector widened (FORMAT_VERSION 40) and discovery probability tables now include the new candidates.

- [ ] **Step 2: Clean verification run**

Run: `cargo test -p anabios-core 2>&1 | tail -5`
Expected: PASS, zero failures — including determinism, round-trip for all opt-in flags, and every scenario sweep.

- [ ] **Step 3: The CI-matching local gate**

Run, each expecting clean output:
```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --no-deps 2>&1 | grep -i warning; test $? -eq 1
```

- [ ] **Step 4: Commit the goldens**

```bash
git add crates/anabios-core/tests/
git commit -m "tests: regenerate goldens for the military branch (FORMAT_VERSION 40)"
```

- [ ] **Step 5: Verify flag-off byte-identity explicitly**

Run: `cargo test -p anabios-core --test determinism 2>&1 | tail -5` and `cargo test -p anabios-core --test inventions flag_off -- --nocapture 2>&1 | tail -5`
Expected: PASS — `flag_off_never_discovers_and_consumes_no_invention_rng` proves the inert path is a strict no-op at the new layout.

---

## Follow-ups (out of this plan, recorded for the roadmap)

- **O3 measurement sweep**: run the pre-registered contest (culture-dominant AND era-active, n≈10 seeds) with the branch live — its own mechanism-cycle writeup.
- **Distinct pixel art** for the four military buildings (viewer polish).
- **Coevolution panel series** (`inv_hafted_spears_frac` etc.) if the sweep wants them plotted.
