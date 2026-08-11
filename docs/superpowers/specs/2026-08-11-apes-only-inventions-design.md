# Apes-only inventions (practices stay open to all)

**Date:** 2026-08-11
**Branch:** `claude/content-policy-apes-inventions-ac832f`
**Status:** design, pending implementation

## Goal

Restrict the cultural **invention** tree (Stone Tools … Nuclear Power) so only
**apes** can discover, socially copy, or inherit it. Leave the maladaptive
**practice** tree (Child Sacrifice, Inbreeding, any future "cannibalism")
untouched: any animal can still discover, hold, and spread a practice.

Content-policy framing: the recognisable technological ascent belongs to the
hominins on screen; the disturbing customs are not gated to them and are not
presented as a uniquely-ape achievement.

## Definition of "ape"

The Godot viewer already classifies an agent as `PRIMATE` when it is an
**omnivore** and **large** (`game/scripts/mammal_sprites.gd:41`):

- `0.34 ≤ diet < 0.66` (omnivore band; `HERB_MAX`/`CARN_MIN`)
- `size ≥ 1.25` world-units (`SIZE_SPLIT`)

`diet` is `module::effective_diet_carnivory(modules)`. The bridge maps
world-size `= 0.5 + 2.5 × genome.Size` (`crates/anabios-godot/src/lib.rs:476`),
so `size ≥ 1.25` ⇔ `genome.Size ≥ 0.30`.

Make this canonical in the core, mirroring the viewer exactly:

```rust
// crates/anabios-core/src/invention/mod.rs
pub const APE_SIZE_MIN: f32 = 0.30; // == viewer SIZE_SPLIT 1.25 world-units
pub const APE_DIET_LO: f32 = 0.34;  // == viewer HERB_MAX
pub const APE_DIET_HI: f32 = 0.66;  // == viewer CARN_MIN

/// True when the agent is the viewer's PRIMATE archetype (omnivore + large):
/// the only archetype permitted to acquire cultural inventions.
pub fn is_ape(genome: &Genome, modules: &module::ModuleList) -> bool {
    let diet = module::effective_diet_carnivory(modules);
    genome.get(GenomeSlot::Size) >= APE_SIZE_MIN
        && diet >= APE_DIET_LO
        && diet < APE_DIET_HI
}
```

Note: `diet`/`size` are effectively birth-fixed (genome `Size` is; module diet
is stable in practice). A rare mid-life diet/size flip is out of scope — the
gate is enforced only at the three transmission points below, matching how the
viewer treats archetype as birth-fixed.

## The three gate points (all inside the existing `inventions_enabled` path)

Every gate lives inside a block already guarded by `world.inventions_enabled`,
so **flag-off scenarios are provably byte-identical** to today.

### 1. Discovery — `invention::invention_step` (`invention/mod.rs:688`)

Change the innovation guard from
`if module::has(.., Communicator)` to
`if module::has(.., Communicator) && is_ape(genome, modules)`.

A non-ape Communicator no longer rolls for a breakthrough, so it no longer
consumes its `world.rng.f32_unit()` discovery draw. **RNG impact:** the stream
shifts for any `inventions_enabled` scenario containing a non-ape Communicator.

### 2. Social copying — `culture::culture_step` (`culture.rs:361`)

Gate the whole invention-copy block (`if world.inventions_enabled { … }`,
lines 361–426) on the receiver being an ape:
`if world.inventions_enabled && is_ape(&genome[i], &modules[i]) { … }`.

The separate practice-copy block (lines 432+) is untouched — practices still
spread to every receiver. **RNG impact:** none (copy is a deterministic lerp,
no draws); only a non-ape receiver's stored invention levels change (they now
stall at whatever they were rather than climbing).

### 3. Vertical inheritance — `reproduce::inherit_child_meme` (`reproduce.rs:211`)

Leave `inherit_meme` and its jitter draws exactly as-is. **After** it returns,
if the child is not an ape, zero the child's invention channels:

```rust
world.agents.meme_vector[child] = inherit_meme(...); // unchanged, same RNG
if world.inventions_enabled
    && !crate::invention::is_ape(&world.agents.genome[child], &world.agents.modules[child])
{
    for k in 0..crate::invention::INVENTION_COUNT {
        world.agents.meme_vector[child][crate::invention::channel(k)] = 0.0;
    }
}
```

**RNG impact:** none — the jitter draws still happen; only the stored invention
channels of a non-ape child are overwritten to 0. This keeps the invariant "a
non-ape holds no inventions" true even under a rare ape→non-ape mutation,
without perturbing any scenario's draw count.

## Viewer

- Restrict invention landmarks / building-holder rendering to the `PRIMATE`
  archetype (`game/scripts/settlement_layer.gd`, `building_sprites.gd`).
- Fix the now-stale comment at `settlement_layer.gd:210-212` that states
  inventions are *not* archetype-restricted — they now are, in the sim.

Because the sim now only lets apes hold tech, this is mostly the viewer's
rendering catching up to the model rather than an independent rule.

## Determinism & goldens

- `inventions_enabled = false`: byte-identical (all three gates are inside the
  enabled path). No golden change.
- `inventions_enabled = true`: goldens **may** move, only for scenarios with
  non-ape Communicators. The flagship `scenarios/inventions.toml` uses
  `innovator`/`traditionalist` archetypes that default to omnivore + large
  (already apes) and an asocial control with no Communicator, so its goldens
  are expected to be **unchanged**. Run the full determinism/golden/inventions
  suite; regen with `UPDATE_HASHES=1` only if hashes legitimately move, and
  record which scenarios moved and why.

## Tests

- `is_ape` unit table: omnivore+large ⇒ true; herbivore, carnivore, or small ⇒
  false; boundary values (`Size = 0.30`, `diet = 0.34`, `diet = 0.66`) match
  the viewer's `test_mammal_sprites.gd` cases.
- Discovery gate: a non-ape Communicator with open candidates never discovers;
  an otherwise-identical ape does.
- Copy gate: a non-ape receiver next to an invention-holding ape does not climb
  invention channels but **does** still copy a practice channel.
- Inheritance gate: a non-ape child of two invention-holding apes is born with
  all invention channels at 0 but inherits practice channels normally.
- Regression: a flag-off scenario's golden hash is unchanged.

## Out of scope

- Mid-life archetype flips stripping already-held inventions.
- Any new "cannibalism" practice (only referenced as an example of the
  open-to-all category; none exists in code today).
- Rebalancing the O1/O2 gene-culture experiments around the new gate.
