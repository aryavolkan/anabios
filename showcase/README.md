# anabios — web showcase

A browser-native, animated, shareable replacement for the static gallery. Scroll
down through **deep time**: each era plays a slice of a real recorded run while the
codex streams the "first emergence" events the detectors actually fired.

Everything on screen is a **deterministic replay of the anabios core** — no scripting,
no hand-authoring. Re-run the same seed and you get the same history, frame for frame.
The world is the sim's real Whittaker **biome map** (forest, savanna, water, tundra —
lushing, polluting and scarring over time), the creatures carry their species colours,
and the era you're in begins at the tick its defining event **actually first fired**.

The biome is drawn as crisp pixel terrain (matching the pixel hominins); the outro carries
a legend for both the codex event colours and the terrain. Settlement sites recorded from the
codex latch appear as hut villages (with tilled farms around the large ones) that linger and
fade after the sim stops reporting them — the same convention as the Godot viewer.
Reduced-motion viewers get a static, representative frame per era (the animation loop suspends
when idle).

**Controls:** scroll to move through time · **space** pauses/resumes · `?era=N` deep-links.

## View it

Open `index.html` in any browser — no server, no build, no Godot:

```sh
open showcase/index.html            # macOS
xdg-open showcase/index.html        # Linux
```

The page loads `replay.js` via a plain `<script>` tag (no fetch, so `file://` works).
Deep-link straight to an era with `?era=N`, e.g. `index.html?era=5` opens on **War & Kin**.

Respects `prefers-reduced-motion` (freezes to a representative frame). Wide screens get
the world plate on the right and the narrative column on the left; it collapses to a
single column on mobile.

## Regenerate the replay

`replay.js` is produced by the `record` subcommand of the headless runner. It runs the
same `step` loop as every other subcommand and only *reads* agent draw data, so the
recorded run is bit-identical to `run` — the `state_hash` printed at the end is the
determinism receipt (it must match `anabios-headless run` for the same scenario/seed/ticks).

```sh
cargo build --release --bin anabios-headless
./target/release/anabios-headless record \
    --scenario scenarios/out-of-africa-saga.toml --seed 318 \
    --out showcase/replay.js
```

Useful flags (defaults tuned to keep the committed file ~1.7 MB):

| flag | default | effect |
|------|---------|--------|
| `--ticks` | 4000 | how much history to record |
| `--sample` | 24 | record a frame every N ticks (player interpolates between them) |
| `--max-agents` | 260 | cap agents per frame via stable-stride subsampling (0 = all) |
| `--max-events` | 1000 | cap codex events, keeping each type's first + a uniform sample (0 = all) |
| `--biome-res` | 96 | biome-map resolution per axis (downsampled from the sim's 128² grid) |
| `--biome-frames` | 6 | biome-map keyframes across the run (0 disables the map) |
| `--out` | `showcase/replay.js` | `.js` wraps `window.ANABIOS_REPLAY=…`; `.json` writes raw JSON |

Any scenario works. The player derives the six era boundaries from the run's real event
stream — Tools & Fire starts at the first invention, Exodus at the first speciation,
Farming & Trade at the first settlement/market, Writing at the first meme-sweep, War & Kin
at the first raid — falling back to even spacing when a signal never fires. Species names
come from the scenario's archetype specs (dynamically speciated splinters show as `species N`).

## Cinematic decks (Godot Movie Maker MP4s)

Beyond the web replay, the Godot viewer records scripted cinematic decks
(`game/showcase/*.json` beat timelines over the live sim). Each deck is
pinned to a same-named scenario and seed and regenerates from a one-line
command (needs a real display + ffmpeg):

| deck | scenario · seed | arc |
|------|-----------------|-----|
| `out-of-africa-saga` | `out-of-africa-saga.toml` · 318 | the flagship: exodus, fire, husbandry, writing |
| `predator-prey` | `predator-prey.toml` · 0 | collapse-and-recovery: grazers vs stalkers |
| `dialects` | `dialects.toml` · 0 | two clusters diverge; a dialect sweeps |
| `inventions` | `inventions.toml` · 0 | the innovators-vs-traditionalists tech race |

```sh
scripts/emergence.sh showcase                 # everything below, one command
# or individually:
scripts/emergence.sh record-web out-of-africa-saga --seed 318   # web replay.js
scripts/emergence.sh record out-of-africa-saga --seed 318       # runs/showcase/*.mp4
scripts/emergence.sh record predator-prey                       # deck-pinned seed
```

`scripts/emergence.sh showcase` regenerates every showcase asset (this
replay + all four deck MP4s into `runs/showcase/`) from the pinned
scenarios/seeds — the roadmap Phase-1 reproducibility bar. Re-run it after
any scenario/deck/sim change; the hosted replay is regenerated again by the
Pages workflow at publish time.

## Host it
The player is published to GitHub Pages via `.github/workflows/showcase.yml` —
gated, never on ordinary pushes:

- **Publish trigger:** push a `v*` tag (e.g. `v0.5.0`), or run the workflow
  manually from the Actions tab (`workflow_dispatch`).
- **Hosted URL:** https://aryavolkan.github.io/anabios/ — live since
  2026-08-09 (first `workflow_dispatch` deploy, run 31332094901; the logged
  `state_hash=0x71f277cb35441357` matches a local release `record` of the
  same scenario/seed bit-for-bit).
- The workflow rebuilds `anabios-headless` in release and regenerates
  `showcase/replay.js` from `scenarios/out-of-africa-saga.toml` at seed `318`
  (see "Regenerate the replay" above) before uploading `showcase/` as the
  Pages artifact, so the hosted deck is reproducible from source rather than
  the committed copy. It logs the run's `state_hash`; compare that against a
  local `record` of the same scenario/seed to confirm the deploy matches.

**Preview locally** without waiting on a deploy:

```sh
scripts/emergence.sh record-web out-of-africa-saga --seed 318 && open showcase/index.html
```

## How it fits together

```
anabios-core (deterministic sim)
      │  step() + read-only agent columns + codex events
      ▼
anabios-headless record   ──►  showcase/replay.js  (compact frames + events + meta)
                                     │  <script>
                                     ▼
                          showcase/index.html  (canvas player + scrollytelling)
```

`index.html` is self-contained (inline CSS/JS); `replay.js` is the only data dependency.
