# anabios — atlas (three.js frontend)

A 3D, in-browser front end for the anabios core: the deterministic simulation
compiled to **WebAssembly** and rendered with **three.js** — terrain relief from
the sim's elevation field under a painterly ground shader (noise-jittered cell
borders, rock on the steep slopes, snow on the peaks, a beach band and a
darkened seabed at the water line), the biome map lushing and scarring in real
time, instanced forests planted from the terrain grid that shrink where a cell
is scarred bare and sway in the wind (multi-lobed canopies, three-tier
conifers, grass tufts on the open ground, rock scatter), depth-shaded water
with sun glints, river sparkle, seabed caustics, wet sand and a foam fringe,
sun shadows and drifting cloud shadows over everything, a sky dome whose sun
glow swells as a gentle day cycle sweeps the sun round the plate toward a
golden dusk, bird flocks circling overhead, every agent as an instanced grazer,
hunter or upright hominid figure (by diet: below, above or inside the
0.34–0.66 omnivore band) in its species' colour, shaded per individual by
its genome, and walking (or sprinting, when it
flees or fights) on articulated legs, a glimmer where an agent is born and a
grey puff where one dies, combat volleys and trade lanes as fading light with
impact sparks, pitched-roof hut villages with hearth smoke at hominid
settlement sites (omnivore lineages only — a settled herd or pack builds
nothing; pinned where they are founded, in a clearing the forest shrinks back
from and grows back into once they are gone, moving only when their people
have plainly left), awninged market stalls at the trade hubs (a hub that
falls in a lake has its stall drawn on the nearest dry shore; the sim's hub
is untouched; huts and stalls terraced into slopes on earth plinths), the
codex streaming "first
emergence" events into a feed (with opt-in rings, light pillars and ember
bursts on the map — the "event markers" layer, off by default), and a
soft bloom over the hot pixels. The same page also plays the **recorded replay** the showcase deck ships
(`showcase/replay.js`), so the hosted deep-time story and the live sandbox are
one product.

Nothing is scripted: the live world is `anabios-core` stepping in the browser
(`crates/anabios-wasm` is the bridge), and it reproduces the native trajectory
bit-for-bit — `scripts/web.sh test` asserts it.

## Run it

```sh
rustup target add wasm32-unknown-unknown     # once
scripts/web.sh build                         # wasm core → web/wasm, three.js → web/vendor, scenarios → web/scenarios
scripts/web.sh serve                         # http://127.0.0.1:8080/
```

The page is plain ES modules behind an import map — no bundler, no framework.
It needs an HTTP server (module scripts and `fetch` don't work from `file://`);
any static server over `web/` works.

Deep links: `?scenario=tribes&seed=3&speed=4`, `?replay=out-of-africa-saga`,
`&color=diet`, `&paused=1`, `&events=1` (event markers on). A recorded replay
plays back its one fixed seed, so the seed box and **Random seed** grey out
while a replay entry is selected.

**Controls:** drag orbits, right-drag pans, wheel zooms · click an agent for its
inspector (energy, age, diet, mood, body plan, held inventions, genome-driven
learning flags) · **space** pause · **1–5** speed (¼× … 64× ticks per frame) ·
**C** cycle colour mode · **F** frame the world · **V** event tour (a slow
orbit that cuts to each fresh codex event) · **L** follow the selected agent ·
**H** hide the HUD · click a codex line to fly to the event · click a species
row to fly to a member.

Framing (**F**, the `frame` button, first load, `?cam=fit`) fits the whole
plate into the part of the window the HUD leaves free — right of the view rail
(when the window is wide enough to spare it) and above the transport row — for
any aspect ratio. While the camera is still at that framed view, resizing the
window, collapsing the rail or hiding the HUD re-frames it; once you orbit, pan
or zoom away the camera is left alone until the next **F**.

Colour modes: species (the species list's colour, shaded per individual by the genome's hue/sat/val; livestock bleached), diet, dialect
hue, energy, and — when the scenario enables the subsystem — mood, arousal and
infection. Layers: relief, water, forests, shadows, combat, trade, villages,
markets, event markers (off by default), sparks & smoke, wireframe, day cycle, bloom, clouds, birds (forests,
shadows and bloom are the three to switch off on a weak GPU: up to 60k trees
and 60k grass tufts, one 2048² shadow cascade and a five-level bloom chain). The day cycle is on when
viewing live and off under `capture=1` so gallery stills stay at noon; `&day=1`
opts a capture in.

## How it fits together

```
anabios-core (deterministic sim)
      │  step() + read-only columns + codex events
      ▼
crates/anabios-wasm  ── cargo build --target wasm32-unknown-unknown ──►  web/wasm/anabios.wasm
  (plain C ABI: opaque Sim handle, fixed-stride f32/u8 buffers, JSON for sparse data)
      │  web/src/sim.js (≈150-line loader, no wasm-bindgen)
      ▼
web/src/sources.js   LiveSource (wasm)  ·  ReplaySource (showcase/replay.js format)
      │  one interface: agents(), biomeRgba(), elevation(), streaks(), trades(), sites(), hubs(), events(), species(), agent(id), meta()
      ▼
web/src/terrain.js   heightmap, ground shader (clouds, shore), water, forests + grass   web/src/layers.js   figures + gait, segments, villages, hubs, birds, event rings + pillars
web/src/scene.js     renderer / bloom / sky dome / camera / lights / daylight  web/src/particles.js   ember, spark, mote and smoke pools
web/src/main.js      loop, HUD, picking, URL state, event tour
```

### Why a hand-rolled ABI

The surface is ~35 functions. A `#[unsafe(no_mangle)] extern "C"` layer plus a
small loader is simpler to read than generated glue, needs no `wasm-bindgen`
CLI (whose version must match the crate's), and builds with nothing but the
`wasm32-unknown-unknown` target. Buffers are viewed straight out of linear
memory (`Float32Array` over `memory.buffer`); JS re-creates the view after every
call because a refill may reallocate and `memory.grow` detaches old views.

### Determinism receipt

The HUD shows two hashes. `state_hash` is the CLI's receipt (FNV over the
bincode snapshot) — but bincode encodes the core's `BitVec<usize>` columns via
their store words, so that hash is **pointer-width dependent** and a wasm run
will not match the native CLI's value even when the trajectory is identical.
`fingerprint` is the portable equivalent (FNV over fixed-width LE bytes of the
trajectory-defining state); it is what `scripts/web.sh test` compares against
the native `fingerprint` example:

```sh
scripts/web.sh test predator-prey 300 7
# → determinism: wasm fingerprint matches the native run
```

### Measured in-browser tick rate (the roadmap's WASM spike)

Single-threaded wasm (rayon falls back to sequential on `wasm32-unknown-unknown`;
no SIMD), release build, node 22 / V8, one core of a cloud container. Measured
before the twelve-world consolidation, when the scenario files ran with most
subsystems off (`inventions` has since been retired into `tribes`); natively
the full-stack `minimal` costs ~3.5× its all-off copy per tick, so expect the
current worlds to run slower than these rows:

| scenario | agents at end | wasm ticks/s | ms/tick |
|---|---|---|---|
| `predator-prey` (seed 7, 300 ticks) | 274 | ~1 800 | 0.55 |
| `inventions` (2000 ticks) | 400 | ~625 | 1.6 |
| `out-of-africa-saga` (seed 318, 500 ticks) | 2 914 | ~15 | 68 |

Small and mid-size worlds run live at 1×–64×. The flagship saga at ~3k agents
runs at roughly a tick per frame; for it the recorded replay stays the smoother
option, which is why the page keeps both sources.

## Reproducible captures (the gallery harness)

`?tick=N` fast-forwards a fresh world to exactly tick N and pauses there;
`?cam=fit | event | x,y,zoom[,polar]` frames it (`zoom` is the Godot viewer's
pixels-per-world-unit at 1280 px, so `gallery/README.md`'s `ANABIOS_CAM_*`
values map 1:1); `?inspect=<id> | sp<species>` pins an agent; `&hud=0` hides
the HUD. `web/scripts/capture.mjs` drives a headless Chromium through a list
of such shots and writes PNGs — `gallery/atlas/` is the atlas counterpart of
the Godot gallery, rendered from `gallery/atlas/shots.json`:

```sh
scripts/web.sh serve &
npm --prefix web i -D playwright && npx --prefix web playwright install chromium   # once
node web/scripts/capture.mjs gallery/atlas/shots.json          # → gallery/atlas/*.png
```

Every shot reproduces from its params alone (the world is deterministic per
seed), so the pinned tick, camera and agent id are the whole recipe.

## Tests

- `cargo test -p anabios-wasm` — pure view builders (well-formed buffers,
  side-effect-free reads, event/catalog parity) and the C-ABI round trip, natively.
- `node web/test/body-scale.mjs`, `node web/test/villages.mjs`,
  `node web/test/forest-clearing.mjs`, `node web/test/hubs.mjs`,
  `node web/test/figures.mjs`, `node web/test/classify-terrain.mjs` — pure
  layer logic under node (after
  `npm --prefix web ci`; CI runs them in the `web` job): the figure-size
  clamp; figures (kind by diet band, legs attached, species colours); villages — built by hominid (omnivore)
  lineages only, pinned where they were founded — no sliding with the
  wandering anchor centroid, one fade-out-and-regrow move when the people
  have really left, no hut flicker at a band edge, no huts on water, huts and
  market stalls terraced on earth plinths so none floats on a slope; the
  forest easing out of a clearing and growing back when it is released
  (only the trees in transition touched, paced by sim ticks within a
  wall-time band, composed with the scarred-bare size, settled at once on a
  time jump or rebuild); market stalls kept off water; and the ground colour
  read back per terrain from `cell_color`'s own palette — replay colours
  classified to their terrain at any lushness (the live out-of-africa-saga
  grid matches the sim's ids exactly once the wasm module is built), and
  only burn scars and heavy pollution, never healthy savanna straw or
  pioneer regrowth, shrinking the planting to its scarred-bare size.
- `scripts/web.sh test [scenario] [ticks] [seed]` — builds the module, runs
  `web/test/wasm-smoke.mjs` under node (every export exercised, malformed TOML
  reported, buffers in range), and asserts the wasm fingerprint equals the
  native one. CI runs this in the `web` job.

## Deploying

`web/` is self-contained once built: `index.html`, `src/`, `vendor/`, `wasm/`,
`scenarios/`, `replays/`. Upload the directory to any static host (the same
`upload-pages-artifact` step the showcase workflow uses).
