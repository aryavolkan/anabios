# Perf/Refactor Batch (a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the SpeedMax/DietCarnivory write-only trap and eliminate two categories of per-tick heap allocation, with the golden determinism test as the behavior-equivalence proof.

**Architecture:** Item 1 deletes two dead `TraitOverrides` knobs + their scenario usages (behavior-identical → one deliberate golden refresh). Items 2–3 reuse buffers (`AgentBuffers.scratch_ids` for the per-tick alive-id snapshots; a persistent `counts` on `UniformSpatialHash`) instead of allocating fresh Vecs each tick (byte-for-byte hash-stable). A `tick_bench` before/after quantifies the win.

**Tech Stack:** Rust (`anabios-core`), deterministic seeded RNG, criterion bench.

## Global Constraints

- **Determinism is load-bearing.** Golden hashes pinned in `crates/anabios-core/tests/determinism.rs`. **Item 1 (Task 1) refreshes them once** (a never-read genome slot's serialized value changes). **Items 2–3 (Tasks 2–3) MUST leave the golden hash byte-identical** to Task 1's refreshed values — that identity IS the proof they're behavior-preserving.
- Iteration order must stay **ascending agent id** everywhere (unchanged by these edits).
- New scratch buffers must be excluded from the state hash: `AgentBuffers` and `World` serialize into `bincode` for `state_hash`, so scratch fields carry `#[serde(skip)]` (matching the existing pattern on `World.spatial`, `World.eval_stack`, etc.).
- Keep `GenomeSlot::SpeedMax` and `GenomeSlot::DietCarnivory` enum variants — indices are load-bearing (serde layout) and `#[cfg(test)]` helpers use them. Only the `TraitOverrides` fields and TOML lines are removed.
- CI gate — stable toolchain: `rustup run stable cargo fmt --all --check`; `... clippy --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" ... doc --workspace --no-deps --document-private-items`; `... test --workspace --lib --tests`. Commit `cargo fmt` output.
- Baseline `tick_bench` (for the final comparison): **1k ≈ 0.95 ms/tick, 10k ≈ 6.15 ms/tick**.

---

### Task 1: Remove the SpeedMax/DietCarnivory trap + golden refresh

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs` (`TraitOverrides` struct + `apply`)
- Modify: `scenarios/minimal.toml`, `scenarios/divergent.toml`, `scenarios/gene-culture-alarm.toml`, `scenarios/predator-prey.toml`
- Modify: `crates/anabios-core/tests/determinism.rs` (refreshed hashes)

**Interfaces:**
- Produces: `TraitOverrides` without `speed_max`/`diet_carnivory` fields.

- [ ] **Step 1: Delete the two struct fields**

In `crates/anabios-core/src/scenario.rs`, remove these two lines from `pub struct TraitOverrides` (lines 39 and 42):

```rust
    pub speed_max: Option<f32>,
```
```rust
    pub diet_carnivory: Option<f32>,
```

- [ ] **Step 2: Delete the two `apply` blocks**

In `TraitOverrides::apply`, remove both blocks:

```rust
        if let Some(v) = self.speed_max {
            g.set(GenomeSlot::SpeedMax, v);
        }
```
```rust
        if let Some(v) = self.diet_carnivory {
            g.set(GenomeSlot::DietCarnivory, v);
        }
```

- [ ] **Step 3: Strip the ignored lines from the 4 scenarios**

Delete these exact lines:
- `scenarios/minimal.toml`: `speed_max = 0.4` and `diet_carnivory = 0.0`
- `scenarios/divergent.toml`: `speed_max = 0.1`, `diet_carnivory = 0.0`, `speed_max = 0.95`, `diet_carnivory = 0.0` (all four)
- `scenarios/gene-culture-alarm.toml`: `diet_carnivory = 1.0`
- `scenarios/predator-prey.toml`: `diet_carnivory = 1.0`

- [ ] **Step 4: Build + confirm the golden test now FAILS (expected)**

Run: `rustup run stable cargo build -p anabios-core`
Expected: builds clean (no references to the removed fields remain — grep to confirm: `grep -rn "speed_max\|diet_carnivory" crates/anabios-core/src scenarios/` returns nothing).

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: FAIL — hashes changed (minimal's genome slots 25/27 moved 0.4/0.0 → neutral 0.5). This is expected and intended.

- [ ] **Step 5: Refresh the golden hashes**

Run: `UPDATE_HASHES=1 rustup run stable cargo test -p anabios-core --test determinism -- --nocapture`
Copy the printed `(tick, 0x...)` triple into `crates/anabios-core/tests/determinism.rs` `GOLDEN`. Update the surrounding comment: "Refreshed 2026-07-15: removed the never-read SpeedMax/DietCarnivory TraitOverrides knobs (behavior identical; only those serialized-but-unread genome slots changed value)."

- [ ] **Step 6: Verify refreshed golden + full core suite**

Run: `rustup run stable cargo test -p anabios-core --test determinism` (twice — must PASS both, deterministic).
Run: `rustup run stable cargo test -p anabios-core --lib --tests`
Expected: PASS. (If any behavioral integration test fails, STOP — that would mean a slot WAS read after all, contradicting the analysis; do not proceed.)

- [ ] **Step 7: Format, lint, commit — RECORD the refreshed hashes for Tasks 2–3**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/scenario.rs scenarios/*.toml crates/anabios-core/tests/determinism.rs
git commit -m "fix(core): remove never-read SpeedMax/DietCarnivory TraitOverrides trap

Behavior identical (neither slot is read; speed/diet come from modules);
golden refreshed once for the changed serialized slot values.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```
Note the refreshed `GOLDEN` triple in the commit message body — Tasks 2–3 must hold it byte-identical.

---

### Task 2: Reuse an `AgentBuffers.scratch_ids` buffer for per-tick alive-id snapshots

Replace ~7 per-tick `iter_alive().collect()` heap allocations with a reused buffer via `std::mem::take` → refill → restore. Hash-safe: identical ids, identical order.

**Files:**
- Modify: `crates/anabios-core/src/agent.rs` (`AgentBuffers` struct — add field)
- Modify: `crates/anabios-core/src/tick.rs` (`decide_all`), `integrate.rs` (`integrate_all`), `interact.rs` (`interact_all`), `reproduce.rs` (`reproduce_all`), `culture.rs` (`culture_step`), `age.rs` (`age_and_starve`), `species.rs` (`species_step`)

**Interfaces:**
- Produces: `AgentBuffers.scratch_ids: Vec<u32>` (`#[serde(skip)]`, `pub`).
- The **restore pattern** (applied at each site): take the buffer, `clear()`, `extend(iter_alive())`, iterate by reference, then assign it back.

- [ ] **Step 1: Add the scratch field to `AgentBuffers`**

In `crates/anabios-core/src/agent.rs`, inside `pub struct AgentBuffers { … }`, add (order among fields doesn't matter; `#[serde(skip)]` + `Default` derive means it auto-inits to empty and is hash-excluded):

```rust
    /// Reusable scratch buffer for per-tick "snapshot the alive ids" loops.
    /// `#[serde(skip)]` — never part of the deterministic state hash.
    #[serde(skip)]
    pub scratch_ids: Vec<u32>,
```

- [ ] **Step 2: Convert `decide_all` (`tick.rs`)**

Replace:
```rust
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
```
with:
```rust
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
    for &id in &alive_ids {
        let i = id as usize;
```
and add, immediately after the `for` loop closes (before `decide_all` returns):
```rust
    world.agents.scratch_ids = alive_ids;
```

- [ ] **Step 3: Convert `integrate_all` (`integrate.rs`)**

Replace:
```rust
    for id in agents.iter_alive().collect::<Vec<_>>() {
        let i = id as usize;
```
with:
```rust
    let mut ids = std::mem::take(&mut agents.scratch_ids);
    ids.clear();
    ids.extend(agents.iter_alive());
    for &id in &ids {
        let i = id as usize;
```
and add after the loop closes (before the function returns):
```rust
    agents.scratch_ids = ids;
```

- [ ] **Step 4: Convert `interact_all` (`interact.rs`)**

Replace:
```rust
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
```
with:
```rust
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
```
The 5 sub-pass calls (`feed_pass(world, &alive_ids)`, `combat_pass`, `scavenge_pass`, `deposit_pass`, `share_pass`) stay unchanged (`&alive_ids` coerces to `&[u32]`). After the last sub-pass call, add:
```rust
    world.agents.scratch_ids = alive_ids;
```

- [ ] **Step 5: Convert `reproduce_all` (`reproduce.rs`)**

Replace:
```rust
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
```
with:
```rust
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
```
The existing loop `for &a_id in &alive_ids {` is already by-reference — unchanged. Reproduction grows the alive set via `spawn`, but `alive_ids` is an owned snapshot taken *before* the loop, so newborns are correctly excluded (same as today). After the loop closes, add:
```rust
    world.agents.scratch_ids = alive_ids;
```

- [ ] **Step 6: Convert `culture_step` (`culture.rs`)**

Replace:
```rust
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
```
with:
```rust
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
```
The loop `for &id in &alive_ids {` is already by-reference — unchanged. After the loop closes, add:
```rust
    world.agents.scratch_ids = alive_ids;
```

- [ ] **Step 7: Convert `age_and_starve` (`age.rs`)**

Replace:
```rust
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
```
with:
```rust
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
    for &id in &alive_ids {
```
After the loop closes, add:
```rust
    world.agents.scratch_ids = alive_ids;
```

- [ ] **Step 8: Convert `species_step` (`species.rs`)**

Replace:
```rust
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
```
with:
```rust
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
```
The loop `for id in &alive_ids {` is already by-reference — unchanged. After the last use of `alive_ids` in the function, add:
```rust
    world.agents.scratch_ids = alive_ids;
```
(If `species_step` has an early `return` between the take and the restore, add the same `world.agents.scratch_ids = alive_ids;` before that return, or restructure so the restore always runs. Not restoring is not a correctness bug — it only forgoes reuse for that call — but restore-on-all-paths is preferred.)

- [ ] **Step 9: Build + verify golden is UNCHANGED (the proof)**

Run: `rustup run stable cargo build -p anabios-core`
Expected: clean.

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS against Task 1's refreshed hashes, **byte-identical**. If it FAILS, the refactor changed behavior (likely an iteration-order or snapshot-timing mistake) — fix it; do NOT refresh the golden in this task.

Run: `rustup run stable cargo test -p anabios-core --lib --tests`
Expected: PASS.

- [ ] **Step 10: Format, lint, commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/agent.rs crates/anabios-core/src/tick.rs crates/anabios-core/src/integrate.rs crates/anabios-core/src/interact.rs crates/anabios-core/src/reproduce.rs crates/anabios-core/src/culture.rs crates/anabios-core/src/age.rs crates/anabios-core/src/species.rs
git commit -m "perf(core): reuse AgentBuffers.scratch_ids for per-tick alive-id snapshots

Replaces ~7 per-tick Vec<u32> heap allocations with one reused buffer.
Hash-safe: golden unchanged.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Reuse the spatial-hash counts buffer

Stop allocating `vec![0u32; 4096]` in every `UniformSpatialHash::rebuild`.

**Files:**
- Modify: `crates/anabios-core/src/spatial.rs`

**Interfaces:**
- Produces: `UniformSpatialHash.counts: Vec<u32>` (private field, reused across rebuilds).

- [ ] **Step 1: Add the field**

In `crates/anabios-core/src/spatial.rs`, add a field to `pub struct UniformSpatialHash`:

```rust
    /// Reusable per-cell count buffer for `rebuild` (avoids a per-tick alloc).
    counts: Vec<u32>,
```
and initialize it in `new()`:

```rust
    pub fn new() -> Self {
        let total_cells = HASH_RES * HASH_RES;
        Self {
            bucket_offsets: vec![0; total_cells],
            bucket_lens: vec![0; total_cells],
            flat: Vec::new(),
            counts: vec![0; total_cells],
        }
    }
```
(`UniformSpatialHash` derives `Clone` only — no `Default` — so `new()` is the sole constructor; no other init site.)

- [ ] **Step 2: Use the reused buffer in `rebuild`**

In `rebuild`, replace:
```rust
        let total_cells = HASH_RES * HASH_RES;
        // Phase 1: count agents per cell.
        let mut counts = vec![0_u32; total_cells];
```
with:
```rust
        let total_cells = HASH_RES * HASH_RES;
        // Phase 1: count agents per cell (reused buffer, no per-tick alloc).
        self.counts.clear();
        self.counts.resize(total_cells, 0);
```
Then, in the same function, replace the remaining bare `counts[...]` references with `self.counts[...]`:
- Phase 1 scatter: `counts[cell] += 1;` → `self.counts[cell] += 1;`
- Phase 2 prefix-sum: `total += counts[i];` → `total += self.counts[i];`

(`clear()` + `resize(total_cells, 0)` reuses the existing capacity — no realloc after warm-up — and zero-fills, so it is robust even though `new()` already sizes it.)

- [ ] **Step 3: Build + verify golden UNCHANGED + spatial tests**

Run: `rustup run stable cargo build -p anabios-core`
Expected: clean (watch for `&self` vs `&mut self` — `rebuild` is already `&mut self`, so `self.counts` writes are fine).

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS, byte-identical to Task 1's hashes.

Run: `rustup run stable cargo test -p anabios-core --lib spatial::`
Expected: PASS (spatial unit tests unaffected).

- [ ] **Step 4: Format, lint, commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/spatial.rs
git commit -m "perf(core): reuse spatial-hash counts buffer across rebuilds

Drops a per-tick vec![0u32; 4096] allocation. Hash-safe: golden unchanged.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Full verification + benchmark delta

**Files:** none (verification only; benchmark numbers go in the commit/PR).

- [ ] **Step 1: Full CI gate (stable toolchain)**

```bash
rustup run stable cargo fmt --all --check
rustup run stable cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc --workspace --no-deps --document-private-items
rustup run stable cargo test --workspace --lib --tests
```
Expected: all PASS.

- [ ] **Step 2: Determinism reproducibility**

Run: `rustup run stable cargo test -p anabios-core --test determinism` (twice)
Expected: PASS both — the refreshed hashes from Task 1, held byte-stable through Tasks 2–3.

- [ ] **Step 3: Benchmark the win**

Run: `rustup run stable cargo bench -p anabios-core --bench tick_bench 2>&1 | grep -E "tick/|time:"`
Expected: completes; record the `tick/1000` and `tick/10000` medians. Compare to baseline (1k ≈ 0.95 ms, 10k ≈ 6.15 ms). Note the delta (even a few % at 10k confirms the allocation removal; criterion's own before/after `change:` line, if present, is the cleanest signal).

- [ ] **Step 4: Commit the recorded numbers (if any doc/notes change; else skip)**

```bash
git commit --allow-empty -m "chore(core): batch (a) verification — bench delta recorded

before: 1k ~0.95ms, 10k ~6.15ms; after: <fill from Step 3>.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- Item 1 remove trap (TraitOverrides fields + apply + 4 scenarios, keep enum variants) + one golden refresh → Task 1. ✅
- Item 2 reuse buffer for the ~6–7 `iter_alive().collect()` sites → Task 2 (all 7: decide/integrate/interact/reproduce/culture/age/species). ✅
- Item 3 spatial counts buffer reuse → Task 3. ✅
- Determinism: refresh once (Task 1), byte-stable after (Tasks 2–3 Step 9/Step 3). ✅
- Scratch fields `#[serde(skip)]` / hash-excluded → Task 2 Step 1 (`AgentBuffers`), Task 3 (private field on the `#[serde(skip)]` `World.spatial`). ✅
- Benchmark before/after → Task 4 Step 3. ✅ CI gate → Task 4 Step 1. ✅
- Out-of-scope items (codex fusion, godot, wiring speed/diet, scavenge) → not present. ✅

**Placeholder scan:** No TBD/TODO. Task 4 Step 4 is conditional-empty (explicitly "else skip"), not a placeholder.

**Type consistency:** `AgentBuffers.scratch_ids: Vec<u32>` used identically across Tasks 2's 7 sites (`world.agents.scratch_ids` / `agents.scratch_ids`). `UniformSpatialHash.counts: Vec<u32>` consistent within Task 3. The take/refill/restore pattern is identical at every site; loop headers changed to by-reference (`for &id in &alive_ids`) only where they previously consumed by value (decide_all, integrate_all, age_and_starve). Golden-refresh workflow (`UPDATE_HASHES=1`) matches the determinism test's documented mechanism.
