# E6 — Named Behaviors Completion — Implementation Plan

**Goal:** +4 named-behavior detectors (EvolvedAmbush, EvolvedTool, EvolvedFlight, StructuredSignaling) per `docs/superpowers/specs/2026-07-23-e6-named-behaviors-design.md`.

**Determinism:** observability only; `FORMAT_VERSION` 11→12 + one golden regen. No behavior change.

---

## Task B1: instrumentation
**Files:** `world.rs`, `tick.rs`, `interact.rs`, `codex/mod.rs`.
- `World.still_ticks` + `prev_desired_direction` (`#[serde(skip)]` scratch); `resize_scratch` covers both.
- `signatures::update_still_ticks` as tick stage 4b (after integrate, before interact).
- `CombatHit` += `ambush`/`tool_boosted` (fire-time context); `SigHit` rolling log in `combat_pass`.
- EventType 34–37 + constants + `EVENT_TYPE_COUNT`.

## Task B2: `codex/signatures.rs` detectors
- `detect_ambush_and_tool` (rolling 400-tick hit-share; tool = ≥30% Metalworking adoption + ≥1 boosted hit), `detect_flight` (sustained fast barrier crossings + 1.5× world-mean speed, lineage-root latch), `detect_structured_signaling` (broadcast → ≥3 same-species receivers *steering toward* the caller; rate-limited).
- observe_all wiring; unit tests positive + negative per detector.

## Task B3: FORMAT_VERSION 11→12 + goldens
## Task B4: wiring (score 38 names, sweep CSV +4, panel +4)
## Task B5: scenario + evidence + gate + PR
- `scenarios/tool-users.toml`; integration test; replay one event; 16-seed sweep; gallery capture; branch `e6-named-behaviors` stacked on `e5-trait-evolution`; PR.

---

## Completion notes (2026-07-23)

Shipped with **two of four chapters discovered in real runs, two honestly undiscovered** — the detectors are unit-tested and the codex's hidden-entry design (§6.5) plus the E1 novelty archive exist precisely for the unobserved ones.

- **Sweep (16 seeds × 6000 ticks, `tool-users.toml`):** EvolvedFlight 14/16, StructuredSignaling 16/16, EvolvedAmbush 0/16, EvolvedTool 0/16.
- **Replay verification:** `PASS structured_signaling tick=781 hash_ok=true refired=true` (gene-culture-alarm).
- **Detector honesty iterations (all observed in real runs):**
  - StructuredSignaling fired at **tick 0** from spawn-tick direction coincidence — now requires steering *change* (alignment improving tick-over-tick via `prev_desired_direction`) plus a per-species rate limit. Fires legitimately at t=460+ now.
  - EvolvedFlight: fast water-crossing alone fired 500×/run (every species walks through lakes). Now requires sustained quarterly crossings **and** mean module speed ≥1.5× the world mean (relative adaptation) with a **lineage-root latch** (splinters don't refire): 552→1 on gene-culture-alarm.
  - EvolvedTool semantics widened from hit-share to **≥30% Metalworking adoption + ≥1 boosted hit** — the sim's combat is bursty (herds are slaughtered in ~1000 ticks), so any hit-share gate washes out.
- **Undiscovered chapters (documented near-misses):**
  - **EvolvedAmbush:** all starter programs are pursuit-style; sit-and-wait hunting must *evolve* and hasn't in 6000-tick runs across any combat scenario. Negative-tested (mobile shooter doesn't fire).
  - **EvolvedTool:** systematic timeline gap — prey herds are slaughtered or hunter-proofed by ~1000–1400 ticks, while Metalworking (era 3) arrives ~1200–2600 even with max-Openness innovators (best observed overlap: discovered t=1179, hunting dead ~1400, adoption incomplete). Tried: hunter count/prey density/world size sweeps, innovator-commune diffusion, gene-culture-hunt chassis. The E1 novelty archive will flag the first world that closes the gap.
- **Tests:** 6 signature unit tests (ambush latch, mobile-shooter negative, boosted-without-adoption negative, adoption-without-hits negative, still-ticks accumulator) + `tests/named_behaviors.rs` integration (8.4 s debug).
- **Determinism:** `FORMAT_VERSION` 11→12 (CombatHit context + scratch); all three golden suites regenerated once, behavior unchanged.
- **Gallery:** `e6-named-behaviors.png` — gene-culture-alarm at t=1531 with `Flight: 1 Signaling: 1` live in the tally.
