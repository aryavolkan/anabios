# Trade-Economy Redesign Implementation Plan

> **⚠️ SUPERSEDED — WRONG DIAGNOSIS (corrected 2026-08-02). DO NOT IMPLEMENT AS WRITTEN.**
> This plan assumes the freeze is a **demand-satiation "absorbing state"**
> (`want→0` once baskets saturate at `STOCK_TARGET`) and proposes **perishability**
> / **non-satiating `want`**. Direct measurement disproved this: at the freeze,
> agent inventories are **empty** (not saturated), every agent's `want` is maxed,
> and goods remain available in the biome — the freeze is a **supply-side
> starvation**, and perishability was measured to make it *worse*. See
> [`../specs/2026-08-02-trade-freeze-diagnosis.md`](../specs/2026-08-02-trade-freeze-diagnosis.md)
> for the evidence and the corrected (supply-side) direction. The Task 1 flag
> mechanics below (opt-in flag + `FORMAT_VERSION` bump + golden rehash) remain a
> useful pattern; Tasks 2–4's *mechanism* (perishability) does not. A future
> redesign should be re-planned around the supply side before execution.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the trade "absorbing state" so cross-species exchange stays alive over long runs, and make trade volume observable in sweeps — without breaking determinism.

**Architecture:** The freeze is structural: swaps only *redistribute* goods, and `want(inv,k)=(STOCK_TARGET-inv[k]).max(0)` saturates to 0 once an agent holds `STOCK_TARGET=2.0` of a good, so once every reachable pair sits at the target basket, `pick_swap` returns `None` forever. Fix by making valuation **non-satiating** — add a small per-tick **perishability decay** to inventory so `want` never latches at 0 — and expose a windowed trade-rate metric. Perishability is an opt-in tuning knob so existing goldens can stay stable behind the flag default.

**Tech Stack:** Rust (`anabios-core`: `resource.rs`, `interact.rs`, `world.rs`, `scenario.rs`, `tick.rs`; `codex/event.rs`), `anabios-headless/src/score.rs`.

## Global Constraints

- **Determinism contract:** bit-identical per seed. Any change to a *default-on* code path rehashes the goldens (`tests/determinism.rs`) — regenerate with `UPDATE_HASHES=1` in the same PR. Prefer gating new behavior behind a config knob defaulting to the current behavior so goldens do NOT move until intended.
- Trade math must draw **zero RNG** (it currently does) — keep it RNG-free so ordering stays deterministic.
- Swaps must stay **conservative in aggregate** except where perishability explicitly removes goods; decay is the only new sink and must be deterministic (no RNG).
- Snapshot: any new persistent `World`/agent field bumps `FORMAT_VERSION` (`snapshot.rs:102`, currently 23) with a changelog line.

## Background (grounded)

- `pick_swap` — `interact.rs:433-461`. Requires `a_gain>0 && b_gain>0` where `a_gain = want(inv_a,recv)-want(inv_a,give)`. Absorbing state = every pair at `want=0` for tradeable goods.
- `want` — `resource.rs:129-131`; `STOCK_TARGET=2.0` (`resource.rs:68`).
- `trade_pass` — `interact.rs:467-514`; increments `world.total_trades` (`interact.rs:492`); pushes `ResourceTraded` **once** via the `first_cross_species_trade` latch (`codex/mod.rs:127`, `interact.rs:502-512`).
- Inflow = `harvest_pass` (`interact.rs:337`); sink = `consume_materials` during invention learning (`invention/mod.rs:415`).
- Observability: `total_trades: u64` (`world.rs:281`, `#[serde(skip)]`); per-tick `trade_routes` (`world.rs:257`, cleared each tick). Sweep CSV has only the latched `resource_traded` 0/1 and `material_learning` count — neither measures volume.
- Guard test today: `geographic_trade_turnover_is_ongoing` (`tests/trade.rs:149`) sums `trade_routes` early(0..400) vs late(400..800), asserts `late > early/4`.

## File Structure

- `crates/anabios-core/src/resource.rs` — add `PERISH_RATE` const + a `perish_step`/decay helper; keep `want` but make it read a non-satiated value once decay is on. **Modify.**
- `crates/anabios-core/src/interact.rs` — call the decay pass in `interact_all`; add a windowed trade counter update. **Modify.**
- `crates/anabios-core/src/world.rs` — add `perishable_goods: bool` flag + a `trade_rate_window: [u32; W]` ring (or a simpler `trades_last_window: u32`). **Modify.**
- `crates/anabios-core/src/scenario.rs` — parse `perishable_goods` and wire into `World`. **Modify.**
- `crates/anabios-headless/src/score.rs` — export a trade-volume proxy so the freeze is visible in sweeps (de-latch is out of scope; add `total_trades` as a CSV column instead). **Modify.**
- `crates/anabios-core/tests/trade.rs` — tighten `geographic_trade_turnover_is_ongoing` under the new flag; add a perishability unit/integration test. **Modify.**
- `scenarios/biome-trade.toml`, `scenarios/geographic-trade.toml` — add `perishable_goods = true` variants (or new `scenarios/perishable-trade.toml`). **Modify/Create.**

---

## Task 1: Add the `perishable_goods` flag (off by default, goldens unchanged)

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs` (`Scenario` struct ~:85 block; `instantiate` ~:439), `crates/anabios-core/src/world.rs` (field + ctor default)
- Test: `crates/anabios-core/tests/trade.rs`

**Interfaces:**
- Produces: `World.perishable_goods: bool` (default false), TOML key `perishable_goods`.

- [ ] **Step 1: Write the failing parse/wire test** (mirror `resources_flag_parses_and_wires_into_world`, `scenario.rs:725`):

```rust
#[test]
fn perishable_goods_flag_parses_and_wires() {
    let toml = "world_size = 64\n[[agents]]\narchetype = \"grazer\"\ncount = 4\nresources_enabled = true\nperishable_goods = true\n";
    let w = Scenario::parse_toml(toml).unwrap().instantiate();
    assert!(w.perishable_goods);
}
```

- [ ] **Step 2: Run — expect FAIL** (`no field perishable_goods`). Run: `cargo test -p anabios-core --test trade perishable_goods_flag_parses_and_wires`
- [ ] **Step 3: Add the field + wiring.** In `scenario.rs` add `#[serde(default)] pub perishable_goods: bool,` (mirror `resources_enabled` at `:85`); in `instantiate` add `w.perishable_goods = self.perishable_goods;` next to `w.resources_enabled` (`:439`). In `world.rs` add `#[serde(default)] pub perishable_goods: bool,` and default `false` in the ctor.
- [ ] **Step 4: Run — expect PASS.**
- [ ] **Step 5: Confirm goldens unmoved.** Run: `cargo test -p anabios-core --test determinism` — must PASS unchanged (flag defaults off, `#[serde(default)]` keeps layout compatible only if this is a NEW field appended; bump `FORMAT_VERSION` to 24 with a changelog line in `snapshot.rs` and update the snapshot version tests).
- [ ] **Step 6: Commit.**

```bash
git add crates/anabios-core/src/scenario.rs crates/anabios-core/src/world.rs crates/anabios-core/src/snapshot.rs crates/anabios-core/tests/trade.rs
git commit -m "feat(trade): add opt-in perishable_goods flag (default off)"
```

---

## Task 2: Perishability decay pass (the fix)

Add a deterministic per-tick decay to agent inventories so held goods slowly return below `STOCK_TARGET`, keeping `want > 0` and re-arming `pick_swap`. Only runs when `perishable_goods` is on.

**Files:**
- Modify: `crates/anabios-core/src/resource.rs` (const + `perish_step`), `crates/anabios-core/src/interact.rs` (call site in `interact_all` ~:28-56)
- Test: `crates/anabios-core/src/resource.rs` inline `#[cfg(test)]`, `crates/anabios-core/tests/trade.rs`

**Interfaces:**
- Consumes: `World { agents.inventory: Vec<[f32; GOOD_COUNT]>, resources_enabled, perishable_goods }`.
- Produces: `pub fn perish_step(world: &mut World)` — multiplies each inventory slot by `(1.0 - PERISH_RATE)`, RNG-free, no-op when `!perishable_goods`. `pub const PERISH_RATE: f32`.

- [ ] **Step 1: Write the failing unit test** in `resource.rs` tests:

```rust
#[test]
fn perish_step_decays_inventory_when_enabled() {
    let mut w = /* minimal World with resources_enabled + perishable_goods, one agent holding [4.0,0,0,0] */;
    let before = w.agents.inventory[0][0];
    crate::resource::perish_step(&mut w);
    assert!(w.agents.inventory[0][0] < before);
    // and is a no-op with the flag off:
    w.perishable_goods = false;
    let held = w.agents.inventory[0][0];
    crate::resource::perish_step(&mut w);
    assert_eq!(w.agents.inventory[0][0], held);
}
```

- [ ] **Step 2: Run — expect FAIL** (`perish_step` undefined).
- [ ] **Step 3: Implement.** In `resource.rs`:

```rust
/// Per-tick fractional spoilage of held goods. Small enough that a home-good
/// surplus lingers, large enough that off-goods drift back below STOCK_TARGET
/// so `want` never latches at 0 (the trade absorbing state). RNG-free.
pub const PERISH_RATE: f32 = 0.01;

pub fn perish_step(world: &mut World) {
    if !world.resources_enabled || !world.perishable_goods {
        return;
    }
    let keep = 1.0 - PERISH_RATE;
    for inv in world.agents.inventory.iter_mut() {
        for slot in inv.iter_mut() {
            *slot *= keep;
        }
    }
}
```

Call it in `interact_all` (`interact.rs`) right after `harvest_pass` / before `trade_pass` so a freshly harvested surplus is available to trade this tick but decays each tick.

- [ ] **Step 4: Run — expect PASS.**
- [ ] **Step 5: Regenerate goldens for any perishable scenario only.** The default-off scenarios must be unchanged; run `cargo test -p anabios-core --test determinism` — PASS unchanged.
- [ ] **Step 6: Commit.**

```bash
git add crates/anabios-core/src/resource.rs crates/anabios-core/src/interact.rs
git commit -m "feat(trade): perishability decay re-arms want() so trade never freezes"
```

---

## Task 3: Prove the freeze is gone (integration test)

**Files:**
- Modify: `crates/anabios-core/tests/trade.rs`
- Create: `scenarios/perishable-trade.toml` (a `geographic-trade` clone with `perishable_goods = true`)

**Interfaces:**
- Consumes: `World.trade_routes` (per-tick swap records), `World.total_trades`.

- [ ] **Step 1: Create the scenario.** Copy `scenarios/geographic-trade.toml` to `scenarios/perishable-trade.toml`, add `perishable_goods = true` to the `[world]`/top-level, keep seed pinned.
- [ ] **Step 2: Write the failing test** — a stronger version of `geographic_trade_turnover_is_ongoing`:

```rust
#[test]
fn perishable_trade_stays_alive_long_term() {
    const P: &str = include_str!("../../../scenarios/perishable-trade.toml");
    let mut w = Scenario::parse_toml(P).unwrap().instantiate();
    assert!(w.perishable_goods);
    let mut early = 0u64;
    let mut late = 0u64;
    for t in 0..2000u64 {
        step(&mut w);
        let n = w.trade_routes.len() as u64;
        if t < 500 { early += n; } else if t >= 1500 { late += n; }
    }
    // Non-satiating demand: late-window volume must NOT collapse toward zero.
    assert!(early > 0, "no early trade");
    assert!(late as f64 > early as f64 * 0.5, "trade decayed: early={early} late={late}");
}
```

- [ ] **Step 3: Run — expect PASS** (if it fails, the ratio reveals decay is too weak/strong; tune `PERISH_RATE` — higher keeps more trade alive but drains the economy faster). Run: `cargo test -p anabios-core --test trade perishable_trade_stays_alive_long_term`.
- [ ] **Step 4: Add golden hashes for the new scenario** if you want it pinned (optional): add to `determinism.rs` GOLDEN via a second scenario, or leave it covered by the trade test only.
- [ ] **Step 5: Commit.**

```bash
git add crates/anabios-core/tests/trade.rs scenarios/perishable-trade.toml
git commit -m "test(trade): perishable economy sustains late-run trade volume"
```

---

## Task 4: Make trade volume visible in sweeps

The CSV can't currently see the freeze (`resource_traded` is a latched 0/1). Export `total_trades` as a column.

**Files:**
- Modify: `crates/anabios-headless/src/sweep.rs` (`RunSummary` + `run_one` capture `world.total_trades`; `write_summary_csv` adds a `total_trades` column), `crates/anabios-headless/src/score.rs` if the column list lives there.
- Test: `crates/anabios-headless/tests/sweep_csv.rs` (assert the column exists).

**Interfaces:**
- Produces: a `total_trades` column in `summary.csv` (u64). Coordinate ordering with the `novel_types` column from the scorecard-sweeps plan — pick one canonical final-column order and update both plans' tests.

- [ ] **Step 1: Write the failing CSV test** asserting the header contains `total_trades`.
- [ ] **Step 2: Run — expect FAIL.**
- [ ] **Step 3: Add `total_trades: u64` to `RunSummary`, set it in `run_one` from the final `world.total_trades`, and emit the column** in `write_summary_csv`.
- [ ] **Step 4: Run — expect PASS.**
- [ ] **Step 5: Commit.**

```bash
git add crates/anabios-headless/src/sweep.rs crates/anabios-headless/tests/sweep_csv.rs
git commit -m "feat(headless): export total_trades so sweeps can see the economy freeze"
```

---

## Testing Plan (summary)

| Level | What | Where |
|---|---|---|
| Unit | `perish_step` decays / is inert when off | `resource.rs` tests (Task 2) |
| Unit | `pick_swap` mutual-benefit (existing, unchanged) | `interact.rs:562-698` |
| Integration | perishable economy sustains late trade | `tests/trade.rs` (Task 3) |
| Integration | default-off scenarios byte-identical | `tests/trade.rs` existing + `determinism.rs` |
| Determinism | goldens unchanged (flag off); new scenario deterministic | `tests/determinism.rs` |
| Observability | `total_trades` in CSV | `tests/sweep_csv.rs` (Task 4) |
| Sweep evidence | archive-weighted sweep of `perishable-trade` shows sustained `total_trades` vs frozen `geographic-trade` | manual, via the scorecard-sweeps runbook |

**Done when:** `perishable-trade.toml` sustains ≥50% of early trade volume in the late window (Task 3 passes), the default scenarios' goldens are unchanged, and `total_trades` is a sweep CSV column so the freeze-vs-alive contrast is measurable.

## Risks / open questions

- **Tuning `PERISH_RATE`:** too high drains the economy (agents can't accumulate a trade basket); too low re-freezes. Sweep both scenarios and compare `total_trades` trajectories before settling.
- **Alternative fix** (if perishability feels wrong for the domain): make `want` non-satiating (e.g. logarithmic marginal utility with no hard cap) instead of decaying inventory. Same test suite applies; swap Task 2's mechanism. Keep whichever is behind the flag.
- **Coupling to the invention sink:** perishability changes how much surplus is available for `consume_materials`. Re-run `tests/inventions.rs:825` (MaterialLearning) on a perishable scenario to confirm learning still funds.
