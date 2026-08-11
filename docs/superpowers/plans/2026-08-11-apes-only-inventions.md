# Apes-only Inventions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restrict the cultural invention tree so only apes (the viewer's PRIMATE archetype) can discover, copy, or inherit it, while maladaptive practices stay open to every animal.

**Architecture:** Add one canonical `is_ape` predicate in the core (mirroring the viewer's omnivore+large PRIMATE band) plus an `enforce_ape_only` meme-stripping helper. Gate the three invention transmission points (discovery, social copy, inheritance) on ape-ness — all inside the existing `inventions_enabled` guards. Reclass the culture cohort (`innovator`/`traditionalist`/`cultural_forager`) to omnivores so they actually qualify as apes and render as primates. Regenerate the goldens that legitimately move.

**Tech Stack:** Rust (`anabios-core`), GDScript (Godot viewer), TOML scenarios.

## Global Constraints

- Every gate MUST live inside a block already guarded by `world.inventions_enabled`; a flag-off scenario's golden hash MUST be byte-identical to before. (Verify: `minimal.toml` / pure-affect goldens do not move.)
- Practices (`practice.rs` channels: Inbreeding, Child Sacrifice) MUST remain acquirable and spreadable by any animal — never gate a practice channel on ape-ness.
- `is_ape` MUST match the viewer's PRIMATE band exactly: omnivore `0.34 ≤ diet < 0.66` (`effective_diet_carnivory`) AND `genome.Size ≥ 0.30` (== viewer `SIZE_SPLIT` 1.25 world-units, from `size = 0.5 + 2.5×Size`).
- Golden regen is deliberate only: run under `UPDATE_HASHES=1`, copy printed tuples, annotate each const with a dated "Refreshed 2026-08-11 (apes-only inventions)" comment in the existing style.
- Never `git add -A`/`.`; stage explicit paths.
- Follow anabios CI gate locally before pushing: `cargo fmt --check` on the committed tree and rustdoc `-D warnings`.
- Do NOT run the full determinism/golden suite on every commit; fast checks per task, full suite only in Task 7.

---

### Task 1: `is_ape` + `enforce_ape_only` core helpers

**Files:**
- Modify: `crates/anabios-core/src/invention/mod.rs` (add consts + two fns near the other pure helpers, e.g. after `is_invention_channel` ~line 302; add tests in the existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::genome::{Genome, GenomeSlot}` and `crate::module::{self, ModuleList}` (both already imported at the top of the file: `use crate::genome::{Genome, GenomeSlot};` and `use crate::module::{self, ModuleType};`), `crate::program::MEME_CHANNELS`, `channel()`, `INVENTION_COUNT`.
- Produces:
  - `pub const APE_SIZE_MIN: f32 = 0.30;`
  - `pub const APE_DIET_LO: f32 = 0.34;`
  - `pub const APE_DIET_HI: f32 = 0.66;`
  - `pub fn is_ape(genome: &Genome, modules: &ModuleList) -> bool`
  - `pub fn enforce_ape_only(meme: &mut [f32; MEME_CHANNELS], genome: &Genome, modules: &ModuleList, inventions_enabled: bool)`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `invention/mod.rs`:

```rust
#[cfg(test)]
fn ape_modules(diet: f32) -> crate::module::ModuleList {
    let mut m = crate::module::ModuleList::new();
    m.push(crate::module::Module::Mouth { bite_size: 0.6, diet_affinity: diet });
    m
}

#[test]
fn is_ape_matches_viewer_primate_band() {
    use crate::genome::{Genome, GenomeSlot};
    let mut large = Genome::neutral();
    large.set(GenomeSlot::Size, 0.5); // -> world 1.75, large
    let mut small = Genome::neutral();
    small.set(GenomeSlot::Size, 0.25); // -> world 1.125, below SIZE_SPLIT

    // omnivore + large = ape
    assert!(is_ape(&large, &ape_modules(0.5)));
    // omnivore boundary: 0.34 in-band, 0.66 out (half-open, matches CARN_MIN)
    assert!(is_ape(&large, &ape_modules(APE_DIET_LO)));
    assert!(!is_ape(&large, &ape_modules(APE_DIET_HI)));
    // herbivore or carnivore large = not ape (Deer / Wolf)
    assert!(!is_ape(&large, &ape_modules(0.0)));
    assert!(!is_ape(&large, &ape_modules(0.9)));
    // omnivore but small = not ape (Boar)
    assert!(!is_ape(&small, &ape_modules(0.5)));
    // size boundary: exactly APE_SIZE_MIN is large
    let mut edge = Genome::neutral();
    edge.set(GenomeSlot::Size, APE_SIZE_MIN);
    assert!(is_ape(&edge, &ape_modules(0.5)));
}

#[test]
fn enforce_ape_only_strips_inventions_from_non_apes() {
    use crate::genome::{Genome, GenomeSlot};
    let mut meme = [0.0f32; crate::program::MEME_CHANNELS];
    meme[channel(STONE_TOOLS)] = 1.0;
    meme[channel(FIRE)] = 0.7;
    // practice channel stand-in: a non-invention channel must be preserved
    let practice_ch = crate::practice::channel(crate::practice::CHILD_SACRIFICE);
    meme[practice_ch] = 1.0;

    let mut g = Genome::neutral();
    g.set(GenomeSlot::Size, 0.5);
    let herb = ape_modules(0.0); // non-ape

    // flag off: no-op even for a non-ape
    let mut off = meme;
    enforce_ape_only(&mut off, &g, &herb, false);
    assert_eq!(off, meme);

    // flag on, non-ape: invention channels zeroed, practice untouched
    let mut on = meme;
    enforce_ape_only(&mut on, &g, &herb, true);
    assert_eq!(on[channel(STONE_TOOLS)], 0.0);
    assert_eq!(on[channel(FIRE)], 0.0);
    assert_eq!(on[practice_ch], 1.0);

    // flag on, ape: unchanged
    let mut ape = meme;
    enforce_ape_only(&mut ape, &g, &ape_modules(0.5), true);
    assert_eq!(ape, meme);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd crates/anabios-core && cargo test --lib is_ape_matches_viewer_primate_band enforce_ape_only_strips_inventions_from_non_apes`
Expected: FAIL — `cannot find function is_ape` / `enforce_ape_only`.

- [ ] **Step 3: Implement the helpers**

Add near the other channel helpers in `invention/mod.rs` (e.g. after `is_invention_channel`):

```rust
/// Minimum genome `Size` for the viewer's "large" bucket. Equals the viewer's
/// `SIZE_SPLIT` (1.25 world-units) under `size = 0.5 + 2.5 × Size`.
pub const APE_SIZE_MIN: f32 = 0.30;
/// Omnivore band lower bound (inclusive) — the viewer's `HERB_MAX`.
pub const APE_DIET_LO: f32 = 0.34;
/// Omnivore band upper bound (exclusive) — the viewer's `CARN_MIN`.
pub const APE_DIET_HI: f32 = 0.66;

/// True when the agent is the viewer's PRIMATE archetype (omnivore + large):
/// the only archetype permitted to acquire cultural inventions. Practices are
/// unaffected — any animal can hold and spread those.
pub fn is_ape(genome: &Genome, modules: &ModuleList) -> bool {
    let diet = module::effective_diet_carnivory(modules);
    genome.get(GenomeSlot::Size) >= APE_SIZE_MIN && diet >= APE_DIET_LO && diet < APE_DIET_HI
}

/// Zero every invention channel of `meme` when the agent is not an ape (no-op
/// when `inventions_enabled` is false, so flag-off scenarios are byte-identical).
/// Practice and base channels are never touched. Consumes no RNG.
pub fn enforce_ape_only(
    meme: &mut [f32; MEME_CHANNELS],
    genome: &Genome,
    modules: &ModuleList,
    inventions_enabled: bool,
) {
    if !inventions_enabled || is_ape(genome, modules) {
        return;
    }
    for k in 0..INVENTION_COUNT {
        meme[channel(k)] = 0.0;
    }
}
```

Also add `use crate::module::ModuleList;` if `ModuleList` is not already in scope via the existing `use crate::module::{self, ModuleType};` — reference it as `module::ModuleList` in the signatures if a bare import would conflict.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd crates/anabios-core && cargo test --lib is_ape_matches_viewer_primate_band enforce_ape_only_strips_inventions_from_non_apes`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/invention/mod.rs
git commit -m "feat(invention): is_ape predicate + enforce_ape_only meme strip"
```

---

### Task 2: Reclass the culture cohort to apes

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs` — `archetype_kit` (`innovator`/`traditionalist` arm ~line 489, `cultural_forager` arm ~line 479)
- Test: add to the `#[cfg(test)] mod tests` in `scenario.rs`

**Interfaces:**
- Consumes: `invention::is_ape` (Task 1), `archetype_kit(name) -> (ModuleList, Program)`.
- Produces: the three reclassed kits (no new public symbols; behavioral change only).

- [ ] **Step 1: Write the failing test**

Add to `scenario.rs` tests:

```rust
#[test]
fn culture_cohort_archetypes_are_apes() {
    use crate::genome::Genome;
    // Default archetype genome (Size 0.5 = large); diet comes from the kit's Mouth.
    for name in ["innovator", "traditionalist", "cultural_forager"] {
        let (modules, _prog) = archetype_kit(name);
        let mut g = Genome::neutral();
        archetype_genome(name, &mut g);
        assert!(
            crate::invention::is_ape(&g, &modules),
            "{name} must be an ape (omnivore + large) so it can carry inventions"
        );
    }
    // Control: the asocial forager stays a non-ape herbivore.
    let (modules, _) = archetype_kit("asocial_forager");
    let mut g = Genome::neutral();
    archetype_genome("asocial_forager", &mut g);
    assert!(!crate::invention::is_ape(&g, &modules), "asocial_forager stays non-ape");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/anabios-core && cargo test --lib culture_cohort_archetypes_are_apes`
Expected: FAIL — the three kits are herbivores (`diet_affinity 0.0`), `is_ape` is false.

- [ ] **Step 3: Reclass the kits**

In `archetype_kit`, give each cohort kit an omnivore Mouth. Replace the `cultural_forager` arm:

```rust
"cultural_forager" => {
    let mut m = starter_kit();
    make_omnivore(&mut m); // reclass to ape (primate) diet
    m.push(crate::module::Module::Communicator { range: 12.0, channel_id: 0 });
    (m, starter_asocial_forager())
}
```

And the `innovator | traditionalist` arm:

```rust
"innovator" | "traditionalist" => {
    let mut m = starter_kit();
    make_omnivore(&mut m); // reclass to ape (primate) diet
    m.push(crate::module::Module::Communicator { range: 12.0, channel_id: 0 });
    (m, starter_grazer())
}
```

Add this file-private helper next to `archetype_kit`:

```rust
/// Retune the kit's Mouth to the primate omnivore band so the lineage renders
/// as (and counts as) an ape — the only archetype allowed to carry inventions.
fn make_omnivore(modules: &mut crate::module::ModuleList) {
    for m in modules.iter_mut() {
        if let crate::module::Module::Mouth { diet_affinity, .. } = m {
            *diet_affinity = 0.5;
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/anabios-core && cargo test --lib culture_cohort_archetypes_are_apes`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/scenario.rs
git commit -m "feat(scenario): reclass innovator/traditionalist/cultural_forager as apes (omnivore)"
```

---

### Task 3: Gate discovery on ape-ness

**Files:**
- Modify: `crates/anabios-core/src/invention/mod.rs` — `invention_step`, innovation guard at line 688
- Modify: `crates/anabios-core/tests/inventions.rs` — `comm_kit()` helper (line ~26) to omnivore
- Test: add to `crates/anabios-core/tests/inventions.rs`

**Interfaces:**
- Consumes: `invention::is_ape` (Task 1); `world.agents.genome[i]`, `world.agents.modules[i]`.
- Produces: gated `invention_step` — a non-ape Communicator never rolls a discovery.

- [ ] **Step 1: Make existing invention-test discoverers apes**

In `crates/anabios-core/tests/inventions.rs`, change `comm_kit()`'s Mouth so its agents are apes (they use `Genome::neutral()`, Size 0.5 = large):

```rust
m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.5 }); // omnivore -> ape
```

- [ ] **Step 2: Write the failing test**

Add to `crates/anabios-core/tests/inventions.rs` (mirrors the existing `communicators_eventually_discover_stone_tools`, but with a non-ape herbivore Communicator that must never discover):

```rust
/// A non-ape (herbivore) Communicator never discovers, even over many ticks —
/// inventions are apes-only. An otherwise-identical ape does (covered by
/// `communicators_eventually_discover_stone_tools`).
#[test]
fn non_ape_communicator_never_discovers() {
    let mut w = World::new(13);
    w.inventions_enabled = true;
    size_scratch(&mut w);
    for n in 0..12u32 {
        let id = w.spawn_agent(Vec2::new(500.0 + n as f32 * 3.0, 500.0), Genome::neutral());
        let mut kit = comm_kit();
        // Force herbivore diet -> non-ape, overriding comm_kit's omnivore Mouth.
        for m in kit.iter_mut() {
            if let Module::Mouth { diet_affinity, .. } = m {
                *diet_affinity = 0.0;
            }
        }
        w.agents.modules[id as usize] = kit;
        w.agents.meme_vector[id as usize][SKILL_CHANNEL] = 1.0;
    }
    for _ in 0..3000 {
        invention::invention_step(&mut w);
    }
    let discovered = w
        .agents
        .iter_alive()
        .any(|id| invention::held_mask(&w.agents.meme_vector[id as usize]) != 0);
    assert!(!discovered, "a non-ape must never discover an invention");
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/anabios-core && cargo test --test inventions non_ape_communicator_never_discovers`
Expected: FAIL — an ungated non-ape Communicator discovers Stone Tools within 3000 ticks.

- [ ] **Step 4: Add the discovery gate**

In `invention_step`, change the innovation guard (line 688) from:

```rust
        if module::has(&world.agents.modules[i], ModuleType::Communicator) {
```

to:

```rust
        if module::has(&world.agents.modules[i], ModuleType::Communicator)
            && is_ape(&world.agents.genome[i], &world.agents.modules[i])
        {
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd crates/anabios-core && cargo test --test inventions non_ape_communicator_never_discovers communicators_eventually_discover_stone_tools`
Expected: PASS (both — the ape still discovers, the non-ape never does).

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/invention/mod.rs crates/anabios-core/tests/inventions.rs
git commit -m "feat(invention): gate discovery on ape archetype; ape-ify invention test kit"
```

---

### Task 4: Gate social copying on ape-ness (practices stay open)

**Files:**
- Modify: `crates/anabios-core/src/culture.rs` — invention-copy block guard at line 361
- Test: add to `crates/anabios-core/tests/inventions.rs`

**Interfaces:**
- Consumes: `invention::is_ape` (Task 1); `world.agents.genome[i]`, `world.agents.modules[i]`.
- Produces: invention channels no longer copy toward a non-ape receiver; practice channels still do.

- [ ] **Step 1: Write the failing test**

Add to `crates/anabios-core/tests/inventions.rs`:

```rust
/// A non-ape receiver next to an ape holding Stone Tools does NOT copy the
/// invention, but DOES still copy a maladaptive practice from a practice-holding
/// neighbour. Practices are open to every animal.
#[test]
fn non_ape_copies_practices_but_not_inventions() {
    use anabios_core::practice;
    let mut w = World::new(23);
    w.inventions_enabled = true;
    w.cognition_enabled = true; // practice spread is cognition-gated
    size_scratch(&mut w);

    // Ape teacher holding Stone Tools + Child Sacrifice.
    let teacher = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    w.agents.modules[teacher as usize] = comm_kit(); // omnivore -> ape
    set_held(&mut w, teacher, invention::STONE_TOOLS);
    w.agents.meme_vector[teacher as usize][practice::channel(practice::CHILD_SACRIFICE)] = 1.0;

    // Non-ape (herbivore) Communicator receiver right next to the teacher.
    let learner = w.spawn_agent(Vec2::new(500.5, 500.0), Genome::neutral());
    let mut kit = comm_kit();
    for m in kit.iter_mut() {
        if let Module::Mouth { diet_affinity, .. } = m {
            *diet_affinity = 0.0; // herbivore -> non-ape
        }
    }
    w.agents.modules[learner as usize] = kit;

    w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
    for _ in 0..50 {
        anabios_core::culture::culture_step(&mut w);
    }

    assert_eq!(
        level_of(&w, learner, invention::STONE_TOOLS),
        0.0,
        "non-ape must not copy an invention"
    );
    assert!(
        w.agents.meme_vector[learner as usize][practice::channel(practice::CHILD_SACRIFICE)] > 0.0,
        "non-ape must still copy a maladaptive practice"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/anabios-core && cargo test --test inventions non_ape_copies_practices_but_not_inventions`
Expected: FAIL — without the gate the non-ape copies Stone Tools (assert on `== 0.0` fails).

- [ ] **Step 3: Add the copy gate**

In `culture_step`, change the invention-copy guard (line 361) from:

```rust
        if world.inventions_enabled {
```

to:

```rust
        if world.inventions_enabled
            && crate::invention::is_ape(&world.agents.genome[i], &world.agents.modules[i])
        {
```

Leave the separate practice-copy block (the `if world.cognition_enabled { … }` at line 432+) untouched.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd crates/anabios-core && cargo test --test inventions non_ape_copies_practices_but_not_inventions spread_copies_toward_holder_neighbour_and_respects_prereqs`
Expected: PASS (both — the ape-to-ape spread test still passes; the non-ape copies only the practice).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/culture.rs crates/anabios-core/tests/inventions.rs
git commit -m "feat(culture): gate invention copy on ape archetype; practices stay open"
```

---

### Task 5: Gate vertical inheritance (post-pass, zero RNG impact)

**Files:**
- Modify: `crates/anabios-core/src/reproduce.rs` — `inherit_child_meme` (line ~211)
- Test: add to the `#[cfg(test)] mod tests` in `reproduce.rs`

**Interfaces:**
- Consumes: `invention::enforce_ape_only` (Task 1); child `genome`/`modules`/`meme_vector`.
- Produces: a non-ape child's invention channels are 0 after inheritance; practice/base channels inherit normally; no change to RNG draw count.

- [ ] **Step 1: Write the failing test**

Add to `reproduce.rs` tests (build two ape parents holding inventions, force a non-ape child by overwriting its modules before the strip, and call the inheritance directly):

```rust
#[test]
fn non_ape_child_inherits_no_inventions() {
    use crate::invention::{self, channel};
    let mut w = World::new(31);
    w.inventions_enabled = true;
    let pos = find_grass_cell_center(&w);
    // Two parents holding Stone Tools.
    let a = w.spawn_agent(pos, fertile_genome());
    let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
    // Give both parents a Communicator so the child gets one too, and Stone Tools.
    for &p in &[a, b] {
        w.agents.modules[p as usize]
            .push(crate::module::Module::Communicator { range: 10.0, channel_id: 0 });
        w.agents.meme_vector[p as usize][channel(invention::STONE_TOOLS)] = 1.0;
    }
    // Spawn a child slot and make it a NON-ape (herbivore Mouth), Communicator.
    let child = w.spawn_agent(pos, fertile_genome());
    let mut kit = crate::module::ModuleList::new();
    kit.push(crate::module::Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 }); // non-ape
    kit.push(crate::module::Module::Communicator { range: 10.0, channel_id: 0 });
    w.agents.modules[child as usize] = kit;

    inherit_child_meme(&mut w, child, a as usize, b as usize);

    assert_eq!(
        w.agents.meme_vector[child as usize][channel(invention::STONE_TOOLS)],
        0.0,
        "a non-ape child must inherit no inventions"
    );
}
```

(If `inherit_child_meme` / `fertile_genome` / `find_grass_cell_center` are not visible in the test module, add the needed `use super::*;` items — they already back the existing `child_sacrifice_culls_about_half_of_newborns` test in this file.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/anabios-core && cargo test --lib non_ape_child_inherits_no_inventions`
Expected: FAIL — the child inherits ~0.5 (parent average) on the Stone Tools channel.

- [ ] **Step 3: Add the inheritance post-pass**

In `inherit_child_meme`, after the line that assigns the child's meme vector (line ~230, `world.agents.meme_vector[child_id as usize] = crate::culture::inherit_meme(...)`), and before `assign_birth_variants`, add:

```rust
    // Apes-only inventions: a non-ape child inherits no invention channels
    // (practice/base channels inherit normally). Runs AFTER inherit_meme so the
    // jitter draw count is unchanged — this only overwrites stored values.
    let ci = child_id as usize;
    let (genome_c, modules_c) = (&world.agents.genome[ci], &world.agents.modules[ci]);
    if !crate::invention::is_ape(genome_c, modules_c) {
        crate::invention::enforce_ape_only(
            &mut world.agents.meme_vector[ci],
            &world.agents.genome[ci],
            &world.agents.modules[ci],
            world.inventions_enabled,
        );
    }
```

(If the borrow checker objects to the `genome_c`/`modules_c` temporaries alongside the `&mut` meme borrow, inline the `is_ape` check into the `enforce_ape_only` call — `enforce_ape_only` already early-returns for apes, so calling it unconditionally is equivalent and avoids the split borrow.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/anabios-core && cargo test --lib non_ape_child_inherits_no_inventions`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/reproduce.rs
git commit -m "feat(reproduce): non-ape offspring inherit no inventions (post-pass strip)"
```

---

### Task 6: Viewer — correct the archetype-restriction note

**Files:**
- Modify: `game/scripts/settlement_layer.gd` (comment block at lines 209-212)

**Interfaces:**
- Consumes: nothing new. The sim now guarantees only apes hold inventions, so `stats_by_sid` `adopted_inventions` is populated only for ape-holding species — invention landmarks are already ape-only downstream.
- Produces: an accurate comment; no behavioral GDScript change.

- [ ] **Step 1: Replace the stale comment**

Change the block at `settlement_layer.gd:209-212` from the current text (claiming inventions are carried in any body and landmarks are NOT restricted by archetype) to:

```gdscript
# Invention landmarks mark the lineages that hold inventions. The sim now gates
# the tech tree to apes (the PRIMATE archetype: omnivore + large), so only ape
# lineages ever carry inventions — `adopted_inventions` is populated for them
# alone, and these landmarks therefore appear only over apes. Each landmark is
# PINNED at the spot the lineage first reached its tech (a monument), not trailed
# after a nomadic herd. One pass over the alive arrays builds each species'
# centroid + head-count; qualifying lineages fold into the linger/fade memory,
# then draw below into the shared per-kind build accumulators.
```

- [ ] **Step 2: Verify the viewer still parses/boots headless**

Run: `cd game && godot --headless res://scenes/main.tscn --quit-after 3 2>&1 | tail -20`
Expected: boots without GDScript parse errors (a comment-only change cannot alter behavior; this just confirms no accidental edit broke the file).

- [ ] **Step 3: Commit**

```bash
git add game/scripts/settlement_layer.gd
git commit -m "docs(viewer): inventions are apes-only; correct landmark archetype note"
```

---

### Task 7: Regenerate goldens and verify the whole suite

**Files:**
- Modify (regen only): `crates/anabios-core/tests/inventions.rs` (`INVENTIONS_GOLDEN`), `crates/anabios-core/tests/determinism.rs` (`tech-gene-coupling`, `cognitive-coevolution` goldens), and any other golden const whose scenario uses a reclassed archetype + `inventions_enabled` (e.g. traditions / domestication / cognition / O1-O2 goldens if present).

**Interfaces:**
- Consumes: all prior tasks merged.
- Produces: a green `cargo test` across the workspace with deliberately-refreshed goldens.

- [ ] **Step 1: Run the full core suite and list every failure**

Run: `cd crates/anabios-core && cargo test 2>&1 | tee /tmp/apes_test.log | grep -E "FAILED|test result|hash drift"`
Expected: failures ONLY in golden tests over scenarios using `innovator`/`traditionalist`/`cultural_forager` with inventions on. If a golden with none of those (e.g. `minimal.toml`, pure-affect) fails, STOP — that violates the flag-off invariant; investigate before regenerating.

- [ ] **Step 2: Regenerate the moved hashes**

Run: `cd crates/anabios-core && UPDATE_HASHES=1 cargo test 2>&1 | tee /tmp/apes_regen.log`
For each failing golden, copy the printed replacement tuples into its const, and add a dated note in the existing style, e.g. to `INVENTIONS_GOLDEN`:

```rust
    // Refreshed 2026-08-11 (apes-only inventions): the culture cohort
    // (innovator/traditionalist) is now omnivore (ape), shifting feeding
    // ecology and the invention-race trajectory. Regenerated on the gated tree.
```

- [ ] **Step 3: Re-run the full suite to confirm green**

Run: `cd crates/anabios-core && cargo test 2>&1 | grep -E "FAILED|test result"`
Expected: `test result: ok.` for every binary; no FAILED.

- [ ] **Step 4: Confirm the demo still climbs the tree**

Run: `cd crates/anabios-core && cargo test --test inventions innovators_discover_before_traditionalists_in_demo_scenario -- --nocapture`
Expected: PASS — reclassed apes still produce the first-era discovery within the demo window (the feature must not silently kill the flagship).

- [ ] **Step 5: CI gate parity**

Run: `cd crates/anabios-core && cargo fmt --check && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -q`
Expected: clean fmt, no rustdoc warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/tests/inventions.rs crates/anabios-core/tests/determinism.rs
# plus any other regenerated golden test files (stage explicit paths only)
git commit -m "test: regenerate goldens for apes-only inventions reclass"
```

---

## Self-Review

**Spec coverage:**
- `is_ape` predicate + viewer band parity → Task 1 (with boundary tests) ✓
- `enforce_ape_only` strip helper → Task 1 ✓
- Reclass culture cohort (innovator/traditionalist/cultural_forager) → Task 2 ✓
- Discovery gate → Task 3 ✓
- Social-copy gate (invention channels only; practices open) → Task 4 ✓
- Inheritance post-pass, no RNG impact → Task 5 ✓
- Viewer comment/behavior alignment → Task 6 ✓
- Golden regen + flag-off invariant verification → Task 7 ✓
- Practices remain open to all → asserted in Task 4; never gated anywhere ✓

**Placeholder scan:** No TBD/TODO; every code and test step has concrete content.

**Type consistency:** `is_ape(&Genome, &ModuleList) -> bool`, `enforce_ape_only(&mut [f32; MEME_CHANNELS], &Genome, &ModuleList, bool)`, `channel(usize) -> usize`, `INVENTION_COUNT`, `practice::channel(practice::CHILD_SACRIFICE)` used consistently across Tasks 1/4/5. Helper names `make_omnivore` (Task 2) and `enforce_ape_only` (Tasks 1/5) match their definitions.

## Out of scope (from spec)
- Mid-life archetype flips stripping already-held inventions.
- A new "cannibalism" practice (none exists; only cited as an example of the open-to-all category).
- Rebalancing O1/O2 experiment outcomes around the new gate.
- Reclassing the communicator-family archetypes (deliberately left herbivore).
