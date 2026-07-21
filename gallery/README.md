# anabios screenshot gallery

Captured with the `debug_capture.gd` harness (`ANABIOS_SHOT*`), 1280x800.
All runs are deterministic per scenario seed, so every shot is reproducible
with the same env vars. Camera close-ups use `ANABIOS_CAM_ZOOM/_CAM_X/_CAM_Y`.

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
| armsrace-t020-volley.png | 58 | The kiting spiner pack (sp4) after its `PackHunting` (t=19) and `CombatRaid` (t=27) — spines kill from the standoff ring before contact weapons can answer. |
| armsrace-t160-standoff.png | 161 | Five species in play: grazers, herd prey, stalkers (sp3), spiners (sp4), bruisers (sp5). Standoff along the northeast flank. |
| armsrace-t400-brawl.png | 461 | The contested northeast border: bruiser clusters (magenta Jaws glyphs) pressing into the herd range. |
| armsrace-t3000-evolved.png | 3091 | End state: 162 species, 171 speciations, a dense migratory swarm sweeping the eastern half of the world. |
| armsrace-t160-inspector.png | 201 | Inspector pinned on a bruiser (id 117, species 5): `Jaws` + `Armor` in its six-module body plan. |

### combat-streak close-ups (feature: attacker→target tracers for ranged fire)

Camera zoomed onto the action so the [combat streaks](../game/scripts/main.gd) read
clearly — the full-world shots above show them only as faint slivers.

| File | Tick | Capture env | What you're seeing |
|---|---|---|---|
| armsrace-volley-closeup.png | 27 | `ZOOM=5.0 X=690 Y=335 TICKS=21 FRAMES=5` | The opening spiner volley up close: four bright cyan tracers stretch from the spiner pack (sp4), each ending on a yellow just-hit target, converging on the clustered herd prey below — the ranged Spines kill *before* contact weapons can close. This is exactly the behavior that was invisible in the viewer before the streak layer landed. |
| armsrace-brawl-closeup.png | 109 | `ZOOM=3.6 X=660 Y=320 TICKS=100 FRAMES=8` | Why the tracers matter: once the lineages collide, the melee is a knot of mixed body plans — cyan spiner, magenta Jaws bruiser, orange/green glyphs — fighting at point-blank. The streaks tint to each attacker's species hue, so ranged fire stays legible even in the scrum. |

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
