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
