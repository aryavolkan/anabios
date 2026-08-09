# Affect Layer — Interface Contract (shared across M-A…M-F)

> Coordination artifact for the primitive-brain affect layer milestone arc.
> Spec: `docs/superpowers/specs/2026-08-02-primitive-brain-affect-layer-design.md`.
> Every milestone plan references these signatures so independently-written
> plans stay type-consistent. **M-A creates them; M-B…M-F extend/consume them.**
> Names here are normative — do not rename in a plan without updating this file.

## Module layout

- New file `crates/anabios-core/src/affect.rs` — all Layer-0/Layer-1 logic +
  read-side hooks + factor helpers. Registered in `lib.rs` (`mod affect;`)
  alongside `mod iq;`.
- Genome temperament accessors live in `genome.rs` (next to
  `cognitive_potential()`, ~`genome.rs:338`).
- Per-agent state column in `agent.rs` (next to `iq`, ~`agent.rs:66`).
- World flag in `world.rs`; Scenario field in `scenario.rs`.
- Tick wiring in `tick.rs`.

## Constants & indices (`affect.rs`) — M-A

```rust
pub const AFFECT_SYSTEMS: usize = 7;
// Activation indices into AffectState:
pub const SEEK: usize = 0;
pub const FEAR: usize = 1;
pub const RAGE: usize = 2;
pub const LUST: usize = 3;
pub const CARE: usize = 4;
pub const PANIC: usize = 5; // PANIC/GRIEF (separation distress)
pub const PLAY: usize = 6;

/// Hijack fires when threat arousal >= this (before Reactivity modulation).
pub const HIJACK_AROUSAL_THRESHOLD: f32 = 0.6;
// Per-system decay (leaky-integrator retention). Tuned; documented in affect.rs.
pub const LAMBDA_DEFAULT: f32 = 0.8;
```

## Per-agent state (`agent.rs`, serialized — NOT `#[serde(skip)]`)

```rust
/// Per-agent subcortical activations, one per Panksepp system, each in [0,1].
/// Persistent (serialized). Neutral default = all zero.
pub type AffectState = [f32; AFFECT_SYSTEMS];

// AgentBuffers column, next to `iq` (agent.rs:66):
pub affect: Vec<AffectState>,
// Init to [0.0; AFFECT_SYSTEMS] in BOTH spawn branches (reuse agent.rs:166-168,
// push agent.rs:190-192) and the dead-slot reset.
```

M-D adds a second serialized column for PANIC kin-loss detection:

```rust
/// Previous-tick crowding, for separation-distress (PANIC) detection. Serialized.
pub affect_prev_crowding: Vec<f32>, // default 0.0, same two-branch init
```

## Genome temperament accessors (`genome.rs`) — M-A

Rename reserved slots **in place** (no index shift): `_DriveReserved17→Boldness`,
`_DriveReserved18→Aggressiveness`, `_DriveReserved19→Nurturance`,
`_ReproReserved34→Sociality`, `_ReproReserved35→Reactivity`. Update `SLOT_NAMES`.
Each accessor maps the stored `[0,1]` slot to signed `[-1,+1]` (`2·v − 1`), so
the neutral genome value `0.5` → `0.0` (identity). Mirror `openness()` etc.

```rust
pub fn boldness(&self) -> f32;        // slot 17: inverse FEAR setpoint / freeze bias
pub fn aggressiveness(&self) -> f32;  // slot 18: RAGE gain
pub fn nurturance(&self) -> f32;      // slot 19: CARE gain
pub fn sociality(&self) -> f32;       // slot 34: PANIC/PLAY/CARE bond weight
pub fn reactivity(&self) -> f32;      // slot 35: arousal gain / hijack threshold, decay
```

**Speciation:** temperament genes are adaptive → they **count** toward
`Genome::distance` (do NOT add them to `PERSONALITY_MASK`). Decision recorded in
M-A.

## World / Scenario flag — M-A

```rust
// world.rs, among the opt-in flags; doc "off => identity / zero RNG":
#[serde(default)] pub affect_enabled: bool, // default false in World::new
// scenario.rs Scenario struct + instantiate():
#[serde(default)] pub affect_enabled: bool, // w.affect_enabled = self.affect_enabled;
```

Do **not** set it in `scenarios/minimal.toml`. Flag-on scenarios set
`affect_enabled = true` at top level.

## Core functions (`affect.rs`)

```rust
// --- M-A ---
/// Layer-0 homeostatic drive: normalized energy deficit in [0,1]. 0 = sated.
pub fn homeostatic_drive(energy: f32) -> f32;

/// Compute stage. Update world.agents.affect from THIS tick's sensors +
/// physiology (energy/age) + genome. STRICT no-op when !world.affect_enabled.
/// ZERO RNG. Index-disjoint par_iter (iq::develop_all template). Runs
/// post-sense / pre-decide (tick.rs, between :24 sense and :36 decide).
pub fn develop_all(world: &mut World);

/// Aggregate threat arousal from defensive activations (FEAR, RAGE, PANIC).
/// M-A: may return SEEK-free 0.0-baseline; M-B finalizes with the hijack.
pub fn arousal(affect: &AffectState) -> f32;

/// Bias hook. Modulate `action` from current affect + percepts + temperament.
/// EXACT IDENTITY at neutral affect (all-zero) — every block guarded `if x != 0.0`
/// (personality.rs idiom). Called in decide_all right AFTER apply_personality
/// (tick.rs:187-192). M-A implements SEEK; later milestones add their systems.
pub fn apply_affect(
    action: &mut ActionRegister,
    affect: &AffectState,
    genome: &Genome,
    sensors: &SensorRegister,
    energy: f32,
);

/// Movement-speed multiplier from SEEKING + arousal. Exactly 1.0 at neutral.
/// Consumed in integrate.rs alongside personality_speed_factor.
pub fn affect_speed_factor(affect: &AffectState) -> f32;

// --- M-B ---
/// Survival-reflex override. When threat arousal (scaled by Reactivity/Boldness)
/// >= HIJACK_AROUSAL_THRESHOLD, OVERWRITE `action`'s live channels with the
/// Bracha reflex (Freeze→Flight→Fight→Fright→Faint) chosen by threat
/// proximity/escapability, and return true. Otherwise leave `action` untouched
/// and return false. No RNG. Called in decide_all after apply_affect + movement
/// biases, BEFORE desired_direction normalization (tick.rs:258).
pub fn apply_hijack(
    action: &mut ActionRegister,
    affect: &AffectState,
    genome: &Genome,
    sensors: &SensorRegister,
    energy: f32,
) -> bool;

// --- M-C ---
/// Reproduction-threshold multiplier from LUST. Exactly 1.0 at neutral.
/// Consumed in reproduce.rs alongside personality_reproduction_factor.
/// M-A ships an identity stub; M-C implements it.
pub fn affect_reproduction_factor(affect: &AffectState) -> f32;
```

## Trigger dispatch (inside `develop_all`)

`develop_all` updates each activation as a leaky integrator:
`a[k] = λ_k·a[k] + (1−λ_k)·trigger_k(drive, sensors, genome)`, then applies a
short, documented set of lateral-inhibition suppressions, then clamps to `[0,1]`.
Each milestone fills in its systems' `trigger_k` + inhibition edges:

- **M-A:** SEEK (from `homeostatic_drive` + `local_biomass`/`plant_dir`). Others
  remain 0.0.
- **M-B:** FEAR (from `hostility`, threatening `other`, damage). Arousal finalized.
- **M-C:** RAGE (derived frustration), LUST (mate-readiness). Inhibition FEAR⊣RAGE.
- **M-D:** CARE (kin present), PANIC (isolation/kin-loss via `affect_prev_crowding`).
  Inhibition PANIC⊣SEEK.
- **M-E:** PLAY (juvenile+safe+peer), coupling into `iq_enrich_acc`.

## Live output channels ONLY (spec §2.2)

Affect writes only: `move_x/move_y`, `fire_intent`, `share_intent`,
`emit_intent[ch]`, `broadcast_intent[ch]`, `target_id`, and the stage factors
(`affect_speed_factor`→integrate, `affect_reproduction_factor`→reproduce).
**Never** `feed_intent` / `mate_intent` (latent; out of scope).

## Determinism checklist (every milestone)

1. Flag-off byte-identical (no RNG, identity read-side).
2. Zero RNG in `develop_all` even flag-on.
3. New serialized columns → refresh `determinism.rs`, `cognition.rs`,
   `inventions.rs` goldens ONCE (layout growth), dated "flag off ⇒ byte-identical"
   note; bump `FORMAT_VERSION` in `snapshot.rs`.
4. `save→load→step` equality test for new serialized state
   (model `determinism.rs:16-36`).
5. `parallel_matches_serial` must pass (index-disjoint writes).
6. New `Node`/`EventType` variants append at END only.
7. Flag-on golden pins real behavior (model `cognition.rs`).

**Controller-runs-gates rule:** per the project's subagent-driven flow,
implementer subagents Edit/Read only; the controller runs all `cargo`/`git`
gates, golden refreshes (`UPDATE_HASHES=1`), and commits.

## Cross-milestone reconciliation (authoritative — execute in M-A→M-F order)

The milestones ship as sequential PRs (M-A, then B, C, D, E, F). Several plans
touch shared append-only registries; the plan text may name absolute numbers,
but **the numbers below win**. Always `git grep` the current value at execution
time and append after it — never reuse a number an earlier milestone took.

- **`FORMAT_VERSION` (snapshot.rs), one bump per serialized-layout change:**
  - M-A: 23 → **24** (adds `affect` column).
  - M-B, M-C, M-E: **no bump** (no new serialized layout; assert flag-off
    goldens do NOT move).
  - M-D: 24 → **25** (adds `affect_prev_crowding` column).
  - M-F: 25 → **26** (adds serialized `CodexState` detector fields).
- **`EventType` discriminants (codex/event.rs), append-only after
  `LivestockHerd = 52`:**
  - M-B: `MassFright = 53`.
  - M-F: `PanicCascade = 54`, `FeedingFrenzy = 55`, `TerritorialRage = 56`,
    `MassGrief = 57`; `EVENT_TYPE_COUNT = MassGrief as usize + 1` (= 58).
- **Layout-golden refresh** (`determinism.rs`/`cognition.rs`/`inventions.rs`)
  happens ONCE per layout-growth milestone — **only M-A, M-D, M-F**. M-B/M-C/M-E
  must instead *assert* those three goldens are byte-identical (flag-off).
- **Combat-sourced affect impulses** (attack → RAGE in M-C; damage → FEAR): the
  combat scratch (`World.combat_damaged`) is `#[serde(skip)]` and `interact`
  runs AFTER `develop_all`, so it must NOT be read into serialized affect in
  `develop_all` (serde-skip cross-tick replay footgun). Pattern: write the
  impulse into the **serialized `affect` column at combat time** inside
  `interact`/`combat_pass` (M-C Task 7). M-B defers the damage→FEAR term (uses
  `hostility` + threatening-neighbor only); if added later it uses this same
  combat-time-write pattern.
- **Shared test/scenario files:** M-A creates `tests/affect.rs` and
  `scenarios/affect-seeking.toml`; each later milestone **extends** `tests/affect.rs`
  and adds its own flag-on scenario (`affect-threat.toml` M-B,
  `affect-showcase.toml` M-F, etc.). The flag-on golden constant lives in
  `tests/affect.rs` and is refreshed per-milestone as behavior grows (this is
  the *flag-on* golden — distinct from the three flag-off layout goldens).
- **M-D "grief event" done-when:** M-D pins its bar at the *action* level
  (above-threshold alarm broadcast); the `MassGrief` `EventType` + detector are
  owned by **M-F** (§7.1). No `EventType` is added in M-D.
