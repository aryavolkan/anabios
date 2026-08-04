# Primitive-Brain Affect Layer — Design

> A subcortical drive-and-affect layer beneath the evolved behavior program, so
> agent behavior arises from a more biologically faithful cognition stack:
> homeostatic drives → primary-process affective systems → a fast survival
> circuit that can override the slow evolved "cortex".

- **Date:** 2026-08-02
- **Track:** E — Simulation Engine & Mechanics (with V + R + T touchpoints)
- **Status:** Design approved; ready for per-milestone implementation plans.
- **Design goal (chosen by user):** *biological fidelity* — model the primitive
  brain honestly, not the pop-neuroscience "lizard brain".

---

## 1. Motivation & framing

### 1.1 The gap

Today an agent's behavior is **memoryless and reactive**. Each tick,
`behavior::decide` evaluates the evolved `Program` fresh from this tick's
percepts, `personality::apply_personality` biases the result from OCEAN traits,
and that's it. There is no persistent internal state between ticks: no hunger
that *builds*, no fear that *lingers*, no arousal that can *hijack* reasoning.
Nothing sits *underneath* the evolved program as a primitive drive/affect
substrate.

This layer fills that gap: persistent per-agent **drive** and **affect** state
that accumulates and decays over time, is driven by physiology + percepts +
heritable temperament, and modulates (and sometimes overrides) the evolved
program.

### 1.2 Honest biological framing — read this before "lizard brain"

The user asked for "primitive brain states like lizard brain". Since the goal is
**biological fidelity**, the design does *not* rest on the triune-brain /
"reptilian complex" model (MacLean). Modern comparative neuroscience treats that
model as **outdated**:

- Brains do not grow by stacking preserved reptilian modules under mammalian
  ones; ancestral regions are reworked and rescaled. Birds and reptiles show
  complex cognition with no neocortex.
- Function is not cleanly segregated by "layer" — the basal ganglia
  ("reptilian") do reward learning and action selection; the cortex does heavy
  emotion regulation.
- The amygdala runs *survival responses*, it does not "produce the feeling of
  fear" (LeDoux's own later revision). We model **survival circuits**, and make
  no claim that agents *feel* anything.

What we build on instead, all current and defensible:

- **Panksepp's 7 primary-process emotional systems** (affective neuroscience) as
  the "primitive states": **SEEKING, FEAR, RAGE, LUST, CARE, PANIC/GRIEF, PLAY**.
- **Homeostatic drives / drive-reduction** (Hull; Keramati–Gutkin "reward = drive
  reduction") as the accumulator layer that powers SEEKING.
- **Subsumption architecture** (Brooks) as the layering pattern: a fast reflexive
  layer that can *subsume/override* a slower deliberative one. Layers here are
  **functional/temporal (speed & overridability), not evolutionary strata.**
- **Bracha's threat ordering** (Freeze → Flight → Fight → Fright → Faint) and
  **LeDoux's dual-route** ("low road" fast subcortical response, cortical cancel
  path) for the override.

The docs may keep the evocative "primitive/lizard brain" language, but state
explicitly that it is a **functional metaphor, not literal neuroanatomy.**

### 1.3 References

- Panksepp, *Affective Neuroscience* (1998); Montag & Davis (Front. Neurosci.
  2018) for a modern summary.
- Cesario, Johnson & Eisthen, "Your Brain Is Not an Onion With a Tiny Reptile
  Inside" (Curr. Dir. Psych. Sci., 2020) — triune critique.
- Hull, *Principles of Behavior* (1943). Keramati & Gutkin, homeostatic RL
  (NeurIPS 2011; eLife 2014).
- LeDoux, *The Emotional Brain* (1996); "Rethinking the Emotional Brain" (Neuron
  2012). Bracha, "Freeze, Flight, Fight, Fright, Faint" (CNS Spectrums 2004).
- Brooks, subsumption architecture (1986). Cañamero, motivational architectures.
  Moerland, Broekens & Jonker, "Emotion in RL agents" (Machine Learning 2018).

---

## 2. Scope

### 2.1 In scope

- Three functional layers (§3): homeostatic drive core, affective command
  systems, and the coupling into the existing evolved program.
- All **7 Panksepp systems**, graded by how faithfully anabios can support them.
- **Heritable temperament** genes (evolvable setpoints/gains) in reserved genome
  slots.
- **Bias + override** coupling: affect biases the action register; acute threat
  arousal hijacks it with a hard-coded survival reflex.
- Opt-in, determinism-neutral integration (new `affect_enabled` world flag).
- Codex detectors + viewer surfacing for the emergent phenomena.

### 2.2 Non-goals / explicit constraints

- **Affect acts only on already-live action channels.** `feed_intent` and
  `mate_intent` are currently *latent* (no tick rule consumes them —
  `behavior.rs`/`personality.rs` notes; feeding derives bite from
  modules/genome/skill, mating is energy-threshold gated). This work does **not**
  activate their consumption. Affect reaches behavior only through channels that
  are already consumed: `move_x/move_y`, `fire_intent`, `share_intent`,
  `emit_intent` (pheromone), `broadcast_intent` (meme), `target_id`, and the
  stage-level genome factors (movement speed in `integrate`, reproduction
  threshold in `reproduce`).
- **No felt emotion / consciousness claims.** We model survival circuits and
  motivational states.
- **PLAY is modeled minimally** (lowest-fidelity system, §4.7) — anabios has no
  play substrate.
- **No new RNG consumption on the flag-off path**, and no new RNG in the affect
  stage at all (§6). Affect is a pure deterministic function of already-drawn
  state, following the `iq::develop_all` precedent.

---

## 3. Architecture — three functional layers

```
Layer 2  Evolved "cortex"    existing Program (unchanged).
         ▲ bias           ▲ hijack (override under high threat-arousal)
Layer 1  Affect core       7 Panksepp activations a[] as persistent per-agent
                           leaky integrators; lateral inhibition; arousal A.
         ▲
Layer 0  Homeostatic core  scalar drive D from physiology (energy deficit;
                           extensible to fatigue/thermo later).
```

Pattern is **subsumption**: Layer 0 says *what is needed*, Layer 1 turns needs +
percepts + temperament into affective activations, and those either **bias**
Layer 2's output or (under threat) **override** it.

### 3.1 Where it plugs into the tick

From the pipeline map (`tick.rs`):

- **Compute stage** — a new `affect.rs` stage, `affect::develop_all(world)`,
  runs alongside the other per-agent state updates. It reads this tick's
  `world.sensors` + physiology (energy/age) + genome and **writes the persistent
  affect columns** (§5). It is index-disjoint `par_iter`, consumes **zero RNG**,
  and early-returns when `!world.affect_enabled`. Ordering: it must run **after
  `sense_all` and before `decide_all`** so the current tick's decision can read
  fresh affect (unlike `iq::develop_all`, which runs late at stage 5b and feeds
  the *next* tick). Concretely: insert the affect update between `tick.rs:24`
  (sense) and `tick.rs:36` (decide).
- **Read-side (bias)** — a new `affect::apply_affect(&mut action, &affect_state,
  &sensors, energy, &genome)` call inside `decide_all`, immediately **after**
  `personality::apply_personality` (`tick.rs:187-192`), mutating the same
  `ActionRegister` in place. Same neutral-identity idiom (§6).
- **Read-side (hijack)** — evaluated at the end of the per-agent body in
  `decide_all`, *after* bias and the habitat/anchor movement biases but *before*
  the final `desired_direction` normalization (`tick.rs:258-261`). When
  threat-arousal exceeds the hijack threshold, it **overwrites** the relevant
  action fields with the reflex selection (§5.4). (Precedent for an override that
  overwrites `move_x/move_y`: the livestock-pen override at `tick.rs:241-252`.)
- **Stage-level factors** — like personality's `personality_speed_factor`
  (consumed in `integrate.rs`) and `personality_reproduction_factor` (consumed in
  `reproduce.rs`), affect exposes factor helpers read at the consuming stage:
  a SEEKING/arousal movement-speed factor (integrate) and a LUST
  reproduction-threshold factor (reproduce). Each is exact identity at neutral.

### 3.2 Layer 0 — homeostatic drive

Start with the single homeostatic variable anabios already has: **energy**.

- Setpoint `e* = SPAWN_ENERGY` (the comfort reference personality already uses,
  `COMFORT_FRAC * SPAWN_ENERGY`).
- Drive `D = clamp01((e* - energy) / e*)` — a normalized energy *deficit*, 0 when
  well-fed, →1 when starving. (A convex `((e*-e)/e*)^2` bowl is the fuller
  drive-reduction form and can be adopted if we later add surplus-aversion; the
  linear deficit is enough for v1 and cheaper.)
- **Extensible:** the design leaves room for a drive *vector* (fatigue from
  exertion, thermoregulation from biome temperature) but v1 ships energy-only to
  keep the surface small. Additional drives are additive milestones, not a
  redesign.

### 3.3 Layer 1 — affective command systems

Seven activations `a[k] ∈ [0,1]`, one per Panksepp system, stored as **persistent
per-agent state** (§5). Each is a **leaky integrator**:

```
a[k] ← λ_k · a[k] + (1 − λ_k) · trigger_k(drive, sensors, genome)
```

- `λ_k` = per-system decay (how long the state lingers). Heritable via a
  temperament gene where it matters (e.g. Reactivity), otherwise a tuned const.
- `trigger_k` = a bounded function of Layer-0 drive + this tick's percepts +
  the agent's temperament setpoint (§4 gives each system's trigger and output).
- **Lateral inhibition** (antagonism between systems), applied after the raw
  updates: FEAR and RAGE suppress PLAY; PANIC suppresses SEEKING (withdrawal);
  FEAR gates down RAGE at high threat proximity (flee-before-fight). Kept to a
  short, documented set of pairwise suppressions — not a full matrix.
- **Arousal** `A = softmax-or-max(a[FEAR], a[RAGE], a[PANIC], ...)` — a scalar
  "how activated" signal that (a) scales the movement-speed factor and (b) gates
  the hijack.

All updates are deterministic functions of already-drawn state → **no RNG**.

### 3.4 Layer 2 — the evolved program (unchanged)

The `Program` / `EvalContext` / `ActionRegister` are untouched structurally. Two
optional, additive touchpoints:

- **(Primary, user's pick) bias + override**, as in §3.1.
- **(Optional, secondary) affect as new senses.** We *may* append `Sense*` nodes
  (e.g. `SenseArousal`, `SenseFear`, `SenseDrive`) to let evolution also read
  affect and discover its own wiring. This is complementary to bias+override and
  cheap (append-only `Node` variants, §6), but it is **not** the primary coupling
  and can be deferred. Kept out of `random_node` if added, to stay hash-neutral
  until deliberately used.

---

## 4. The 7 systems → anabios substrate (fidelity-graded)

Each system lists its **trigger** (from real sensors) and its **output on a
live channel only** (§2.2). Substrate grade: 🟢 strong / 🟡 moderate / 🔴 weak.

Sensor/intent fields referenced are from the wiring map:
`EvalContext`/`SensorRegister` (`sense.rs`, `program/mod.rs`) and
`ActionRegister` (`program/mod.rs`).

### 4.1 SEEKING — the master appetitive engine 🟢

- **Trigger:** Layer-0 drive `D` (energy deficit) + local food cues
  (`local_biomass`, `plant_dir`). SEEKING is the general "go" engine; hook drives
  into it rather than modeling each drive's motor output separately.
- **Output:** bias `move_x/move_y` toward `plant_dir` (forage) when food is
  sensed, else a mild exploratory wander bias; and raise the **movement-speed
  factor** (integrate-stage) with `D` (forage harder when hungrier).
- **Not** `feed_intent` (latent). Feeding still happens via the existing
  interact/bite path once the agent is on food — SEEKING just gets it there.

### 4.2 FEAR — threat/survival circuit 🟢

- **Trigger:** `hostility`, a threatening neighbor (`other_dir` with large
  `rel_size`/`rel_energy`), and damage taken this tick (combat scratch).
- **Output (bias):** flee — bias `move_x/move_y` **away** from the threat;
  dampen non-defensive intents.
- **Output (override):** at high arousal, hand off to the hijack (§4/§5.4:
  Freeze→Flight→Fight…).

### 4.3 RAGE / ANGER 🟡

- **Trigger:** *frustration* — a derived signal: high drive `D` while blocked
  from a needed resource (e.g. high `crowding` + low recent intake), or having
  been attacked. (No native "frustration" field exists; we derive it and
  document the heuristic.)
- **Output:** raise `fire_intent` toward `target_id`, and bias movement to
  approach it. Gated down by FEAR via lateral inhibition (flee before fight).

### 4.4 LUST 🟡

- **Trigger:** mate-readiness (energy ≥ reproduction threshold) + a same-species
  neighbor (`same_dir`, `nearest_same_id`).
- **Output:** lower the **reproduction-threshold factor** (reproduce-stage,
  mirroring `personality_reproduction_factor`) when LUST is high, and bias
  movement to approach the mate. **Not** `mate_intent` (latent) — mating remains
  energy-threshold gated; LUST modulates that gate and the approach.

### 4.5 CARE 🟢

- **Trigger:** kin present (`nearest_kinship` high, `nearest_same` close).
- **Output:** raise `share_intent` toward the kin `target_id` (already live in
  interact), and bias movement to stay near kin (protect).

### 4.6 PANIC / GRIEF — separation distress 🟡

- **Trigger:** social isolation for a social-temperament agent — low `crowding`
  (and/or loss of previously-near kin) combined with high Sociality temperament.
  (No explicit bond-partner tracking; modeled from isolation. Documented as a
  derivation.)
- **Output:** emit a **distress pheromone** (`emit_intent[ch]`) and/or
  **broadcast alarm** (`broadcast_intent[ch]`) — both live — and bias movement
  toward `nearest_same` (reunion-seeking). Suppresses SEEKING (withdrawal) via
  lateral inhibition.

### 4.7 PLAY 🔴 — minimal (per user)

- **Trigger:** juvenile (`age < IQ_MATURATION_AGE`) + safe (low FEAR/hostility) +
  a peer nearby (`nearest_same` close).
- **Output (minimal):** a small social-approach movement bias toward the peer,
  coupled to the **existing IQ social-enrichment term** (`iq_enrich_acc` already
  folds sensed crowding — PLAY nudges that enrichment for juveniles who play).
  No consummatory or combat output. Explicitly flagged lowest-fidelity;
  deferrable / foldable into M-D if we choose to cut it.

---

## 5. Per-agent state, genes, and the hijack

### 5.1 New serialized columns (`AgentBuffers`, `agent.rs`)

Add persistent, **serialized (NOT `#[serde(skip)]`)** columns next to the `iq`
columns (`agent.rs:66`):

- `affect: Vec<[f32; 7]>` — the 7 activations (or a small named struct
  `AffectState`). Neutral default `[0.0; 7]`.
- Optionally `affect_prev_crowding: Vec<f32>` or similar if PANIC needs a
  one-tick memory to detect kin-loss (a persistent accumulator → **must be
  serialized**, per the still-ticks v13 footgun).

Initialize/reset in **both** spawn branches (reuse `agent.rs:166-168`, push
`agent.rs:190-192`) and the dead-slot path, exactly like `iq`. Because
`AgentBuffers` derives Serialize/Deserialize, non-skip columns are automatically
part of the snapshot and the golden hash.

### 5.2 Heritable temperament genes (`genome.rs`)

Collapse the 7 systems onto **~5 temperament dimensions** (more biologically real
than one gene per system — the shy–bold continuum, aggressiveness, etc.). Place
them by **renaming reserved slots in place** so no index shifts (the OCEAN traits
were introduced this way):

| Temperament gene | Reserved slot | Governs |
|---|---|---|
| **Boldness** | `_DriveReserved17` | inverse FEAR setpoint / freeze threshold |
| **Aggressiveness** | `_DriveReserved18` | RAGE gain |
| **Nurturance** | `_DriveReserved19` | CARE gain |
| **Sociality** | `_ReproReserved34` | PANIC/PLAY/CARE bond weight |
| **Reactivity** | `_ReproReserved35` | arousal gain / hijack threshold, decay λ |

Each accessor sits next to `cognitive_potential()` (`genome.rs:338`); each is
signed to `[-1,+1]` (or `[0,1]`) with **neutral default → identity**. Heritable
automatically via existing `crossover` + `mutate_in_place_scaled` at birth. Add
names to `SLOT_NAMES` (locked by the unit test).

**Open decision (flagged for the plan):** temperament is *adaptive*, unlike the
OCEAN personality traits which are deliberately excluded from speciation distance
(`PERSONALITY_MASK`). Recommendation: **include** temperament in
`Genome::distance` (it can drive ecological/behavioral divergence), but confirm
during M-A and record the choice.

### 5.3 World flag & scenario threading (`world.rs`, `scenario.rs`)

- `#[serde(default)] pub affect_enabled: bool` on `World`, doc'd "off ⇒
  identity / zero-RNG"; default `false` in `World::new`.
- Thread through `Scenario` struct + `instantiate()` (mirror `cognition_enabled`
  at `scenario.rs:54`, `scenario.rs:433`).
- **Do not** add it to `scenarios/minimal.toml` (absence + serde-default = off →
  the golden scenario stays neutral). A new flag-ON scenario TOML pins real
  behavior.

### 5.4 The hijack (survival reflex)

When threat-arousal `A_threat` (from FEAR + damage + threat proximity) exceeds a
**heritable threshold** (Reactivity/Boldness), Layer 2's action is **overridden**
by a hard-coded reflex selector, ordered by threat proximity/escapability
(Bracha):

1. **Freeze** — zero `move_x/move_y` (orient, don't be seen). Default first
   response when threat is distant/ambiguous.
2. **Flight** — max flee vector away from threat, speed factor boosted.
3. **Fight** — approach + `fire_intent` (only when cornered: threat very close
   and flight blocked).
4. **Fright / Faint** — tonic immobility as a last resort (zero movement, intents
   suppressed) at extreme, inescapable threat.

Implemented as a pure selection over sensors + affect (no RNG). It writes only
live channels. This is LeDoux's "low road" with a cortical cancel path: if
arousal is below threshold, the evolved program's action stands unchanged.

---

## 6. Determinism, opt-in & save/load (hard constraints)

Follow the established playbook exactly:

1. **Neutral-off = byte-identical.** All behavior gated on `affect_enabled`; the
   affect stage early-returns as a strict no-op when off (iq.rs template), and
   read-side effects are **exact identity at neutral affect (0.0)** using
   `1.0 + k·x` / `+ k·x` forms each guarded `if x != 0.0` (personality idiom,
   `personality.rs:52-102`). Flag-off draws no RNG and runs no affect float ops.
2. **Zero RNG in the affect stage**, even flag-on — affect is a pure function of
   genome + sensors + physiology (iq's discipline). This keeps the tick RNG draw
   order identical, so enabling affect does not perturb other subsystems' draws.
3. **Serialized state, never `#[serde(skip)]`** for the affect columns — they are
   path-dependent accumulators that feed hashed state (movement, combat, codex
   latches). The still-ticks/prev-direction v13 regression is the cautionary
   precedent. Add a `save→load→step` equality test (model:
   `determinism.rs:16-36`).
4. **Index-disjoint parallelism** — the affect `par_iter` writes only slot `i`,
   takes shared fields by `&`; must pass `parallel_matches_serial_across_thread_
   counts` (`determinism.rs:175-209`).
5. **Append-only enums** — any new `Node` (affect senses), `EventType` (codex
   detectors) append at the END (`program/mod.rs:88-91`, `event.rs:80`). Genome
   temperament genes reuse **reserved** slots (no index shift).
6. **Golden refresh (once, layout growth only)** — new serialized columns grow
   the bincode payload, so `determinism.rs`, `cognition.rs`, `inventions.rs`
   goldens each move **once**. Regenerate with `UPDATE_HASHES=1`, paste back, add
   a dated "layout growth only; flag off ⇒ byte-identical behavior" changelog
   note. Bump `FORMAT_VERSION` in `snapshot.rs` with a `///` changelog entry. Any
   golden moving for a *non-layout* reason = a determinism bug.
7. **Flag-ON golden** — a new scenario + golden/behavior test pins the layer's
   real behavior (model: `cognition.rs`).

---

## 7. Observability

### 7.1 Codex detectors (R/T)

Append-only `EventType`s + detectors for the emergent phenomena that make the
layer legible and give each milestone a "done-when" bar:

- **Panic cascade** — FEAR propagating through a cluster via distress
  signals/alarms (first mass fright event).
- **Feeding frenzy** — synchronized high-SEEKING convergence on a food patch.
- **Territorial rage** — sustained high-RAGE aggression clusters.
- **Mass grief / separation** — population-level PANIC after a die-off/isolation.

Detectors follow the existing codex observer cadence and determinism discipline.

### 7.2 Viewer (V)

Surface dominant affect as agent tint (e.g. fear = pale/frozen pose, rage =
red/approach, seeking = active wander) and/or a small arousal meter, folded into
the existing tier-1/2 viewer effects. A flag-ON showcase scenario demonstrates a
panic cascade / feeding frenzy for the Out-of-Africa showcase pipeline.

---

## 8. Milestone arc

One spec, a per-milestone PR arc (branch `mNN-<name>` off main; Explore →
TDD plan in `docs/superpowers/plans/` → subagent-driven-development →
per-milestone PR), mirroring the M11–M16 collaboration arc. Each milestone keeps
flag-off byte-identical and adds a flag-ON golden.

- **M-A — Subcortical framework + SEEKING.** Layer-0 energy drive; Layer-1 affect
  columns (serialized) + `affect::develop_all` stage (post-sense/pre-decide, zero
  RNG); `affect_enabled` flag + scenario threading; the ~5 temperament genes in
  reserved slots; `affect::apply_affect` bias hook; SEEKING wired end-to-end
  (forage movement bias + speed factor). *Done when:* flag-off byte-identical
  (goldens refreshed for layout only); flag-on golden pins SEEKING; save→load→step
  passes; speciation-distance decision recorded.
- **M-B — Threat/survival circuit: FEAR + hijack.** FEAR activation; arousal;
  the Freeze→Flight→Fight→Fright→Faint override. *Done when:* a freeze/rout codex
  event fires in a flag-on scenario; determinism + replay hold.
- **M-C — Agonistic + reproductive: RAGE + LUST** with FEAR↔RAGE lateral
  inhibition and the LUST reproduction-threshold factor. *Done when:* behavior
  test shows frustration→aggression and mate-readiness→approach; goldens stable.
- **M-D — Social bonding: CARE + PANIC/GRIEF.** Kin proximity → share/protect;
  isolation → distress signal + reunion. *Done when:* a grief/separation event
  fires; determinism + replay hold.
- **M-E — PLAY (minimal) + enrichment coupling.** *Stretch / foldable into M-D.*
  Juvenile safe-peer approach coupling into IQ social enrichment.
- **M-F — Observability & showcase.** Codex detectors (panic cascade, feeding
  frenzy, territorial rage, mass grief), viewer affect surfacing, flag-on
  showcase scenario, and a full save/load + determinism hardening pass over all
  new state.

---

## 9. Open decisions (resolve during planning)

1. **Speciation distance** — include temperament genes in `Genome::distance`
   (recommended: yes, they're adaptive) vs. exclude like OCEAN. Decide in M-A.
2. **Affect-as-senses** (§3.4) — ship the optional `Sense*` nodes, or defer.
   Default: defer; primary coupling is bias+override.
3. **Drive vector** — energy-only for v1 (recommended) vs. adding fatigue/thermo
   drives. Default: energy-only; additional drives are later additive milestones.
4. **PLAY** — keep minimal M-E vs. fold into M-D vs. cut. Default: minimal M-E,
   cuttable.
5. **Arousal aggregation** — `max` vs. softmax of the defensive activations.
   Decide in M-B with the hijack.
