# E8 — Settlements & Economy — Implementation Plan

**Goal:** Home-range anchoring + market field + harvest experience substrate and SettlementFormed/MarketEmerged/SpecializationSplit detectors, per `docs/superpowers/specs/2026-07-23-e8-settlement-economy-design.md`.

**Determinism:** `FORMAT_VERSION` 13→14 + one golden regen (layout only). Anchoring + anchor Sense nodes gated behind `settlement_enabled`; market/experience effects ride `resources_enabled`. Flags off ⇒ byte-identical.

---

## Task S1: buffers + flags + constants
- EventTypes 42–44 + constants; `AgentBuffers.{anchor, harvest_exp}`; `World.{settlement_enabled, market_field}`; scenario flag + market field sizing at instantiate.

## Task S2: `settlement.rs` substrate
- `anchor_step` (EMA, torus-safe), `anchor_pull_parts`/`anchor_sense_parts` (rayon-safe), `market_deposit`/`market_decay_step`, `experienced_harvest`/`gain_harvest_exp`. Tick stage 4c.
- Reproduction: child anchor = torus-midpoint + drift (flag-gated RNG).
- decide_all homing pull (gated); SenseAnchorDirX/Y/Dist (kinds 44–46, appended) joining the mutation pool gated on `settlement_enabled` (20-base pool preserved flag-off).

## Task S3: trade/harvest hooks
- `trade_pass` → `market_deposit` per swap; `harvest_pass` → experience-adjusted rate + exp gain.

## Task S4: `codex/settlement.rs` detectors
- SettlementFormed (anchor RMS ≤ spread-max over 400-tick streak), MarketEmerged (per-cell density streak, re-arm below half), SpecializationSplit (≥2 producer classes ≥20% at ≥60% exp share).
- Unit tests per detector incl. dispersed-anchor, thin-cell, uniform-producer negatives.

## Task S5: FORMAT_VERSION 13→14 + goldens; wiring (score 45 names, CSV +3, panel +3)

## Task S6: viewer + scenario + evidence + PR
- gdext `resources_active`/`market_colors`; "markets" ground overlay mode (gated); `scenarios/settlement.toml` (geographic-trade chassis + settlement_enabled) + menu; integration test; replay; 16-seed sweep; gallery market capture; branch `e8-settlement-economy` stacked on `e7-kin-war`; PR.

---

## Completion notes (2026-07-23)

All tasks complete. Evidence:

- **Sweep (16 seeds × 6000 ticks, `settlement.toml`):** MarketEmerged 16/16, SpecializationSplit 16/16, SettlementFormed 4/16 (settlements are the honest minority — anchors cohere only when the local dynamics let them).
- **Replay verification:** `PASS market tick=413 hash_ok=true refired=true`.
- **Tuning iterations (real-run driven):**
  - Anchor EMA 0.002 → 0.01 and pull 0.3 → 0.5: at the original rate (time constant 1000 ticks) no settlement ever formed before the world crashed; at 200-tick tc they form by t≈1650.
  - Market threshold 20 → 40 (node quality over node count; 67→39 events on the scenario seed, each a distinct crystallizing node).
  - Settlement spread gate 60 → 100 after zero formations (anchor spread tracks border roaming at ~80 units).
- **Tests:** 9 settlement unit tests (incl. anchor EMA/inheritance, dispersed-anchor negative, market latch, thin-cell negative, bimodal-vs-uniform) + `tests/settlement_economy.rs` integration (15 s debug).
- **Determinism:** `FORMAT_VERSION` 13→14; all three golden suites regenerated once (flags off — layout only).
- **Gallery:** `e8-market.png` — amber market node at the geographic-trade hub with trade-route streaks crossing it; `Market: 36 Specialists: 2` in the tally, 113,486 trades in the HUD.
- **Deferred (noted in spec):** market reinforcement (traders routing to nodes) lands with E9's institutional memory; anchor-line overlay (market overlay shipped instead).
