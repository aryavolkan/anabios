# anabios screenshot gallery

Recaptured 2026-07-31 from the current Godot viewer via the `debug_capture.gd`
harness (`ANABIOS_SHOT*`), 1280x800. Every run is deterministic per scenario
seed, so each shot reproduces from the env vars below; the population/tally
figures reflect the **current** sim and have drifted from the prose — the images
are the source of truth. Wide overview shots use the `ANABIOS_CAM_FIT` whole-world
([F]) framing; close-ups use `ANABIOS_CAM_ZOOM/_CAM_X/_CAM_Y`. HUD tick =
`ANABIOS_SHOT_TICKS + ANABIOS_SHOT_FRAMES + 1`. `ANABIOS_SEED` overrides the
viewer default 12345; grand-theater/settlement/geographic-trade use the tuned
hub seed 424242, and the E-series use their scenario seeds. Captures run windowed
(not `--headless`): the harness reads the viewport after `frame_post_draw`.

Five heavy scenarios that explode to ~10k agents / hundreds of species —
`e3-population-dynamics`, `trophic-t1661-boom`, `predprey-t2500-evolved`,
`sandbox-large-t1200`, `e6-named-behaviors` — are too slow to re-step
synchronously in the harness (`step_n` over a 6,000-species speciation pass runs
for many minutes), so they keep their prior stills; the other 37 are fresh.

**2026-08-07 mammal-roster refresh:** the ecological/combat shots (arena,
weapons-arms-race, predator-prey, trophic-cascade, divergent, grand-theater
arena, gene-culture-hunt, and the E2 replay/event-camera) were recaptured after
the viewer's quadruped mammal roster landed — grazers/stalkers/herds now render
as Deer/Wolf/Hare/etc. by diet+size+role instead of hominins, and the body-mode
legend reads "each animal in its own coat colours". Culture/DIT scenarios stay
Primate (apes), so their stills are unchanged. `territories-t400-pher` was also
recaptured with the mammal roster: the two marker herds now render as animals,
and the territorial scent reads on the `phero-3` channel (`ANABIOS_GROUND=4`) as
faint red halos — the current sim leaves the old Marker channel (`phero-0`)
empty for this scenario, so the overlay channel moved.

## pixel world at scale (Phases 0–5, 2026-09-13)

The `scale-*` stills come from the pixel-world plan
(`docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md`): streamed
autotiled ground, hand-drawn tiles and flora, 24 px hand-authored creature
figures, village footprints, market squares and the pixel-font HUD. Rendered
on software Vulkan (llvmpipe) under Xvfb at 1280×800 with the same
`debug_capture.gd` env vars as the rest of this gallery; `ANABIOS_CODEX_TAB`
picks the codex page (0 research, 1 species, 2 biomes) and `ANABIOS_INSPECT=1`
pins an agent in the unit card.

| File | Scenario / tick / zoom | What you're seeing |
|---|---|---|
| scale-settlement-t891-village-4x.png | `settlement` t891 4× (default camera, `ANABIOS_INSPECT=1`) | A camp on packed-earth yards with paths to the hearth, tents spaced a hut-width apart, the crowd capped to one figure per 17 units at 4× (12 at 2×, staggered rows), a market square with awning stalls beside it, pixel-puff hearth smoke, the outcrop east of the camp as warm stone, the unit card. |
| scale-settlement-t891-fortified-2x.png | `settlement` t891 2× (`ANABIOS_VILLAGE_ERA=2 ANABIOS_VILLAGE_FLAGS=123`) | The same camp forced into its fortified era: a log palisade hugging the huts, a gate with banners to the south, a watchtower, fenced fields outside the wall, the forest standing back from the clearing. |
| scale-settlement-t891-raided-4x.png | `settlement` t891 4× (`ANABIOS_VILLAGE_ERA=1 ANABIOS_VILLAGE_FLAGS=127`) | The same camp as a raided thatch village: burnt ruins with pixel flames and black smoke over them, the palisade, the fenced field. |
| scale-settlement-t3120-camp-4x.png | `settlement` t3120 4× | The same camp later: hearth smoke, herds at the square, codex species page. |
| scale-settlement-t1020-codex-species-2x.png | `settlement` t1020 2× (`ANABIOS_CODEX_TAB=1`) | The settlement peninsula: flat lake water with block ripples, dithered biome borders, mixed oak silhouettes, olive savanna, resting herds with their heads up, the HUD. |
| scale-settlement-t1591-1x.png | `settlement` t1591 1× | The same world at 1×. |
| scale-settlement-t691-overview.png | `settlement` t691, `ANABIOS_CAM_FIT=1` | Whole-world framing: the overview mip with the density dots. |
| scale-settlement-t891-market-8x.png | `settlement` t891 8× at (1060, 298) | The market square up close: stalls, goods on the counters, carts, the square's earth at the ground tiles' grain. |
| scale-trade-hubs-t600-market-4x.png | `trade-hubs` t600 4× at (1060, 298) | A trade hub as a market square in the woods, a snow patch with an ordered-dither edge on the outcrop behind it. |
| scale-riverlands-t291-relief-2x.png | `riverlands` t291 2× | Rivers with waterfalls on their steep stretches, contour-ledged mountains in warm olive stone with a dithered snow line, forests mixing three oaks, clutter on the grass. |
| scale-riverlands-t291-river-4x.png | `riverlands` t291 4× at (2367, 1017) | A one-cell river running diagonally through the oaks as one stream: the diagonal coast tiles carry a channel instead of pinching the cells into beads. |
| scale-riverlands-t291-clutter-4x.png | `riverlands` t291 4× at (2500, 1200) | Flowers, tufts, mushrooms and stumps at 4×. |
| scale-riverlands-t291-lake-4x.png | `riverlands` t291 4× at (2427, 1357) | A desert lake: one flat blue, sparse strokes, foam and beach from the coast autotile. |
| scale-inventions-t2620-coast-2x.png | `inventions` t2620 2× (`ANABIOS_CODEX_TAB=1`) | Hominin bands along a coast. |
| scale-inventions-t2620-hominins-4x.png | `inventions` t2620 4× (`ANABIOS_INSPECT=1`, `ANABIOS_CODEX_TAB=0`) | Hand-authored hominins in the field (one wading offshore, cut at the waterline), the research list with an icon per invention, the unit card; the species, adaptation and tech tables stay hidden until `[P]`. |
| scale-inventions-t2620-unit-card-1x.png | `inventions` t2620 1× (`ANABIOS_INSPECT=1`) | The same at 1×. |
| scale-huge-steppe-t191-2x.png | `huge-steppe` t191 2× | The 8192-unit Huge tier streaming its ground chunks. |
| scale-minimal-t111-shoreline-8x.png | `minimal` t111 8× | Shoreline autotile at 8×. |
| scale-hero-pose-sheet.png | `hero_sheet.gd` tool | The 16-pose sheet of the ten quadruped masters. |
| scale-hominin-pose-sheet.png | `hominin_sheet.gd` tool | The 22-pose sheet of the five hominin masters, weapons included. |

## grand theater (all subsystems at once)

Every opt-in flag on in one world (`grand-theater`, seed 424242): seasonal +
drifting climate, living biome, disasters, the four-goods material economy (goods fund invention learning),
settlements, cognition + invention tree with gene↔tech coupling, war, and
the full archetype cast. Terrain palette shows each biome in its own hue
family (grass/forest greens, sandy desert, blue water, gray rock); biomass
brightens within the family, succession scars and pollution smudge show
through on the default ground view.

| File | Tick | What you're seeing |
|---|---|---|
| grand-theater-t061-full.png | 61 | Whole 1024-world at spawn: the trade capital on the north-seam four-goods junction (top edge, center-right), the predator arena at map center, frontier colonies scattered. 1,414 alive, 31 species. |
| grand-theater-t091-capital.png | 91 | Zoom 3.0 on the capital (948,4): the four goods species boiling around the junction, innovator commune on the west flank. 6,712 trades already. |
| grand-theater-t091-arena.png | 91 | Zoom 3.0 on map center: herds and packs working the grass/desert/lake mosaic. |
| grand-theater-t1531-evolved.png | 1531 | The burn-down: 476 alive, sp5 at tech era 1 (`stone_tools`), tally reads Settlement 17 · Market 17 · War 4 · KinNetwork 12 · Dialect 3 · Maladaptation 30 — every detector class has fired. |

## settlements & economy (E8)

Home-range anchoring (agents learn and inherit a home point), a decaying
market-density field fed by every trade swap, and harvest experience. The
`markets` ground overlay ([G] cycle, gated on the trade economy) renders
density as amber heat. `ANABIOS_SEED=424242` (the geographic-trade hub seed),
`ANABIOS_CAM_X/Y` on the four-way junction.

| File | Tick | What you're seeing |
|---|---|---|
| e8-market.png | 931 | `settlement` seed 424242: the amber market node crystallized at the four-way terrain hub, trade-route streaks (cyan) crossing straight through it, the four goods species ringed around their shared marketplace. Tally reads `Market: 36 Specialists: 2`; the HUD counts 113,486 trades. |

## war & alliance (E7)

Cross-faction combat hits and deaths feed a decaying hostility record per
lineage-faction pair; score ≥ 12 declares WarOrRaid, 200 quiet ticks ends
it (WarEnded). Alliance (shared meme + zero cross-kills + sustained
sharing) and KinNetworkStable (1500-tick cohesive kin cluster) round out
the chapter. `ANABIOS_SEED=0` (scenario default).

| File | Tick | What you're seeing |
|---|---|---|
| e7-war.png | 51 | `weapons-arms-race` seed 0: the event list shows the full conflict hierarchy in one frame — `t=17 Predation`, `t=32 CombatRaid sp=4`, and directly beneath it `t=32 War sp=2` in blood red, the stalker pack's opening campaign. `War: 1` in the tally. |

## named behaviors (E6)

Fire-time behavioral context on every combat hit (was the attacker lying in
wait? was the damage invention-boosted?) feeds the named-behavior detectors.
Two chapters discovered in real runs so far (Flight, Signaling); Ambush and
ToolUse remain honestly undiscovered codex entries — pursuit starters never
sit in wait, and the Metalworking timeline misses the hunting window by a
few hundred ticks (see the E6 plan notes).

| File | Tick | What you're seeing |
|---|---|---|
| e6-named-behaviors.png | 1531 | `gene-culture-alarm` seed 0 mid-bloom (4,360 alive, 513 species): the tally's bottom line carries the new chapters — `Flight: 1 Signaling: 1` — alongside `AlarmCall: 1 TraitFixed: 1 Corridor: 1 Segregation: 2` from earlier milestones. |

## trait evolution (E5)

Genome-moment history (mean/variance per slot per species, 10-tick cadence)
feeds three detectors — TraitFixation, RapidAdaptation, ConvergentEvolution
(LCA-disciplined: sister splinters don't count) — and the [T] evolution
panel: trait-drift lines for the dominant species plus the living phylogeny.
`ANABIOS_SEED=50505` (the convergent scenario seed), `ANABIOS_EVO=1`.

| File | Tick | What you're seeing |
|---|---|---|
| e5-evolution-panel.png | 6061 | `convergent` seed 50505: the evolution panel mid-run — four trait-drift lines (size/metabolism/perception/openness) for the dominant sp1568, and the indented phylogeny below it (sp1568 → sp1 → sp1669 …). The tally carries the new chapters: `TraitFixed: 1 RapidAdapt: 1`. |

## disturbance & succession (E4)

Fire/drought/freeze disasters scar the biome into succession states (bare →
pioneer → climax); the `succession` ground overlay ([G] cycle) shows Climax
as dim green, Pioneer as bright new growth, Bare as scorched umber, and
active disasters tinted (fire orange / drought sepia / freeze pale).
`ANABIOS_SEED=40723` (the scenario seed).

| File | Tick | What you're seeing |
|---|---|---|
| e4-fire-ring.png | 2321 | `disturbance` seed 40723: the t=2231 fire mid-expansion — the orange burn ring consuming the grassland while a migration stream of agents threads straight through it. The tally already carries the new chapters (`RangeExpand: 4 Segregation: 2 Corridor: 5`). |
| e4-succession.png | 2631 | The same scar ~300 ticks later, re-vegetated: bright pioneer growth fills the burn with a thin umber seam still healing down the middle. `Succession: 1` in the tally — the scar's recovery event (t=2460) fired when half its cells were vegetated again. |

## population dynamics (E3)

Four new detectors (PopCycle / BoomBust / CarryingCap / TrophicCascade) read
400-tick guild histories — per-species lines churn too fast under 200-tick
reclustering, so the oscillators tracked are the herbivore guild, carnivore
guild, and world total. `ANABIOS_SEED=0` (the scenario seed; the viewer
default 12345 diverges into a different, dying trajectory).

| File | Tick | What you're seeing |
|---|---|---|
| e3-population-dynamics.png | 1821 | `predator-prey` seed 0 mid-maelstrom: 9,992 alive and 6,785 species, with the codex tally carrying all four new chapters — `PopCycle: 1 BoomBust: 1 CarryingCap: 3 TrophicCascade: 1`. The cascade (t=1690) is the real thing: stalker guild collapse → grazer release (555 → 9,989) → plant field grazed from 109k down to 13k. |
| trophic-t157-hunt.png | 157 | `trophic-cascade` seed 20260722, the *top* of the cascade (`ZOOM=3 X=512 Y=512 TICKS=150 FRAMES=6`): the whole cast in one cluster — 65 grazers (`sp 1`) and the 10-stalker pack (`sp 2`) intermixed on the central lake. The tally already carries the opening predator chain, `Predation: 1 CombatRaid: 1 PackHunting: 1`, and the log reads it back in order: `t=8 Predation`, `t=26 PackHunting sp=2`, `t=29 CombatRaid sp=2`. |
| trophic-t1661-boom.png | 1661 | The same seed ~1,500 ticks later, the cascade's payoff: the stalker guild has collapsed and the released grazers have exploded to 9,989 alive / 6,958 species, carpeting the whole biome (default full-map view). The tally now carries the full E3 chapter set — `TrophicCascade: 1 BoomBust: 2 PopCycle: 5 CarryingCap: 6` — with the event log streaming `PopCrash` as the boom overshoots carrying capacity. The founder grazer lineage `sp 1` is still present (n=279). |

## replay & event camera (E2)

The [R]/[U]/[V] modes ride a GDScript snapshot ring (250-tick cadence, 16
entries). `ANABIOS_EVENT_CAM=1` starts the event-camera tour after the tick
jump; `ANABIOS_REPLAY=1` replays the latest event (the harness forces a ring
capture at the jump tick first — Main steps before ReplayManager in tree
order, so the first organic capture would land one tick late).

| File | Tick | What you're seeing |
|---|---|---|
| e2-event-camera.png | 271 | `predator-prey`: the event camera mid-tour, parked on the t=113 `Predation` site (banner top-center, "[V]/Esc exit") with the camera eased in to zoom 2.0; the codex panel below shows the event log it cycles through. |
| e2-replay-t080.png | 80 | `weapons-arms-race` seed 3: replay of the t=79 `Territory sp=2` event — rewound to the snapshot at tick 79, fast-forwarded exactly one tick (note the HUD: tick 80, paused), camera on the territory centroid with the pulsing gold highlight ring. The codex panel re-accumulated from the rewind and shows the event re-firing (`Territory: 1`) — replay determinism made visible. |

## geographic-trade (border-seeking terrain pull + marketplace trade reach)

Four goods species (Salt/Desert, Obsidian/Rock, Amber/Forest, Spice/Grass)
spawn INTERMIXED in one cluster on a four-way terrain junction that straddles
the torus seam at (948, 4); the `terrain_habitat` pull sorts them onto borders
of their home terrain, where `TRADE_RANGE` 10.0 lets border neighbors
transact. Successful swaps render as trade routes: thin links tinted by the
initiating trader's hue, held on a 24-tick fading trail so recurring trades
along species borders accumulate into visible lanes (thinner and dimmer than
the 8-tick combat streaks). The HUD tallies the run's cumulative swaps
(`· N trades`). Capture env: `ANABIOS_SEED=424242
ANABIOS_CAM_X=948 ANABIOS_CAM_Y=60 ANABIOS_CAM_ZOOM=2`. The ground and agent
layers wrap across the seam, so the junction reads as one continuous
landscape.

| File | Tick | What you're seeing |
|---|---|---|
| geotrade-t041-mixed.png | 41 | Opening state: 962 agents of all four lineages intermixed in one swarm on the junction; the first cross-species `Trade` has already latched (t=2, sp3) and routes thread the swarm core. |
| geotrade-t461-sorted.png | 461 | Sorting underway: the swarm has spread along the forest band and terrain borders; `DowryBirth: 47` and counting — dowry-gated reproduction running on traded goods. Population growing (1,002 alive). |
| geotrade-t461-routes.png | 461 | Close-up (zoom 4) of the same tick: trade-route lanes lighting the species borders — each link is one cross-species swap, tinted by the initiating trader. |
| geotrade-t868-economy.png | 868 | Mature border economy: `DowryBirth: 72`, `NichePartition: 10`; energy declining (E=25-42) as the ~900 agents press the junction's carrying capacity. |

## weapons-arena (new scenario: stalkers + pack hunters + fast hunters vs herds)
| File | Tick | What you're seeing |
|---|---|---|
| arena-t080-ambush.png | 145 | Opening ambush: stalkers seeded inside the grazer range. First `Predation` and `PackHunting` already in the log. |
| arena-t300-melee.png | 365 | Five-species melee around the central lake: grazers (sp1), herd prey (sp2), stalkers (sp3), pack hunters (sp4), fast hunters (sp5). |
| arena-t620-raid.png | 661 | Aftermath of a `CombatRaid` (sp3, t=475); two fresh `Speciation` events at t=600. 8 species now live. |
| arena-t3000-evolved.png | 3091 | The arena at carrying capacity: 59 species, 14 extinctions, 68 speciations, raids and pack hunts in the record. |
| arena-t300-inspector.png | 361 | Inspector pinned on agent id 24 (species 1): genome, modules, and learning stats. |

## weapons-arms-race (three weapon systems: contact Weapon vs ranged Spines vs heavy Jaws)

| File | Tick | What you're seeing |
|---|---|---|
| armsrace-t031-volley.png | 31 | The kiting spiner pack (sp4) mid-volley after its `PackHunting` (t=19) and `CombatRaid` (t=27): species-tinted cyan tracers streak attacker→target from the standoff ring, yellow impact flashes on the struck herd prey. |
| armsrace-t031-inspector.png | 31 | Same volley with the inspector pinned on spiner id 101 (species 4): diet 1.00 carnivore, `Spines` in its five-module body plan — the module firing the tracers. |
| armsrace-t160-standoff.png | 161 | Five species in play: grazers, herd prey, stalkers (sp3), spiners (sp4), bruisers (sp5). Standoff along the northeast flank. |
| armsrace-t400-brawl.png | 461 | The contested northeast border: bruiser clusters (magenta Jaws glyphs) pressing into the herd range. |
| armsrace-t3000-evolved.png | 3091 | End state: 162 species, 171 speciations, a dense migratory swarm sweeping the eastern half of the world. |
| armsrace-t160-inspector.png | 201 | Inspector pinned on a bruiser (id 117, species 5): `Jaws` + `Armor` in its six-module body plan. |

### combat-streak close-ups (feature: attacker→target tracers for ranged fire)

Camera zoomed onto the action so the [combat streaks](../game/scripts/main.gd) read
clearly — the full-world shots above show them only as faint slivers. Ranged fire
in this scenario is concentrated in the opening spiner skirmish (sp4's
`PackHunting` at t=19 and `CombatRaid` at t=27); later fights are contact-weapon
only, so both close-ups sit early.

| File | Tick | Capture env | What you're seeing |
|---|---|---|---|
| armsrace-t027-volley-closeup.png | 27 | `ZOOM=5.0 X=690 Y=335 TICKS=21 FRAMES=5` | The opening spiner volley up close: three thin cyan tracers stretch from the kiting spiner pack (sp4) into the herd prey below, one ending on a yellow just-hit flash — the ranged Spines kill *before* contact weapons can close. This is exactly the behavior that was invisible in the viewer before the streak layer landed. |
| armsrace-t029-raid-closeup.png | 29 | `ZOOM=4.5 X=695 Y=340 TICKS=27 FRAMES=1` | Moments after the `CombatRaid sp=4` (t=27, top of the log): two cyan tracers end on yellow hit flashes as the raiders finish their volley. The streaks tint to the attacker's species hue, which is what keeps ranged fire legible once lineages mix. |

(`ZOOM`/`X`/`Y` are `ANABIOS_CAM_*`; `TICKS`/`FRAMES` are `ANABIOS_SHOT_*`.)

Reproduce from `game/` — needs the real renderer, `--headless` hangs at
`frame_post_draw` under the dummy driver:

```
ANABIOS_SHOT=out.png ANABIOS_SCENARIO="res://../scenarios/weapons-arms-race.toml" \
  ANABIOS_CAM_ZOOM=4.5 ANABIOS_CAM_X=695 ANABIOS_CAM_Y=340 \
  ANABIOS_SHOT_TICKS=27 ANABIOS_SHOT_FRAMES=1 \
  godot --path . res://scenes/main.tscn
```

The HUD tick lands a few ticks past `ANABIOS_SHOT_TICKS` because the sim keeps
running at 1x during the warm-up/wait frames, and streaks live only
`STREAK_TTL` (8) ticks — keep `FRAMES` small when hunting tracers.

## gene↔tech coupling (TG)

Dual inheritance made visible: every invention carries a gene affinity (its
buff scales with a coupled genome slot) plus a hard `GeneReq` gate and an
era-scaled material basket, so culture both selects and waits on the genome.
The `[X]` helix panel draws the genome (left strand) and memome (right
strand) with rungs for every coupling; the `[Y]` chart's "gene↔tech
selection" small-multiple plots the holder−nonholder differential over time.

| File | Tick | What you're seeing |
|---|---|---|
| coupling-t4001-helix.png | 4031 | `tech-gene-coupling` with the [X] dual-inheritance helix: fire adopted (1.43 mean level) and its Openness rung lit red — holders currently carry *less* Openness than non-holders (the traditionalist copy wave); stone_tools/farming/metalworking spreading behind it. |
| coupling-t4001-coevo-selection.png | 4031 | Same run with the [Y] chart: all 10 inventions in the era-split adoption charts, and the ±1 "gene↔tech selection" panel catching Δfarming's green positive bump mid-sweep (zero line = no differential; flat zeros pre-adoption, not spurious negatives). |

Reproduce: `ANABIOS_HELIX=1` / `ANABIOS_COEVO=1` with
`ANABIOS_SCENARIO="res://../scenarios/tech-gene-coupling.toml"`,
`ANABIOS_SHOT_TICKS=4000 ANABIOS_SHOT_FRAMES=30`.

## classic scenarios

| File | Tick | What you're seeing |
|---|---|---|
| predprey-t150-hunt.png | 215 | `predator-prey`: 8 stalkers working the 68-strong grazer herd; first `Predation` at t=14. |
| predprey-t2500-evolved.png | 2591 | The aftermath: stalkers (sp2) went extinct at t=2058 after 199 population crashes; 18 grazers inherit the world. |
| gchunt-t400-dialect.png | 461 | `gene-culture-hunt`, dialect coloring: fast and slow weapon hunters with `PackHunting` x2 and a double `MemeSweep` at t=79. |
| gchunt-t1200-evolved.png | 1261 | Boom-bust endgame: both hunter lineages crashing (`PopCrash` storm) while grazers persist. |
| inventions-t6000.png | 6091 | Tech race won: 35 species, and the TECH panel shows multiple lineages at era 4 running stone_tools + fire + farming. |
| divergent-t150-swarm.png | 166 | `divergent`'s population explosion underway: 1,926 alive and climbing toward the 10k cap, swarm visible bottom-right. |
| territories-t400-pher.png | 431 | Pheromone-channel view (Marker channel): two 30-agent species' scent-marked territory clouds; `Territory` events at t=59. |
| sandbox-large-t1200.png | 1201 | The 2048-world mega-sandbox at its 6k cap: 1,653 (!) species logged, herds streaming across the map. |
| dialects-t800.png | 861 | Two isolated populations in dialect coloring after four `MemeSweep` events — same species, different cultures. |
| coevo-t3000-chart.png | 3001 | `cognitive-coevolution` with the [Y] co-evolution chart: gene-culture, dialect divergence, invention adoption, and cognition curves over 3k ticks. |

## mood overlay (grazers & wolves)

The `grazers-and-wolves` scenario exists to show off the new mood body-color
mode: every agent is tinted by its current winner-take-all drive. Affect +
basic needs + cognition are all on, so most of the palette is alive at once —
the herd runs green (`seek food`), blue (`seek water`), purple (`sleep`) and
pink/magenta (`seek mate`/`mate`), and flares yellow (`flee`) when a wolf pack
closes. Red (`fight`) is the one color this scenario does *not* deliver: over
2,000 ticks at the capture seed the wolves never enter it once (the herd totals
7 agent-ticks of it), because `mood::compute_mood` puts FLEE ahead of FIGHT and
the pack's FEAR keeps winning — the wolves themselves spend ~14% of their time
yellow. Tuning the pack toward RAGE is open work; these stills are honest about
what the current scenario shows.

| File | Tick | What you're seeing |
|---|---|---|
| wolves-t000-grazing.png | 181 | `grazers-and-wolves` at the viewer's default seed 12345 — the viewer overrides the scenario's own `seed = 0`, see the repro note below — in mood mode, full-world overview: the herd has already grown from its 80 founders to the 1,000-agent cap and covers the central grass patch, ~51% of it green (`seek food`) and ~30% pink/magenta (`seek mate`/`mate`), while the 14-wolf pack (grey `content`) holds north of it. Tally reads `AlarmCall: 1` (t=158) and `HerdCohesion: 1`; the pack's first `Predation` lands at t=191, just after this frame. |
| wolves-t150-hunt.png | 331 | The same run 150 ticks later, mid-hunt: `MassFright: 198` and `PanicCascade: 2` in the tally, blue (`seek water`) now mixed through the herd (~6%), and the yellow (`flee`) agents are mostly the wolves themselves — 10 of the 15 — plus a dozen scattered grazers. Founder lineages are both still present (sp1 grazer n=982, sp2 wolf n=15). |

Reproduce from `game/` (windowed, `--headless` hangs on `frame_post_draw`).
No `ANABIOS_SEED` here on purpose: the viewer always loads a scenario through
`load_scenario_with_seed(text, GameConfig.seed)`, so the TOML's `seed = 0` is
replaced by the viewer default 12345 — which is the seed both stills were
captured on.

```
ANABIOS_BODY=5 ANABIOS_CAM_FIT=1 \
  ANABIOS_SCENARIO="res://../scenarios/grazers-and-wolves.toml" \
  ANABIOS_SHOT=wolves-t000-grazing.png \
  godot --path . res://scenes/main.tscn

ANABIOS_BODY=5 ANABIOS_CAM_FIT=1 \
  ANABIOS_SCENARIO="res://../scenarios/grazers-and-wolves.toml" \
  ANABIOS_SHOT_TICKS=150 ANABIOS_SHOT=wolves-t150-hunt.png \
  godot --path . res://scenes/main.tscn
```
