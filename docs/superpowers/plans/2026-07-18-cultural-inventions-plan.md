# Mutation-Gated Cumulative Cultural Inventions — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** A mutation-gated cumulative cultural-invention ratchet with robust, stacking benefits, so an inventive-culture lineage robustly out-reproduces control in the abundant living biome (Stage 1), and — reach — the `Inventiveness` gene sweeps from standing variation (Stage 2).

**Architecture:** Three phases. **P1** adds the `Inventiveness` gene (slot 42), the `cultural_inventions` flag, and the invention-level ratchet (meme channel 7, gated on gene + Communicator). **P2** adds the three-tier tech-tree of robust benefits. **P3** adds the scenario + two staged reporting harnesses.

**Tech Stack:** Rust (`anabios-core`), scenario TOML, `cargo test` + a golden-hash determinism gate.

## Global Constraints

- **Flag-gated, off = byte-identical.** All new behaviour early-exits unless `World.cultural_inventions` is true. Existing scenarios byte-identical; the `dims::default_dims_byte_identical` trajectory fingerprint must stay green.
- **Golden discipline.** Only P1 adds a serialized `World` field (`cultural_inventions: bool`) → exactly one reviewed golden refresh, dated, attributing the move to the field. Later tasks add no `World` field (they reuse `genome`/`meme_vector` arrays + gate on the flag) → **golden must NOT move**; if it does, default behaviour changed — investigate, do not refresh.
- **`Inventiveness` counts toward speciation** (must NOT be added to `PERSONALITY_MASK`).
- **Determinism of new math:** fixed iteration order (ascending id in `culture_step`), no new RNG in hot paths. Bump `FORMAT_VERSION` for the new field.
- **Exact values (verbatim):** slot `Inventiveness = 42`; `INVENTION_CHANNEL = 7`; `INVENT_RATE = 0.01`; `INVENT_SOCIAL_RATE = 0.15`; `INVENTIVE_THRESHOLD = 0.5`; tech-tree thresholds `0.34 / 0.67 / 1.0`; `EFFICIENCY_DISCOUNT = 0.004`; `TOOL_BONUS = 0.3`; `PROVISION_DISCOUNT = 0.05`. All are starting values tuned in P3.

### Determinism recipe (V)

```bash
cd /Users/aryasen/projects/anabios/.claude/worktrees/coevolution
cargo test -p anabios-core 2>&1 | tail -20                 # golden fails only if a World field was added (P1)
cargo test -p anabios-core default_dims_byte_identical      # MUST stay green every task
# P1 only — refresh after confirming default_dims green + only determinism failed:
UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings \
  && RUSTDOCFLAGS="-D warnings" cargo doc -p anabios-core --no-deps
```

---

## File Structure

- `crates/anabios-core/src/genome.rs` — rename slot 42 → `Inventiveness` (P1).
- `crates/anabios-core/src/world.rs`, `scenario.rs` — `cultural_inventions` flag + knob (P1).
- `crates/anabios-core/src/culture.rs` — `INVENTION_CHANNEL` + consts + the ratchet's social-copy step; helper `invention_level`/`is_inventive` (P1) and tech-tree thresholds (P2).
- `crates/anabios-core/src/interact.rs` — invent-solo (P1) + Tooling additive graze (P2).
- `crates/anabios-core/src/module.rs` — Efficiency upkeep discount (P2).
- `crates/anabios-core/src/reproduce.rs` — Provisioning threshold discount (P2).
- `crates/anabios-core/src/scenario.rs` — `inventive_forager` archetype (P3).
- `scenarios/cultural-inventions.toml` — Stage-1 scenario (P3).
- `crates/anabios-core/tests/inventions.rs` (unit), `tests/cultural_inventions.rs` (Stage-1 harness), `tests/inventions_sweep.rs` (Stage-2 harness) — P1–P3.
- `crates/anabios-core/tests/determinism.rs` — golden refresh (P1 only).

---

# PHASE 1 — Gene, flag, and the ratchet

### Task 1.1: `Inventiveness` gene + `cultural_inventions` flag

**Files:** `genome.rs`, `world.rs`, `scenario.rs`, `snapshot.rs`, `tests/determinism.rs`

**Interfaces:**
- Produces: `GenomeSlot::Inventiveness = 42`; `World.cultural_inventions: bool`; `Scenario.cultural_inventions: bool`.

- [ ] **Step 1: Rename the gene slot**

In `crates/anabios-core/src/genome.rs`, rename `_SensoryReserved42 = 42,` to `Inventiveness = 42,` with a doc line: `/// Genetic propensity to invent/adopt cultural inventions (>0.5 = inventive).` Confirm slot 42 is NOT present in `PERSONALITY_MASK` (genome.rs ~259) — it must count toward speciation. (Reserved slots are excluded from the mask by default; verify nothing lists 42.)

- [ ] **Step 2: World flag + Scenario knob**

In `world.rs` add `#[serde(default)] pub cultural_inventions: bool,` (next to `biome_adaptation`), set `cultural_inventions: false,` in `new`. In `scenario.rs` add `#[serde(default)] pub cultural_inventions: bool,` and wire `w.cultural_inventions = self.cultural_inventions;` in `instantiate` (next to `w.biome_adaptation = ...`). Bump `FORMAT_VERSION` in `snapshot.rs`.

- [ ] **Step 3: Verify + golden refresh + commit**

Run recipe **V**. `default_dims_byte_identical` green; `determinism` fails (new field) → refresh with `// Refreshed 2026-07-18: added World.cultural_inventions flag + renamed genome slot 42 to Inventiveness (flag off = byte-identical; only serialized layout grew — the slot value semantics are unchanged).` Commit `feat(core): Inventiveness gene (slot 42) + cultural_inventions flag`.

---

### Task 1.2: The invention-level ratchet

Add the invention meme channel and its gated ratchet (invent slow solo, copy fast socially). No `World` field → golden-neutral.

**Files:** `culture.rs`, `interact.rs`, `tests/inventions.rs`

**Interfaces:**
- Produces: `culture::INVENTION_CHANNEL: usize = 7`; consts `INVENT_RATE`, `INVENT_SOCIAL_RATE`, `INVENTIVE_THRESHOLD`; `pub fn is_inventive(g: &Genome) -> bool`; `pub fn invention_level(meme: &[f32; MEME_CHANNELS]) -> f32`.
- Consumes: `World.cultural_inventions`, `GenomeSlot::Inventiveness`, `module::has(.., Communicator)`.

- [ ] **Step 1: Consts + helpers**

In `culture.rs` add (near `SKILL_CHANNEL`):
```rust
/// Meme channel carrying the cumulative cultural INVENTION LEVEL in [0,1].
pub const INVENTION_CHANNEL: usize = 7;
pub const INVENT_RATE: f32 = 0.01;        // slow solo progress per foraging tick
pub const INVENT_SOCIAL_RATE: f32 = 0.15; // fast copy from the best neighbour
pub const INVENTIVE_THRESHOLD: f32 = 0.5;

pub fn is_inventive(g: &crate::genome::Genome) -> bool {
    g.get(crate::genome::GenomeSlot::Inventiveness) > INVENTIVE_THRESHOLD
}
pub fn invention_level(meme: &[f32; crate::program::MEME_CHANNELS]) -> f32 {
    meme[INVENTION_CHANNEL]
}
```

- [ ] **Step 2: Failing test (ratchet gated on gene + Communicator + flag)**

Add `crates/anabios-core/tests/inventions.rs` with a test that builds a small world (`cultural_inventions = true`), spawns an inventive agent WITH a Communicator on food, steps ~200 ticks, and asserts its `meme_vector[7]` rose above ~0.1; a non-inventive agent (gene ≤ 0.5) with a Communicator keeps `meme_vector[7] == 0`; and with the flag OFF the inventive agent's level stays 0. (Build worlds via `World::new` + `spawn_seeded` with `communicator`-appended `starter_kit`, mirroring `tests/gene_culture.rs`.) Run → FAILS (no invent code yet).

- [ ] **Step 3: Invent-solo in `feed_pass`**

In `interact.rs` `feed_pass`, alongside the existing skill learn-by-doing block (the `if world.env_period == 0 && is_comm { skill += SKILL_LEARN_RATE ... }` region), add, gated on `world.cultural_inventions && crate::culture::is_inventive(&world.agents.genome[i]) && is_comm` and a successful graze:
```rust
let inv = &mut world.agents.meme_vector[i][crate::culture::INVENTION_CHANNEL];
*inv += crate::culture::INVENT_RATE * (1.0 - *inv);
```
(Match the surrounding borrow pattern; do not consume RNG.)

- [ ] **Step 4: Copy-social in `culture_step`**

In `culture.rs` `culture_step`, alongside the existing SKILL social-copy (the max-skilled-neighbour block), add a gated invention copy: when `world.cultural_inventions` and the agent `is_inventive` and has a Communicator, find the highest `invention_level` among inventive Communicator neighbours in range and move toward it:
```rust
// after computing best_neighbour_invention over inventive Communicator neighbours:
let cur = world.agents.meme_vector[i][INVENTION_CHANNEL];
if best_neighbour_invention > cur {
    world.agents.meme_vector[i][INVENTION_CHANNEL] =
        cur + INVENT_SOCIAL_RATE * (best_neighbour_invention - cur);
}
```
Reuse the same ascending-id neighbour iteration the SKILL block uses (deterministic, no RNG). The INVENTION channel must be EXCLUDED from the generic broadcast-mean lerp (like SKILL/TECH already are) so it isn't dragged toward 0.

- [ ] **Step 5: Verify (golden-neutral) + commit**

Run recipe **V**. The ratchet test passes; `determinism` PASSES WITHOUT refresh (flag-off byte-identical, no new field); `default_dims_byte_identical` green. If the golden moved, a gate is missing — fix. Commit `feat(core): cultural-invention ratchet (gene+Communicator-gated meme channel 7)`.

---

# PHASE 2 — The tech-tree (stacking robust benefits)

### Task 2.1: Efficiency, Tooling, Provisioning

All three gated on `world.cultural_inventions && is_inventive(genome) && has(Communicator)` and the level threshold. Golden-neutral.

**Files:** `culture.rs` (thresholds + a `tech_tier` helper), `module.rs` (Efficiency), `interact.rs` (Tooling), `reproduce.rs` (Provisioning), `tests/inventions.rs`

**Interfaces:**
- Produces: `culture` consts `EFFICIENCY_THRESHOLD=0.34`, `TOOLING_THRESHOLD=0.67`, `PROVISION_THRESHOLD=1.0`, `EFFICIENCY_DISCOUNT=0.004`, `TOOL_BONUS=0.3`, `PROVISION_DISCOUNT=0.05`; a helper to test each tier given `(genome, meme, has_comm, flag)`.

- [ ] **Step 1: Thresholds + a gating helper**

In `culture.rs` add the six consts and:
```rust
/// Does an agent currently benefit from the invention tier unlocked at `threshold`?
pub fn invention_active(
    flag: bool, g: &crate::genome::Genome,
    meme: &[f32; crate::program::MEME_CHANNELS], has_comm: bool, threshold: f32,
) -> bool {
    flag && has_comm && is_inventive(g) && invention_level(meme) >= threshold
}
```

- [ ] **Step 2: Failing tests (each benefit, thresholded + stacking)**

Add tests to `tests/inventions.rs`: (a) Efficiency reduces an inventive Communicator's per-tick upkeep by `EFFICIENCY_DISCOUNT` only when `inv ≥ 0.34` and flag on; (b) Tooling adds `TOOL_BONUS` energy on a graze only when `inv ≥ 0.67`; (c) Provisioning lowers the effective reproduction threshold only when `inv ≥ 1.0`; (d) all three active at `inv = 1.0`; (e) all inert when flag off or gene ≤ 0.5. Run → FAIL.

- [ ] **Step 3: Efficiency (module.rs upkeep)**

In `module::upkeep_all` (where `total_upkeep` is deducted per agent), after computing the module upkeep sum, subtract the discount when the tier is active, clamped ≥ 0:
```rust
if crate::culture::invention_active(world.cultural_inventions, g, meme, has_comm, crate::culture::EFFICIENCY_THRESHOLD) {
    total = (total - crate::culture::EFFICIENCY_DISCOUNT).max(0.0);
}
```
(Thread whatever `upkeep_all` needs — it already has `agents`; it must also see the `cultural_inventions` flag. If `upkeep_all` takes only `&mut AgentBuffers` today, change its signature to also take `cultural_inventions: bool` and update the one call site in `tick.rs`.)

- [ ] **Step 4: Tooling (interact.rs feed_pass)**

In `feed_pass`, after a successful graze yields energy, add the flat bonus when the Tooling tier is active:
```rust
if crate::culture::invention_active(world.cultural_inventions, &world.agents.genome[i], &world.agents.meme_vector[i], is_comm, crate::culture::TOOLING_THRESHOLD) {
    world.agents.energy[i] += crate::culture::TOOL_BONUS;
}
```
This is ADDITIVE to gained energy (not a multiplier on the bite) — it cannot saturate.

- [ ] **Step 5: Provisioning (reproduce.rs threshold)**

In `reproduce_all`, where the reproduction-energy threshold is read from `GenomeSlot::ReproductionThreshold`, subtract `PROVISION_DISCOUNT` from the effective threshold when the Provisioning tier is active (clamped to a sane floor, e.g. `≥ 0.05`). Thread the flag + meme/genome/has_comm at that site.

- [ ] **Step 6: Verify (golden-neutral) + commit**

Run recipe **V**. Tech-tree tests pass; `determinism` PASSES WITHOUT refresh; `default_dims_byte_identical` green. Commit `feat(core): invention tech-tree — efficiency, tooling, provisioning`.

---

# PHASE 3 — Experiment (staged)

### Task 3.1: `inventive_forager` archetype + Stage-1 scenario

**Files:** `scenario.rs`, `scenarios/cultural-inventions.toml`, `tests/all_scenarios.rs`

- [ ] **Step 1: Archetype** — in `scenario.rs` `archetype_kit`, add `"inventive_forager" => { let mut m = starter_kit(); m.push(crate::module::Module::Communicator { range: 12.0, channel_id: 0 }); (m, starter_asocial_forager()) }`; in `archetype_genome`, add `"inventive_forager" => g.set(GenomeSlot::Inventiveness, 1.0),`. (Culture = starter_kit + Communicator + Inventiveness gene; control = `asocial_forager`, neither — so cohorts differ in the GENOME.)

- [ ] **Step 2: Scenario** — create `scenarios/cultural-inventions.toml`: the 2048 living-biome settings (copy from `living-sandbox-coevolution.toml`) plus `cultural_inventions = true`; culture `[[agents]]` archetype `inventive_forager` (species 1), control `asocial_forager` (species 2), `placement = { kind = "uniform" }`, count 400 each. Add a `cultural_inventions_smoke` test (200 ticks, survives; species-1 agents carry the Inventiveness gene + Communicator, species-2 neither) to `all_scenarios.rs`.

- [ ] **Step 3: Verify + commit** — `cargo test -p anabios-core` green; golden UNCHANGED (new scenario/archetype inert for existing scenarios). Commit `feat(scenario): cultural-inventions Stage-1 scenario + inventive_forager`.

### Task 3.2: Stage-1 differential harness

**Files:** `tests/cultural_inventions.rs`

- [ ] **Step 1** — copy the structure of `tests/living_sandbox.rs` (cohort-by-species-ancestry tally, env-overridable seeds/ticks, living-on/off, reporting verdict — NO hard assert). Point it at `scenarios/cultural-inventions.toml`, founders 1 (culture) / 2 (control). Report culture-vs-control differential vs the ≥7/10 bar.

- [ ] **Step 2: RUN IT** — `cargo test -p anabios-core --release --test cultural_inventions -- --ignored --nocapture`. Read the per-seed table. This is the Stage-1 result. If culture does not robustly win, tune (`INVENT_*`, the three benefit magnitudes/thresholds) via a short sweep and record the numbers. Capture the final table + verdict in the module docs.

- [ ] **Step 3: Commit** the harness + documented result.

### Task 3.3: Stage-2 gene-sweep harness

**Files:** `tests/inventions_sweep.rs`

- [ ] **Step 1** — mirror `gene_culture.rs::gene_culture_firstprinciples_B`: one interbreeding species (id 0), `cultural_inventions = true`, ~15% of agents seeded with `Inventiveness = 1.0` + a Communicator (rest `starter_kit`, gene neutral), run N seeds × T ticks, track the `Inventiveness`-gene frequency (fraction with `genome[42] > 0.5`) at intervals + final vs initial. Reporting verdict (rose/fell), no hard assert.

- [ ] **Step 2: RUN IT** — capture whether the gene sweeps. Document the result (positive = the coevolution finally holds; negative = a legitimate result, note it and the next lever). Commit.

---

## Self-Review

**Spec coverage** — gene (1.1) = spec §3.1; ratchet (1.2) = §3.2; tech-tree (2.1) = §3.3; flag/determinism (1.1) = §4; experiment (3.1–3.3) = §5; Stage-1/Stage-2 bars = §1.

**Placeholder scan** — exact slot (42), channel (7), consts, and archetype/scenario are complete; the four hook sites (feed_pass, culture_step, upkeep_all, reproduce threshold) are named with the exact gated edit — the implementer reads the real function bodies (they were not re-pasted here; each edit is a precise, gated insertion, not a vague directive). Two in-flight confirmations flagged inline: `upkeep_all`'s current signature (may need a `cultural_inventions` param + one call-site update, Task 2.1 Step 3), and the exact `ReproductionThreshold` read site in `reproduce_all` (Task 2.1 Step 5).

**Type consistency** — `is_inventive(&Genome)`, `invention_level(&[f32; MEME_CHANNELS])`, `invention_active(flag, &Genome, &meme, has_comm, threshold)` are defined in P1/P2 and used identically in P2's hooks and P3's cohorts. Founders 1 (culture) / 2 (control) consistent across scenario, harness. `INVENTION_CHANNEL = 7` used everywhere.

**Determinism discipline** — P1.1 does the single golden refresh (new bool field); every other task is golden-neutral and must keep `default_dims_byte_identical` green, with the INVENTION channel excluded from the generic broadcast lerp (like SKILL/TECH) to avoid drift.

**Scope note** — Task 3.2 (Stage 1) is the primary deliverable and likely to succeed (robust above-threshold benefit + genetically-distinct cohorts). Task 3.3 (Stage 2, gene sweep) is the ambitious reach and a genuine research bet — a negative is a documented result, not a plan defect.
