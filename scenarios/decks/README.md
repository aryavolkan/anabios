# scenarios/decks — the showcase garden

Deck-dedicated scenarios: TOMLs that exist to back **showcase assets** (the web
replay player in `showcase/` and the Godot cinematic decks in `game/showcase/`),
distinct from the science-pinned core set in `scenarios/`. A core scenario is
pinned to a *phenomenon claim* (its integration test, its corpus evidence); a
garden scenario is pinned to a *recording* — it may be tuned for cinematic
quality without destabilizing a science claim, and it must never be retuned
without regenerating the asset it backs.

## The pin convention

Every curated deck JSON in `game/showcase/` declares its pin:

- `"seed": N` — the seed the recording was made at, and
- `"_comment"` containing `scenario=<name>` — the backing TOML.

The pin contract is enforced by `crates/anabios-core/tests/deck_scenarios.rs`:
each curated deck's scenario must resolve (searched in `scenarios/decks/` first,
then the core `scenarios/` set), instantiate at the pinned seed, and survive a
200-tick smoke run. Decks named `smoke-*.json` are exempt — they are
scenario-agnostic timelines used for pipeline smoke checks.

## Current pin registry

The four curated decks currently back onto **core** scenarios (they were
science-pinned first), so the garden holds no dedicated TOMLs yet. When a
future deck needs tuning that a science-pinned scenario can't absorb, its
scenario lands here — same name as the deck.

| deck (`game/showcase/`) | scenario · seed | asset |
|---|---|---|
| `out-of-africa-saga.json` | `out-of-africa-saga.toml` · 318 | `showcase/replay.js` (hosted web replay) + `runs/showcase/out-of-africa-saga.mp4` |
| `predator-prey.json` | `predator-prey.toml` · 0 | `runs/showcase/predator-prey.mp4` |
| `dialects.json` | `dialects.toml` · 0 | `runs/showcase/dialects.mp4` |
| `inventions.json` | `inventions.toml` · 0 | `runs/showcase/inventions.mp4` |

## Regenerating the assets

```sh
scripts/emergence.sh showcase                 # web replay + all four deck MP4s
scripts/emergence.sh record <deck>            # one deck MP4 (deck-pinned seed)
scripts/emergence.sh record-web out-of-africa-saga --seed 318   # web replay.js
```

Re-run after any change to a pinned scenario, a deck, or the sim. The hosted
web replay is regenerated again by the Pages workflow at publish time — see
`showcase/README.md` for the full asset pipeline.
