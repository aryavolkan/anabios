# M3 Code Review Follow-ups (deferred to M4 or later)

Captured from the M3 final code review (2026-05-23). M3 was approved with no
Critical issues; the items below are consistency / documentation polish and
should be addressed opportunistically during M4 work.

## Important

### I1 — `Communicator.channel_id` is raw `u8`, inconsistent with typed enums elsewhere

`PheromoneChannel` is a proper enum (variants Alarm/Mate/Trail/Marker), but
`Communicator.channel_id` is a bare `u8` set from `(rng.f32_unit() * 4.0) as u8`.
Mutation operators don't touch it today, but the field is reachable as data
in snapshots and could grow invalid values if a future operator perturbs it.

**Fix:** introduce a `CommChannel` enum analogous to `PheromoneChannel` and
change the field type. Touches `module.rs`, `random_of_type`, and the
snapshot format (one breaking change to GOLDEN hashes).

### I2 — `effective_speed_max` documentation gap

`integrate_all` clamps the sum of all Locomotor `max_speed` values to `[0, 1]`,
so a 3rd Locomotor at 0.4 above an already-saturated pair adds nothing to
speed but still costs upkeep — evolutionarily disfavored, which is the
correct pressure. Just not documented in `module.rs::effective_speed_max`.

**Fix:** add a sentence to the function's doc comment explaining the
clamp-vs-upkeep asymmetry.

### I3 — Asymmetric module aggregation (sum vs max)

`effective_speed_max` sums across Locomotors; `effective_perception_radius`
takes the max across Sensors; `effective_bite_size` takes the max across
Mouths. These are defensible semantics individually but the inconsistency
isn't documented.

**Fix:** add doc comments explaining the per-aggregation choice (legs
add force; you can only see as far as your best eye; you take one
bite at a time).

## Minor

### M1 — All `SensorType` variants currently contribute identically to perception

The spec says "Vision sees plants and other agents; smell/heat/sound are
reserved for later milestones and have no effect in M3". The code currently
treats any `Sensor` module as a perception source regardless of type.

**Fix:** either restrict `effective_perception_radius` to `SensorType::Vision`,
or update the doc comment to acknowledge "any sensor channel currently
contributes; channel-specific effects land later".

### M2 — Use `rng.index(n)` instead of `(rng.f32_unit() * n_f) as usize`

`random_of_type` and `random_any` use the float-cast pattern in 3 places.
`Rng::index(n)` is cleaner and sidesteps any float-to-int subtleties.

### M3 — `random_of_type` shadows `p` closure unnecessarily

The closure `let p = |rng: &mut Rng| rng.f32_unit();` is trivial; inline the
direct call.

### M4 — Add heterogeneous-parent crossover test

`crossover_with_identical_parents_yields_same_length_distribution` doesn't
exercise the `a.len() != b.len()` fallback path. Add a test with `a.len() = 6`,
`b.len() = 3` asserting children land in `[3, 6]`.

### M5 — `find_mate`'s spatial-hash-ordering comment is brittle

The code is correct (defensive lowest-id selection) but the comment talks
about spatial hash traversal order. Future changes to `UniformSpatialHash::query`
must preserve the `other_id < cur` guard — worth a heads-up there too.

### M6 — `GenomeSlot::SpeedMax` is now inert; remove from `fertile_genome` helper

`reproduce.rs::tests::fertile_genome` sets `SpeedMax = 0.4` but that slot no
longer drives motion (Locomotor module does). Remove the setter to avoid
misleading future readers.

### M7 — `behavior.rs::tests::zero_speed_max_yields_zero_velocity` test name stale

The test now asserts wander returns a unit-length vector. Rename to
`wander_returns_unit_vector` or similar.

### M8 — `AgentBuffers.velocity` doc still says "Reserved for M3"

M3 is done and the field still isn't read. Update to "Reserved for M4+" or
implement correlated-wander reading it.

## Still-open M1 follow-ups

See `docs/superpowers/m1-followups.md` for any remaining M1 items. M1 I1
(cross-OS determinism) was closed during M2's CI cycle. M1 I2 (needless
`Vec<u32>` allocations in tick stages) is still open and remains a clean
perf win.

## Still-open M2 follow-ups

See `docs/superpowers/m2-followups.md`. M2 I1 (incremental species count
tracking) was closed as M3 Task 0. M2 I2 (mate-seeking comment drift) and
the minors remain open.
