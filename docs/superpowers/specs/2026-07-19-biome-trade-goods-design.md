# Biome Trade Goods — Design

**Status:** approved design, pre-implementation
**Date:** 2026-07-19
**Author:** brainstormed with Claude Code
**Crate:** `anabios-core`

## Summary

Four unique natural resources spawn in their home biomes. Agents harvest
whatever good is local to them, so each species ends up rich in its biome's
good but starved of the other three. Agents swap surpluses with adjacent
**other-species** agents, and a *balanced basket of all four goods* is the
dowry required to reproduce.

The causal chain: **geography → specialization → trade → breeding.**

This is a **richer-world mechanic**, not an instrumented experiment. Success =
the trade economy is legible, self-balancing, deterministic, and cleanly
integrated with the existing biome / module / reproduction systems. There is no
single hypothesis being measured; the codex observability is for watching, not
proving.

## Goals

- Discrete natural resources that spawn on the map (first-class map entities).
- Genuine **bilateral trade** between different species (agents swap goods).
- Trade is **mutually beneficial by construction** via need-based diminishing
  marginal utility — every agent needs a little of every good.
- Reproduction is gated on a balanced basket (the economic sink + the reason
  trade matters).
- Fully deterministic; zero impact on existing golden hashes when disabled.

## Non-goals (v1, YAGNI)

- Per-good *secondary* benefit effects (e.g. Obsidian → weapon multiplier).
  With the dowry model each good's benefit is already "a required, non-
  substitutable basket slot." Secondary effects are a clean future extension.
- Neural / program-driven decisions to harvest or trade. Harvest and trade are
  rule-based automatic mechanics in v1. Program-gated trading is a future
  research extension.
- Prices, currency, markets, storage of energy-as-money. The only medium of
  exchange is goods-for-goods.
- Trade with same-species agents. Only cross-species swaps count as "trade."

## Design decisions (locked)

| Question | Decision |
|----------|----------|
| Purpose | Richer-world mechanic (not a hypothesis test). |
| Trade model | Bilateral swap between agents (harvest & carry inventory). |
| Valuation | Need-based dynamic; **everyone needs a little of every good**; diminishing marginal utility. |
| Payoff | Balanced basket **gates reproduction** (a dowry, consumed on birth). |
| Specialization source | Resources are **biome-linked** (geography). |
| Number of goods | **4**, one per land terrain. |
| Carrying capacity | Flat base cap **+ bonus for agents with a `Storage` module**. |
| Harvest & trade | **Automatic / rule-based** (not neural). |

## The four goods

One per land terrain (Water excluded — agents avoid water):

| Good | Home terrain | Flavor |
|------|-------------|--------|
| **Salt** | Desert | mineral flats |
| **Obsidian** | Rock | volcanic glass |
| **Amber** | Forest | fossil resin |
| **Spice** | Grass | grassland herbs |

`enum Good { Salt, Obsidian, Amber, Spice }` — variant order is **append-only**
for serde stability. `Good::N = 4`. A helper maps `TerrainType -> Option<Good>`
(Water → None).

## Architecture

The feature copies the existing **carcass** precedent for "discrete entities
that spawn on the map" and the **invention** precedent for "opt-in subsystem,
zero RNG when off."

### Data model

New file `crates/anabios-core/src/resource.rs`:

```rust
pub const GOOD_COUNT: usize = 4;

#[derive(Clone, Copy, Serialize, Deserialize, ...)]
pub enum Good { Salt, Obsidian, Amber, Spice } // append-only

#[derive(Clone, Serialize, Deserialize, ...)]
pub struct Resource {
    pub pos: Vec2,
    pub kind: Good,
    pub amount: f32,
}
```

`World` (in `world.rs`) gains:

- `resources: Vec<Resource>` — serialized.
- `resource_spatial: UniformSpatialHash` — `#[serde(skip)]`, rebuilt per tick
  exactly like `carcass_spatial`.
- `resources_enabled: bool` — `#[serde(default)]`, mirrors the scenario flag.

`AgentBuffers` (in `agent.rs`) gains one parallel array:

- `inventory: Vec<[f32; GOOD_COUNT]>` — indexed by `AgentId`, one float per
  good. Serialized (→ `FORMAT_VERSION` bump). New agents spawn with all-zero
  inventory. Must be maintained by the alloc/free-list path alongside the other
  SoA arrays.

### Constants (tunable, in `resource.rs`)

- `RESOURCE_STEP_INTERVAL` — reuse the biome cadence (10 ticks).
- `NODE_TARGET_DENSITY` — target resource nodes per terrain (or per N matching
  cells).
- `NODE_MAX_TOTAL` — hard cap on live nodes.
- `NODE_START_AMOUNT`, `HARVEST_RATE` (per tick per agent over a node).
- `INVENTORY_BASE_CAP`, `INVENTORY_STORAGE_BONUS` (added if `Storage` module
  present).
- `TRADE_RANGE` (≤ `perception_max_radius()`), `TRADE_UNIT` (units moved per
  good per swap event).
- `DOWRY_REQ` (units of each good consumed per birth).

### Spawning — `resource_step`

Runs on the biome cadence (`tick % RESOURCE_STEP_INTERVAL == 0`), gated on
`resources_enabled`. **Draws zero RNG when disabled.**

1. If node count < target: for matching-terrain cells currently under target
   density, spawn `Resource { pos, kind, amount: NODE_START_AMOUNT }` with
   `pos` drawn from `world.rng` (fixed draw order), `kind` from the cell's
   terrain. Respect `NODE_MAX_TOTAL`.
2. Remove nodes with `amount <= 0`.

The `resource_spatial` hash is rebuilt from `resources` each tick (like
`carcass_spatial`) so harvest/trade queries are O(local).

### Harvesting — harvest pass

Alive agents ascending. For each agent, query `resource_spatial` for the
nearest node within harvest radius; pull `min(HARVEST_RATE, node.amount,
remaining_capacity)` into `inventory[kind]`; deplete the node. Remaining
capacity = `cap(agent) - sum(inventory)` where `cap` = `INVENTORY_BASE_CAP`
(+`INVENTORY_STORAGE_BONUS` if the agent has a `Storage` module). No RNG.

### Trade — `trade_pass` in `interact.rs`, immediately after `share_pass`

Need-based diminishing marginal utility: `want(agent, k) = 1.0 / (1.0 +
inventory[agent][k])`. You value what you are short of.

For each alive agent **A** (ascending):

1. Find the nearest **other-species** agent **B** within `TRADE_RANGE` (mirror
   `share_pass`/`combat_pass` target selection; skip dead/self/same-species).
2. Choose `g` = the good A is most willing to give (lowest `want_A`) that B
   still wants; choose `r` = the good B is most willing to give (lowest
   `want_B`) that A wants.
3. Execute a swap of `TRADE_UNIT` of `g` (A→B) for `TRADE_UNIT` of `r` (B→A)
   **iff both sides strictly gain**: `want_A(r) > want_A(g)` **and**
   `want_B(g) > want_B(r)`. Clamp to available amounts and to each side's
   carrying capacity.
4. Record a `ResourceTraded` codex event.

Determinism: agents processed in fixed ascending order; a swap mutates two
inventories but reads current state, so the result is a pure function of state.
No RNG. (B may also initiate its own trade later in the same pass — fine, order
is fixed.)

### Reproduction dowry — `reproduce.rs`

The existing energy-threshold gate stays. Reproduction additionally requires,
for **all four** goods, `inventory[parent][k] >= DOWRY_REQ`. When
`resources_enabled` is **false**, this check is skipped entirely (no behavior
change). On successful birth, subtract `DOWRY_REQ` of each good from the
parent(s) — the economic sink. (For two-parent mating, spec the split
explicitly during implementation; default: each contributing parent pays the
full dowry, or split evenly — decide and document, keep deterministic.)

### Determinism & serialization

- New tick stage for spawn/harvest in `tick.rs::step`, in a **fixed position**;
  `trade_pass` slots into `interact_all()` after `share_pass`.
- Everything gated on `resources_enabled`; **zero RNG consumed when off**, so
  every existing golden hash is unchanged.
- Iterate `iter_alive()` (ascending) only; any per-good aggregation uses
  `BTreeMap`/arrays, never `HashMap`.
- `Good`, `EventType::{ResourceHarvested, ResourceTraded, DowryBirth}` appended
  at the **end** of their enums.
- Bump `snapshot.rs` `FORMAT_VERSION` (new serialized fields: `resources`,
  `inventory`, `resources_enabled`).
- Regenerate golden hashes with `UPDATE_HASHES=1` **only** for a new trade
  scenario; the existing `minimal.toml` golden hashes must not move (feature
  off by default). Document the `FORMAT_VERSION` bump in the `determinism.rs`
  changelog comment.

### Config / scenario

- `Scenario` (in `scenario.rs`) gains `resources_enabled: bool`
  (`#[serde(default)]`); wired in `instantiate()` onto `World`.
- New `scenarios/biome-trade.toml`: several species, each `Cluster`-placed in a
  different biome region with `EnvAffinity` traits pinning them to that terrain,
  so they specialize in different goods and trade routes form at the borders.

### Observability

- `EventType::{ResourceHarvested, ResourceTraded, DowryBirth}` (appended) with a
  detector in `codex::observe_all` (tick stage 9). Detector state in
  `BTreeMap`/`BTreeSet` (deterministic).
- Optionally add per-species good holdings to `SpeciesAggTable` for a "who's
  rich in what" readout.

### Frontend (optional, follow-up)

Rendering resource nodes and per-agent inventory in the Godot sandbox
(`anabios-godot`) is out of scope for the core-crate v1 but is a natural
follow-up; the data is all exposed on `World`.

## Testing strategy

**Unit:**
- Harvesting pulls from a node into inventory and depletes the node; respects
  carrying cap; `Storage` module raises the cap.
- A swap is mutually beneficial (both `want` inequalities hold), moves
  `TRADE_UNIT` each way, and **conserves total units** of each good across the
  two agents.
- Dowry: reproduction is blocked when any good < `DOWRY_REQ`, permitted when all
  ≥, and consumes `DOWRY_REQ` of each on birth.
- `TerrainType -> Good` mapping (Water → None).

**Integration:**
- `resources_enabled = false`: `minimal.toml` golden hashes are **unchanged**
  (regression guard for zero-RNG-when-off).
- A seeded `biome-trade.toml` runs deterministically (repeat run → identical
  state hash) and produces actual cross-species `ResourceTraded` events and at
  least one `DowryBirth`.
- Snapshot round-trip at the new `FORMAT_VERSION` (serialize → deserialize →
  identical state; spatial hashes rebuilt on load).

## Open implementation details (decide during coding, keep deterministic)

- Exact node-density spawn rule (per-cell vs per-terrain-region) and RNG draw
  order.
- Two-parent dowry split.
- Whether harvest is folded into `feed_pass` or a dedicated pass (leaning
  dedicated for clarity).
- Tuning of all constants so trade actually happens and populations persist
  (balance pass with the trade scenario).

## Integration points (file map)

- **new** `crates/anabios-core/src/resource.rs` — `Good`, `Resource`,
  constants, `resource_step`, `TerrainType -> Good`.
- `world.rs` — `resources`, `resource_spatial`, `resources_enabled`.
- `agent.rs` — `inventory` SoA array + alloc/free maintenance.
- `interact.rs` — harvest pass + `trade_pass`.
- `reproduce.rs` — dowry gate + consumption.
- `tick.rs` — spawn/harvest stage; rebuild `resource_spatial`.
- `scenario.rs` — `resources_enabled` flag + wiring.
- `codex/mod.rs` (+ a codex submodule) — new `EventType`s + detector.
- `snapshot.rs` — `FORMAT_VERSION` bump.
- `determinism.rs` — changelog note; new-scenario golden hashes.
- **new** `scenarios/biome-trade.toml`.
- tests: unit tests in `resource.rs`/`interact.rs`; integration in
  `tests/` (trade scenario determinism + minimal-hash regression).
