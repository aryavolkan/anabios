# Pixel world at scale — reaching the reference look on much larger maps

**Date:** 2026-09-12
**Status:** Proposed — phased plan (each phase gets its own TDD plan under
`docs/superpowers/plans/` when it starts)
**Track:** V (viewer & showcase) with an E/T slice for the engine-side scale plumbing
**Branch:** `claude/graphics-larger-maps-bra60w`

## 1. Target

Two reference boards define the destination (kept out of the repo; they are
concept renders, not assets):

1. **Settlement under raid** — a river with a waterfall and a water mill, dense
   conifer/broadleaf forest, a palisaded village (thatch huts, fenced fields,
   forge, watchtower, banners, a burning hut), two armies of ~40 figures with
   spears/bows/shields and a siege engine, and a full HUD: top bar (day, pause /
   play / fast-forward, four resource counters, population), a **Research**
   panel with icons and progress bars, a minimap with a compass rose and the
   view rectangle, an icon-tagged event log, a bottom hotbar, and a **unit card**
   with portrait, HP/stamina bars and four stat pips.
2. **Ecosystem** — the same terrain language at ecosystem scale: mammoths,
   antlered herds, a predator flagged with a `!` emote, water birds, turtles,
   a small camp, and a **Codex** panel with Species / Biomes / Tech / Culture
   tabs, a species portrait, traits, and related-species thumbnails; bottom
   right an *Ecosystem Health* / *Biodiversity* meter pair.

What the boards have in common, and what this plan therefore commits to:

- **Crisp, integer-scaled pixel art everywhere.** No linear filtering, no
  shader blur; one consistent texel size for ground, props, buildings and
  figures.
- **Sub-cell terrain detail.** Coastlines, river banks, cliffs, forest edges
  and paths are drawn with transition tiles, and props (trees, rocks, reeds)
  are individual sprites dense enough to read as forest.
- **Structures are architecture, not markers.** Villages have a footprint:
  several building kinds, fences, fields, a palisade, smoke and fire, all
  larger than the figures walking between them.
- **Figures are 16–24 px characters** with weapons, shields and emotes, drawn
  above the ground and y-sorted against buildings and trees.
- **A themed HUD** with pixel frames, icons, progress bars and portraits,
  replacing text-only panels.
- **All of it at "much larger" map sizes** — worlds where the current
  whole-world texture, the 3×3 torus clones and the per-frame GDScript loops
  no longer fit in a frame.

Everything stays **presentation over read-only sim state**: no new simulation
substrate (the ROADMAP guardrail — buildings and institutions are not sim
objects), no determinism change on any default path, and every engine-side
addition is a read-only bridge query or an opt-in scenario field.

## 2. Where we stand

The viewer (`game/`, Godot 4.7, gdext bridge in `crates/anabios-godot`) already
has most of the *vocabulary*; what it lacks is scale and crispness.

| Layer | Today | Limits at scale |
|---|---|---|
| Ground | One `res×res` RGBA texture of the whole biome grid (`biome_renderer.gd`), scaled to `world_size`, with `terrain.gdshader` doing hillshade, water, coast, a 70 % blend of a 16 px tile atlas, and a `soften` blur. Linear filtering. | `sim.biome_colors()` returns `res²` `Color`s (16 B each) and `_blit` repacks them in a GDScript loop: 262 k iterations at res 512, ~4 M at 2048. One texture per world means no streaming. Blur + linear filter are the opposite of the target look. |
| Torus wrap | Every world layer is drawn 9× (origin + 8 clones) | 9× instance counts and draw calls; at 12 k props that is 108 k instances before agents. |
| Props | `terrain_scatter.gd` (8 kinds, budget 12 000, cell-hash deterministic) + `biome_props.gd` (5 kinds) | Whole-world plan, rebuilt from the full id grid; no view culling, no y-sort, 16 px only. |
| Agents | One `MultiMeshInstance2D` per render bucket (11), 16 px poses in square 128² atlases, `field_agent.gdshader` animates from `INSTANCE_CUSTOM` | `_refresh_bodies` runs O(alive) GDScript per frame with per-id `Dictionary` state (facing, gait, locomotion, actions): fine at 3 k, marginal at 10 k, and every agent is uploaded whether on screen or not. |
| Settlements | `settlement_layer.gd`: ≤ 6 huts + ≤ 4 farms at the species anchor centroid, invention landmarks, smoke pool, flame flicker | Markers, not villages: no footprint, no fences/palisade, no era progression, no water-mill placement, 16 px cells. |
| Camera | Continuous zoom 0.25–8, cursor-anchored | Non-integer zoom shimmers pixel art; 0.25 cannot frame a world wider than ~5 k units. |
| Minimap | Draws the whole-world texture + one `draw_rect` per agent per frame | O(alive) `_draw` every frame; needs the full-world texture that streaming removes. |
| HUD | Flat translucent teal panels (`ui_theme.gd`), text lists | No icons, bars, portraits, frames or tabs. |
| Art source | Hand-typed block lists / row strings in GDScript (`ape_sprites.gd`, `terrain_sprites.gd`, `building_sprites.gd`, `mammal_data/*`) | Deterministic and reviewable, but ~600 lines per family; the reference look needs 10× more sprites at 32–48 px. |

Engine side, the biome grid is `Vec<BiomeCell>` (≈ 40 B per cell), regrown
every tick over **all** cells (`regrow_step`); the largest shipped worlds are
`world_size 4096 / biome_res 512` (262 k cells, cell = 8 units). The bridge
exports whole-world arrays only.

Sizes used below (world units): a biome cell is **8**, a body **6–7**, a hut
**16**, a prop **10**. So at the current mapping one 16 px tile already covers
one cell: **1 texel = 0.5 world unit**. That ratio is kept — the target look is
reached by drawing *more* at that ratio, not by changing it.

## 3. Gap analysis (reference element → what closes it)

| Reference element | Sim signal it maps to | Gap | Phase |
|---|---|---|---|
| Crisp tiles, no blur | — | pixel-perfect pipeline, integer zoom, nearest filtering, drop `soften` | 2 |
| Coast / river bank / cliff / forest-edge transitions | `terrain_ids`, `elevation`, `river_flow` | dual-grid autotiling in the ground shader | 2 |
| Waterfall, river with banks | `river_flow` + elevation drop between river cells | river-edge tiles + waterfall sprite where a river cell's downstream neighbour is ≥ 2 elevation bands lower | 2 |
| Dense forests of individual trees, rocks, reeds | terrain id + biomass (lushness) | per-chunk prop generation, density from biomass, 32 px canopy sprites, y-sort | 2 |
| Villages with footprint, fences, fields, palisade | `settlement_sites` (anchor, members), `Territory`/`War` codex events, invention masks per species | deterministic village layout generator, era architecture sets, event-driven palisade/banners/fire | 4 |
| Water mill, forge, watchtower, siege engine | invention landmarks (Machinery, Metalworking, weapons tree) | landmark placement rules (mill needs a river/water cell), 32–48 px landmark sprites | 4 |
| Two armies with spears/bows/shields | combat streaks, `fire_intent`, `ACT_SPEAR/BOW`, held-invention mask | already mapped; add shield/armour pose variants and a 24 px "hero" atlas option | 3 |
| Mammoths, antlered herds, birds, turtles | archetype from diet/size/livestock; module make-up (Spines, Armor, Jaws) | more archetypes keyed on modules (armour → tortoise, spines → boar/porcupine, large herbivore → mammoth) | 3 |
| `!` predator emote, thought bubbles | `emote_layer.gd` exists | icon set upgrade only | 5 |
| Day counter, resource counters, population | tick, `alive_count`, biomass, traded goods, era | top bar bound to bridge queries | 5 |
| Research panel with progress bars | `tech_panel.gd` adoption fractions | restyle into icon + bar rows | 5 |
| Codex tabs, species portrait, traits, related species | `codex_panel.gd`, `phylogeny()`, `agent_detail()` | new tabbed Codex panel; portrait = atlas cell ×4 | 5 |
| Event log with icons | `codex_events_since` + `event_fx.gd` colours | icon per event class | 5 |
| Minimap with compass and view box | `minimap_panel.gd` | frame + compass; use the overview mip, agents from a Rust density grid | 2, 5 |
| Unit card (portrait, HP/stamina, stat pips) | `inspector_panel.gd`, `agent_detail()` | restyle; energy → HP bar, stamina ← needs/sleep, pips ← module counts | 5 |
| Ecosystem Health / Biodiversity meters | `coevo_metrics()`, species count, diversity | two bound meters | 5 |
| Much larger maps | `world_size`, `biome_res`, `hash_res` | chunked bridge export, streaming ground, culled agents, regrowth cost at 1–4 M cells | 1, 2, 3 |

## 4. Decisions

**D1 — Visual scale contract.** 1 texel = 0.5 world unit at zoom 1. A biome
cell is one 16 px ground tile; figures are 16 px (24 px hero atlas optional);
canopy trees 32 px; huts 32 px; landmarks 48 px. All atlases stay **square**
(128², 256², 512²) because extreme-aspect atlases corrupt on the Metal
MultiMesh path (the existing contract in `ape_sprites.gd`).

**D2 — Sim grid and visual grid are decoupled.** Terrain detail below one
cell (coast tiles, prop placement, cliffs, waterfall) is derived
deterministically on the viewer side from cell ids, elevation, river flow and
a cell hash. "Much larger maps" in sim terms means a bigger `world_size` at the
same 8-unit cell (more cells), never a finer cell — sensing radii, perception
caps and `river_threshold` all assume the cell size.

**D3 — Chunked streaming.** The world is split into 64×64-cell chunks
(512 world units). The bridge exports one chunk at a time as packed bytes with
a per-chunk version counter; the viewer keeps only the chunks intersecting the
view (+1 ring) resident and re-uploads a chunk only when its version moved.
Whole-world arrays remain for tests and the headless recorder but are no
longer on the frame path.

**D4 — Pixel-perfect pipeline.** The world renders into a `SubViewport` at
integer texel scale with `snap_2d_transforms_to_pixel`; the camera zooms in
integer steps (1×, 2×, 3×, 4×, 6×, 8×) above 1× and switches to a
whole-world **overview mip** (a Rust-downsampled 512² texture, 4 s cadence)
below 1×. Smooth wheel zoom eases between steps; the UI layer stays at the
window's native resolution.

**D5 — Asset pipeline (needs a decision, see §9).** The reference density is
not reachable by hand-typed block lists. Proposal: keep the block-list families
that exist, and add an **imported atlas pipeline** — `game/assets/atlases/*.png`
(Aseprite sources under `game/assets/src/`, exported by `scripts/atlas-export.sh`),
each with a sidecar `*.atlas.json` (cell size, grid, named cells) validated by
a headless test (square grid, cell size, no empty named cells). Nearest
filtering and no mipmaps are pinned in `.import`. Generated concept boards are
reference only, never shipped. This overrides the "no imported sprite sheets"
non-goal of the 2026-09-11 visual-system spec, deliberately.

**D6 — Torus wrap is view-relative.** Instead of 9 clones per layer, each
streamed chunk and each agent is placed at its wrapped position nearest the
camera; a layer draws at most the copies that intersect the view. Existing
`_make_wrap_clones()` users migrate layer by layer; until they do they keep
working.

**D7 — Structures stay presentation.** Village footprints are a pure function
of (settlement species id, anchor, member count, era, recent events). Nothing
is written back to the sim; replay stays bit-identical.

**D8 — Determinism and goldens are untouched.** Engine work is read-only
bridge queries plus opt-in scenario fields (`biome_step_interval`, larger
`biome_res`). No default path changes; `FORMAT_VERSION` moves only if a
scenario field is persisted.

**D9 — 2.5D is layering plus relief, not projection.** The reference boards
are top-down ground with front-facing objects (the Zelda/Stardew "3/4" read),
so the world stays a flat 2D plane with the camera unchanged and depth comes
from three cheap cues, all presentation-only:

1. *Occlusion by height.* Every standing sprite (canopy tree, hut, hall,
   fence, landmark) is cut at ~60% of its opaque height (`sprite_split.gd`)
   into a crown/roof layer drawn **above** the figures (z ≥ 1) and a
   trunk/wall layer drawn **below** them (z ≤ −1). Both halves share one
   MultiMesh, so the cut costs one extra draw and no per-frame work. A figure
   north of a tree vanishes under its crown; a figure south of a hut stands
   in front of its wall. This is the classic split-sprite approximation of a
   y-sort; the residual error (a figure exactly at the cut line) is a few
   pixels and accepted. A true cross-layer y-sort (§6 Phase 3, deferred)
   would replace it only if the split shows in play.
2. *Contact shadows.* One soft ellipse under every visible figure
   (`agent_layer.gd`, z −1, 34% black), sized to the body, so figures stand
   on the ground instead of floating over it. Trees and huts already bake
   their ground shadow into the sprite.
3. *Terraced relief.* The terrain shader quantises the packed elevation
   (texture alpha) into `terrace_levels` steps; where a cell stands a step
   above its southern neighbour the cell's bottom band wears a dark cliff
   face with a lit lip, a plateau's northern edge a bright rim, and east/west
   ledges a thin dark line; land brightens with altitude. Mountains and river
   valleys read as stacked ledges. All of it is per-cell arithmetic on
   existing chunk data — no new bridge queries, no sim change.

Rejected: an isometric or oblique projection (every atlas, the hash-grid
picking, the chunk streaming and the torus wrap would need re-deriving for a
look the reference boards do not use), and a normal-mapped 2D lighting pass
(Godot 2D lights work per-sprite and would not touch the MultiMesh figures).

## 5. Scale tiers and budgets

| Tier | `world_size` | `biome_res` (cells) | `hash_res` | Cells | Biome memory | Chunks | Status |
|---|---|---|---|---|---|---|---|
| Default | 1024 | 128 | 64 | 16 k | 0.7 MB | 4 | shipped |
| Large | 4096 | 512 | 256 | 262 k | 10 MB | 64 | shipped (`continental`, `riverlands`) |
| Huge | 8192 | 1024 | 512 | 1 M | 42 MB | 256 | Phase 1 target |
| Vast | 16384 | 2048 | 1024 | 4.2 M | 170 MB | 1024 | Phase 1 stretch; needs `biome_step_interval` |

Viewer budgets, measured on the `run` screenshot harness at 1280×800:

- ≥ 55 fps at Huge with 10 k agents, ≥ 45 fps at Vast, with fewer than 8
  chunk uploads per frame and no GDScript loop over more than the *visible*
  agent set.
- Resident chunk textures ≤ 64 (a 3×3 view ring at 8× zoom is 9; the overview
  mip covers the rest).
- Ground: one draw per resident chunk; props: one MultiMesh per (chunk, prop
  atlas); agents: one MultiMesh per bucket, visible instances only.

Engine budgets:

- `regrow_step` at Huge ≤ 2 ms/tick on the bench machine; at Vast the opt-in
  `biome_step_interval = 4` keeps the amortised cost under the same line.
- ≤ 10 % whole-tick regression at 10 k agents on existing scenarios (roadmap
  perf budget) — expected 0 %, since nothing on the default path changes.

## 6. Phases

Sizes follow the ROADMAP convention (S ≈ days, M ≈ 1–2 weeks, L ≈ 3+ weeks).
Phases 1 and A can run in parallel; 2 depends on 1; 3 and 4 depend on 2 and A;
5 depends only on A; 6 closes.

### Phase 0 — Baseline and instrumentation `[V, S]`

- Capture the current look at Default, Large and the largest run that loads,
  at 1×, 4× and fit-to-world, into `gallery/` with a `baseline-` prefix.
- Add a frame-time readout to the HUD (`F3`): fps, chunk uploads, resident
  chunks, visible agents, GDScript ms in `_refresh_bodies`.
- Add `scripts/viewer-bench.sh`: runs the screenshot harness for N frames on
  a scenario and prints the readout as CSV — the number every later phase
  quotes.
- Generate one pixel-art reference board per family (terrain transitions,
  trees/rocks, buildings by era, figures with weapons, HUD frames/icons) for
  design QA. Reference only.

*Done when:* baseline captures and the CSV exist in the PR, and every budget
in §5 has a measured "before" number.

### Phase 1 — Engine-side scale plumbing `[E/T, M]`

Files: `crates/anabios-godot/src/lib.rs`, `crates/anabios-core/src/biome.rs`,
`crates/anabios-core/src/scenario.rs`, `crates/anabios-headless/src/record.rs`.

1. **Chunk export.** `biome_chunk_bytes(cx, cy) -> PackedByteArray`
   returning `64×64×4` RGBA8 (colour from the shared `cell_color`, elevation in
   alpha, river tint applied) and `biome_chunk_ids(cx, cy)` returning `64×64`
   bytes with a 1-cell apron on each side (66×66) so autotiling never needs a
   neighbouring chunk. `biome_chunk_version(cx, cy) -> i64`: a counter bumped
   by any write into that chunk (graze, regrow above an epsilon, disturbance,
   pollution, succession). Cheapest correct implementation: a per-chunk dirty
   flag set by the cell-mutation paths and folded into a version on read.
2. **Overview mip.** `biome_overview(size) -> PackedByteArray`: area-averaged
   downsample to `size²` (512 max) computed in Rust; the minimap and the far
   zoom read this.
3. **Agent queries.** `alive_in_rect(x0, y0, x1, y1) -> PackedInt32Array`
   (alive indices, torus-aware) and `agent_density(res) -> PackedByteArray`
   (counts per `res²` cell, saturating) for the far-zoom dot layer and the
   minimap.
4. **Packed render state.** `alive_render_state() -> PackedFloat32Array`
   with per-alive `(x, y, size, diet, bucket, act_hint, heading_x)` so
   `_refresh_bodies` reads one array instead of seven; facing and gait
   smoothing move to Phase 3.
5. **Scale fields.** Opt-in `biome_step_interval` (regrow/recolonize every N
   ticks; default 1 = today, goldens unchanged) and validation that
   `world_size / hash_res ≈ 16` still holds at Huge/Vast. Two new
   test-pinned scenarios: `scenarios/huge-steppe.toml` (8192/1024/512) and
   `scenarios/experiments/vast-steppe.toml` (16384/2048/1024, interval 4).
6. **Measure** `regrow_step`/`recolonize_step` and `snapshot_bytes` at both
   tiers in `tick_bench.rs`; record in `docs/perf-notes.md`.

*Done when:* the chunk API round-trips against `biome_colors()` in a Rust
test (byte-identical per chunk), versions bump exactly when a cell in the chunk
changed, both scenarios pass `all_scenarios.rs`, goldens are unchanged, and the
bench numbers meet §5.

### Phase A — Asset pipeline `[V/T, S–M]` (parallel with Phase 1)

Files: `game/assets/`, `scripts/atlas-export.sh`,
`game/scripts/atlas_registry.gd`, `game/scripts/test_atlas_registry.gd`, CI.

1. Land the D5 decision: `game/assets/atlases/<family>.png` + `.atlas.json`
   manifest (`cell_px`, `cols`, `cells: {name: index}`), `.import` pinned to
   nearest/no-mipmaps, Aseprite sources under `game/assets/src/`.
2. `AtlasRegistry.load(family) -> {texture, cells}` with a headless test that
   every manifest is a square grid, every named cell has opaque pixels, and
   `cell_px ∈ {16, 32, 48}`.
3. Migration shim: existing block-list families can be *exported* into the
   same format by a one-off script, so the shader and MultiMesh contracts see
   one kind of atlas from here on. Existing tests keep running against the
   block lists until each family is replaced.
4. First real atlases (from the Phase 0 boards, cleaned in Aseprite):
   `terrain_transitions` (16 px, dual-grid sets for 6 terrain pairs + river
   banks + cliff), `flora` (32 px: 6 trees, 3 bushes, rocks, reeds, logs),
   `structures` (32/48 px, three eras), `hud` (9-slice frames, icons).

*Done when:* CI runs the registry test, one family renders from an imported
atlas with no visible change in a capture, and the manifest format is
documented in `game/assets/README.md`.

### Phase 2 — Terrain at scale `[V, L]`

Files: `game/scripts/biome_renderer.gd` → `ground_layer.gd` +
`ground_chunk.gd`, `game/shaders/terrain.gdshader`, `camera_controller.gd`,
`minimap_panel.gd`, `terrain_scatter.gd` → `prop_chunk.gd`, `main.tscn`.

1. **Pixel-perfect pipeline (D4).** World `SubViewport` at
   `window / zoom_step`, nearest upscale, pixel snap on; integer zoom steps
   with eased transitions; `F` frames the world on the overview.
2. **Chunk streaming (D3).** `GroundLayer` keeps a `Dictionary` of resident
   `GroundChunk`s keyed by `(cx, cy)`, each a `Sprite2D` (64×64 texel texture,
   colour + id apron) placed at its view-relative wrapped position (D6).
   Per frame: compute the visible chunk set, evict outside the ring, upload at
   most `UPLOAD_BUDGET = 8` new/dirty chunks (version changed), oldest first.
3. **Crisp tiles + autotiling.** Drop `soften` and linear filtering; the
   shader keeps hillshade (from alpha) and water but samples **dual-grid
   transition tiles**: for each half-offset corner the 4 surrounding ids pick
   one of 16 tiles from the pair set (water/land, grass/forest, sand/grass,
   rock/land, river bank, tundra/taiga). Cliffs where the elevation step
   between neighbours crosses `CLIFF_STEP`; a waterfall sprite where a river
   cell drops ≥ 2 bands. Data overlays (`biome_mode = 0`) still pass through
   untouched.
4. **Props per chunk.** `PropChunk` builds one MultiMesh per (chunk, atlas)
   from the chunk ids + biomass: density per terrain scaled by lushness,
   deterministic cell hash → kind/variant/jitter, **y-sorted** with the agent
   and structure layers (`Node2D.y_sort_enabled` on a shared parent). Canopy
   trees use the 32 px flora atlas. Props vanish with their chunk; no
   whole-world plan, no clones.
5. **Overview and minimap.** Below 1× the ground draws the overview mip
   alone; the minimap draws the mip, the `agent_density` grid as dots, the
   compass rose and the wrapped view box. No per-agent `draw_rect`.
6. **Retire** the 3×3 ground tiles, `_blit`, and `PROP_BUDGET`; `[B]` keeps
   toggling tiles+props for A/B captures.

*Done when:* Huge streams at ≥ 55 fps with the readout showing ≤ 8 uploads
per frame in steady pan, coastlines/rivers/forest edges read as in the
reference at 2×–4×, the far zoom is the overview mip, and `test_terrain_*`
plus a new `test_ground_chunk.gd` (visible-set, wrap placement, budget,
version-skips) pass headless.

### Phase 3 — Figures at scale `[V, M]`

Files: `main.gd` → `agent_layer.gd`, `mammal_sprites.gd`, `mammal_data/*`,
`field_agent.gdshader`, `crates/anabios-godot/src/lib.rs`.

1. **Visible set only.** `_refresh_bodies` iterates
   `alive_in_rect(view + margin)`; off-screen agents are neither smoothed nor
   uploaded. Far zoom (< 1×) draws the density grid instead of bodies.
2. **Animation state out of Dictionaries.** Facing, gait phase, walk weight,
   action hold move to parallel `Packed*Array`s indexed by a stable slot
   (id → slot map maintained on birth/death), or to a small bridge helper
   (`agent_anim_step(dt)`) — pick whichever the Phase 0 readout shows to be
   the bottleneck; target ≤ 1.5 ms GDScript at 3 k visible.
3. **More archetypes.** Module-keyed families on top of diet/size: armour →
   tortoise, spines → boar/porcupine, large herbivore + Storage → mammoth,
   small aquatic-adjacent herbivore → wading bird; livestock keeps its rig.
   Shield/armour pose variants for the ape weapon pairs.
4. **Hero atlas option.** A 24 px atlas variant for buckets when zoom ≥ 4×
   (same pose contract, `CELL_PX` as a uniform), behind a config toggle so
   the 16 px look remains the default.
5. Death ghosts, trails and emotes follow the same visible-set rule.

*Done when:* 10 k agents at Huge hold ≥ 55 fps with 3 k visible, the
per-frame GDScript budget is met and shown in the readout, the new
archetypes have `test_mammal_sprites.gd` coverage, and replay captures are
pixel-identical between the Dictionary and packed paths at 1× (animation is
presentation, but the pose choice must not drift).

### Phase 4 — Villages and structures `[V, L]`

Files: `settlement_layer.gd` → `village_layer.gd` + `village_layout.gd`,
`building_sprites.gd` → `structures` atlas, `hub_layer.gd`, `caravan_layer.gd`,
`event_fx.gd`, `viewer_effects.gd`.

1. **Layout generator (D7).** `VillageLayout.plan(sid, anchor, members, era,
   flags, is_water_at) -> Array[Placement]` on a local 8-unit grid: a central
   hearth, huts on a spiral (count = f(members)), fields on the leeward side
   for ≥ 24 members, a fence around fields, a palisade ring when the species
   has fired `Territory` or `War` in the last N ticks, a watchtower at the
   palisade gate, banners in the species colour, a mill on the nearest river
   or water cell for Machinery holders, a forge for Metalworking, a granary
   for Farming, a scriptorium for Writing. Deterministic from its inputs;
   unit-tested for overlap-freedom and water avoidance.
2. **Era sets.** Three architecture sets in the `structures` atlas: camp
   (tents, windbreak), thatch (huts, fences, fields), timber/stone (halls,
   palisade, tower, mill). Era from the species' held-invention mask.
3. **Life.** Smoke from hearths and forges, fire on huts inside a recent
   `CombatRaid`/`War` radius (with the existing cooldown/budget model),
   construction pop when a placement first appears, linger/fade as today.
4. **Y-sort.** Structures, props and agents share one y-sorted parent so
   villagers walk behind huts and in front of fences.
5. **Trade.** Hubs become a market square in the same language; caravans keep
   their cart sprites and use the village gates as endpoints.

*Done when:* a `settlement.toml` run at 4× shows a village with fences,
fields, smoke and an era-appropriate landmark; a `war.toml` run shows the
palisade, tower and burning huts at the raid; `test_village_layout.gd` pins
determinism, overlap-freedom and water avoidance; nothing in the sim or the
replay hashes changes.

### Phase 5 — HUD `[V, M]`

Files: `ui_theme.gd`, `main.tscn`, new `top_bar.gd`, `research_panel.gd`
(from `tech_panel.gd`), `codex_panel.gd` (tabbed), `event_log.gd`,
`unit_card.gd` (from `inspector_panel.gd`), `minimap_panel.gd`, `hotbar.gd`,
`ecosystem_meters.gd`, the `hud` atlas.

1. **Theme.** 9-slice pixel frames (`StyleBoxTexture`), a pixel font at 2×,
   icon registry from the `hud` atlas; UI stays on the native-resolution
   `CanvasLayer`.
2. **Top bar.** Day (ticks / `DAY_TICKS`), pause / 1× / 4× / 16× / 64× as
   icons, counters bound to bridge queries: biomass, traded goods (when
   `resources_active`), population, highest era.
3. **Research panel.** Per-species tech rows with invention icon, name,
   adoption-fraction bar and "Completed" checkmarks (the reference's Stone
   Tools → Bronze rows), bound to the existing `tech_panel.gd` data.
4. **Codex panel.** Tabs Species / Biomes / Tech / Culture. Species: portrait
   (atlas idle cell ×4 in the species coat), diet, size, generation, traits
   from `agent_detail` means, related species from `phylogeny()`. Biomes:
   terrain legend with tile thumbnails. Tech/Culture: invention catalogue and
   meme channels. The codex *event* stream moves to the event log.
5. **Event log.** Icon + text per codex event, coloured as `event_fx.gd`
   already colours them; click = jump camera (existing `V` behaviour).
6. **Minimap.** Frame, compass rose, view box, density dots (Phase 2 data).
7. **Unit card.** Portrait, name (archetype + species), HP ← energy, stamina
   ← sleep/thirst drives when `basic_needs` is on, four pips ← weapon /
   armour / locomotor / sensor module counts.
8. **Hotbar.** Overlay and tool shortcuts as icons (ground `[G]`, body `[C]`,
   module pips `[M]`, charts, replay `[R]`, event cam `[V]`, settings).
9. **Meters.** Ecosystem Health (mean biomass / capacity) and Biodiversity
   (live species count, normalised) bottom right.

*Done when:* every panel in the two reference boards has a counterpart bound
to live data, `ui_scale` still works, the layout holds at 1280×800 and
1920×1080, and the showcase director's overlay switches still resolve.

### Phase 6 — Showcase and close-out `[V/T, S]`

- Regenerate `gallery/` and the four showcase decks; side-by-side
  before/after captures at each zoom step.
- Update `README.md` (viewer section, controls), `docs/scenarios.md` (new
  tiers), `docs/perf-notes.md` (viewer numbers), and the ROADMAP entry.
- Record the WASM/web-player implication: the replay player renders from
  `record.rs` bytes and stays curated; the chunk API is a prerequisite if the
  Horizon-2 spike chooses live in-browser simulation.

## 7. Verification (every phase)

- `gdformat --check game/scripts && gdlint game/scripts`; headless script
  tests via `scripts/godot-smoke.sh scripts …`; scene smoke on
  `main.tscn`/`menu.tscn`.
- `cargo test --workspace` and the determinism gate; goldens must not move.
- Fennara `validate_scene` and rendered captures at 1×, 2×, 4× for every
  visual subsystem, checked for atlas bleed, seams at chunk and torus edges,
  and y-sort errors.
- `scripts/viewer-bench.sh` numbers quoted in each PR against the Phase 0
  baseline.

## 8. Risks

- **Regrowth cost at Vast** (4 M cells/tick). Mitigated by the opt-in
  `biome_step_interval`; if that is not enough, the next lever is chunk-active
  regrowth (skip chunks at carrying capacity), which changes float results and
  therefore needs its own flag and goldens.
- **Metal MultiMesh atlas corruption.** Every atlas stays square and every
  per-instance value goes through `INSTANCE_CUSTOM` or a flat varying, as the
  existing shaders document. Test on macOS before merging Phase 2 and 3.
- **GDScript per-frame ceilings.** If the packed-array rewrite in Phase 3
  does not reach the budget, the animation step moves into the bridge (Rust),
  which the `alive_render_state` API already anticipates.
- **Chunk seams.** The 1-cell id apron and world-space hashing (as
  `terrain_scatter.gd` already does) keep tiles and props continuous across
  chunk and torus edges; a seam test scans a captured frame for straight
  discontinuities at chunk borders.
- **Art volume.** The reference density is ~10× today's sprite count. Phase A
  exists to make that tractable; without the D5 decision the plan degrades
  to procedural art at lower density.

## 9. Open decisions for the owner

1. **D5 asset policy** — allow imported PNG atlases with manifests (proposed),
   or stay fully procedural (lower ceiling, slower).
2. **Hero atlas** — ship 24 px figures as the default at ≥ 4× zoom, or keep
   16 px everywhere and only add weapon/shield variants.
3. **Vast tier** — commit to 16384-unit worlds now (needs the regrowth flag
   and the memory budget), or stop at Huge for this arc.

## 10. Sequencing

```
P0 baseline (S) ─► P1 engine plumbing (M) ─► P2 terrain at scale (L) ─► P3 figures (M) ─┐
                └► PA asset pipeline (S–M) ──────────────────────────► P4 villages (L) ──┼─► P6 close-out (S)
                                                                    └► P5 HUD (M) ───────┘
```

Roughly one quarter at the ROADMAP's sizing, with P2 the critical path.

## Out of scope

- Any simulation-side building, road or institution substrate (ROADMAP
  guardrail).
- Rewriting the viewer in another engine, or a 3D/isometric renderer.
- A finer sim cell than 8 world units.
- Replacing the web replay player; it consumes the same bytes and is decided
  by the Horizon-2 WASM spike.
