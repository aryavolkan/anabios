# anabios atlas gallery (three.js frontend)

The [`../README.md`](../README.md) gallery, re-shot in the **atlas** — the
three.js frontend in [`web/`](../../web/README.md) driving `anabios-core`
compiled to WebAssembly. Every still here is the *same world* as its Godot
counterpart: same scenario, seed and HUD tick, framed by the same camera recipe
(`ANABIOS_CAM_FIT` → `cam=fit`, `ANABIOS_CAM_ZOOM/X/Y` → `cam=x,y,zoom`, the
viewer's pixels-per-world-unit at 1280 px), pinned on the same agent id where
the original pinned one. All 62 rendered headless (Chromium, software GL via
swiftshader) at 1280×800 on 2026-09-25 by:

```sh
scripts/web.sh build && scripts/web.sh serve &
node web/scripts/capture.mjs gallery/atlas/shots.json      # every recipe below
```

[`shots.json`](shots.json) is the recipe list; each entry reproduces its PNG
from `scenario · seed · tick · cam · inspect · color` alone, because the wasm
build is bit-identical to the native core (`scripts/web.sh test`). The
population/species figures in the tables are what the *current* sim produces
at those ticks (e.g. `predator-prey` now runs under a 2,000-agent cap), so they
differ from the older prose in the Godot gallery exactly as its header warns.

**What the atlas draws instead of the pixel viewer.** Terrain relief from the
elevation field under a painterly ground shader (noise-softened cell borders,
rock on steep slopes, snow on the peaks, a beach band and darkened seabed at
the shore), instanced forests and rock scatter planted from the terrain grid,
depth-shaded water with sun glints, river sparkle, seabed caustics and a foam
fringe, sun shadows, a soft bloom over the hot pixels; agents as instanced
grazer or hunter figures on articulated legs, coloured by genome (species mode)
or by diet / dialect / energy / mood / arousal / infection; combat volleys and
trade lanes as fading light; hut villages at settlement sites and stalls at
trade hubs; codex events as expanding rings and light pillars plus the feed at
the bottom right; the species table in the rail; a click-to-inspect agent
card. Stills are shot at noon: the day cycle is off under `capture=1`, and the
short-lived motion (gait, wind, sparks, smoke, pillars) is mostly between
frames at software-GL frame rates.

**What has no counterpart yet** (the still is captured anyway, framed on the
same world at the same tick, and the gap is noted in its row): the codex tabs,
the evolution / co-evolution / helix charts, pheromone ground overlays, the
replay *mode* (the atlas has an event tour, **V**, but no rewind-to-snapshot),
village era/flag overrides, hand-drawn tiles, flora and 24 px figures, and the
two pose sheets. The ground does not wrap across the torus seam, so recipes
that sit on the seam (`x=948,y=4`, `x=1060`) look at the plate edge; the
settlement and market stills therefore use `cam=site,<zoom>` (largest village)
and `cam=hub,<zoom>` (a market) to frame what the original framed.

## pixel world at scale (`scale-*`)

| File | Scenario / tick / camera | What you're seeing |
|---|---|---|
| scale-settlement-t891-village-4x.png | `settlement` 424242 t891, `cam=site,4`, `inspect=sp1` | The largest settlement as a hut ring on packed ground, its species' figures around it, the inspector pinned on a founder-lineage forager (energy, age, diet, mood, body plan). |
| scale-settlement-t3120-camp-4x.png | `settlement` 424242 t3120, `cam=site,4` | The same camp later: 2,197 alive across 700 species, villages having grown and faded with the codex settlement latch. |
| scale-settlement-t1020-codex-species-2x.png | `settlement` 424242 t1020, `cam=site,2` | The settlement peninsula at 2×: water at sea level, terrain relief, the species table standing in for the codex species page. |
| scale-settlement-t1591-1x.png | `settlement` 424242 t1591, `cam=512,512,1` | The whole world at 1×: market stalls at the trade hubs and villages threading the biome. |
| scale-settlement-t691-overview.png | `settlement` 424242 t691, `cam=fit` | Whole-world framing (the [F] view). |
| scale-settlement-t891-market-8x.png | `settlement` 424242 t891, `cam=hub,8` | A market stall up close at 8×, traders around it. |
| scale-trade-hubs-t600-roads-2x.png | `trade-hubs` 424242 t600, `cam=hub,2` | A trade hub with the lanes of recent swaps fading around it (no worn dirt roads in the atlas — lanes are per-trade light). |
| scale-trade-hubs-t600-market-4x.png | `trade-hubs` 424242 t600, `cam=hub,4` | The same hub at 4×: the stall with its pennant, caravan lanes. |
| scale-riverlands-t291-relief-2x.png | `riverlands` 12345 t291, `cam=2048,2048,2` | The 4096² world with its 512² biome grid: forested banks around a lake, rivers blue-tinted along the carved channels, mountains in real relief with rock and snow from the ground shader. |
| scale-riverlands-t291-river-4x.png | `riverlands` 12345 t291, `cam=2367,1017,4` | A river running through the forest cells at 4×. |
| scale-riverlands-t291-clutter-4x.png | `riverlands` 12345 t291, `cam=2500,1200,4` | Grassland at 4×: painterly ground grain and the scattered trees of the grass planting (no flowers, tufts or stumps). |
| scale-riverlands-t291-lake-4x.png | `riverlands` 12345 t291, `cam=2427,1357,4` | A desert lake: the translucent water plane over the sunken cells, the beach as the sand cells around it. |
| scale-inventions-t2620-coast-2x.png | `inventions` 12345 t2620, `cam=512,512,2` | Hominin bands at era 3: 400 alive, 73 species, the tech era in the readout. |
| scale-inventions-t2620-hominins-4x.png | `inventions` 12345 t2620, `cam=512,512,4`, `inspect=sp1` | The bands at 4× with the inspector pinned on an innovator: its held inventions as chips (the research list's counterpart). |
| scale-inventions-t2620-unit-card-1x.png | `inventions` 12345 t2620, `cam=512,512,1`, `inspect=sp1` | The same at 1×. |
| scale-huge-steppe-t191-2x.png | `huge-steppe` 12345 t191, `cam=4096,4096,2` | The 8192² Huge tier: a 1024² biome grid as one relief mesh, 1,531 alive. |
| scale-minimal-t111-shoreline-8x.png | `minimal` 12345 t111, `cam=512,512,8` | The shoreline at 8×: the beach band, the foam fringe and the turquoise shallows of the water shader meeting the ground (no autotile). |

Not reproduced: `scale-settlement-t891-fortified-2x` / `-raided-4x` (viewer-side
era/flag overrides), `scale-hero-pose-sheet` / `scale-hominin-pose-sheet`
(sprite tools).

## grand theater (all subsystems at once)

| File | Tick | What you're seeing |
|---|---|---|
| grand-theater-t061-full.png | 61 | Whole 1024-world at spawn, seed 424242: 2,982 alive, 31 species; the market stall on the north-seam junction (top edge), the arena cast at map centre. |
| grand-theater-t091-capital.png | 91 | `cam=948,4,3` — the capital on the seam: the goods species boiling around the junction's stall, the plate edge visible where the Godot ground would wrap. |
| grand-theater-t091-arena.png | 91 | `cam=512,512,3` — herds and packs working the grass/desert/lake mosaic at map centre. |
| grand-theater-t1531-evolved.png | 1531 | The burn-down: 2,998 alive and 1,120 species logged; villages, stalls and event rings across the map. |

## settlements & economy (E8)

| File | Tick | What you're seeing |
|---|---|---|
| e8-market.png | 931 | `settlement` 424242, `cam=hub,2`: a market stall ringed by traders, trade lanes tinted by the initiating trader (the `markets` overlay's amber heat has no atlas equivalent). |

## war & alliance (E7)

| File | Tick | What you're seeing |
|---|---|---|
| e7-war.png | 51 | `weapons-arms-race` seed 0: the codex feed carries the opening campaign (predation, combat raid, war in ember red); rings mark the sites. |

## named behaviors (E6)

| File | Tick | What you're seeing |
|---|---|---|
| e6-named-behaviors.png | 1531 | `gene-culture-alarm` seed 0 mid-bloom: 2,000 alive (the cap), 513 species; the feed shows the latest detector firings. The tally line has no atlas counterpart. |

## trait evolution (E5)

| File | Tick | What you're seeing |
|---|---|---|
| e5-evolution-panel.png | 6061 | `convergent` seed 50505: the world at the panel's tick — 1,994 alive, 941 species in the rail's table. No trait-drift chart in the atlas. |

## disturbance & succession (E4)

| File | Tick | What you're seeing |
|---|---|---|
| e4-fire-ring.png | 2321 | `disturbance` seed 40723: the t=2231 fire scar as bare umber cells in the ground texture (succession colours come straight from `cell_color`). |
| e4-succession.png | 2631 | The same scar ~300 ticks later, re-vegetated: bright pioneer green filling the burn. |

## population dynamics (E3)

| File | Tick | What you're seeing |
|---|---|---|
| e3-population-dynamics.png | 1821 | `predator-prey` seed 0: 1,999 alive at the current 2,000 cap, 720 species, the grazer carpet over the biome. |
| trophic-t157-hunt.png | 157 | `trophic-cascade` seed 20260722, `cam=512,512,3`: the whole cast in one cluster on the central lake — grazers and the stalker pack intermixed. |
| trophic-t1661-boom.png | 1661 | The cascade's payoff: 2,000 alive (cap), 277 species carpeting the biome. |

## replay & event camera (E2)

| File | Tick | What you're seeing |
|---|---|---|
| e2-event-camera.png | 271 | `predator-prey` seed 0, `cam=event`: the atlas parked on the latest codex event's site (the herd's centre) — the event camera's parking spot without the tour. |
| e2-replay-t080.png | 80 | `weapons-arms-race` seed 3, `cam=event`: the world at the replay's tick, framed on the last event location. No rewind/replay mode in the atlas. |

## geographic-trade

| File | Tick | What you're seeing |
|---|---|---|
| geotrade-t041-mixed.png | 41 | `cam=948,60,2`: 2,189 agents of the four goods lineages intermixed on the junction, the first trade lanes threading the swarm. |
| geotrade-t461-sorted.png | 461 | Sorting underway along the terrain borders; 1,082 species logged as the swarm splinters. |
| geotrade-t461-routes.png | 461 | `cam=948,60,4`: trade lanes lighting the species borders, each tinted by its initiating trader. |
| geotrade-t868-economy.png | 868 | The mature border economy at 2,198 agents. |

## weapons-arena

| File | Tick | What you're seeing |
|---|---|---|
| arena-t080-ambush.png | 145 | Opening ambush: stalkers seeded inside the grazer range, first predation in the feed. |
| arena-t300-melee.png | 365 | Five-species melee around the central lake. |
| arena-t620-raid.png | 661 | After the first raids and speciations: 500 alive, 63 species. |
| arena-t3000-evolved.png | 3091 | The arena at carrying capacity. |
| arena-t300-inspector.png | 361 | The inspector pinned on agent id 24: energy, age, diet, body plan, learning flags. |

## weapons-arms-race

| File | Tick | What you're seeing |
|---|---|---|
| armsrace-t031-volley.png | 31 | The spiner pack's opening volley from the [F] framing. |
| armsrace-t031-inspector.png | 31 | The inspector pinned on spiner id 101: `Spines` in its body plan. |
| armsrace-t160-standoff.png | 161 | Five species in play, standoff along the northeast flank. |
| armsrace-t400-brawl.png | 461 | The contested northeast border: 499 alive, 53 species. |
| armsrace-t3000-evolved.png | 3091 | End state: 500 alive, 70 species. |
| armsrace-t160-inspector.png | 201 | The inspector pinned on bruiser id 117: `Jaws` + `Armor`. |
| armsrace-t027-volley-closeup.png | 27 | `cam=690,335,5`: the opening volley up close — attacker→target streaks tinted by the attacker's hue, fading over 14 ticks. |
| armsrace-t029-raid-closeup.png | 29 | `cam=695,340,4.5`: moments after the `CombatRaid`. |

## gene↔tech coupling

| File | Tick | What you're seeing |
|---|---|---|
| coupling-t4001-helix.png | 4031 | `tech-gene-coupling`: 400 alive, 84 species, tech era 2 in the readout. The helix and selection charts have no atlas counterpart (one still stands for both originals). |

## classic scenarios

| File | Tick | What you're seeing |
|---|---|---|
| predprey-t150-hunt.png | 215 | `predator-prey` 12345: 8 stalkers working the grazer herd. |
| predprey-t2500-evolved.png | 2591 | The aftermath: 26 grazers, one species, inherit the world. |
| gchunt-t400-dialect.png | 461 | `gene-culture-hunt` in dialect colouring. |
| gchunt-t1200-evolved.png | 1261 | Boom-bust endgame: 36 alive, 2 species. |
| inventions-t6000.png | 6091 | Tech race won: 70 species, era 4 in the readout and the rail. |
| divergent-t150-swarm.png | 166 | `divergent`'s population explosion underway. |
| territories-t400-pher.png | 431 | `territories` in species colouring (no pheromone overlay in the atlas). |
| sandbox-large-t1200.png | 1201 | The 2048-world mega-sandbox, herds streaming across the map. |
| dialects-t800.png | 861 | Two isolated populations in dialect colouring. |
| coevo-t3000-chart.png | 3001 | `cognitive-coevolution` at the chart's tick (no co-evolution chart in the atlas). |

## mood overlay (grazers & wolves)

| File | Tick | What you're seeing |
|---|---|---|
| wolves-t000-grazing.png | 181 | `grazers-and-wolves` 12345 in mood colouring (`color=mood`): the herd green (seek food) and pink (seek mate), the wolf pack grey (content) to the north. |
| wolves-t150-hunt.png | 331 | The same run mid-hunt: yellow (flee) flaring through the herd as the pack closes. |
