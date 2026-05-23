# M1 Code Review Follow-ups (deferred to M2 or later)

Captured from the final M1 code review (2026-05-23). None of these block M2 work from beginning, but they should be resolved before M2 ships.

## Important

### I1 — Cross-OS determinism is not validated in CI

The simulation calls `f32::sin`, `f32::cos`, `f32::ln` (in `Rng::gaussian`, `decide` wander, and `Scenario` cluster placement). These transcendentals are not bit-identical across glibc / musl / macOS libm / Windows ucrt, so the pinned golden-tick hash in `crates/anabios-core/tests/determinism.rs` would diverge between Linux and macOS today. The `rust` CI job's per-OS `cargo test` runs would both pass, but each platform would be pinning a *different* hash. The `determinism smoke` job only runs on `ubuntu-latest`, so divergence wouldn't be caught.

**Options:**

- (a) Run the determinism job on the full OS matrix and assert identical `state_hash` across platforms.
- (b) Replace `sin`/`cos`/`ln` with the `libm` crate's pure-Rust implementations, which guarantee cross-platform identity.
- (c) Explicitly document that determinism is only guaranteed within a single OS family for M1; defer (b) until M4 when the behavior program needs nonlinear ops anyway.

Recommended path: (c) for M1 — add a note to the design spec — and tackle (b) as part of the M4 behavior-program work.

### I2 — `decide_all` and `integrate_all` allocate a `Vec<u32>` of alive ids per tick

At 10k agents this is a 40 KB allocation per stage per tick — wasted work. `interact_all` and `age_and_starve` legitimately need the snapshot because they mutate the alive set; `decide_all` and `integrate_all` do not.

Fix: iterate `agents.iter_alive()` directly in both stages. In `decide_all`, the borrow of `world.agents` needs to be released before mutating `world.desired_velocity` and drawing from `world.rng`; cache the count up front or destructure the world to split disjoint borrows.

## Minor

### M1 — Snapshot load leaves `spatial` empty

A freshly-loaded `World` from `snapshot::load_from_bytes` has an empty `spatial` hash. Calling `sense_all()` before the next `step()` returns a quiet wrong answer (no neighbors found). Either document this constraint on `World` or rebuild scratch buffers inside `load_from_bytes`.

### M2 — Regrowth fires at tick 0

`step()` checks `world.tick.is_multiple_of(BIOME_STEP_INTERVAL)` *before* incrementing, so the first regrowth happens at tick 0. For the seeded scenario it's a no-op, but consider whether you want the first regrowth at tick 10 instead.

### M3 — `Genome` Deserialize does not clamp values into `[0, 1]`

A hand-edited or corrupted snapshot could load out-of-range traits. Add a defensive `.clamp(0.0, 1.0)` in `visit_seq` before assigning each element.

### M4 — Dead `std_rng` feature on the workspace `rand` dep

`rand = { ..., features = ["std", "std_rng"] }` but the codebase never uses ChaCha12. Drop `std_rng` to trim compile time.

### M5 — Tunable constants are scattered across modules

`MUTATION_SIGMA_MAX`, `BITE_MAX`, `MOVE_ENERGY_COST`, `LIFESPAN_MAX_TICKS`, etc. live in their own modules. Before M3+ multiplies them, centralize into a `consts.rs` or a `SimConfig` struct so sensitivity sweeps can vary them cleanly.

### M6 — Missing inline justification for `#[allow(clippy::needless_range_loop)]` in `spatial.rs`

Other `#[allow(...)]` sites have rationale comments; add one to `spatial.rs:43` ("Two-pass prefix-sum needs explicit index access into parallel arrays.").

### M7 — `anabios-headless info` uses `{:#?}` Debug formatting

Functional but ugly. Replace with structured (yaml/table) output once scenarios get richer.
