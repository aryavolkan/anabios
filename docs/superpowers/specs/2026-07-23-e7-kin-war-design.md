# E7 — Kin Networks & War — Design Spec

**Date:** 2026-07-23
**Status:** Approved
**Milestone:** E7 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crate:** `anabios-core` (war substrate + detectors), wiring + minimal viewer. Behavior-affecting only via the new `SenseHostility` program input — the mutation pool only emits it behind a new `war_enabled` scenario flag, so baseline scenarios stay byte-identical. `FORMAT_VERSION` 12→13 (hostility records, detector scratch, program Node variant appended), goldens regenerated once.

## 1. Goal & success criteria

Group-level conflict and durable kin structure (design §4.4). Four new event types: `WarOrRaid` (38), `WarEnded` (39), `AllianceFormed` (40), `KinNetworkStable` (41).

Success criteria:

1. Sustained inter-species killing produces an explicit hostility record (score, kill count, front location) — distinct from the one-off `CombatRaid` burst detector.
2. Wars end: `WarEnded` fires when hostility decays without reinforcement, with the war's duration as its value.
3. Agents can *sense* war: `SenseHostility` program input reads the hostility of the nearest other-species neighbor's species toward the agent's own (0 when `war_enabled` is off / no hostility). Mutation only introduces the node behind the flag.
4. Each detector has positive + negative handcrafted tests; 16-seed sweep on `war.toml` fires war events in most runs and at least one `AllianceFormed`/`KinNetworkStable` somewhere.

## 2. Hostility substrate (`codex/war.rs`)

- Feed: combat-attributed deaths (`combat_damaged` + `combat_attacker` already recorded per death in `age_and_starve`). On a death where attacker species ≠ victim species: `hostility[ordered_pair].score += 1`, `kills += 1`, front = running mean of death locations, `last_kill_tick`.
- Decay: every tick, every pair's score ×`WAR_DECAY = 0.995`.
- **WarOrRaid:** score crosses `WAR_THRESHOLD = 12` (rising edge, latched `war_since`) — `value` = kills so far, loc = front. Re-arms only after `WarEnded`.
- **WarEnded:** a latched war whose score stays below `WAR_THRESHOLD / 2` for `WAR_END_TICKS = 200` consecutive ticks — `value` = war duration in ticks.
- `SenseHostility`: `SensorRegister.hostility = hostility[(own, nearest_other_species)].score / WAR_THRESHOLD`, clamped [0,1]. New `Node::SenseHostility` (appended variant — existing program encodings byte-identical); evaluator reads it; random-node mutation emits it only when `world.war_enabled`.

## 3. Alliance & kin detectors

- **`AllianceFormed`** — ordered species pair with: mean-meme L2 distance < `ALLIANCE_MEME_MAX = 0.3`, **zero** cross kills over `ALLIANCE_WINDOW = 400` ticks, and ≥ `ALLIANCE_MIN_SHARES = 5` energy shares between them (share records gain the recipient species). One-shot per pair. `value` = shares in window.
- **`KinNetworkStable`** — a species (genetic cluster = kin by construction) sustaining ≥ `KIN_MIN_MEMBERS = 10` members and RMS spatial spread ≤ `KIN_SPREAD_MAX = 100` units over `KIN_WINDOW = 1500` ticks. One-shot per species (re-arms on collapse). `value` = window ticks.

## 4. Viewer & scenario

- HUD: `· N wars` when any pair is at war (gdext `active_wars()`); war events appear in the codex panel/ticker automatically.
- `scenarios/war.toml`: two armed territorial species (stalker pack + bruiser band) contesting one central resource corridor, plus a neutral communicator species as alliance candidate. Menu entry.

## 5. Testing & evidence

- Unit: hostility accumulation/decay; war declare/end edges; SenseHostility reads 0 when disabled; alliance requires no-kills (one cross-kill resets); kin network needs both size and persistence.
- Integration: war.toml long run fires ≥1 war event; replay one.
- Sweep: 16 seeds × 6000 ticks; counts in completion notes.
- Gallery: codex tally with WarOrRaid live during a border war.
