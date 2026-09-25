# anabios — atlas (three.js frontend)

A 3D, in-browser front end for the anabios core: the deterministic simulation
compiled to **WebAssembly** and rendered with **three.js** — terrain relief from
the sim's own elevation field, the biome map lushing and scarring in real time,
every agent as an instanced figure coloured by its genome, combat volleys and
trade lanes as fading light, hut villages at settlement sites, markets at the
trade hubs, and the codex streaming "first emergence" events as rings on the
map. The same page also plays the **recorded replay** the showcase deck ships
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

Deep links: `?scenario=inventions&seed=3&speed=4`, `?replay=out-of-africa-saga`,
`&color=diet`, `&paused=1`.

**Controls:** drag orbits, right-drag pans, wheel zooms · click an agent for its
inspector (energy, age, diet, mood, body plan, held inventions, genome-driven
learning flags) · **space** pause · **1–5** speed (¼× … 64× ticks per frame) ·
**C** cycle colour mode · **F** frame the world · **L** follow the selected
agent · **H** hide the HUD · click a codex line to fly to the event · click a
species row to fly to a member.

Colour modes: species (genome hue/sat/val, livestock bleached), diet, dialect
hue, energy, and — when the scenario enables the subsystem — mood, arousal and
infection. Layers: relief, water, combat, trade, villages, markets, event rings,
wireframe.

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
web/src/terrain.js   heightmap + water          web/src/layers.js   instanced agents, segments, villages, hubs, event rings
web/src/scene.js     renderer / camera / lights  web/src/main.js     loop, HUD, picking, URL state
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
no SIMD), release build, node 22 / V8, one core of a cloud container:

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
- `scripts/web.sh test [scenario] [ticks] [seed]` — builds the module, runs
  `web/test/wasm-smoke.mjs` under node (every export exercised, malformed TOML
  reported, buffers in range), and asserts the wasm fingerprint equals the
  native one. CI runs this in the `web` job.

## Deploying

`web/` is self-contained once built: `index.html`, `src/`, `vendor/`, `wasm/`,
`scenarios/`, `replays/`. Upload the directory to any static host (the same
`upload-pages-artifact` step the showcase workflow uses).
