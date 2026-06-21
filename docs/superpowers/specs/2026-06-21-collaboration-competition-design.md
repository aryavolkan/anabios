# anabios — collaboration & competition behaviors (M11–M16)

**Status:** approved, pre-implementation
**Date:** 2026-06-21
**Parent design:** [`2026-05-23-anabios-design.md`](2026-05-23-anabios-design.md)

## 1. Goal

The base design (§4.3, §4.4) names collaboration and competition as targeted
emergent phenomena, and the program AST already carries the relevant output
nodes — but they are **inert stubs**. This spec plans the milestones that turn
those stubs into working substrate, adds the detectors that recognize the
resulting behaviors, and confirms each with integration tests.

### Current gap (verified against the codebase)

| Area | Designed | Implemented today |
|---|---|---|
| `interact.rs` | feeding, combat, mating, pheromone emission (§3.7 step 5) | **feeding (grazing) only** |
| Program outputs | `FireWeapon`, `EmitPheromone`, `Broadcast` | parsed but **no-ops** in the evaluator (`program.rs:344`) |
| `SenseMeme` | reads received meme value | always returns **`0.0`** (`program.rs:282`) |
| Sensing | kin, threat, pheromone, meme channels | only plant + a **single undifferentiated nearest neighbor** (+ its species id) |
| `culture.rs` / `meme_vector` | meme transmission & drift (§3.1, §3.7 step 7, §4.4) | **does not exist** |
| Pheromone fields | per-channel `128×128` grids + decay (§3.6, §3.7 step 9) | **does not exist** |
| `EventType` | population, spatial, evolution, culture, named-behavior detectors | **6 variants**, none social/competitive |
| Sweep CSV | per-event columns | 6 columns, hardcoded in `sweep.rs` |

### Decisions captured from brainstorming

- **Scope:** all four families — competition (combat+predation, territorial/resource)
  and collaboration (signaling/communication, cooperation/kin).
- **Test bar:** both **mechanism** tests (deterministic, CI) and **emergence**
  tests (multi-seed, reusing the M10 sweep machinery).
- **Emergence bar:** **seeded starters are acceptable** (design §3.4 archetypes).
  Behaviors need not evolve from neutral genomes; tests confirm the
  mechanic + detector work and the behavior persists/spreads under selection.
- **Structure:** Approach A — dependency-ordered vertical slices. Foundation
  first, one milestone per family, co-evolution capstone last. Each milestone is
  independently shippable (design §12).

## 2. Shared conventions

### 2.1 Per-milestone deliverable template

Every behavior milestone (M12–M15) ships four things; M11 is foundation-only
(no detectors/emergence) and M16 is integration/hardening.

1. **Substrate** — new sim mechanics in `anabios-core` (sense channels, action
   wiring, fields, interaction rules). Determinism rules of design §7.2 apply:
   id-ordered iteration in serial stages, no reductions over unordered
   collections, no `HashMap` iteration in the tick path.
2. **Detector(s)** — new `EventType` variants + pure detector fns in `codex.rs`,
   wired into `observe_all`, and into the headless sweep (`sweep::event_name`
   match + `write_summary_csv` header — both currently hardcoded for 6 events).
3. **Mechanism tests** (`crates/anabios-core/tests/`) — handcrafted minimal
   worlds proving each substrate rule, and proving each detector fires exactly
   once on a positive case and never on a matched negative case (design §7.1
   "detector tests").
4. **Emergence test + scenario** — a tuned `scenarios/*.toml` with seeded
   starters, plus a multi-seed test asserting the behavior appears in ≥X% of
   seeds.

### 2.2 How emergence tests stay CI-safe

Determinism is already guaranteed (single `Xoshiro256++`, golden-tick tests,
`headless-determinism` job — design §7.2, §9.2). An emergence test is therefore
itself deterministic: a fixed set of seeds run for a fixed tick budget produces
identical events every run, so the assertion is a **hard threshold**, not a
probabilistic one. Procedure:

- Use a small seed count (target 16) and a short per-scenario tick budget.
- During implementation, measure the actual pass rate and assert a floor
  comfortably below it (e.g. observed 14/16 → assert ≥10/16) so unrelated
  tuning drift doesn't make the test flaky.
- Gate these behind `--release` (a dedicated test group) so debug CI stays fast;
  they reuse the in-process equivalent of `sweep::run`.

### 2.3 Golden-tick / snapshot impact

Changes to `SensorRegister`, `ActionRegister`, agent buffers (`meme_vector`),
and `World` (pheromone fields) alter snapshot layout and tick-hash values. Each
such milestone refreshes the committed golden-tick hashes via the existing
manual-approval path (design §9.2 step 5) and keeps `snapshot.rs` round-trip
tests green.

## 3. Milestones

### M11 — Interaction substrate foundation

**Goal:** make the inert AST nodes real and give programs something to react to.
Pure enablement; no emergent behavior claimed. **Mechanism tests only.**

**Substrate:**
- Extend `SensorRegister` + `sense.rs`, computed from the existing spatial-hash
  neighbor scan:
  - nearest **same-species** vs nearest **other-species** neighbor (dist + dir).
  - nearest neighbor **relative size/energy** (predator-vs-prey signal).
  - scalar **local crowding** (neighbor count within radius).
- Add matching live `Sense*` AST nodes for the new channels (generalizing the
  pattern that currently leaves `SenseMeme` hardcoded to `0.0`).
- Extend `ActionRegister` beyond `move_x/move_y/feed_intent/mate_intent` with
  `fire_intent`, `emit_intent(channel)`, `broadcast_intent(channel, value)`, and
  a resolved **action target** agent id (derived from the nearest-neighbor
  sense). **Plumbed and stored in M11, consumed starting M12** — M11 asserts
  they are populated correctly, not that they have effects.
- Expand the starter-program library (design §3.4) with the social archetypes
  later milestones seed: `Stalker`, `PackHunter`, `Sentinel`, `Herd`.

**Detectors:** none.

**Mechanism tests:**
- Sense channels: 2–3 agent worlds asserting same/other-species nearest,
  relative-size sign, and crowding count are exact.
- Action plumbing: programs built from the new nodes produce the expected
  intents and target id.
- Starter programs parse, stay under the 64-node cap, evaluate without panic.
- Golden-tick refresh (layout change).

### M12 — Competition I: combat & predation

**Goal:** first real emergent behavior — predator/prey dynamics. Seeded with
`Stalker`/`PackHunter`.

**Substrate (consumes M11 intents):**
- **Combat — wire `FireWeapon`.** In `interact.rs`, an agent with `fire_intent`
  above threshold *and* a `Weapon` module (gating, §3.5) deals
  `Weapon.damage` to its action-target within contact range, reduced by the
  target's `Armor.protection`, spending the attacker's `Weapon.energy_cost`.
  Lethal damage uses the normal death path (freelist, lineage preserved).
- **Predation — carnivore Mouth on flesh.** Extend feeding so a meat-affinity
  `Mouth` biting a killed/dead agent in range converts a fraction of the
  victim's remaining energy into the eater's energy. Closes the trophic loop
  (carnivores currently have no food source).
- **Determinism:** combat and predation run in the already-serial `interact()`,
  iterating in id order; attacker-id breaks ties on a shared target.

**Detectors (new `EventType`):**
- `Predation` — first agent death attributable to another agent's bite/kill (vs
  starvation/age). Payload: predator species id.
- `CombatRaid` — combat deaths between two species cross a rolling regional rate
  threshold (design §4.4 `WarOrRaid`); distinguishes sustained conflict from
  one-off predation.
- `ArmsRace` — mean `Weapon.damage` of one species and mean `Armor.protection`
  of an interacting species both trend upward over a window. **Detector ships in
  M12; emergence confirmation deferred to M16** (co-evolution capstone).

**Mechanism tests:**
- Attacker + target in range → target loses exactly `damage − armor`; attacker
  loses `energy_cost`. No `Weapon` → no damage (gating). Out of range → none.
- Carnivore Mouth + carcass → eater gains, carcass depletes; herbivore Mouth →
  no flesh gain.
- `Predation` fires once on a kill, never on starvation. `CombatRaid` fires on
  sustained two-species conflict, not on a single kill.

**Emergence test + scenario:**
- `scenarios/predator-prey.toml`: grazers + seeded `Stalker` carnivores on a
  supporting biome. Multi-seed run asserts `Predation` fires in ≥X% of seeds and
  both populations persist past a crash-only baseline (minimal coexistence —
  predators don't instantly starve, prey aren't instantly wiped).

### M13 — Pheromone fields & territorial competition

**Goal:** spatial competition. Also builds the pheromone infrastructure M14
reuses, hence before communication. Food-patch resource contest stays implicit
in existing shared-biomass grazing (no dedicated detector).

**Substrate (consumes `emit_intent`):**
- **Pheromone fields** (design §3.6): per-channel `128×128` grids (start 4) owned
  by `World`. Agents with a `Pheromone` module + `emit_intent` deposit into the
  cell at their position during `interact()`; fields decay exponentially each
  tick (`pheromone_decay()`, design §3.7 step 9, currently unwired).
- **Pheromone sensing:** the M11 sense layer samples the local pheromone
  gradient, gated by a `smell` `Sensor` module (§3.6). Programs read it via a
  `SensePheromone(channel)` node (the `SenseBiome(pheromone_channel)` path).
- **Determinism:** field writes in serial `interact()` in id order; decay is a
  pure per-cell map.

**Detectors (new `EventType`):**
- `TerritoryFormation` — a species maintains a spatially-clustered, persistent
  pheromone footprint others avoid (design §4.2). Uses per-species centroid
  history + a new pheromone-coverage stat.
- `NichePartitioning` — two species' distributions over biome terrain types
  diverge below an overlap threshold and stay there (design §4.2/§4.3).

**Mechanism tests:**
- Emit → correct cell gains pheromone; no `Pheromone` module → nothing (gating).
  Decay reduces a cell by the exact rate over K ticks. `smell`-sensored agent
  reads a planted gradient; sensorless agent reads zero (gating).
- `TerritoryFormation` fires on clustered marking, not uniform mixing.
  `NichePartitioning` fires on disjoint terrain occupancy, not overlap.

**Emergence test + scenario:**
- `scenarios/territories.toml`: mark-and-avoid starter on a patchy biome.
  Multi-seed run asserts `TerritoryFormation` (and/or `NichePartitioning` on a
  two-species variant) fires in ≥X% of seeds with stable clustering.

### M14 — Communication & culture (signaling)

**Goal:** information flow — alarm calls, dialects, meme spread. Introduces
`culture.rs`, the one major designed module never implemented.

**Substrate (consumes `broadcast_intent`):**
- **`culture.rs` + meme vectors.** Add the `meme_vector: Vec<[f32; 8]>` agent
  buffer (design §3.1, currently absent). A new `culture_step()` tick stage
  (§3.7 step 7, unwired) transmits meme values to neighbors with imperfect copy
  (drift), gated by a `Communicator` module.
- **`Broadcast` / `SenseMeme` wiring.** `Broadcast(channel)` writes the agent's
  meme value to a channel; `SenseMeme(slot)` returns the real received value
  (today hardcoded `0.0`). Makes memes adaptive, not decorative (§4.4).
- **Inheritance:** child meme vector = parent average + jitter (§3.8).
- **Determinism:** transmission iterates neighbor pairs in id order; meme
  averaging is order-fixed.

**Detectors (new `EventType`):**
- `DialectFormed` — geographically separated subpopulations of one species
  develop persistently divergent meme distributions (§4.4).
- `MemeSweep` — a meme value rises rare→dominant across a species in a window.
- `AlarmCall` — a broadcast on a channel reliably precedes a flee/`MoveAway`
  response in nearby receivers (§4.3/§4.4). **Detector ships in M14; if its
  standalone emergence proves marginal, emergence confirmation folds into M15.**

**Mechanism tests:**
- Broadcast → neighbor with `Communicator` + `SenseMeme` reads it next step; no
  `Communicator` → nothing (gating). Drift moves the value by the configured
  amount. Child inherits parent meme average.
- `DialectFormed` fires on two isolated divergent subpops, not a mixed pop.
  `MemeSweep` fires rare→dominant. `AlarmCall` fires on broadcast→flee
  correlation above threshold, not on uncorrelated broadcasts.

**Emergence test + scenario:**
- `scenarios/dialects.toml`: `Communicator` population split across a biome
  barrier (terrain-affinity isolation, §4.2), seeded `Sentinel`. Multi-seed run
  asserts `DialectFormed` or `MemeSweep` fires in ≥X% of seeds.

### M15 — Cooperation & kin

**Goal:** the collaboration payoff. Combines M12 (combat), M13 (pheromones),
M14 (signaling). Pack hunting and herd cohesion are **compositions of earlier
primitives, not new primitives** — so this milestone is detector- and
tuning-heavy rather than a large new-substrate build.

**Substrate:**
- **Kin recognition in sensing.** Extend the M11 sense layer with a kinship
  channel from `parent_ids`/`lineage_id` (shared ancestry) + genome distance
  (buffers exist, §3.1, but aren't sensed). New `SenseKinship` node returns
  relatedness of the nearest neighbor — gates altruism on kin, which makes
  cooperation stable.
- **Food sharing / altruism action.** New interact rule: an agent with
  `share_intent` transfers a fraction of its energy to a target, scaled by the
  `altruism` genome slot (§3.2). Donor loses, recipient gains.
- **Pack hunting** = no new primitive: `AlarmCall`/broadcast (M14) coordinating
  multiple `FireWeapon` attackers (M12) on one target. Adds the `PackHunter`
  starter tuning + detector.
- **Herd cohesion** = M11 crowding sense + `MoveToward` same-species reducing
  per-capita predation risk. Adds starter + detector.

**Detectors (new `EventType`):**
- `EvolvedCooperation` — reciprocal/kin-directed energy sharing persists above a
  rate threshold (§4.3).
- `PackHunting` — ≥N same-species agents deal combat damage to one target in a
  short window (§3.4 Pack Hunter).
- `HerdCohesion` — a species maintains persistent clustering that measurably
  lowers its predation rate vs a dispersed baseline (§4.2).
- Plus `AlarmCall` emergence confirmation if it was marginal in M14.

**Mechanism tests:**
- Share transfers exact energy donor→recipient, scaled by `altruism`; zero
  altruism → no transfer. `SenseKinship` high for siblings/parent-child, low for
  unrelated.
- `EvolvedCooperation` fires on a sharing population, not a selfish one.
  `PackHunting` fires when 3 agents hit one target in a window, not on solo
  kills. `HerdCohesion` fires on a clustered low-predation species, not a
  dispersed one.

**Emergence test + scenarios:**
- `scenarios/cooperation.toml` (kin-sharing under starvation pressure →
  `EvolvedCooperation`) and `scenarios/pack-vs-herd.toml` (seeded `PackHunter`
  vs `Herd` → `PackHunting` and/or `HerdCohesion`). Multi-seed runs assert the
  relevant detectors fire in ≥X% of seeds.

### M16 — Co-evolution capstone & emergence gates

**Goal:** confirm the families interact, host the deferred emergence claims, and
harden emergence into CI. Light on new substrate; heavy on scenarios, tuning,
and tooling.

**Substrate / tooling:**
- **Sweep integration audit.** Ensure every `EventType` added in M12–M15 has a
  `sweep::event_name` entry and a `write_summary_csv` column (both currently
  hardcoded). Add an in-process emergence-harness helper shared by all
  per-milestone emergence tests (avoids each test re-implementing seed loops).
- **Co-evolution scenarios** where competition and collaboration co-occur:
  - `scenarios/arms-race.toml` — weaponed predators vs armored/herding prey;
    confirms the **`ArmsRace`** emergence deferred from M12 (rising
    weapon-vs-armor trend across interacting species).
  - `scenarios/pack-vs-herd.toml` (shared with M15) extended to long runs for
    sustained dynamics.
- **Balancing pass.** Tune energy costs (weapon/armor upkeep, share fraction,
  pheromone decay, communicator range) so no single strategy trivially
  dominates and the emergence thresholds in M12–M15 hold with margin.

**Detectors:** no new variants; `ArmsRace` (defined M12) gets its emergence
scenario and test here.

**Tests:**
- **`ArmsRace` emergence** in `scenarios/arms-race.toml` (≥X% of seeds show the
  co-trend).
- **CI emergence gate.** A `--release` test group runs the full set of emergence
  scenarios and asserts each family's headline detector clears its floor. This
  is the standing regression guard that collaboration/competition behaviors keep
  working as the sim evolves.
- Final golden-tick + `headless-determinism` confirmation across all new
  scenarios.

## 4. Dependency graph

```
M11 foundation (sensing + action plumbing + starters)
 ├─► M12 combat & predation ─────────────┐
 ├─► M13 pheromone fields & territory ────┤
 │      (pheromone infra) ───────────────►│
 └─► M14 communication & culture ─────────┤
        (signaling) ─────────────────────►│
                                          ▼
                              M15 cooperation & kin
                          (pack hunting = M12+M14+kin;
                           herd cohesion = M11+M12)
                                          │
                                          ▼
                              M16 co-evolution capstone
                          (ArmsRace + AlarmCall emergence,
                           sweep gates, balancing)
```

M12, M13, M14 each depend only on M11 and are mutually independent (could be
built in any order or in parallel). M15 depends on M12 + M14 (+ kin sensing).
M16 depends on all.

## 5. New `EventType` variants (summary)

Appended to the existing enum (`Extinction=0 … NovelBehaviorPattern=5`), each
also wired into `sweep::event_name` + the CSV header:

| Milestone | Variants |
|---|---|
| M12 | `Predation`, `CombatRaid`, `ArmsRace` |
| M13 | `TerritoryFormation`, `NichePartitioning` |
| M14 | `DialectFormed`, `MemeSweep`, `AlarmCall` |
| M15 | `EvolvedCooperation`, `PackHunting`, `HerdCohesion` |

## 6. Out of scope

- Evolving these behaviors from neutral genomes (seeded starters accepted).
- A dedicated resource-contest detector (covered implicitly by grazing).
- Rendering of new substrates (pheromone viz, combat flashes) — that is a
  Godot-side concern tracked under the M7-rendering line, not here.
- W&B sweep dashboards beyond the existing CSV/JSONL outputs.

## 7. Definition of done

- Every milestone's mechanism tests pass in debug CI.
- Every behavior family's emergence test clears its floor in the `--release`
  emergence group, and the M16 gate runs them as a standing regression.
- `headless-determinism` and golden-tick jobs green across all new scenarios.
- All 11 new `EventType` variants appear as `summary.csv` columns from
  `anabios-headless sweep`.
