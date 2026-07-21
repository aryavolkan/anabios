# anabios screenshot gallery

Captured with the `debug_capture.gd` harness (`ANABIOS_SHOT*`), 1280x800.
All runs are deterministic per scenario seed, so every shot is reproducible
with the same env vars. Camera close-ups use `ANABIOS_CAM_ZOOM/_CAM_X/_CAM_Y`.
The harness freezes the sim while the scene builds, so a capture lands on
exactly `ANABIOS_SHOT_TICKS + ANABIOS_SHOT_FRAMES (+1)` ticks — the Tick
column below is the actual HUD tick in each shot. `ANABIOS_SEED` overrides
the viewer's default seed (12345); geographic-trade is shot on its tuned
scenario seed 424242 (its four-way terrain junction hub is seed-specific),
the other scenarios on the viewer default.

## geographic-trade (border-seeking terrain pull + marketplace trade reach)

Four goods species (Salt/Desert, Obsidian/Rock, Amber/Forest, Spice/Grass)
spawn INTERMIXED in one cluster on a four-way terrain junction that straddles
the torus seam at (948, 4); the `terrain_habitat` pull sorts them onto borders
of their home terrain, where `TRADE_RANGE` 10.0 lets border neighbors
transact. Capture env: `ANABIOS_SEED=424242 ANABIOS_CAM_X=948
ANABIOS_CAM_Y=60 ANABIOS_CAM_ZOOM=2`. The ground and agent layers wrap across
the seam, so the junction reads as one continuous landscape.

| File | Tick | What you're seeing |
|---|---|---|
| geotrade-t041-mixed.png | 41 | Opening state: 962 agents of all four lineages intermixed in one swarm on the junction; the first cross-species `Trade` has already latched (t=2, sp3). |
| geotrade-t461-sorted.png | 461 | Sorting underway: the swarm has spread along the forest band and terrain borders; `DowryBirth: 47` and counting — dowry-gated reproduction running on traded goods. Population growing (1,002 alive). |
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
