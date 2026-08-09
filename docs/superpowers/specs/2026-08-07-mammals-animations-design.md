# Mammals with Good Animations — Viewer Roster Design

**Date:** 2026-08-07
**Branch:** `claude/mammals-animations-b60e24`
**Scope:** Godot viewer only (`game/`, plus one batch accessor in `crates/anabios-godot`). No changes to the sim core, determinism gate, goldens, or the snapshot format.

## Problem

The viewer renders **every** agent as one of five procedural hominins (`ape_sprites.gd` — the DIT "the agents ARE the apes" framing). anabios is a general sandbox: many scenarios are predator/prey, wild herbivores, or domesticated livestock, and rendering a wolf-vs-deer world as apes reads wrong. We want a **quadruped mammal roster** so ecological roles render as recognizable animated animals, matching the animation quality the apes already set (walk cycle, idle breath-bob, squash-&-stretch, facing flip, action poses).

## Decisions (locked during brainstorming)

1. **Viewer-only.** No new sim traits or morphology; the sim core is untouched.
2. **Diet + role driven mapping.** An agent's visual is chosen from its carnivory score, body size, and livestock status — all already exposed to the viewer — so the roster reads as a real food web.
3. **Apes are one archetype.** The five hominins fold into the roster as the **Primate** archetype (omnivore + large niche); `species_id` still selects which hominin sub-skin.
4. **Full grid (7 archetypes).** Hare, Deer, Boar, Primate, Fox, Wolf, plus a Livestock herd override.
5. **Standard + signature move animations.** Quadruped walk/trot gait, idle secondary motion, graze/eat, attack, gallop-flee — plus one signature flourish per rig.
6. **Approach A (extend the authored block-list pattern) with shader-driven secondary motion.** Keep the 8-bit look; generalize the plumbing; hit "good animation" via shader flourishes rather than dozens of hand-drawn frames.
7. **Per-instance tint.** One atlas per archetype; per-species coat is a per-instance modulate (unlimited coats), plus one baked secondary zone. This decouples atlas count from the (unbounded) species count.

## Current system (as-is)

- `game/scripts/ape_sprites.gd` (416 lines): five hominins as 16×16 pixel-block pose lists. Builds a per-species **vertical 12-pose atlas** (`build_species_atlas`) via `_build_pose`/`_build_cell`, zone-coloured (`FIELD_ZONE_COLORS`: coat `c`, skin `s`, accent `a`). Pose layout: `0` neutral, `1` contact-L, `2` passing (body lifted 1px — the bob), `3` contact-R, then action **pairs** `4/5` eat, `6/7` fight, `8/9` trade, `10/11` flee. `ape_for_species(id) = abs(id) % 5`.
- `game/shaders/field_agent.gdshader` (84 lines): one MultiMesh per species draws that species' atlas; per-instance `INSTANCE_CUSTOM = (phase, moving, face_left, action/4)`. Cycles gait `1→2→3→2` when moving, holds `0` idle with a procedural breath-bob, plays an action pair when `action != 0`, and adds a fight lunge / flee tremble in the vertex stage. `frames` uniform = pose count.
- `game/scripts/main.gd` (800 lines): `_ready` builds `_body_mmis`/`_death_mmis` (one per species) + materials (`frames`, atlas texture). `_refresh_bodies` (~L472) buckets alive indices by `ape_for_species`, then per instance writes transform (scaled by `alive_sizes() * BODY_SCALE`), per-instance colour (`body_colors`), and the INSTANCE_CUSTOM animation state (action derived from proximity to combat streaks / trade routes / rising energy). `_on_agent_death`/`_refresh_death_effects` render fallen-figure ghosts, also bucketed per species.
- `crates/anabios-godot/src/lib.rs`: already exposes `alive_sizes()` (`0.5 + 2.5*genome.Size` → 0.5..3.0), `alive_diet()` (`effective_diet_carnivory` 0..1), `alive_species_ids()`. Livestock ownership (`livestock_of`) is exposed per-agent in the inspector dict and gated by `domestication_enabled`, but **not** yet as a batch array.
- Consumers of `ApeSprites`: `inspector_panel.gd` (avatar header + names), `settlement_layer.gd` (hut zone colour).

## Target design

### Component 1 — Archetype registry (`mammal_sprites.gd` + `mammal_data/`)

A new `game/scripts/mammal_sprites.gd` owns the roster and is the single seam the rest of the viewer talks to. Because 7 rigs of pose data would exceed the **1000-line gdlint cap**, per-rig pose block-lists live in `game/scripts/mammal_data/<rig>.gd` (one small const-only file each); `mammal_sprites.gd` imports them.

Public surface (names indicative):

```
const ARCHETYPE_COUNT := 7
enum { HARE, DEER, BOAR, PRIMATE, FOX, WOLF, LIVESTOCK }
const NAMES: PackedStringArray = ["Hare","Deer","Boar","Primate","Fox","Wolf","Livestock"]

# rig_kind drives the shader's signature-move branch.
enum RigKind { PREY, PREDATOR, PRIMATE_RIG, LIVESTOCK_RIG }
static func rig_kind(archetype: int) -> int

# Pure selection. size in world units (0.5..3.0), diet carnivory 0..1.
static func archetype_for(diet: float, size: float, livestock: bool) -> int

# One atlas per archetype (Primate delegates to ApeSprites for its sub-skins).
static func build_archetype_atlas(archetype: int) -> ImageTexture
static func build_fallen_texture(archetype: int) -> ImageTexture

# Primate sub-skin selection (delegates to ApeSprites).
static func primate_skin_for(species_id: int) -> int
```

`ape_sprites.gd` **stays** and becomes the Primate implementation; `mammal_sprites.gd` delegates Primate atlas/skin building to it. All other `ApeSprites.ape_for_species` / `SPECIES_COUNT` call-sites move to the registry.

### Component 2 — Selection function

Pure, deterministic, stable per agent (diet and size are fixed at birth, so no per-frame flicker):

```
static func archetype_for(diet, size, livestock) -> int:
    if livestock:            return LIVESTOCK
    var large := size >= SIZE_SPLIT
    if diet < HERB_MAX:      return DEER if large else HARE      # herbivore
    elif diet < CARN_MIN:    return PRIMATE if large else BOAR   # omnivore
    else:                    return WOLF if large else FOX       # carnivore
```

Named tunable consts: `SIZE_SPLIT := 1.25`, `HERB_MAX := 0.34`, `CARN_MIN := 0.66`. Livestock is only ever `true` when the scenario has `domestication_enabled` (the batch accessor returns all-false otherwise).

### Component 3 — Atlas & tint model

- **One atlas per archetype** (7), each a vertical 12-pose strip built once at load — atlas count is independent of how many species emerge.
- Atlases authored with **baked value shading**: dark outline, mid-value coat, counter-shaded lighter belly. **Per-species coat hue = per-instance modulate** (the existing `body_colors` / `set_instance_color` path) → unlimited distinct coats with correct shading.
- **One secondary zone** (Primate face/skin; quadruped muzzle/underbelly) survives as a **1-bit zone mask** baked into the atlas that the shader tints to a fixed secondary tone. Exact mask encoding (e.g. a reserved channel or a sentinel value the fragment stage tests) is a plan-stage detail; the constraint is that it must not consume an INSTANCE_CUSTOM channel (all four are in use).
- Primate is the exception that keeps its existing multi-zone hominin look via `ape_sprites.gd` (its five sub-skins are already authored); the per-instance-tint model applies to the six quadruped rigs.

### Component 4 — Animation (shader extension, same slot contract)

Quadruped atlases fill the **same 12 pose slots** so a single shader serves every rig:

| Slot | Biped (today) | Quadruped (new) |
|------|---------------|-----------------|
| 0 | neutral stand | neutral stand |
| 1/2/3 | contact-L / passing / contact-R | 4-leg diagonal gait; body-lift bob baked into "passing" |
| 4/5 | eat A/B | graze A/B (head down) |
| 6/7 | fight A/B | attack A/B (bite / headbutt) |
| 8/9 | trade A/B | alert A/B (head-up social/wary) |
| 10/11 | flee A/B | gallop A/B (stretched run) |

The art carries the quadruped read; `field_agent.gdshader` gains the "good animation" flourishes, gated by a new per-material `rig_kind` uniform:

- **Gallop-flee:** quadruped flee adds a body-stretch pulse on top of the existing flee branch.
- **Signature move:** PREDATOR → forward pounce arc on attack; PREY → periodic bounding leap on flee; PRIMATE → the existing lunge (unchanged); LIVESTOCK → slow chewing head-bob at idle.
- **Secondary motion:** keep the existing idle breath-bob + stride squash-&-stretch; add a gentle rear/tail sway (mild UV-position-weighted `VERTEX.x`) so idle animals still breathe. Tail up/down delta is also baked between gait poses.

The existing INSTANCE_CUSTOM contract `(phase, moving, face_left, action/4)` is unchanged, so `main.gd`'s per-instance write is reused verbatim; only the bucketing key changes.

### Component 5 — Viewer integration

- **`crates/anabios-godot/src/lib.rs`:** add `alive_livestock_flags() -> PackedInt32Array` (1/0 per alive agent from `livestock_of != AGENT_NULL`; all-zero when `domestication_enabled` is false). Expose `domestication_enabled()` if not already batch-reachable.
- **`main.gd`:** `_ready` builds 7 archetype MultiMeshes/materials (loop over `ARCHETYPE_COUNT`, set `frames` + `rig_kind` + atlas). `_refresh_bodies` fetches `alive_diet()` (+ livestock flags when enabled) alongside the sizes/species it already reads, and buckets by `MammalSprites.archetype_for(...)`. Transform / colour / INSTANCE_CUSTOM writes unchanged. `_on_agent_death` / `_refresh_death_effects` bucket by archetype and use each rig's fallen pose.
- **`inspector_panel.gd`:** avatar header shows the archetype name (and the hominin sub-skin name when Primate).
- **`settlement_layer.gd`:** hut zone colour follows the archetype coat instead of `ape_for_species`.

## Non-goals (YAGNI)

- No new sim traits, morphology, diet mechanics, or action types — quadrupeds reinterpret the existing eat/fight/trade/flee signals.
- No per-species authored atlases; species variety is per-instance tint only.
- No change to the sim core, determinism gate, goldens, or snapshot `FORMAT_VERSION`.
- No new shipped image assets — all sprites remain procedural.

## Testing & verification

- Viewer is **not** part of the determinism gate → no golden regeneration.
- `archetype_for` is pure → unit-test it against the project's GDScript `test_runner` if present (boundary cases at each threshold and `SIZE_SPLIT`; livestock override precedence).
- Headless boot: `godot --headless res://scenes/main.tscn --quit-after` compiles/loads clean.
- `debug_capture` screenshots on a herbivore-heavy, a predator/prey, and a domestication scenario to eyeball each archetype and the food-web read; run the showcase director for motion.
- CI gates: `gdformat --check` and `gdlint` green; every file under the 1000-line cap (hence `mammal_data/` split). Rust: `cargo fmt --check` + `cargo test -p anabios-godot` for the new accessor.

## Risks & mitigations

- **Authoring load (6 quadruped rigs × ~12 poses of pixel blocks)** is the bulk of the effort → mitigate by building one rig end-to-end (Wolf or Deer) as the template, verifying it on-screen, then replicating.
- **Secondary-zone mask encoding** interacting with per-instance modulate → prototype the shader path on the template rig before authoring all six.
- **Diet/size thresholds mis-bucketing** emergent species → thresholds are named consts, tuned against real scenarios during the capture pass.
- **1000-line cap** → enforced by the `mammal_data/` split from the start.

## Build order (for the plan)

1. Rust batch accessor (`alive_livestock_flags`) + test.
2. Archetype registry skeleton + pure `archetype_for` + unit test (no art yet; delegate all rendering to Primate/ApeSprites so the viewer still runs).
3. Shader: add `rig_kind` uniform + gallop/signature/secondary-motion branches (Primate path byte-identical).
4. Template quadruped rig (e.g. Wolf) end-to-end: pose data, atlas builder, secondary-zone mask, on-screen verification.
5. Wire `main.gd` bucketing + death ghosts to the registry.
6. Author the remaining five rigs against the template.
7. Inspector + settlement-layer call-site updates.
8. Capture pass across three scenarios; tune thresholds; CI green.
