# anabios — web showcase

A browser-native, animated, shareable replacement for the static gallery. Scroll
down through **deep time**: each era plays a slice of a real recorded run while the
codex streams the "first emergence" events the detectors actually fired.

Everything on screen is a **deterministic replay of the anabios core** — no scripting,
no hand-authoring. Re-run the same seed and you get the same history, frame for frame.
The world is the sim's real Whittaker **biome map** (forest, savanna, water, tundra —
lushing, polluting and scarring over time), the creatures carry their species colours,
and the era you're in begins at the tick its defining event **actually first fired**.

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
| `--biome-res` | 72 | biome-map resolution per axis (downsampled from the sim's 128² grid) |
| `--biome-frames` | 8 | biome-map keyframes across the run (0 disables the map) |
| `--out` | `showcase/replay.js` | `.js` wraps `window.ANABIOS_REPLAY=…`; `.json` writes raw JSON |

Any scenario works. The player derives the six era boundaries from the run's real event
stream — Tools & Fire starts at the first invention, Exodus at the first speciation,
Farming & Trade at the first settlement/market, Writing at the first meme-sweep, War & Kin
at the first raid — falling back to even spacing when a signal never fires. Species names
come from the scenario's archetype specs (dynamically speciated splinters show as `species N`).

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
