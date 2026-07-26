# TG1 — Gene↔Tech Feedback Loop — Design Spec

**Date:** 2026-07-25
**Status:** Draft (awaiting review)
**Roadmap:** `2026-07-25-tech-gene-coevolution-roadmap-design.md` (milestone TG1)
**Depends on:** `invention.rs`, `genome.rs`, `iq.rs`, `CoevoSample` (`anabios-godot/src/lib.rs`).

## 1. Goal

Close the **tech → gene** arm of coevolution and instrument both arms, so downstream milestones (DAG view, lead–lag view, HTML artifact) have real coupled data to render. Behind a new `gene_tech_coupling` scenario flag; **bit-identity when off**.

## 2. Problem recap

`invention.rs` today applies buffs via multipliers that take only the held-invention **bitmask** (`graze_multiplier(mask)`, `lifespan_multiplier(mask)`, …). Every holder of an invention gets the identical multiplier regardless of genome, so culture exerts **no directional selection** on genes. The one gene→tech arm (`Openness` discovery rate, `IQ` acquisition ceiling) already exists. We add the return arm without new RNG or hidden hashed state.

## 3. Design

### 3.1 Affinity metadata (static const)

Add to `struct Invention`:

```rust
/// The genome slot this invention couples to, and the strength of that
/// coupling. `None` = no gene coupling (buff is genome-independent, as today).
pub affinity: Option<GeneAffinity>,

pub struct GeneAffinity {
    pub slot: GenomeSlot,
    /// Selection coefficient: fraction of the invention's buff that is scaled
    /// by (gene - 0.5). At gene = 0.5 the buff is unchanged vs today; above,
    /// larger; below, smaller. Range chosen so the multiplier stays positive.
    pub coeff: f32,
}
```

This is a compile-time constant table — **no serialized state, no `#[serde(skip)]`**, so it cannot break replay.

**Proposed initial pairings** (open question 1 in the roadmap — these are the first proposal, not settled):

| Invention | Affinity gene | Rationale |
|-----------|---------------|-----------|
| Writing | `CommunicationStrength` (slot 23, currently inert) | Writing doubles meme spread; make that scale with the gene → adopting Writing selects for communicators. Finally wires a declared-but-dead slot. |
| Medicine | `CognitivePotential` | Medicine's lifespan buff rewards the cognitive lineage that could reach era-3 tech. |
| Electricity | `Openness` | Electricity's discovery multiplier scales with the innovation gene → runaway innovation feedback. |
| Farming | `Conscientiousness` (slot 21) | Sedentary farming rewards prudence; couples an OCEAN slot. |

Inventions not in the table keep `affinity: None` and behave exactly as today.

### 3.2 The tech → gene arm (gene-conditional buffs)

For each affinity-bearing invention, the relevant existing multiplier becomes gene-conditional **only when `gene_tech_coupling` is on**. The effect-site functions gain a gene argument; the mask-only path is preserved for the flag-off case.

Multiplier form (keeps buff positive, identity at `gene = 0.5`, and **exact identity when the invention is not held**, because `held_f32` is 0):

```
effective_buff = base_buff * (1.0 + coeff * (gene - 0.5))
```

So a holder with `gene = 1.0` gets `base_buff * (1 + 0.5*coeff)`; `gene = 0.0` gets `base_buff * (1 - 0.5*coeff)`. Because these buffs are fitness-relevant (yield, lifespan, spread), higher-gene holders out-reproduce → the affinity gene mean climbs among holders. When the meme collapses (atrophy/extinction) the pressure disappears → drift back (hysteresis).

**Threading the gene in:** effect sites already index per-agent (`i`). They read `world.agents.genome[i].get(slot)` at the call site and pass the scalar to the multiplier. No new loops, no new allocations.

**Flag-off identity:** with `gene_tech_coupling` false, the effect sites call the existing mask-only multipliers unchanged → **bit-identical** to today. This is the property the determinism gate checks.

### 3.3 The gene → tech arm (generalize discovery gate)

`invention_step`'s discovery roll currently scales every candidate's probability by global `Openness`. When `gene_tech_coupling` is on, scale candidate `k`'s probability *additionally* by its affinity gene (if any), so a lineage rich in a tech's affinity gene discovers that tech faster — the anticipatory arm that makes the lead–lag view show the gene *leading*. **No new RNG draw**: it reweights the existing single `f32_unit()` draw's `probs[]`/`total` exactly as the `iq_permits` filter already does. Flag-off: identical to today.

### 3.4 Instrumentation (feeds TG4/TG5)

Extend `CoevoSample` (the per-tick view-only history in `anabios-godot`) with, for each affinity-bearing invention:

- `affinity_gene_mean_holders[k]` — mean of the affinity gene over agents holding invention `k`.
- `affinity_gene_mean_nonholders[k]` — same over non-holders.
- The **selection differential** is their difference, computed in the viewer, not stored redundantly.

Add pure helpers in `coevo.rs` (`mean_slot_over(genomes, keep, slot)`) unit-tested in isolation, reusing the existing `SampleScratch` buffers so there is **zero new per-tick allocation**. This is view-layer only — it reads `World`, never mutates it, so it cannot affect the hash.

> **Design note (speciation):** `CommunicationStrength` is currently excluded from speciation distance (like the other personality/reserved slots). Once Writing selects on it, it becomes *adaptive*, matching the precedent set by `CognitivePotential` and `EnvAffinity` (which **do** count toward speciation because they are adaptive). Open question 4 in the roadmap: decide whether tech-selected genes should count toward speciation. Default proposal: **yes**, for consistency with the existing adaptive-slot rule — but this shifts speciation behavior, so it is a deliberate golden-regen and must be flagged, not silent.

## 4. Flag & scenario wiring

- `World::gene_tech_coupling: bool` with `#[serde(default)]` (matches `inventions_enabled`, so old snapshots deserialize).
- `Scenario::gene_tech_coupling` mirror + apply in `scenario.rs` (`w.gene_tech_coupling = self.gene_tech_coupling`).
- A showcase scenario (e.g. `coevolution` or extend `inventions`) sets it true alongside `inventions_enabled` + `cognition_enabled`.

## 5. Testing (evidence trio)

1. **Unit:** `effective_buff` = base at `gene = 0.5`; monotonic in gene; positive across `[0,1]` for the chosen `coeff`. Flag-off effect sites return the exact mask-only value. New `coevo.rs` helper: holder/non-holder means over a hand-built population.
2. **Determinism:** flag-off golden hashes **unchanged** (`cargo test determinism`); flag-on scenarios get freshly regenerated goldens (`UPDATE_HASHES=1`) copied into `tests/determinism.rs`; a save→load→step replay test with the flag on (guards the serde/hidden-state footgun).
3. **Headless demonstration:** a seed run where, flag-on, an affinity gene's holder-mean rises measurably above the flag-off control (directional selection), and reverts after an induced meme extinction (hysteresis). Recorded in the TG1 plan.

## 6. Determinism & perf

- **Identity when off:** guaranteed by preserving the mask-only path (§3.2, §3.3).
- **No new RNG** on any path (§3.3).
- **No new hashed state:** affinity is a const table; instrumentation is view-only (§3.1, §3.4).
- **Perf:** one extra `genome[i].get(slot)` per held affinity-invention per effect site — a slice read inside loops that already touch `genome[i]`. Expect ≪10% at 10k agents; verify on the criterion suite.

## 7. Out of scope (deferred to later TG milestones)

- New invention nodes / branches / `MEME_CHANNELS` growth → **TG2**.
- DAG rendering → **TG3**. Lead–lag correlation view → **TG4**. HTML artifact → **TG5**.
- Tech→module unlocks → TG2 optional, YAGNI-gated.
