# TG1 — Gene↔Tech Feedback Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make holding an invention exert directional selection on a coupled genome slot (the missing tech→gene arm), behind a `gene_tech_coupling` flag, and instrument both arms for later visualization.

**Architecture:** Add static `affinity` metadata to each `Invention` (a `GenomeSlot` + selection coefficient). A `coupled_held` helper scales an invention's buff term by its holder's affinity-gene value; base multipliers delegate to coupled variants with coupling **off**, guaranteeing bit-identity when the flag is off. Effect sites (graze, food-energy, lifespan, meme-spread) and the discovery roll read the flag and pass the per-agent gene. Instrumentation lives in the view-only `CoevoSample` (Godot crate) and never touches sim state.

**Tech Stack:** Rust (`anabios-core`, `anabios-godot`/gdext), criterion benches, golden-hash determinism tests.

## Global Constraints

- **Determinism gate stays green.** `cargo test -p anabios-core determinism` must pass with **unchanged** golden hashes on flag-**off** paths. Flag-on scenarios get deliberately regenerated goldens via `UPDATE_HASHES=1 cargo test -p anabios-core determinism`, values copied into `crates/anabios-core/tests/determinism.rs`.
- **No new RNG draws.** The discovery arm reweights the existing single `f32_unit()` draw's probability table; it must not add a draw on any path.
- **No new hashed hidden state.** Affinity is a compile-time `const` table. Instrumentation is view-only (reads `&World`). No new `#[serde(skip)]` fields.
- **Identity when off.** With `gene_tech_coupling == false`, every buff multiplier returns exactly its current value.
- **Perf.** ≤10% tick-time regression at 10k agents on `cargo bench -p anabios-core`.
- **Local gate before push** (matches CI): `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + rustdoc `-D warnings`. Heavy determinism/golden suite runs on PR CI, not every local commit.
- **Stage explicit paths** in every commit (never `git add -A`/`.`).

---

### Task 1: Affinity metadata on `Invention`

**Files:**
- Modify: `crates/anabios-core/src/invention.rs` (struct `Invention` ~line 53; `INVENTIONS` table ~line 75)
- Test: `crates/anabios-core/src/invention.rs` (`#[cfg(test)]` module at bottom)

**Interfaces:**
- Produces: `pub struct GeneAffinity { pub slot: GenomeSlot, pub coeff: f32 }`; `Invention::affinity: Option<GeneAffinity>`; `pub fn affinity_gene(genome: &Genome, inv: usize) -> f32`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn affinity_table_is_well_formed() {
    use crate::genome::{Genome, GenomeSlot};
    // Exactly the four coupled inventions carry an affinity; the rest are None.
    assert!(INVENTIONS[FIRE].affinity.is_some());
    assert!(INVENTIONS[FARMING].affinity.is_some());
    assert!(INVENTIONS[MEDICINE].affinity.is_some());
    assert!(INVENTIONS[WRITING].affinity.is_some());
    assert!(INVENTIONS[STONE_TOOLS].affinity.is_none());
    // Coeffs keep the buff strictly positive across the gene range [0,1]:
    // 1 + coeff*(gene-0.5) > 0  <=>  |coeff| < 2.
    for inv in INVENTIONS.iter() {
        if let Some(a) = inv.affinity {
            assert!(a.coeff.abs() < 2.0, "{} coeff too large", inv.name);
        }
    }
    // affinity_gene returns the slot value for coupled inventions, 0.5 otherwise.
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Openness, 0.9);
    assert!((affinity_gene(&g, FIRE) - 0.9).abs() < 1e-6);
    assert_eq!(affinity_gene(&g, STONE_TOOLS), 0.5);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core affinity_table_is_well_formed`
Expected: FAIL (compile error — `GeneAffinity`/`affinity`/`affinity_gene` undefined).

- [ ] **Step 3: Write minimal implementation**

Add near the top of `invention.rs` (after imports):

```rust
use crate::genome::{Genome, GenomeSlot};

/// Couples an invention to a genome slot. When `World::gene_tech_coupling` is
/// on, holding the invention scales its buff by the holder's slot value, so
/// adoption exerts directional selection on that gene (the tech→gene arm).
#[derive(Clone, Copy)]
pub struct GeneAffinity {
    pub slot: GenomeSlot,
    /// Fraction of the buff scaled by `(gene - 0.5)`. `|coeff| < 2` keeps the
    /// buff positive across `gene ∈ [0,1]`.
    pub coeff: f32,
}
```

Add `pub affinity: Option<GeneAffinity>,` as the last field of `struct Invention`.

In the `INVENTIONS` table set `affinity` on every entry — `None` for the six uncoupled, and for the four coupled:

```rust
// Fire entry:
affinity: Some(GeneAffinity { slot: GenomeSlot::Openness, coeff: 0.8 }),
// Farming entry:
affinity: Some(GeneAffinity { slot: GenomeSlot::Conscientiousness, coeff: 0.8 }),
// Medicine entry:
affinity: Some(GeneAffinity { slot: GenomeSlot::CognitivePotential, coeff: 0.8 }),
// Writing entry:
affinity: Some(GeneAffinity { slot: GenomeSlot::CommunicationStrength, coeff: 0.8 }),
```

Add the lookup helper:

```rust
/// The holder's value of invention `inv`'s affinity gene, or `0.5` (neutral →
/// identity in `coupled_held`) when the invention has no affinity.
#[inline]
pub fn affinity_gene(genome: &Genome, inv: usize) -> f32 {
    match INVENTIONS[inv].affinity {
        Some(a) => genome.get(a.slot),
        None => 0.5,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core affinity_table_is_well_formed`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/invention.rs
git commit -m "feat(invention): add per-invention gene affinity metadata"
```

---

### Task 2: `coupled_held` + gene-conditional buff multipliers

**Files:**
- Modify: `crates/anabios-core/src/invention.rs` (multiplier section ~lines 309–397; tests module)

**Interfaces:**
- Consumes: `GeneAffinity`, `affinity` field (Task 1).
- Produces: `pub fn coupled_held(mask, inv, gene, coupling) -> f32`; coupled variants `graze_multiplier_coupled`, `food_energy_multiplier_coupled`, `lifespan_multiplier_coupled`, `spread_multiplier_coupled` (all `(mask: u32, gene: f32, coupling: bool) -> f32`). Base `*_multiplier(mask)` fns delegate to their coupled variant with `gene = 0.5, coupling = false`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn coupling_is_identity_when_off_and_monotonic_when_on() {
    let farm = bit(FARMING);
    // OFF: coupled == base, regardless of gene.
    assert_eq!(graze_multiplier_coupled(farm, 0.0, false), graze_multiplier(farm));
    assert_eq!(graze_multiplier_coupled(farm, 1.0, false), graze_multiplier(farm));
    // ON, gene = 0.5: neutral, equals base.
    assert!((graze_multiplier_coupled(farm, 0.5, true) - graze_multiplier(farm)).abs() < 1e-6);
    // ON: strictly increasing in the gene, and the Farming term is what moves.
    let lo = graze_multiplier_coupled(farm, 0.0, true);
    let hi = graze_multiplier_coupled(farm, 1.0, true);
    assert!(hi > graze_multiplier(farm) && graze_multiplier(farm) > lo);
    assert!(lo > 0.0, "buff must stay positive");
    // Unheld invention: gene has no effect (coupled_held returns 0).
    assert_eq!(graze_multiplier_coupled(0, 1.0, true), graze_multiplier(0));
    // Writing spread scales the same way.
    let w = bit(WRITING);
    assert_eq!(spread_multiplier_coupled(w, 0.5, true), spread_multiplier(w));
    assert!(spread_multiplier_coupled(w, 1.0, true) > spread_multiplier(w));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core coupling_is_identity_when_off_and_monotonic_when_on`
Expected: FAIL (coupled variants undefined).

- [ ] **Step 3: Write minimal implementation**

```rust
/// Held-weight for `inv` inside a buff multiplier. With `coupling` off, or the
/// invention unheld, or no affinity, this is exactly `held_f32(mask, inv)`
/// (0.0/1.0) — so base multipliers stay bit-identical. With coupling on and
/// `inv` held with an affinity, it is `1.0 + coeff*(gene-0.5)`, scaling that
/// invention's buff term by the holder's gene.
#[inline]
pub fn coupled_held(mask: u32, inv: usize, gene: f32, coupling: bool) -> f32 {
    let h = held_f32(mask, inv);
    if !coupling || h == 0.0 {
        return h;
    }
    match INVENTIONS[inv].affinity {
        Some(a) => h * (1.0 + a.coeff * (gene - 0.5)),
        None => h,
    }
}
```

Add coupled variants and make the bases delegate. Graze (Farming coupled):

```rust
pub fn graze_multiplier_coupled(mask: u32, farming_gene: f32, coupling: bool) -> f32 {
    1.0 + STONE_TOOLS_BITE * held_f32(mask, STONE_TOOLS)
        + FARMING_BITE * coupled_held(mask, FARMING, farming_gene, coupling)
        + MACHINERY_BITE * held_f32(mask, MACHINERY)
}
#[inline]
pub fn graze_multiplier(mask: u32) -> f32 {
    graze_multiplier_coupled(mask, 0.5, false)
}
```

Food-energy (Fire coupled):

```rust
pub fn food_energy_multiplier_coupled(mask: u32, fire_gene: f32, coupling: bool) -> f32 {
    1.0 + FIRE_ENERGY * coupled_held(mask, FIRE, fire_gene, coupling)
}
#[inline]
pub fn food_energy_multiplier(mask: u32) -> f32 {
    food_energy_multiplier_coupled(mask, 0.5, false)
}
```

Lifespan (Medicine coupled):

```rust
pub fn lifespan_multiplier_coupled(mask: u32, medicine_gene: f32, coupling: bool) -> f32 {
    1.0 + MEDICINE_LIFESPAN * coupled_held(mask, MEDICINE, medicine_gene, coupling)
}
#[inline]
pub fn lifespan_multiplier(mask: u32) -> f32 {
    lifespan_multiplier_coupled(mask, 0.5, false)
}
```

Spread (Writing coupled — gated form, scale the bonus above 1.0):

```rust
pub fn spread_multiplier_coupled(mask: u32, writing_gene: f32, coupling: bool) -> f32 {
    if mask & bit(WRITING) == 0 {
        return 1.0;
    }
    let bonus = WRITING_SPREAD_MULT - 1.0;
    let scale = match (coupling, INVENTIONS[WRITING].affinity) {
        (true, Some(a)) => 1.0 + a.coeff * (writing_gene - 0.5),
        _ => 1.0,
    };
    1.0 + bonus * scale
}
#[inline]
pub fn spread_multiplier(mask: u32) -> f32 {
    spread_multiplier_coupled(mask, 0.5, false)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p anabios-core invention::`
Expected: PASS (new test + all existing `multipliers_are_identity_at_zero_mask`, `graze_multiplier_stacks_all_three_bonuses`, etc. still green — they call the base fns, now delegating with identity).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/invention.rs
git commit -m "feat(invention): gene-conditional buff multipliers (identity when off)"
```

---

### Task 3: `gene_tech_coupling` scenario flag

**Files:**
- Modify: `crates/anabios-core/src/world.rs` (flag block ~lines 55–85; defaults ~lines 269–272)
- Modify: `crates/anabios-core/src/scenario.rs` (fields ~lines 24–40; apply ~lines 322–325)
- Test: `crates/anabios-core/src/scenario.rs` tests module (or an inline `world.rs` test)

**Interfaces:**
- Produces: `World::gene_tech_coupling: bool` (default false); `Scenario::gene_tech_coupling: bool`; apply line `w.gene_tech_coupling = self.gene_tech_coupling;`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn gene_tech_coupling_defaults_off_and_scenario_applies() {
    let w = crate::world::World::default();
    assert!(!w.gene_tech_coupling, "must default off for baseline identity");
    let mut s = Scenario::default();
    s.gene_tech_coupling = true;
    let w2 = s.build(); // or the crate's scenario->world constructor name
    assert!(w2.gene_tech_coupling);
}
```

> Note for implementer: match the existing scenario→world constructor used by `inventions_enabled`'s test (grep `w.inventions_enabled = self.inventions_enabled`). Use that exact method name in the test.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core gene_tech_coupling_defaults_off_and_scenario_applies`
Expected: FAIL (field undefined).

- [ ] **Step 3: Write minimal implementation**

In `world.rs`, beside `inventions_enabled`:

```rust
/// When true, holding an invention scales its buff by the holder's affinity
/// gene (`invention::GeneAffinity`), so adoption exerts directional selection
/// on the genome, and per-candidate discovery is reweighted by the affinity
/// gene. Off by default; opt-in per scenario. Bit-identity when false.
/// Defaulted so old snapshots without this field still deserialize.
#[serde(default)]
pub gene_tech_coupling: bool,
```

Add `gene_tech_coupling: false,` to the `World` default block.

In `scenario.rs`, add `pub gene_tech_coupling: bool,` (with `#[serde(default)]` matching the neighbours) and the apply line `w.gene_tech_coupling = self.gene_tech_coupling;` beside `w.inventions_enabled = ...`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-core gene_tech_coupling_defaults_off_and_scenario_applies`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/world.rs crates/anabios-core/src/scenario.rs
git commit -m "feat(world,scenario): add gene_tech_coupling flag (default off)"
```

---

### Task 4: Wire coupled buffs into effect sites (tech→gene arm)

**Files:**
- Modify: `crates/anabios-core/src/interact.rs:113` (graze) and `:134` (food energy)
- Modify: `crates/anabios-core/src/age.rs:20` (lifespan)
- Modify: `crates/anabios-core/src/culture.rs:295` and `:347` (spread)
- Test: `crates/anabios-core/tests/` new integration test file `tg1_coupling.rs`

**Interfaces:**
- Consumes: coupled multipliers + `affinity_gene` (Tasks 1–2); `World::gene_tech_coupling` (Task 3).

- [ ] **Step 1: Write the failing integration test**

Create `crates/anabios-core/tests/tg1_coupling.rs`:

```rust
// A holder with a high affinity gene must out-buff a low-gene holder when the
// flag is on, and get the identical buff when it's off. Uses graze (Farming↔
// Conscientiousness) as the exemplar via the public multiplier path.
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::invention::{self, affinity_gene, bit, FARMING};

#[test]
fn coupling_creates_a_buff_differential_only_when_on() {
    let mask = bit(FARMING);
    let mut hi = Genome::neutral();
    hi.set(GenomeSlot::Conscientiousness, 1.0);
    let mut lo = Genome::neutral();
    lo.set(GenomeSlot::Conscientiousness, 0.0);

    // OFF: no differential.
    let off_hi = invention::graze_multiplier_coupled(mask, affinity_gene(&hi, FARMING), false);
    let off_lo = invention::graze_multiplier_coupled(mask, affinity_gene(&lo, FARMING), false);
    assert_eq!(off_hi, off_lo);

    // ON: high-gene holder gets the larger buff.
    let on_hi = invention::graze_multiplier_coupled(mask, affinity_gene(&hi, FARMING), true);
    let on_lo = invention::graze_multiplier_coupled(mask, affinity_gene(&lo, FARMING), true);
    assert!(on_hi > on_lo);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --test tg1_coupling`
Expected: PASS *for this pure test already* (multipliers exist) — this test guards the multiplier contract. The behavioral wiring below is verified by the determinism/headless tasks; keep this test as the contract guard.

> If you want a failing-first step here, temporarily assert `on_hi == on_lo` to see it fail, then correct to `>`. Otherwise proceed — the real behavioral change is at the effect sites next.

- [ ] **Step 3: Wire the effect sites**

`interact.rs` — replace the graze + food-energy calls (the `inv_mask` is already in scope; add the gene + flag):

```rust
let inv_mask = crate::invention::held_mask(&world.agents.meme_vector[i]);
let coupling = world.gene_tech_coupling;
let g = &world.agents.genome[i];
desired_bite *= crate::invention::graze_multiplier_coupled(
    inv_mask,
    crate::invention::affinity_gene(g, crate::invention::FARMING),
    coupling,
);
```

and at the payout:

```rust
world.agents.energy[i] += taken
    * FOOD_ENERGY_PER_BIOMASS
    * crate::invention::food_energy_multiplier_coupled(
        inv_mask,
        crate::invention::affinity_gene(&world.agents.genome[i], crate::invention::FIRE),
        world.gene_tech_coupling,
    );
```

`age.rs` — lifespan:

```rust
let lifespan = (lifespan_of(&world.agents.genome[i]) as f32
    * crate::invention::lifespan_multiplier_coupled(
        crate::invention::held_mask(&world.agents.meme_vector[i]),
        crate::invention::affinity_gene(&world.agents.genome[i], crate::invention::MEDICINE),
        world.gene_tech_coupling,
    )) as u32;
```

`culture.rs` — both `spread_multiplier(self_mask)` call sites (lines 295, 347). The agent index `i` and `world` are in scope:

```rust
let writing_gene = crate::invention::affinity_gene(&world.agents.genome[i], crate::invention::WRITING);
let meme_copy_rate = MEME_COPY_RATE
    * crate::invention::spread_multiplier_coupled(self_mask, writing_gene, world.gene_tech_coupling);
```

and likewise the invention-spread `rate` at line 347. Compute `writing_gene` once above both uses.

- [ ] **Step 4: Verify build + full core tests (flag-off identity)**

Run: `cargo test -p anabios-core`
Expected: PASS. Existing behavior tests unchanged because `gene_tech_coupling` defaults false and coupled variants are identity when off.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/src/age.rs crates/anabios-core/src/culture.rs crates/anabios-core/tests/tg1_coupling.rs
git commit -m "feat(sim): apply gene-conditional invention buffs at effect sites"
```

---

### Task 5: Gene→tech arm — affinity-weighted discovery

**Files:**
- Modify: `crates/anabios-core/src/invention.rs` `invention_step` discovery roll (~lines 447–495)
- Test: `crates/anabios-core/src/invention.rs` tests module (pure reweight helper)

**Interfaces:**
- Produces: `pub fn discovery_affinity_weight(genome: &Genome, inv: usize, coupling: bool) -> f32` — `[0.5, 1.5]`, `1.0` at neutral / no affinity / coupling off.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn discovery_affinity_weight_is_neutral_off_and_scales_on() {
    use crate::genome::{Genome, GenomeSlot};
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Openness, 1.0);
    // OFF -> 1.0 always.
    assert_eq!(discovery_affinity_weight(&g, FIRE, false), 1.0);
    // ON, coupled invention: > 1.0 for a high affinity gene.
    assert!(discovery_affinity_weight(&g, FIRE, true) > 1.0);
    // ON, uncoupled invention: exactly 1.0.
    assert_eq!(discovery_affinity_weight(&g, STONE_TOOLS, true), 1.0);
    // Neutral gene -> 1.0 (keeps near-identity so the tuning stays legible).
    assert_eq!(discovery_affinity_weight(&Genome::neutral(), FIRE, true), 1.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core discovery_affinity_weight_is_neutral_off_and_scales_on`
Expected: FAIL (undefined).

- [ ] **Step 3: Implement helper + fold into the roll**

```rust
/// Multiplier on invention `inv`'s per-tick discovery probability from the
/// holder's affinity gene. `1.0` when coupling is off, the invention has no
/// affinity, or the gene sits at the `0.5` neutral point; ranges `[0.5, 1.5]`.
/// Reweights the existing single RNG draw's probability table — adds no draw.
#[inline]
pub fn discovery_affinity_weight(genome: &Genome, inv: usize, coupling: bool) -> f32 {
    match (coupling, INVENTIONS[inv].affinity) {
        (true, Some(a)) => 0.5 + genome.get(a.slot),
        _ => 1.0,
    }
}
```

In `invention_step`, inside the `candidates(mask, |k| { ... })` closure, multiply the per-candidate probability by the weight (the `genome`/`world.gene_tech_coupling` are in scope — capture `let coupling = world.gene_tech_coupling;` and `let genome = &world.agents.genome[i];` before the closure):

```rust
let p = (BASE_DISCOVERY * openness * (0.3 + skill) * disc_mult
    / INVENTIONS[k].era as f32)
    * discovery_affinity_weight(genome, k, coupling);
let p = p.min(DISCOVERY_CAP);
probs[k] = p;
total += p;
```

Because the weight is `1.0` when `coupling` is off, the flag-off probability table — and therefore the RNG-consuming branch and its outcomes — is unchanged.

- [ ] **Step 4: Run tests**

Run: `cargo test -p anabios-core invention::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/invention.rs
git commit -m "feat(invention): affinity-weighted discovery (gene->tech arm)"
```

---

### Task 6: Instrumentation — per-invention selection differential in `CoevoSample`

**Files:**
- Modify: `crates/anabios-godot/src/coevo.rs` (add `mean_slot_over`)
- Modify: `crates/anabios-godot/src/lib.rs` (`CoevoSample` ~line 24; `sample_into` ~line 1061; `sample_to_dict` ~line 1132; `coevo_series` key match ~line 325)
- Test: `crates/anabios-godot/src/coevo.rs` tests module

**Interfaces:**
- Consumes: `invention::{INVENTIONS, held_mask, INVENTION_COUNT}`.
- Produces: `coevo::mean_slot_over(genomes, keep, slot) -> f32`; `CoevoSample::affinity_holder_mean: [f32; INVENTION_COUNT]`, `affinity_nonholder_mean: [f32; INVENTION_COUNT]`; series keys `aff_<key>_holder` / `aff_<key>_nonholder`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn mean_slot_over_respects_keep_mask() {
    use anabios_core::genome::{Genome, GenomeSlot, GENOME_LEN};
    let mut a = Genome([0.5; GENOME_LEN]);
    a.set(GenomeSlot::Openness, 1.0);
    let mut b = Genome([0.5; GENOME_LEN]);
    b.set(GenomeSlot::Openness, 0.0);
    let gs = [a, b];
    // Only the first agent kept -> mean == its value.
    assert_eq!(mean_slot_over(&gs, &[true, false], GenomeSlot::Openness), 1.0);
    // Both kept -> average.
    assert_eq!(mean_slot_over(&gs, &[true, true], GenomeSlot::Openness), 0.5);
    // None kept -> 0.0 (not NaN).
    assert_eq!(mean_slot_over(&gs, &[false, false], GenomeSlot::Openness), 0.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-godot mean_slot_over_respects_keep_mask`
Expected: FAIL (undefined).

- [ ] **Step 3: Implement helper + wire into the sample**

In `coevo.rs`:

```rust
/// Mean of one genome slot over agents where `keep[i]` is true. No kept agents
/// returns 0.0 (never NaN).
pub(crate) fn mean_slot_over(genomes: &[Genome], keep: &[bool], slot: GenomeSlot) -> f32 {
    let mut sum = 0.0;
    let mut n = 0u32;
    for (g, &k) in genomes.iter().zip(keep) {
        if k {
            sum += g.get(slot);
            n += 1;
        }
    }
    if n == 0 { 0.0 } else { sum / n as f32 }
}
```

In `lib.rs` add the two arrays to `CoevoSample`:

```rust
/// Per affinity-bearing invention: mean affinity gene over holders vs
/// non-holders (the selection differential is holder − nonholder). Zero for
/// inventions with no affinity. All zero when the tree is inactive.
affinity_holder_mean: [f32; anabios_core::invention::INVENTION_COUNT],
affinity_nonholder_mean: [f32; anabios_core::invention::INVENTION_COUNT],
```

In `sample_into`, after `genomes`/`memes` are populated, fill them (reuse a scratch `keep` Vec from `SampleScratch` — add a `keep: Vec<bool>` field to it):

```rust
use anabios_core::invention::{held_mask, INVENTIONS, INVENTION_COUNT};
let mut holder_mean = [0.0f32; INVENTION_COUNT];
let mut nonholder_mean = [0.0f32; INVENTION_COUNT];
for (k, inv) in INVENTIONS.iter().enumerate() {
    if let Some(a) = inv.affinity {
        scratch.keep.clear();
        scratch.keep.extend(memes.iter().map(|m| held_mask(m) & (1 << k) != 0));
        holder_mean[k] = coevo::mean_slot_over(genomes, &scratch.keep, a.slot);
        // non-holders: invert the mask into a second pass
        let inv_keep: Vec<bool> = scratch.keep.iter().map(|b| !b).collect();
        nonholder_mean[k] = coevo::mean_slot_over(genomes, &inv_keep, a.slot);
    }
}
```

Add the two fields to the `CoevoSample { .. }` literal, `sample_to_dict` (as nested arrays or `aff_<key>_holder` scalars — pick scalars for the flat `coevo_series` API), and extend the `coevo_series` key match so `aff_fire_holder`, `aff_fire_nonholder`, etc. resolve.

- [ ] **Step 4: Run tests + build the gdext crate**

Run: `cargo test -p anabios-godot && cargo build -p anabios-godot`
Expected: PASS / builds.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-godot/src/coevo.rs crates/anabios-godot/src/lib.rs
git commit -m "feat(coevo): record per-invention affinity-gene selection differential"
```

---

### Task 7: Showcase scenario + determinism regen

**Files:**
- Modify: the scenario table/registry (grep for where `inventions` scenario is defined — likely `scenario.rs` or a `scenarios/` data dir)
- Modify: `crates/anabios-core/tests/determinism.rs` (golden hash table)

**Interfaces:**
- Consumes: `gene_tech_coupling` flag (Task 3).

- [ ] **Step 1: Add a coupling showcase scenario**

Add a scenario (e.g. `coevolution_coupled`) identical to the richest inventions+cognition scenario but with `gene_tech_coupling: true`. Follow the exact pattern of the existing `inventions` scenario definition.

- [ ] **Step 2: Confirm flag-OFF goldens are unchanged**

Run: `cargo test -p anabios-core determinism`
Expected: PASS with **no** hash edits. If any existing golden changed, STOP — coupling leaked into a flag-off path; re-check Tasks 2/4/5 for a non-identity branch.

- [ ] **Step 3: Generate the new scenario's golden**

Run: `UPDATE_HASHES=1 cargo test -p anabios-core determinism`
Copy the emitted hash for the new scenario into `determinism.rs`. Do **not** overwrite unrelated hashes.

- [ ] **Step 4: Save→load→step replay guard**

Add a determinism test that builds the coupled scenario, steps N ticks, serializes, deserializes, steps one more tick on both, and asserts equal state hashes (guards the serde/hidden-state footgun for the new flag).

Run: `cargo test -p anabios-core determinism`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/scenario.rs crates/anabios-core/tests/determinism.rs
git commit -m "test(determinism): coupled showcase scenario golden + replay guard"
```

---

### Task 8: Headless evidence — selection differential + hysteresis

**Files:**
- Use: `scripts/emergence.sh` / `anabios-headless` to run the coupled scenario
- Modify: this plan file (record seed + numbers) and add a gallery caption per the roadmap evidence rule

- [ ] **Step 1: Run the coupled scenario headless, capture the affinity-gene series**

Run the coupled showcase for a long horizon on a fixed seed; capture per-tick `aff_fire_holder`/`aff_fire_nonholder` (or via the JSONL the headless writer emits). Record the seed.

- [ ] **Step 2: Run the flag-off control**

Same scenario/seed with `gene_tech_coupling: false`. Confirm the affinity gene mean stays flat (no directional selection) while the coupled run's holder-mean climbs.

- [ ] **Step 3: Demonstrate hysteresis**

Extend the run through a meme collapse (extinction/atrophy of the coupled invention) and confirm the holder-mean relaxes back toward baseline — the loop's hysteresis.

- [ ] **Step 4: Record evidence in the plan + one gallery capture**

Write the seed, tick window, and before/after holder-vs-nonholder differential into this plan under a new "## Evidence" section. Add a gallery capture with an honest caption.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-07-25-tg1-gene-tech-coupling.md gallery/
git commit -m "docs(tg1): headless evidence of tech->gene selection + hysteresis"
```

---

## Self-Review

**Spec coverage:**
- §3.1 affinity metadata → Task 1. ✓
- §3.2 tech→gene gene-conditional buffs → Tasks 2, 4. ✓
- §3.3 gene→tech discovery reweight → Task 5. ✓
- §3.4 instrumentation → Task 6. ✓
- §4 flag & scenario wiring → Tasks 3, 7. ✓
- §5 testing (unit/determinism/headless) → Tasks 1–2, 5–6 (unit), 7 (determinism/replay), 8 (headless). ✓
- §6 determinism/perf → Global Constraints + Task 7; perf bench is a Global Constraint checked before the final PR. ✓
- Speciation open-question (design note): **deferred to review** — the initial pairings use `CommunicationStrength` (not currently in speciation distance); Task 1 leaves speciation untouched, so this ships as "tech-selected genes do NOT yet count toward speciation." Flagged for the reviewer; a follow-up flips it deliberately with its own golden regen.

**Placeholder scan:** No TBD/TODO left; every code step carries real code. Task 4 Step 2 notes the pure test passes immediately and how to force a red first — acceptable (the behavioral change is at effect sites, covered by Task 7's determinism split).

**Type consistency:** `coupled_held(mask, inv, gene, coupling)`, `affinity_gene(genome, inv)`, `discovery_affinity_weight(genome, inv, coupling)`, coupled multipliers `(mask, gene, coupling)`, `mean_slot_over(genomes, keep, slot)`, `CoevoSample::affinity_holder_mean/affinity_nonholder_mean` — names are consistent across tasks.

## Note carried from spec (open questions for the reviewer)

The **selection strength** (`coeff = 0.8` initial) and the four **pairings** (Fire↔Openness, Farming↔Conscientiousness, Medicine↔CognitivePotential, Writing↔CommunicationStrength) are tunable constants chosen so selection is *measurable within a run* while the buff stays positive. Adjust in Task 1's table without touching mechanism code.
