# anabios screenshot gallery

Captured with the `debug_capture.gd` harness (`ANABIOS_SHOT*`), 1280x800.
All runs are deterministic per scenario seed, so every shot is reproducible
with the same env vars. Camera close-ups use `ANABIOS_CAM_ZOOM/_CAM_X/_CAM_Y`.
The harness freezes the sim while the scene builds, so a capture lands on
exactly `ANABIOS_SHOT_TICKS + ANABIOS_SHOT_FRAMES (+1)` ticks — the Tick
column below is the actual HUD tick in each shot. `ANABIOS_SEED` overrides
the viewer's default seed (12345); geographic-trade is shot on its tuned
scenario seed 424242 (its four-way terrain junction hub is seed-specific),
the other scenarios on the viewer default. Captures run windowed (not
`--headless`): the harness reads the viewport texture after
`frame_post_draw`, which never completes on the dummy renderer.

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
