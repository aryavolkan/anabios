# Supply-Side Trade Fix — Design (2026-08-02)

Design for the Phase-2 roadmap item *Trade-economy redesign*, re-scoped after the
[freeze diagnosis](2026-08-02-trade-freeze-diagnosis.md) disproved the original
(demand-satiation) plan. **This design targets the measured cause: supply-side
starvation.** The prior perishability plan is superseded.

## Problem (measured)

The biome trade economy freezes over long runs (`biome-trade` stops trading
permanently by ~t10k). At the frozen tick every agent holds an **empty** basket
(`[0,0,0,0]`) while resource nodes remain available and every agent's `want` is
maxed. The cause is that goods **bleed out of the living population**: `kill()`
(`agent.rs:215`) marks a slot dead but never redistributes its trade-goods
inventory — the goods are simply zeroed when a newborn later reuses the slot
(`agent.rs:174`). Under stable-population birth/death churn, this loss outpaces
the harvest inflow that a dispersed ~2000-agent population can achieve through
the small `HARVEST_RANGE`, so total goods drain to zero and trade — being pure
redistribution — cannot restart with nothing to redistribute.

## Mechanism: conserve trade goods on death (opt-in)

When an agent dies, transfer its trade-goods inventory to the **nearest living
agent** before the goods are lost. This makes the goods economy **conservative**:
goods no longer leave circulation at death, so the population's total goods are
non-decreasing (rising with harvest), and trade cannot starve to zero as long as
harvest inflow is positive — which it always is, since nodes spawn near the
population (`SPAWN_NEAR_RADIUS`).

This is a structural guarantee (conservation), not a tuning knob balanced near a
sensitivity cliff — the failure mode of the abandoned perishability approach.

## Design decisions

- **Transfer to nearest living agent, not drop-as-node.** A direct transfer
  guarantees the goods land on a living agent immediately, independent of harvest
  access (which is itself marginal and part of the problem). Dropping a
  harvestable node would re-introduce a dependence on `HARVEST_RANGE`. "Nearest"
  is a deterministic spatial query with a lowest-index tie-break (the same
  discipline `pick_swap`/harvest use) — **zero RNG**.
- **Unbounded nearest (no distance cap).** The search returns the single closest
  living agent with no maximum radius, so in a populated world a recipient always
  exists and *no goods are ever dropped for lack of a neighbour* (true
  conservation). In practice "nearest" is a cluster-mate, since deaths occur among
  the living, so locality is preserved without needing an explicit range. (If the
  spatial query is radius-based, the implementation widens the radius until a
  living agent is found, or falls back to a linear nearest scan — an
  implementation detail, kept deterministic.)
- **No carrying-cap clamp on the inheritance.** Conservation takes priority: the
  recipient may transiently exceed `carrying_cap`. This does not create a hoarder
  sink, because `want()` still caps *demand* at `STOCK_TARGET`, so a recipient's
  surplus is exactly what it will trade away next — the excess redistributes
  through normal `trade_pass`. Clamping would re-introduce a (small) leak.
- **Cross-species is allowed.** Trade goods are fungible; the nearest living
  agent inherits regardless of species. (Trade itself is cross-species; the
  goods pool is shared.)
- **Fungible sum, per-good.** Each of the 4 good slots transfers additively
  (`recipient[k] += dead[k]`), then the dead agent's inventory is zeroed.
- **Placement in the tick.** A deterministic stage keyed off the agents that died
  **this tick**, processed in ascending id order, gated on
  `resources_enabled && conserve_goods_on_death`. It must run at the point where
  deaths for the tick are known and before the dead slots can be reused by
  spawning, so no inventory is lost. (Exact hook — a death list vs. a
  before/after live-set diff, and interaction with the existing `carcass_step`
  death handling — is an implementation detail for the plan; it must be
  RNG-free and order-deterministic.)

## Determinism & gating

- New serialized field `World.conserve_goods_on_death: bool` (default `false`) +
  matching `Scenario` TOML key `conserve_goods_on_death`, wired in `instantiate`
  (mirror `resources_enabled`).
- A new serialized field grows the bincode `World` layout, so — exactly as the
  FORMAT_VERSION v2..v23 history and the abandoned flag did — this bumps
  **`FORMAT_VERSION` 23→24** with a changelog line and **regenerates the three
  golden hash tables** (`determinism.rs`, `inventions.rs`, `cognition.rs`) via
  `UPDATE_HASHES=1`. The regen is a pure layout rehash: with the flag off, no
  behavior changes, so only the hash *values* move (no structural test change).
- The transfer draws **zero RNG** and must not change iteration order of any
  other stage, so flag-off runs stay bit-identical (post-rehash) and the
  `parallel_matches_serial` determinism property holds.

## Success criteria (the proof)

Learning from the prior false-positive (a cherry-picked 2000-tick window with no
baseline), the proof must be a **long-horizon baseline contrast**:

- New `scenarios/conserve-trade.toml` = `biome-trade` clone + `conserve_goods_on_death = true`, seed pinned.
- Integration test over **≥16 000 ticks** asserting: flag-**off** `biome-trade`
  freezes (late-window trade ≈ 0 by ~t10k) **and** flag-**on** `conserve-trade`
  sustains **nonzero** trade in the same late window — i.e. the contrast is real
  and directional, not a single-sided threshold.
- **Conservation unit test:** a dead agent holding goods → after the
  death-conservation stage, those goods appear on a living agent and the dead
  slot is zeroed (nothing lost).
- **Determinism:** flag-off goldens pass post-rehash; the flag-on scenario is
  deterministic (run-twice-identical) and covered by
  `parallel_matches_serial`.
- **Observability (small, folds in the abandoned Task 4):** add `total_trades` as
  a sweep CSV column so the freeze-vs-alive contrast is visible in sweeps
  (canonical column order `…coverage, total_trades, novel_types`).

## Out of scope

- Boosting harvest inflow (bigger `HARVEST_RANGE`, node co-location) and newborn
  inheritance — candidate levers B/C from the diagnosis. Conservation (lever A)
  is the structural fix; B/C are deferred unless the measurement shows A
  insufficient.
- Perishability / non-satiating `want` — disproven (see the diagnosis doc).
- Demand-driven pricing — a demand-side idea, orthogonal to the supply cause.

## Open questions (resolve during implementation)

- **Exact death hook.** Whether a death list already exists to iterate, or the
  stage must diff the live set / run inside the existing death-handling path
  (alongside `carcass_step`). The plan's first task should locate this and
  confirm the RNG-free, order-deterministic capture of dead agents' inventories.
- **Is conservation alone sufficient?** Expected yes (conservation + any positive
  harvest ⇒ non-decreasing goods). The plan should verify with the long-horizon
  contrast; if the late-window volume is merely nonzero but very low, note a
  follow-up to add lever B, but do not pre-build it.
