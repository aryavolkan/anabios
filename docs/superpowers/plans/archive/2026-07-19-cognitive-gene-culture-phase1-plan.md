# Cognitive Gene–Culture — Phase 1 Implementation Plan

> Phase 1 of the [cognitive gene–culture design](../specs/2026-07-19-cognitive-gene-culture-design.md).
> Scope: the `CognitivePotential` gene + realized-IQ phenotype (nature+nurture
> development, no RNG) + IQ metabolic cost + the `cognition_enabled` flag.
> **No gating and no maladaptive memes yet** (Phases 2–3).

**Goal:** every agent has a realized IQ that develops from a heritable gene
modulated by juvenile nutrition + social enrichment, costs basal metabolism,
and is fully flag-gated so baseline scenarios stay behavior-identical.

## Global constraints

- Determinism is load-bearing. Flag-off must be **behavior**-identical; the new
  serialized `AgentBuffers` IQ fields grow the layout, so the minimal + inventions
  goldens move **once, by layout only** (documented refresh) — no trajectory change.
- IQ development consumes **zero RNG**. No new RNG draws anywhere in Phase 1.
- Sensor reads use the per-agent bounds check (`i < sensors.len()`).
- `x * 1.0 == x`, so the metabolic multiplier is exact identity at `iq == 0`.

## Task 1 — `CognitivePotential` gene (rename only)

**Files:** `crates/anabios-core/src/genome.rs:63`

- Rename `_DriveReserved16 = 16` → `CognitivePotential = 16` with a doc comment:
  heritable cognitive potential in `[0,1]`; the *nature* baseline for realized IQ.
- No `distance()` change: slot 16 is not in `PERSONALITY_MASK`, so it is already
  counted toward speciation (as an adaptive gene should be). Renaming changes no
  value (was `0.5` in every neutral genome) — byte-identical.
- Add `pub fn cognitive_potential(&self) -> f32 { self.get(GenomeSlot::CognitivePotential) }`.

**Verify:** `cargo build -p anabios-core`; determinism unaffected at this step.

## Task 2 — IQ phenotype fields on `AgentBuffers`

**Files:** `crates/anabios-core/src/agent.rs`

- Add three SoA `Vec` fields (serialized — part of persistent state):
  - `pub iq: Vec<f32>` — realized IQ, the value all gates will read (Phase 2+).
  - `pub iq_enrich_acc: Vec<f32>` — running sum of juvenile enrichment samples.
  - `pub iq_enrich_ticks: Vec<u32>` — juvenile sample count.
- In `spawn`, both branches: init `iq = 0.0`, `iq_enrich_acc = 0.0`,
  `iq_enrich_ticks = 0` (push in the extend branch, assign in the reuse branch).
  IQ starts at 0 for everyone; the development stage raises it when cognition is on.

**Verify:** `cargo build`; `spawn_*` tests still pass.

## Task 3 — `cognition_enabled` flag

**Files:** `crates/anabios-core/src/world.rs`, `crates/anabios-core/src/scenario.rs`

- `World`: add `#[serde(default)] pub cognition_enabled: bool` (mirror
  `inventions_enabled`); default `false` in `World::new`.
- `Scenario`: add `#[serde(default)] pub cognition_enabled: bool`; in
  `instantiate`, `w.cognition_enabled = self.cognition_enabled;`.

**Verify:** `cargo build`.

## Task 4 — `iq` module: development + metabolic multiplier

**Files:** new `crates/anabios-core/src/iq.rs`; register in `crates/anabios-core/src/lib.rs`.

Constants (tunable): `IQ_MATURATION_AGE = 100`, `IQ_PLASTICITY = 0.5`,
`IQ_METABOLIC_COST = 0.25`, `IQ_NUTRITION_REF = SPAWN_ENERGY` (50.0),
`IQ_SOCIAL_REF = 8.0`.

```rust
/// Basal-metabolism multiplier from realized IQ. Identity at iq==0 (so a
/// flag-off world, where iq stays 0, is byte-identical).
#[inline]
pub fn metabolism_multiplier(iq: f32) -> f32 { 1.0 + IQ_METABOLIC_COST * iq }
```

`pub fn develop_all(world: &mut World)`: early-return unless `cognition_enabled`.
For each alive agent with `age < IQ_MATURATION_AGE`:
- `nutrition = (energy / IQ_NUTRITION_REF).clamp(0.0, 1.0)`
- `social = if i < sensors.len() { (sensors[i].crowding as f32 / IQ_SOCIAL_REF).clamp(0.0,1.0) } else { 0.0 }`
- `iq_enrich_acc[i] += 0.5*nutrition + 0.5*social; iq_enrich_ticks[i] += 1`
- `enrich = iq_enrich_acc[i] / iq_enrich_ticks[i]`
- `iq[i] = lerp(genome.cognitive_potential(), enrich, IQ_PLASTICITY)`
Agents at/after maturation are skipped → IQ frozen. Uses the alive-id scratch
buffer (take/restore) like the other stages. No RNG.

**Verify:** unit tests below.

## Task 5 — Wire development + metabolic cost

**Files:** `crates/anabios-core/src/tick.rs`, `crates/anabios-core/src/integrate.rs`

- `tick.rs`: call `crate::iq::develop_all(world)` immediately after
  `crate::module::upkeep_all(...)` and before `reproduce_all` (energy reflects
  this tick's feeding; sensors from this tick's `sense` are still valid).
- `integrate.rs`: in **both** basal-metabolism sites, multiply by
  `crate::iq::metabolism_multiplier(agents.iq[i])`.

## Task 6 — Snapshot version + golden refresh

**Files:** `crates/anabios-core/src/snapshot.rs:25`, `tests/determinism.rs`, `tests/inventions.rs`

- Bump `FORMAT_VERSION` 5 → 6 (AgentBuffers layout grew).
- Regenerate with `UPDATE_HASHES=1`; update the minimal golden (layout-only move)
  and the inventions golden (same), each with a dated changelog line.

## Task 7 — Tests

**Files:** `crates/anabios-core/src/iq.rs` (`#[cfg(test)]`)

- `metabolism_multiplier` identity at 0, `1.25` at 1.0.
- `develop_all` no-op when flag off (iq stays 0).
- Nature+nurture blend: bright gene + rich env (high energy, crowded) yields
  higher realized IQ than bright gene + poor env; average gene + rich env beats
  average gene + poor env. (Drive `develop_all` directly with hand-set
  energy/sensors/age.)
- Crystallization: IQ stops changing once `age >= IQ_MATURATION_AGE`.

**Verify:** full `anabios-core` suite green; `fmt`/`clippy`/`doc` clean; minimal +
inventions goldens pass at their refreshed values.
