# Anabios Feature Showcase — Plan & Runbook

A reproducible, narrated tour that demonstrates **every** anabios subsystem in the
Godot viewer, culminating in **domesticated animals**. The output is (a) a live
walk-through script you can present, and (b) a refreshed `gallery/` of captioned
stills captured with the existing screenshot harness.

## Key finding: nothing needs to be built

The showcase is a *presentation* problem, not a feature-build. Everything is
already shipped:

- **Every subsystem is exposed by a scenario flag** and there is already a single
  scenario — `scenarios/out-of-africa.toml` (seed 318) — that turns **all** of them
  on at once, including `sexual_dimorphism_enabled` and `domestication_enabled`
  (which even `grand-theater.toml` leaves off).
- **Domesticated animals already exist** as the shipped E13 feature
  (`crates/anabios-core/src/domestication.rs`, detector `codex/domestication.rs`,
  scenario `scenarios/domestication.toml`). Husbandry-invention holders tame wild
  juvenile herbivores into penned, milk-yielding livestock; born-tamed offspring
  inherit domestication.
- **The visual layer** is the Godot 4 frontend in `game/`, driven by the Rust
  GDExtension in `crates/anabios-godot/`.
- **The capture pipeline already exists** — `game/scripts/debug_capture.gd`, an
  env-gated screenshot harness that produced the current `gallery/*.png`.

So this plan is about *sequencing, narration, and capture*, and about verifying
that the headline behaviors actually fire.

### Verified: domestication fires

`scripts/emergence.sh run domestication --ticks 12000 --seed 0` produced, among
others:

| Codex event | Count |
|---|---|
| `AnimalDomesticated` | 70 |
| `HerdCohesion` | 6 |
| `LivestockHerd` | 4 |
| `InventionDiscovered` | 10 |

The domestication chain (invent Husbandry → tame juveniles → sustain a penned
herd) is confirmed reproducible on the focused scenario. The grand scenario's
emergent version must still be spot-checked (see Phase 0).

---

## Structure: two tracks

**Track A — The Grand Tour (`out-of-africa`).** One continuous run in the
windowed viewer where all subsystems are live simultaneously. This is the
headline: a themed "Africa → Eurasia" geography where the exodus, trade,
invention, war, and domestication all emerge together. Best for a single
impressive sitting.

**Track B — Feature Spotlights (focused scenarios).** Each subsystem also has a
small dedicated scenario that isolates it so a viewer can see *one* mechanism
clearly without the grand-run noise. Best for teaching / documenting one feature
at a time, and for clean gallery stills.

Present Track A live; use Track B to capture the labeled gallery and to explain
any mechanism that gets lost in the crowd during the grand run.

---

## The viewer: controls you'll drive

Launch windowed (needs Godot 4.x; `GODOT=/path/to/godot` to override lookup):

```
scripts/emergence.sh view out-of-africa --seed 318
```

Confirmed hotkeys (source: `game/scripts/*.gd`):

| Key | Effect |
|---|---|
| **G** | Cycle the **ground overlay**: biome → env-optimum → succession → markets → pheromone channels 0–3. Auto-skips overlays whose subsystem is off, so on `out-of-africa` *all* of them are available. |
| **C** | Cycle **agent coloring** (diet / dialect-hue / energy / species …). |
| **T** | Toggle the **evolution panel** (trait time-series). |
| **Y** | Toggle the **coevolution panel** (arms-race / predator–prey series). |
| **H** | Toggle the **legend / help** panel. |
| **Click agent** | **Pin** it in the inspector — shows species, lineage, OCEAN personality, body modules, and (when domestication is on) the `livestock of agent N` line. |
| **WASD / arrows** | Pan camera. **F** toggles camera follow. |
| **R / U / V / Esc** | Replay controls / back to menu. |

The HUD shows tick / alive / rate / trades. The codex/tech/dit/population panels
render alongside the field.

---

## Feature spotlight matrix

Each row: the subsystem, the scenario that isolates it, what to look for on screen,
which overlay/panel surfaces it, and the codex events that prove it fired.

| # | Subsystem | Spotlight scenario | Watch for (screen) | Overlay / panel | Codex events |
|---|---|---|---|---|---|
| 1 | **Climate & Whittaker biomes / worldgen** | `desert-tropical`, `biome-adaptation` | Latitude bands: rainforest belt, deserts, taiga/tundra poles; terrain relief shading | **G** → biome | — (structural) |
| 2 | **Living water & terrain relief** | any (out-of-africa) | Animated water shader, ocean belt, elevation shading | default field | `Migration`, `CorridorUse` |
| 3 | **Biome adaptation / EnvAffinity gene** | `biome-adaptation` | Agents sorting onto preferred terrain; cline forming | **G** → env-optimum; **T** | `RangeExpansion`, `NichePartitioning` |
| 4 | **Agents as 8-bit apes + body modules** | `weapons-arena` | Procedural hominin sprites (Chimp/Gorilla/Orang/Australopith/Sapiens); spines/armor/weapons | **C** → species; click to inspect modules | `NovelModuleAppeared`, `EvolvedAmbush` |
| 5 | **Material-learning economy & trade goods** | `geographic-trade`, `biome-trade` | Trade lanes drawn between species; four-goods economy | **G** → markets | `ResourceTraded`, `MarketEmerged` |
| 6 | **Cumulative inventions / inventiveness** | `inventions`, `tool-users` | Tech panel climbing the 10-tech tree (Stone Tools→…→Nuclear) | tech panel | `InventionDiscovered`, `InventionAdopted` |
| 7 | **Gene–culture coevolution / DIT** | `gene-culture-skill`, `dit-env-fast`, `traditions` | dit panel: innate/individual/social learning mix under a drifting optimum | **G** → env-optimum; dit panel | `MemeSweep`, `TraditionPreserved`, `InstitutionalRatchet` |
| 8 | **Personality (Big-Five / OCEAN)** | any (inspect) | Per-agent openness/conscientiousness/… in inspector | click to pin | latent (drives feed/mate) |
| 9 | **Cognition (IQ / practices)** | `cognitive-coevolution` | IQ series on coevolution/evolution panels | **Y**, **T** | `NovelBehaviorPattern` |
| 10 | **Combat / predation / weapons** | `predator-prey`, `weapons-arms-race` | Additive combat glow, ranged Spine volleys, carcasses | **Y** coevolution | `Predation`, `CombatRaid`, `ArmsRace`, `PackHunting` |
| 11 | **War substrate** | `war` | Raids/alliances between settled groups | codex panel | `WarOrRaid`, `AllianceFormed`, `WarEnded` |
| 12 | **Speciation & phylogeny** | `convergent`, `divergent` | Phylogeny/tree; convergent vs divergent trait fixation | evolution panel | `SpeciationEvent`, `ConvergentEvolution`, `TraitFixation` |
| 13 | **Sexual dimorphism (E12)** | `dimorphism` | Two sexes, female mate choice; size/trait split | **C**; inspect | `SexualSelection`, `SexRatioCollapse` |
| 14 | **Disasters / disturbance / succession** | `disturbance`, `drifting-climate` | Disaster wipes then biome succession regrowth | **G** → succession | `Extinction`, succession/`Disturbance` |
| 15 | **Settlement / markets / institutions** | `settlement` | Anchored settlements, market hubs | **G** → markets | `SettlementFormed`, `MarketEmerged`, `InstitutionalRatchet` |
| 16 | **Pheromones / territory / kin / dialects** | `territories`, `dialects`, `cooperation` | Pheromone fields; territory borders; dialect-hue clusters | **G** → pheromone 0–3; **C** → dialect | `TerritoryFormation`, `DialectFormed`, `StructuredSignaling`, `KinNetworkStable` |
| 17 | **★ Domesticated animals (E13)** | `domestication` → then `out-of-africa` | Penned livestock near owners; milk yields; `livestock of agent N` in inspector | click herder + neighbors; tech panel (Husbandry) | `AnimalDomesticated`, `LivestockHerd`, `HerdCohesion` |

---

## ★ The finale: domesticated animals

Domestication is the story's climax because it *composes* the other systems: it
needs inventions (Husbandry is tech #7), a wild herbivore herd to tame, and
enough population stability that the tech race — not collapse — dominates.

**Spotlight run (clean teaching shot):**

```
scripts/emergence.sh view domestication --seed 0
```

Narration beats:
1. Open the **tech panel** — watch the innovator culture climb
   Stone Tools → Fire → Farming → **Husbandry**. Nothing tames before Husbandry.
2. Once Husbandry is held, watch juvenile grazers east of the innovators get
   tamed — the codex fires `AnimalDomesticated`.
3. **Click a herder**, then click its neighbors: the inspector shows
   `livestock of agent N`, and penned stock stays inside the owner's pen radius.
4. As a tamed herd sustains itself, `LivestockHerd` / `HerdCohesion` fire —
   born-tamed offspring keep the herd going without new taming.

**Emergent version (the payoff):** in `out-of-africa`, the megafauna belt is the
tameable stock. Domestication here is *not* scripted — it only happens if a
lineage independently invents Husbandry near the herd. Phase 0 confirms whether
seed 318 delivers it within the tour's tick budget; if not, we either extend
ticks or pick the seed that does (documented in the runbook).

---

## Gallery capture pipeline

The stills in `gallery/` are made by the env-gated harness in
`debug_capture.gd`. It requires a **windowed** run (the headless renderer can't
read back the viewport). Env vars:

| Env var | Meaning |
|---|---|
| `ANABIOS_SHOT` | Output PNG path — also the on/off switch. |
| `ANABIOS_SHOT_TICKS` | `step_n` this many ticks before the shot (evolved state). |
| `ANABIOS_SHOT_FRAMES` | Frames to settle before capture (default 180). |
| `ANABIOS_INSPECT` | Pin a representative agent so the inspector is in frame. |
| `ANABIOS_COEVO` / `ANABIOS_EVO` | Reveal the [Y] / [T] panels for the shot. |
| `ANABIOS_SCENARIO` / `ANABIOS_SEED` | Scenario / seed (set by `emergence.sh view`). |

Naming convention (matches existing gallery): `<scenario>-t<tick>-<label>.png`.

**Example — the domestication money shot** (evolved state, inspector pinned):

```
ANABIOS_SHOT="$PWD/gallery/domestication-t4000-livestock.png" \
ANABIOS_SHOT_TICKS=4000 ANABIOS_INSPECT=1 \
scripts/emergence.sh view domestication --seed 0
```

**Capture set to produce** (one representative still per subsystem, plus a
grand-tour montage):

- `out-of-africa-t0-worldmap.png` — the themed geography at t0 (biome overlay, **G**).
- `out-of-africa-t3000-exodus.png` — migration corridors mid-run.
- `out-of-africa-t6000-alltrades.png` — trade lanes + markets overlay.
- `domestication-t4000-livestock.png` — pinned herder + `livestock of` line. ★
- `predator-prey-t*-combat.png` — combat glow / volleys (`ANABIOS_COEVO=1`).
- `dimorphism-t*-sexes.png`, `dialects-t*-hues.png`, `disturbance-t*-succession.png`,
  `inventions-t*-techtree.png` (`ANABIOS_EVO=1`), `territories-t*-pheromones.png`.

---

## Quantitative backing (for a one-pager / captions)

Alongside the visuals, generate the numbers that prove emergence:

```
scripts/emergence.sh run   out-of-africa --ticks 8000          # event tally
scripts/emergence.sh sweep out-of-africa --seeds 16 --ticks 8000  # cross-seed scorecard
scripts/emergence.sh demo  inventions --ticks 8000             # narrated tech race
scripts/emergence.sh replay out-of-africa --seed 318           # determinism proof
```

`run` prints a codex event histogram; `sweep` gives a multi-seed scorecard
(`summary.csv`); `replay` re-simulates each detected event and asserts identical
state hashes — a nice "this is fully deterministic" talking point.

---

## Determinism & reproducibility notes

- The engine is golden-hash-locked; `FORMAT_VERSION` is currently **22**
  (`crates/anabios-core/src/snapshot.rs`). This showcase adds **no engine code**,
  so no version bump and no golden regen — every scenario used here already
  exists.
- Always pin `--seed` in the runbook so a presentation is reproducible; the
  viewer's default seed can mask scenarios tuned around a specific biome field.
- `emergence.sh view` rebuilds the `anabios-godot` cdylib (debug) and
  `anabios-headless` (release) as needed; first launch is slower.

---

## Phased execution plan

**Phase 0 — Verify the headline behaviors (do first).**
- [x] `domestication` fires `AnimalDomesticated`/`LivestockHerd` (confirmed, seed 0, 12k ticks).
- [ ] Run `emergence.sh run out-of-africa --ticks 8000 --seed 318` and confirm the
  tally includes `AnimalDomesticated` + a broad spread of the 53 event types. If
  domestication doesn't emerge by 8k ticks on seed 318, sweep a few seeds and
  record the first that delivers it; update the runbook seed.
- [ ] Confirm a Godot 4 binary is available (`GODOT=` path) on the presentation machine.

**Phase 1 — Grand-tour live script.**
- Write the minute-by-minute narration for `view out-of-africa`: which overlay
  (**G**) and coloring (**C**) to show when, when to open **T**/**Y**, and the
  agents to click. End on the domestication beat.

**Phase 2 — Gallery refresh.**
- Run the capture set above; drop labeled PNGs into `gallery/`. One clean still
  per subsystem + 2–3 grand-tour frames.

**Phase 3 — One-pager (optional).**
- Combine the `run`/`sweep` numbers with the gallery stills into a single
  `docs/showcase.md` (or an artifact) — a feature × evidence table with a picture
  and an event count for each subsystem.

---

## Scope boundaries (YAGNI)

- **No new engine code, no new scenarios** unless Phase 0 shows `out-of-africa`
  can't deliver domestication at a reasonable tick budget on any seed — in which
  case the only change is a small tuning tweak to that one scenario TOML (flags
  off in `minimal` are untouched, so determinism/goldens are unaffected).
- **No web frontend** — none exists; the Godot viewer is the showcase surface.
- Keep the tour to the 17 subsystems above; resist adding every codex detector as
  its own beat.
