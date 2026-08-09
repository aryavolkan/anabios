# M2 Code Review Follow-ups (deferred to M3 or later)

Captured from the final M2 code review (2026-05-23). The M2 implementation
landed clean (no Critical issues); these are quality-of-life improvements
and one prerequisite to address before M3.

## Important — prerequisite for M3

### I1 — Track `species_member_counts` incrementally on spawn/kill

`World.species_member_counts` is only authoritative immediately after
`species::species_step` runs. Between any `agents.spawn` / `agents.kill`
and the next `species_step`, the counts are stale. Today nothing reads
the field outside of `species_step`, so this is benign — but the field
is `pub` and M3 code (modules, behavior programs) may begin reading
species state during gameplay.

**Fix:** Add `add_to_species(species_id)` and `remove_from_species(species_id)`
helpers on `World`. Call them from:
- `World::spawn_agent` (after `agents.spawn`)
- `reproduce::reproduce_all` (after spawning the child)
- The death path in `age::age_and_starve` (before `agents.kill`)
- `species::species_step`'s reassignment branch (already does this manually;
  unify with the helper)

Also need to regenerate the golden tick hashes in `tests/determinism.rs`
because `species_member_counts` is included in the snapshot and the timing
of when the value reaches its final shape changes.

Recommend doing this as the first task of M3, before adding any new
gameplay state.

## Important — minor functional drift

### I2 — Mate-seeking can lead toward wrong same-species mate

`behavior.rs` mate-seeking heads toward `sensor.nearest_neighbor_dir`
which is the nearest *any-species* neighbor that *happens to* match the
agent's species. Meanwhile `reproduce::find_mate` scans the full
mating-range neighborhood for any eligible same-species partner. So an
agent can be pulled away from a closer mate.

In practice the closer mate gets selected by `find_mate` anyway, so the
wasted velocity is one tick of drift. Functionally fine. Add a comment
in `behavior.rs:54` noting the two stages use different "mate" notions.

## Minor

### M1 — Genome::crossover RNG efficiency

`genome.rs::crossover` does one `f32_unit()` per slot (50 draws per
offspring). Replace with bit-packed `u64` draws: one 64-bit RNG call
yields enough bits for all 50 slot selections. At 2k agents reproducing
~once per generation × 50 slots = ~100k `f32_unit` calls; switching to
~2k `u64` calls is measurable in the bench. M3 cleanup.

### M2 — Dead resize in `reproduce_all`

`reproduce.rs:39-41` resizes `reproduced_this_tick` to `agents.capacity()`,
but `World::resize_scratch` (called at the top of every `tick::step`)
already does this. The redundant resize is defensive in case
`reproduce_all` is ever called standalone (as the unit tests do). Add a
one-line comment, or remove it if standalone tests are dropped.

### M3 — `MAX_POPULATION` cutoff is order-biased

When the population reaches the cap, `reproduce_all` stops further
reproduction this tick. Since iteration is ascending-id, lower-id agents
have a fitness advantage when the cap engages. Proper fix is a per-cell
density penalty rather than a global cutoff — but that's a Lotka-Volterra
carrying-capacity question and belongs in M5 codex tuning, not M3.

### M4 — Empty species centroid drifts to last-known value

When all members of a species die, the centroid in `species.rs` stays
at its last computed value forever (the slot is preserved for phylogeny
stability). At 2000-pop M2 scale this is fine, but on a multi-million-
tick world it would slowly bloat memory. Address in M9 if it becomes a
problem.

### M5 — Documentation drift between fields and named constants

Sentinel values referenced by literal (e.g. `u32::MAX`) instead of by
named constant (`NO_NEIGHBOR_SPECIES`) in doc comments. The
`sense.rs::SensorRegister.nearest_neighbor_species` doc was patched in
the post-review pass; do a sweep for similar instances when touching
related code.

## Outstanding M1 follow-ups (still open)

See `docs/superpowers/m1-followups.md` for the M1 follow-ups that haven't
been addressed:

- **M1 I1** — Cross-OS determinism not validated in CI
- **M1 I2** — `decide_all` / `integrate_all` allocate a `Vec<u32>` per tick
- **M1 minors** — snapshot-load spatial rebuild, regrowth-at-tick-0, etc.

M1 I1 should be tackled before M4 when the behavior program adds more
transcendental ops. M1 I2 is a clean perf win that benefits every milestone.
