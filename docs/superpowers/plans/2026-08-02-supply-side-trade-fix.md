# Supply-Side Trade Fix (Conserve Goods on Death) Implementation Plan

> **⚠️ PARTIALLY EXECUTED — the mechanism does NOT fix the freeze.** Tasks 1–2
> (the `conserve_goods_on_death` flag + `conserve_goods_step`) were implemented,
> reviewed, and kept as a correct opt-in mechanism. Task 3's proof **failed**:
> conservation conserves goods but concentrates them on a few hoarders, so trade
> still freezes; seven mechanisms were ultimately eliminated and the freeze is
> intrinsic to the bilateral-barter primitive (see
> [`../specs/2026-08-02-trade-freeze-diagnosis.md`](../specs/2026-08-02-trade-freeze-diagnosis.md)
> Updates 1–2). Tasks 3–4 were NOT completed. Do not treat this plan as a
> freeze fix; the `conserve_goods_on_death` mechanism ships as a standalone
> feature only.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the biome trade economy from freezing by making goods conservative — when an agent dies, its trade-goods inventory transfers to the nearest living agent instead of vanishing.

**Architecture:** An opt-in `World.conserve_goods_on_death` flag. `kill()` snapshots each dying agent's `(position, inventory)` into a `#[serde(skip)]` scratch buffer (only when it holds goods). A new `conserve_goods_step` tick stage redistributes each snapshot to the nearest living agent (deterministic, RNG-free) and clears the buffer. Off by default → existing scenarios unchanged (behavior); the new serialized flag grows the `World` layout, so goldens rehash once.

**Tech Stack:** Rust (`anabios-core`: `world.rs`, `scenario.rs`, `agent.rs`, `resource.rs`, `tick.rs`, `snapshot.rs`; tests in `tests/trade.rs`), `anabios-headless` (`sweep.rs`). Design: [`../specs/2026-08-02-supply-side-trade-design.md`](../specs/2026-08-02-supply-side-trade-design.md). Diagnosis: [`../specs/2026-08-02-trade-freeze-diagnosis.md`](../specs/2026-08-02-trade-freeze-diagnosis.md).

## Global Constraints

- **Determinism is the contract.** Bit-identical per seed. Trade/conservation math draws **zero RNG**. Iteration order stays deterministic (ascending id / lowest-index tie-break).
- **Opt-in flag.** `conserve_goods_on_death` defaults `false`; every existing scenario is unaffected in behavior.
- **Adding a serialized `World` field rehashes goldens.** `state_hash = fnv1a_64(bincode::serialize(world))` hashes the whole `World`; bincode is not self-describing, so a new serialized field changes `state_hash` for every scenario even flag-off. That is expected: bump `FORMAT_VERSION` and regenerate the **three** golden tables (`tests/determinism.rs`, `tests/inventions.rs`, `tests/cognition.rs`) via `UPDATE_HASHES=1` in the same task. A rehash must move only hash *values*, never test structure — if a golden test's non-hash assertions fail, that is a real regression, STOP.
- **Scratch buffers are `#[serde(skip)]`** and never affect `state_hash` (precedent: `AgentBuffers.scratch_ids`, `agent.rs:98`).
- **Conservation, not creation/destruction.** Goods are transferred, never minted or clamped away.
- **Stage explicit paths only** — never `git add -A`/`.`.

## File Structure

- `crates/anabios-core/src/world.rs` — **Modify**: add `conserve_goods_on_death: bool` field + ctor default `false`.
- `crates/anabios-core/src/scenario.rs` — **Modify**: add TOML key `conserve_goods_on_death` + wire into `World` in `instantiate`.
- `crates/anabios-core/src/snapshot.rs` — **Modify**: `FORMAT_VERSION` 23→24 + changelog line.
- `crates/anabios-core/src/agent.rs` — **Modify**: add `#[serde(skip)] deaths_scratch: Vec<(Vec2, [f32; GOOD_COUNT])>`; `kill()` snapshots a dying agent's `(position, inventory)` into it when the inventory is non-zero.
- `crates/anabios-core/src/resource.rs` — **Modify**: add `pub fn conserve_goods_step(world: &mut World)` — redistribute snapshots to the nearest living agent; clears the buffer.
- `crates/anabios-core/src/tick.rs` — **Modify**: call `conserve_goods_step` after the death stages.
- `crates/anabios-core/tests/trade.rs` — **Modify**: flag parse/wire test, conservation unit test, long-horizon baseline-contrast test.
- `scenarios/conserve-trade.toml` — **Create**: `biome-trade` clone + `conserve_goods_on_death = true`.
- `crates/anabios-headless/src/sweep.rs` — **Modify**: add `total_trades` column to `RunSummary` + `write_summary_csv`.
- `crates/anabios-core/tests/{determinism,inventions,cognition}.rs` — **Modify** (Task 1 only): regenerated golden hash tables.

---

## Task 1: Add the `conserve_goods_on_death` flag (opt-in, FORMAT_VERSION 24, golden rehash)

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs` (`Scenario` struct near `:85`; `instantiate` near `:439`), `crates/anabios-core/src/world.rs` (field near `:135`; ctor default near `:336`), `crates/anabios-core/src/snapshot.rs` (`:102`)
- Test: `crates/anabios-core/tests/trade.rs`
- Regenerate: `crates/anabios-core/tests/{determinism,inventions,cognition}.rs`

**Interfaces:**
- Produces: `World.conserve_goods_on_death: bool` (default `false`); TOML key `conserve_goods_on_death`.

- [ ] **Step 1: Write the failing parse/wire test** in `crates/anabios-core/tests/trade.rs` (mirror `perishable_goods_flag_parses_and_wires`'s corrected form — flag at TOML top level, includes `name`/`seed`):

```rust
#[test]
fn conserve_goods_on_death_flag_parses_and_wires() {
    let toml = "name = \"t\"\nseed = 1\nworld_size = 64\nresources_enabled = true\nconserve_goods_on_death = true\n[[agents]]\narchetype = \"grazer\"\ncount = 4\n";
    let w = anabios_core::scenario::Scenario::parse_toml(toml).unwrap().instantiate();
    assert!(w.conserve_goods_on_death);
}
```

- [ ] **Step 2: Run — expect FAIL** (`no field conserve_goods_on_death`).

Run: `cargo test -p anabios-core --test trade conserve_goods_on_death_flag_parses_and_wires`
Expected: FAIL (compile error: unknown field).

- [ ] **Step 3: Add the field + wiring.**
  - `world.rs`: add near the other opt-in flags (e.g. after `resources_enabled` at `:135`):
    ```rust
    /// Opt-in: on death, an agent's trade-goods inventory transfers to the
    /// nearest living agent instead of being lost, keeping the goods economy
    /// conservative so long-run trade never starves to zero. `false` (default)
    /// leaves the economy unchanged.
    pub conserve_goods_on_death: bool,
    ```
    and in the ctor struct literal (near `resources_enabled: false,` at `:336`) add `conserve_goods_on_death: false,`.
  - `scenario.rs`: add the struct field (mirror `resources_enabled` at `:85`):
    ```rust
    /// Opt-in: conserve trade goods on death (transfer to nearest living
    /// agent) so long-run trade doesn't freeze. Default off.
    #[serde(default)]
    pub conserve_goods_on_death: bool,
    ```
    and in `instantiate` after `w.resources_enabled = self.resources_enabled;` (`:439`) add `w.conserve_goods_on_death = self.conserve_goods_on_death;`.

- [ ] **Step 4: Run — expect PASS.**

Run: `cargo test -p anabios-core --test trade conserve_goods_on_death_flag_parses_and_wires`
Expected: PASS.

- [ ] **Step 5: Bump `FORMAT_VERSION` + changelog.** In `snapshot.rs`, change `pub const FORMAT_VERSION: u32 = 23;` (`:102`) to `24`, and add a changelog line after the `v23` entry (near `:99`), mirroring the existing style:

```rust
/// v24: supply-side trade fix — World.conserve_goods_on_death flag. Off in
///      every existing scenario; serialized layout grew by one byte.
```

The version round-trip test (`snapshot.rs:212`) uses `FORMAT_VERSION - 1` (relative) — no edit needed. Grep for any literal `23` assertion and update if present: `grep -rn "== 23\|= 23\b" crates/anabios-core/src/snapshot.rs crates/anabios-core/tests/`.

- [ ] **Step 6: Regenerate the three golden tables** (they WILL move — layout grew; this is correct):

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism --test inventions --test cognition`
Then copy the printed hash values into each file's `GOLDEN`/`INVENTIONS_GOLDEN`/`COGNITIVE_GOLDEN` table (follow the in-file instructions; the pattern already exists in each). If any of these tests fails a NON-hash assertion (a structural failure, not a hash mismatch), STOP and report — that means a real regression, not a layout shift.

- [ ] **Step 7: Verify goldens pass green (no env var):**

Run: `cargo test -p anabios-core --test determinism --test inventions --test cognition`
Expected: all PASS.

- [ ] **Step 8: fmt + clippy.**

Run: `cargo fmt && cargo clippy -p anabios-core --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 9: Commit.**

```bash
git add crates/anabios-core/src/scenario.rs crates/anabios-core/src/world.rs crates/anabios-core/src/snapshot.rs crates/anabios-core/tests/trade.rs crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/inventions.rs crates/anabios-core/tests/cognition.rs
git commit -m "feat(trade): add opt-in conserve_goods_on_death flag (FORMAT_VERSION 24, goldens rehashed)"
```

---

## Task 2: Death-snapshot + `conserve_goods_step` redistribution (the fix)

`kill()` records each dying agent's goods; a new stage redistributes them to the nearest living agent. Off by default → **goldens must NOT move in this task** (scratch field is `#[serde(skip)]`; the flag added in Task 1 is off in every golden scenario).

**Files:**
- Modify: `crates/anabios-core/src/agent.rs` (`AgentBuffers` struct near the scratch fields `:96-99`; `kill()` `:215-235`), `crates/anabios-core/src/resource.rs` (new fn), `crates/anabios-core/src/tick.rs` (call site near the age/starve stage `:79`)
- Test: `crates/anabios-core/tests/trade.rs`

**Interfaces:**
- Consumes: `World { agents, spatial, resources_enabled, conserve_goods_on_death }`, `AgentBuffers { position: Vec<Vec2>, inventory: Vec<[f32; GOOD_COUNT]>, alive }`.
- Produces:
  - `AgentBuffers.deaths_scratch: Vec<(Vec2, [f32; crate::resource::GOOD_COUNT])>` (`#[serde(skip)]`).
  - `pub fn conserve_goods_step(world: &mut World)` in `resource.rs`.

- [ ] **Step 1: Write the failing conservation unit test** in `crates/anabios-core/tests/trade.rs`:

```rust
#[test]
fn conserve_goods_step_moves_dead_inventory_to_living() {
    use anabios_core::prelude::Vec2;
    use anabios_core::genome::Genome;
    let toml = "name = \"c\"\nseed = 1\nworld_size = 64\nresources_enabled = true\nconserve_goods_on_death = true\n";
    let mut w = anabios_core::scenario::Scenario::parse_toml(toml).unwrap().instantiate();
    // Two agents a few units apart; A holds goods, B is the nearest (only) living neighbour.
    let a = w.spawn_agent(Vec2::new(10.0, 10.0), Genome::neutral());
    let b = w.spawn_agent(Vec2::new(12.0, 10.0), Genome::neutral());
    w.agents.inventory[a as usize] = [3.0, 1.0, 0.0, 2.0];
    let before_b: f32 = w.agents.inventory[b as usize].iter().sum();
    // Kill A; its goods must land on B after the conservation stage.
    w.agents.kill(a);
    anabios_core::resource::conserve_goods_step(&mut w);
    let after_b: f32 = w.agents.inventory[b as usize].iter().sum();
    assert!((after_b - before_b - 6.0).abs() < 1e-4, "B should gain A's 6 units, got {after_b} from {before_b}");
    assert_eq!(w.agents.inventory[b as usize], [3.0, 1.0, 0.0, 2.0]);
    // Buffer is drained.
    assert!(w.agents.deaths_scratch.is_empty());
}
```

(Note: `spawn_agent` rebuilds nothing; `conserve_goods_step` must (re)build a live-agent spatial index itself — see Step 3. Verify `spawn_agent`'s signature in `world.rs` if the skeleton doesn't compile and adjust the calls, keeping the intent: A holds goods, is killed, B is the nearest living agent.)

- [ ] **Step 2: Run — expect FAIL** (`deaths_scratch` / `conserve_goods_step` undefined).

Run: `cargo test -p anabios-core --test trade conserve_goods_step_moves_dead_inventory_to_living`
Expected: FAIL (compile error).

- [ ] **Step 3: Implement.**
  - `agent.rs`: add the scratch field to `AgentBuffers` beside `scratch_ids` (`:96-99`):
    ```rust
    /// Snapshots (position, trade-goods inventory) of agents killed this tick,
    /// drained by `resource::conserve_goods_step` when `conserve_goods_on_death`
    /// is on. `#[serde(skip)]` — scratch, never part of the state hash.
    #[serde(skip)]
    pub deaths_scratch: Vec<(crate::prelude::Vec2, [f32; crate::resource::GOOD_COUNT])>,
    ```
    Add `deaths_scratch: Vec::new(),` to every `AgentBuffers` constructor/`Default` (grep `AgentBuffers {` / the `Default`/`new` impl and add the field so it compiles).
    In `kill()` (`:215`), after the `self.alive.set(i, false);` block, snapshot when goods are held (cheap; naturally no-op when `resources_enabled` is off, since inventory is all-zero then):
    ```rust
    let inv = self.inventory[i];
    if inv.iter().any(|&g| g > 0.0) {
        self.deaths_scratch.push((self.position[i], inv));
    }
    ```
    (This is determinism-neutral: `deaths_scratch` is scratch; it is only *consumed* when the flag is on, and only holds entries when goods exist. Do not zero `self.inventory[i]` — nothing reads a dead slot's inventory before reuse zeroes it, and leaving it avoids any behavior change when the flag is off.)
  - `resource.rs`: add the stage. It rebuilds a fresh live-agent spatial index (so the nearest query is correct after motion/reproduction), then for each snapshot adds the goods to the nearest living agent (deterministic: expanding-radius query, then lowest-index tie-break on equal distance):
    ```rust
    /// Conserve trade goods on death: redistribute each snapshot recorded by
    /// `AgentBuffers::kill` this tick to the nearest living agent, so goods stay
    /// in the economy instead of vanishing. RNG-free. No-op (and drains the
    /// buffer) unless both `resources_enabled` and `conserve_goods_on_death`.
    pub fn conserve_goods_step(world: &mut World) {
        if !world.resources_enabled || !world.conserve_goods_on_death {
            world.agents.deaths_scratch.clear();
            return;
        }
        if world.agents.deaths_scratch.is_empty() {
            return;
        }
        // Fresh live-agent index for a correct, deterministic nearest query.
        world.spatial.rebuild(&world.agents.position, |i| world.agents.alive[i]);
        let deaths = std::mem::take(&mut world.agents.deaths_scratch);
        for (pos, inv) in &deaths {
            // Expand the search radius until a living agent is found; pick the
            // nearest, lowest-index on ties. World is toroidal → torus_distance.
            let mut best: Option<u32> = None;
            let mut best_d = f32::INFINITY;
            let mut radius = crate::resource::HARVEST_RANGE.max(8.0);
            while best.is_none() && radius <= world.world_size {
                world.spatial.query(*pos, radius, |id| {
                    let j = id as usize;
                    if !world.agents.alive[j] {
                        return;
                    }
                    let d = crate::spatial::torus_distance(*pos, world.agents.position[j], world.world_size);
                    if d < best_d || (d == best_d && best.is_some_and(|b| id < b)) {
                        best_d = d;
                        best = Some(id);
                    }
                });
                radius *= 2.0;
            }
            if let Some(id) = best {
                let j = id as usize;
                for k in 0..GOOD_COUNT {
                    world.agents.inventory[j][k] += inv[k];
                }
            }
            // If no living agent exists at all (empty world), the goods are
            // simply dropped — there is nobody to hold them.
        }
        // `deaths` (the taken buffer) is dropped; `deaths_scratch` is now empty.
    }
    ```
    Add `use crate::world::World;` to `resource.rs` if not already imported. `GOOD_COUNT` is already in scope (defined there).
  - `tick.rs`: call the stage right after the age/starve death stage (so it sees this tick's deaths; earlier-stage kills also accumulated in `deaths_scratch`). Insert after `crate::carcass::carcass_step(world);` (Stage 7b, `:81`):
    ```rust
    // Stage 7c: conserve trade goods on death (opt-in; no-op + buffer drain
    // when off). Runs after all death stages so it captures this tick's deaths.
    crate::resource::conserve_goods_step(world);
    ```

- [ ] **Step 4: Run — expect PASS.**

Run: `cargo test -p anabios-core --test trade conserve_goods_step_moves_dead_inventory_to_living`
Expected: PASS.

- [ ] **Step 5: Confirm goldens UNCHANGED** (flag off in all golden scenarios; scratch field is serde-skip):

Run: `cargo test -p anabios-core --test determinism`
Expected: PASS unchanged (do NOT regenerate). If a golden moves, `kill()` changed behavior when the flag is off — fix it (the snapshot must not alter serialized state) and re-run.

- [ ] **Step 6: fmt + clippy.**

Run: `cargo fmt && cargo clippy -p anabios-core --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit.**

```bash
git add crates/anabios-core/src/agent.rs crates/anabios-core/src/resource.rs crates/anabios-core/src/tick.rs crates/anabios-core/tests/trade.rs
git commit -m "feat(trade): conserve goods on death — transfer dead agents' inventory to nearest living"
```

---

## Task 3: Prove the freeze is gone (scenario + long-horizon baseline contrast)

The proof must be a directional contrast over a long horizon — flag-off `biome-trade` freezes, flag-on `conserve-trade` stays alive (learning from the prior false-positive that checked one window with no baseline).

**Files:**
- Create: `scenarios/conserve-trade.toml`
- Test: `crates/anabios-core/tests/trade.rs`

**Interfaces:**
- Consumes: `World.trade_routes` (per-tick swaps), `step`.

- [ ] **Step 1: Create the scenario.** Read `scenarios/biome-trade.toml` fully, then write `scenarios/conserve-trade.toml` = a verbatim copy with (a) `name = "biome-trade"` → `name = "conserve-trade"`, (b) `conserve_goods_on_death = true` added at top level right after the `resources_enabled = true` line, (c) `seed` and everything else identical.

- [ ] **Step 2: Write the failing contrast test** in `crates/anabios-core/tests/trade.rs`:

```rust
const CONSERVE: &str = include_str!("../../../scenarios/conserve-trade.toml");

/// Conservation keeps trade alive long-term. Baseline `biome-trade` freezes
/// (goods bleed out via death-churn) — its late-window swap volume collapses to
/// ~0 by ~t10k. With `conserve_goods_on_death`, goods stay in circulation and
/// the late window keeps trading. This asserts the CONTRAST, not just a
/// single-sided threshold (the prior perishability test's mistake).
#[test]
fn conserve_trade_stays_alive_vs_frozen_baseline() {
    fn late_window_trades(toml: &str, ticks: u64) -> u64 {
        let mut w = anabios_core::scenario::Scenario::parse_toml(toml).expect("parse").instantiate();
        let mut late = 0u64;
        for t in 0..ticks {
            step(&mut w);
            if t >= ticks - 2000 {
                late += w.trade_routes.len() as u64;
            }
        }
        late
    }
    let ticks = 16000u64;
    let base_late = late_window_trades(TRADE, ticks);       // biome-trade (flag off)
    let cons_late = late_window_trades(CONSERVE, ticks);    // conserve-trade (flag on)
    // Baseline has frozen: essentially no trade in the final 2k ticks.
    assert!(base_late < 100, "expected baseline frozen, got base_late={base_late}");
    // Conservation keeps a live economy: orders of magnitude more late trade.
    assert!(cons_late > 5000, "expected conservation to sustain trade, got cons_late={cons_late}");
}
```

- [ ] **Step 3: Run — expect PASS.**

Run: `cargo test -p anabios-core --test trade conserve_trade_stays_alive_vs_frozen_baseline`
Expected: PASS.

If `base_late` is not < 100 (baseline didn't fully freeze at 16k on this build), raise `ticks` to 20000 (the diagnosis measured a hard 0 from ~t10k). If `cons_late` is low (conservation insufficient alone), STOP and report the numbers — per the design's open question, that would signal a follow-up lever (B), not a tuning tweak. Report the observed `base_late` and `cons_late`.

- [ ] **Step 4: Determinism smoke** — the flag-on scenario must be reproducible:

```rust
#[test]
fn conserve_trade_is_deterministic() {
    let run = || {
        let mut w = anabios_core::scenario::Scenario::parse_toml(CONSERVE).expect("parse").instantiate();
        for _ in 0..500 { step(&mut w); }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "conserve-trade must be deterministic");
}
```

Run: `cargo test -p anabios-core --test trade conserve_trade_is_deterministic`
Expected: PASS.

- [ ] **Step 5: fmt + clippy.**

Run: `cargo fmt && cargo clippy -p anabios-core --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/anabios-core/tests/trade.rs scenarios/conserve-trade.toml
git commit -m "test(trade): conservation sustains trade vs the frozen biome-trade baseline"
```

---

## Task 4: Make trade volume visible in sweeps (`total_trades` CSV column)

Folds in the observability goal from the abandoned plan: the CSV currently can't see the freeze (`resource_traded` is a latched 0/1). Export `total_trades`.

**Files:**
- Modify: `crates/anabios-headless/src/sweep.rs` (`RunSummary` `:19`; `run_one` `:89-140`; `write_summary_csv` `:172-193`)
- Test: `crates/anabios-headless/tests/sweep_csv.rs` (Create)

**Interfaces:**
- Consumes: `world.total_trades: u64`.
- Produces: a `total_trades` column in `summary.csv`, placed immediately after `coverage` (canonical order `…,coverage,total_trades`; a future `novel_types` column follows it).

- [ ] **Step 1: Write the failing CSV test.** Create `crates/anabios-headless/tests/sweep_csv.rs`:

```rust
// Guards the summary.csv header contract the trade freeze-vs-alive contrast
// depends on: total_trades must be a column, right after coverage.
use std::process::Command;

#[test]
fn summary_csv_has_total_trades_column_after_coverage() {
    let out = std::env::temp_dir().join(format!("anabios_sweep_csv_{}", std::process::id()));
    let manifest = env!("CARGO_MANIFEST_DIR"); // crates/anabios-headless
    let scenario = format!("{manifest}/../../scenarios/biome-trade.toml");
    let status = Command::new(env!("CARGO_BIN_EXE_anabios-headless"))
        .args(["sweep", "--scenario", &scenario, "--seeds", "1", "--ticks", "50", "--out"])
        .arg(&out)
        .status()
        .expect("run sweep");
    assert!(status.success(), "sweep exited non-zero");
    let header = std::fs::read_to_string(out.join("summary.csv")).expect("read csv");
    let header = header.lines().next().expect("header line");
    assert!(header.contains(",total_trades"), "missing total_trades column: {header}");
    assert!(header.contains("coverage,total_trades"), "total_trades must follow coverage: {header}");
    let _ = std::fs::remove_dir_all(&out);
}
```

- [ ] **Step 2: Run — expect FAIL** (no `total_trades` column).

Run: `cargo test -p anabios-headless --test sweep_csv`
Expected: FAIL (assertion: missing column).

- [ ] **Step 3: Add the column.**
  - `sweep.rs`: add `total_trades: u64,` to `struct RunSummary` (`:19`).
  - In `run_one` (the `Ok(RunSummary { … })` near `:126-135`), add `total_trades: world.total_trades,` alongside the other fields (set from the final world).
  - In `write_summary_csv`: change the header line `writeln!(f, ",emergence_score,novel_events,coverage")?;` to `writeln!(f, ",emergence_score,novel_events,coverage,total_trades")?;`, and the row line `writeln!(f, ",{:.3},{},{:.3}", r.emergence_score, r.novel_events, r.coverage)?;` to `writeln!(f, ",{:.3},{},{:.3},{}", r.emergence_score, r.novel_events, r.coverage, r.total_trades)?;`.

- [ ] **Step 4: Run — expect PASS.**

Run: `cargo test -p anabios-headless --test sweep_csv`
Expected: PASS.

- [ ] **Step 5: fmt + clippy.**

Run: `cargo fmt && cargo clippy -p anabios-headless --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/anabios-headless/src/sweep.rs crates/anabios-headless/tests/sweep_csv.rs
git commit -m "feat(headless): export total_trades so sweeps can see the economy freeze"
```

---

## Testing Plan (summary)

| Level | What | Where |
|---|---|---|
| Unit | flag parses/wires | `tests/trade.rs` (Task 1) |
| Unit | dead agent's goods land on nearest living, buffer drained | `tests/trade.rs` (Task 2) |
| Determinism | goldens rehash once (Task 1), then stay put (Task 2) | `tests/{determinism,inventions,cognition}.rs` |
| Integration | conservation sustains trade vs frozen baseline (16k+ ticks, contrast) | `tests/trade.rs` (Task 3) |
| Determinism | flag-on scenario run-twice identical | `tests/trade.rs` (Task 3) |
| Observability | `total_trades` CSV column after `coverage` | `tests/sweep_csv.rs` (Task 4) |

**Done when:** `conserve-trade.toml` sustains nonzero late-window trade at ≥16k ticks while flag-off `biome-trade` is frozen (Task 3 contrast passes), default scenarios' behavior is unchanged (goldens rehashed for layout only), and `total_trades` is a sweep CSV column.

## Risks / open questions

- **Is conservation alone sufficient?** Expected yes (conservation + any positive harvest ⇒ non-decreasing goods). Task 3 measures it. If late-window volume is nonzero but weak, note a follow-up lever (B: harvest access) — do not pre-build it.
- **Nearest-query cost.** `conserve_goods_step` rebuilds the agent spatial index once per tick *only when the flag is on and deaths occurred*; the expanding-radius query is bounded by `world_size`. Flag-off cost is one `is_empty()` check + a buffer clear. If profiling later shows the rebuild is hot on a perishable... conserve scenario, reuse the stage-1 `world.spatial` instead (accept slightly stale positions) — deferred, not needed for correctness.
- **Death hook coverage.** `kill()` is the single choke point for every death (starvation, combat, stillbirth, husbandry), so the snapshot catches them all regardless of stage; `conserve_goods_step` runs after the last major death stage (age/starve + carcass). Kills in a later stage (rare) are caught the next tick — acceptable, they still aren't lost (the snapshot persists until drained).
