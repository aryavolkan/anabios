# E8 — Settlements & Economy — Design Spec

**Date:** 2026-07-23
**Status:** Approved
**Milestone:** E8 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crate:** `anabios-core` (substrate + detectors), wiring + overlay. `FORMAT_VERSION` 13→14; goldens regenerated once. The anchoring substrate is gated behind a new `settlement_enabled` scenario flag (off = byte-identical baselines, same pattern as `war_enabled`); the market field and harvest experience are observability+effect fields that only exist when `resources_enabled`.

## 1. Goal & success criteria

Emergence of *place* — the step from roaming trade to anchored civilization, building on the trade-goods and geographic-trade batches. Three new event types: `SettlementFormed` (42), `MarketEmerged` (43), `SpecializationSplit` (44).

Success criteria:

1. Agents learn a home anchor (EMA of position), inherit it with drift, and can steer home — anchor cohesion measurably changes where species live when `settlement_enabled`.
2. Trade deposits a decaying density field on the biome; dense cells read as markets without any hardcoded marketplace object.
3. Harvest experience makes producers specialize; the split detector sees bimodal producer classes inside one species.
4. Positive + negative tests per detector; 16-seed sweep on `settlement.toml` fires ≥2 of the 3 types in most runs.

## 2. Home-range anchoring

- `AgentBuffers.anchor: Vec<Vec2>` (serialized). New agents anchor at spawn position.
- Each tick (gated `settlement_enabled`): `anchor += (pos − anchor) × ANCHOR_LEARN_RATE × Territoriality` (genome slot 15, previously reserved — now read; neutral 0.5 → half-rate learning).
- Reproduction: child anchor = mean of parents' anchors + Gaussian jitter (`ANCHOR_DRIFT_SIGMA = 4`).
- Program inputs (appended, serde-stable): `SenseAnchorDirX`, `SenseAnchorDirY`, `SenseAnchorDist` — the *direction home*, evolvable. Mutation pool gated on `settlement_enabled` (war_enabled pattern).
- Homing pull (gated): `desired += Territoriality × ANCHOR_PULL × unit(anchor − pos)` in `decide_all` — same pattern as the biome-adaptation pull.

## 3. Market field

- `World.market_field: Vec<f32>` per biome cell (serialized; sized to biome res at instantiate when `resources_enabled`, empty otherwise — no layout effect when the trade economy is off).
- Each successful swap deposits `MARKET_DEPOSIT = 1.0` at the initiator's cell; the field decays ×`MARKET_DECAY = 0.999` per tick. Pure field property — markets emerge where trade is, they are not placed.
- `MarketEmerged`: a cell sustains density ≥ `MARKET_NODE_THRESHOLD = 20` over a 400-tick streak → fire once per cell neighborhood (latch keyed by cell, re-arm below half). `value` = density; loc = cell center. (Reinforcement — traders preferentially routing to markets — lands with E9's institutional memory; documented.)

## 4. Harvest experience & specialization

- `AgentBuffers.harvest_exp: Vec<[f32; GOOD_COUNT]>` (serialized). Each harvest of good k adds `exp[k] += HARVEST_EXP_RATE`; harvest amount × `(1 + min(exp[k], EXP_CAP) × SPECIALIZATION_GAIN)` — practice makes the specialist.
- `SpecializationSplit`: within one species, two distinct goods each claimed by ≥ `SPECIALIZATION_MIN_CLASS = 20%` of members whose experience share in that good ≥ 60% → fire once per species. `value` = smaller class fraction.

## 5. Settlement detector

`SettlementFormed`: per species (with `settlement_enabled`), RMS spread of member **anchors** ≤ `SETTLEMENT_SPREAD_MAX = 60` over a 400-tick streak with ≥ `SETTLEMENT_MIN_MEMBERS = 10` → fire once per species (re-arm on dispersal). `value` = anchor spread; loc = anchor centroid. Anchors decouple the settlement from the agents' day-to-day wandering — the settlement is the *place they return to*.

## 6. Viewer & scenario

- gdext `market_colors() -> PackedColorArray` (amber heat over the biome layer) + a ground overlay mode "markets" in the [G] cycle (gated on `resources_enabled`); `anchor_data() -> PackedVector2Array` (anchor→agent line endpoints for the settlement overlay, drawn like trade routes in umber).
- `scenarios/settlement.toml`: extends `geographic-trade.toml` — four goods species on the terrain junction, `settlement_enabled = true`. Menu entry.

## 7. Testing & evidence

- Unit: anchor EMA + inheritance drift; market deposit/decay/threshold latch; specialization bimodal split vs uniform negative; settlement anchor-spread vs position-spread distinction.
- Integration: settlement.toml long run fires ≥1 type; replay one event.
- Sweep: 16 seeds × 8000 ticks; counts in completion notes.
- Gallery: market overlay showing the amber node at a border crossing + settlement anchor lines.
