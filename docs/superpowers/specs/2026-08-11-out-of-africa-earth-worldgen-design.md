# Out of Africa on a Real Earth Map — Design (2026-08-11)

## Summary

Add a new grand-scale scenario, `out-of-africa-earth`, that runs the full
out-of-africa subsystem stack on **real Earth geography** instead of the current
hand-probed procedural (seed-318) world. Real elevation, temperature, and
precipitation rasters drive the biome field; founding lineages are placed on
real coordinates so the human dispersal funnels through the *actual* African
exits (Sinai / Bab-el-Mandeb, Gibraltar).

This is **sub-project A** of the user's request ("fully implement out of africa
with large-scale map generation and inventions completed"). It is a concrete,
well-bounded deliverable *and* the substrate for **sub-project B** (emergent
era-3 invention climb), which is deferred to a probe-first research cycle
documented at the end.

Scale is fixed at **256×256 biome resolution / 4096-unit world** — matching the
existing `continental` scenario, which is proven to run at that size.

## Motivation & prior context

The out-of-africa scenario already exists at grand scale
(`scenarios/out-of-africa.toml`, `-saga.toml`) on a *procedural* climate-driven
world. Two constraints shape this design:

1. **The map is procedural, not real.** The current geography is fBm gradient
   noise + a continent mask + domain warping, hand-probed at seed 318 to read
   "Earth-like" (equatorial Africa south, archaic Eurasia north, a mid-map sea
   with two Sahara crossings). The user wants a *bigger* world built from a
   *real* Earth map.

2. **The emergent invention climb is a documented open problem.**
   [`2026-08-02-ooa-climb-findings.md`](2026-08-02-ooa-climb-findings.md): a
   16-seed × 20k-tick sweep shows the grand run reaches **0/16 era-3
   emergently** — a fast r-selected `asocial_forager` competitively excludes the
   culture-bearing lineages before they climb past era-1. The findings doc names
   three untried mechanisms — **niche separation**, a culture fitness floor, a
   competitor cap — and explicitly recommends targeting the *ecological* stage,
   not the discovery/IQ math. Real geography is the most promising **niche
   separation** substrate: a real African cradle isolated from the r-selected
   forager by sea, desert, and mountain. That is why A comes first and B builds
   on it.

## Non-goals

- **Not** modifying `out-of-africa.toml` / `-saga.toml` or any existing
  scenario. They are referenced by the findings and showcase docs and stay
  bit-identical. This is a purely additive new scenario + new codegen path.
- **Not** building sub-project B (the emergent climb mechanism) in this spec.
  B is a separate probe-first cycle; A only lays the substrate and records the
  probe plan.
- **Not** adding any field to `BiomeCell` or `World`. (See "Determinism"— this is
  the load-bearing discipline that keeps the existing golden tests untouched.)
- **Not** making the biome grid non-square. The grid stays `res × res`; Earth's
  2:1 equirectangular aspect is fit by a mild vertical stretch (see Projection).

## Architecture

### Data flow

```
offline (scripts/, NOT in the deterministic core)
  NOAA ETOPO elevation ┐
  climatology temp     ├─ fetch → resample to 256×256 equirectangular
  climatology precip   ┘        → quantize to u8 → assets/earth/*.bin (checked in)

core (anabios-core, deterministic)
  include_bytes!(assets/earth/*.bin)
    → BiomeField::from_earth(res, world_size)
        dequantize → existing elevation[] / temperature[] / moisture[] arrays
        → existing classify(elevation, temperature, moisture) per cell
        → BiomeField (real coastlines + Earth-matching biomes)
```

The key architectural decision: **real data enters through the existing
`elevation` / `temperature` / `moisture` arrays and the existing `classify()`.**
Nothing downstream of the biome field changes — no Whittaker-math change, no new
cell fields, no new world state.

### Components

1. **Offline asset builder** — `scripts/build_earth_map.py` (or a small Rust
   `examples/` binary). Runs once, off the deterministic path. Fetches
   public-domain rasters, resamples to 256×256, quantizes each channel to `u8`,
   writes `assets/earth/{elevation,temperature,precip}.bin` (+ a small header /
   README recording the source, resolution, and quantization range). This tool's
   output is what gets checked in; the tool itself is not run by the sim.

2. **`BiomeField::from_earth(res, world_size)`** in `crates/anabios-core/src/biome.rs`
   — `include_bytes!` the three assets, assert their length matches `res*res`,
   dequantize each `u8` back to the channel's `f32` range, write into the
   existing `elevation` / `temperature` / `moisture` arrays, then run the
   existing per-cell `classify()`. No RNG. `res`/`world_size` are asserted to
   match the asset resolution (256) so a mismatched scenario fails loudly rather
   than silently misreading the raster.

3. **Scenario opt-in** — a new field on the scenario struct,
   `world_map: Option<WorldMapSource>` (`#[serde(default)]`), where
   `WorldMapSource` is an enum with `Earth` for now. When present, the biome
   field is built by `from_earth` instead of `generate`/`generate_with`. Absent
   (every existing scenario) = today's procedural path, bit-identical.

4. **`geo` placement kind** — a new `Placement::Geo { lat, lon, radius }`
   variant. At spawn it resolves to a cluster center via the equirectangular
   transform `x = (lon + 180)/360 · world_size`, `y = (90 − lat)/180 ·
   world_size`, then distributes exactly like `Cluster` (same RNG draw order, so
   determinism is preserved). Self-documenting coordinates for "place the cradle
   in East Africa."

5. **`out-of-africa-earth.toml`** — the new scenario. Full subsystem stack
   (copied from `out-of-africa.toml`'s flags), `world_size = 4096`,
   `biome_res = 256`, `hash_res = 256` (keeping `world_size / hash_res ≈ 16`),
   `world_map = "earth"`, and founding lineages placed with `geo` on real
   coordinates (cradle in equatorial East Africa; archaics in Eurasia; herds on
   the savanna/steppe).

### Projection & topology

- **Full Earth, equirectangular, stretched to the square grid.** Longitude
  −180..180 → x 0..W; latitude 90..−90 → y 0..W. Earth is 2:1; the square grid
  applies a mild 2× vertical stretch that keeps every continent and coastline.
- **E–W torus wrap is authentic** — Earth wraps in longitude, and the sim world
  is already a torus in x. The dateline seam is a real wrap, not an artifact.
- **Poles at the y-edges** — ice/ocean at top and bottom. The engine already
  treats y-edges as poles / a torus seam ("the polar route"). Real polar ice is
  Water/Rock under `classify`, so agents naturally avoid it; N–S torus wrap
  across the poles is harmless (both edges are uninhabitable ice).

## Determinism, goldens, format version

- **The real map is static data (no RNG)** → deterministic by construction. The
  quantized asset is the single source of truth; `from_earth` is a pure
  dequantize + classify.
- **Existing golden tests stay untouched** *because* no shared `BiomeCell` /
  `World` field is added and no existing scenario's biome field changes. This is
  the deliberate discipline: route everything through existing arrays.
- **Scenario struct gains two optional fields** (`world_map`, and the `Geo`
  placement variant). Both are additive `#[serde(default)]` / new enum variants —
  they do not change the deserialization of any existing TOML. Whether this
  requires a `FORMAT_VERSION` bump depends on whether snapshot serialization is
  affected; if a bump is needed, follow the multi-branch-collision + golden-regen
  drill (reserve the next version, regen with `UPDATE_HASHES`).
- **New tests:**
  - `from_earth` produces a deterministic, plausible field (land fraction in a
    sane range; known landmark cells read land not ocean — e.g. central Africa
    is land, mid-Pacific is water).
  - Save→load→step round-trip for `out-of-africa-earth` (per the
    every-opt-in-subsystem determinism harness).
  - `geo` placement maps a known lat/lon to the expected cell and preserves RNG
    draw order vs. an equivalent `cluster`.

## Viewer

The Godot viewer already reads `world_size` / `biome_res` dynamically (the
`continental` scenario runs at 256/4096 today), so the real map should render
without viewer changes. Verify: the field atlas (square 64×64 grid, per the
Metal-corruption fix) and the world minimap both show real coastlines correctly.
Fix only if the bigger/real field exposes a rendering gap.

## Risks

1. **Data acquisition (highest).** "Real elevation + real biomes" hinges on
   actually obtaining public-domain rasters in this environment. **Task 0 is a
   data-acquisition spike** — verify fetch + resample works *before* building the
   core path. If network/data is unavailable, the fallback is a **hand-encoded
   coarse Earth** (recognizable continents authored directly into the asset),
   which keeps everything else in the design identical (same `from_earth` path,
   same asset format) at lower fidelity.
2. **Licensing.** Use public-domain sources only (NOAA/NASA). Record source +
   provenance in `assets/earth/README`. Avoid CC-BY datasets (e.g. WorldClim)
   unless attribution is acceptable.
3. **Perf at 256/4096.** Continental already runs at this size, so the map cost
   is known-acceptable; the new cost is agent count. Keep `max_population` and
   founding counts in the proven range; benchmark a short run before finalizing.
4. **Projection distortion.** The 2× vertical stretch is visually mild and
   narratively harmless (the dispersal reads correctly). Documented, not fixed.

## Sub-project B — emergent climb (probe-first, deferred)

Recorded here so A's placement choices serve B; **not built in this spec.**

After `out-of-africa-earth` runs, test the one untried mechanism the findings
doc names — **geographic niche separation** — *before* writing any mechanism
code:

1. **Probe:** sweep (16 seeds × 20k ticks, the findings-doc protocol) whether a
   real African cradle, isolated from the r-selected `asocial_forager` by sea /
   Sahara / rift, lets a culture-bearing lineage survive past era-1 and climb.
   Derive era-reached from `InventionDiscovered` events exactly as the findings
   doc does, for apples-to-apples comparison against the 0/16 baseline.
2. **Decision gate (from the findings doc):** promote only if a single
   configuration yields **≥50% of seeds at era-3 without ecosystem collapse**.
3. **If a probe shows signal** (culture survives longer / reaches higher era than
   the procedural baseline), *then* spec the mechanism (niche protection, culture
   floor, or competitor cap) as its own cycle. **If not, report the negative
   result honestly** and extend the findings doc — the success criterion is a
   real answer, not a forced era-3.

## Build sequence (for the plan)

0. Data-acquisition spike (verify sources + resample; else fall back to
   hand-encoded coarse Earth).
1. Offline asset builder + checked-in `assets/earth/*` + provenance README.
2. `BiomeField::from_earth` + unit tests (deterministic, plausible, landmarks).
3. `WorldMapSource` scenario field + wire the `from_earth` path in scenario
   codegen.
4. `Placement::Geo` + tests (mapping + RNG-order preservation).
5. `out-of-africa-earth.toml` + save/load round-trip determinism test.
6. Viewer verification (coastlines render; fix only if broken).
7. Short benchmark run; tune founding counts / `max_population` to stay
   watchable; write scenario header comment (like the existing OoA files).
8. (Follow-on cycle) Sub-project B probe.
