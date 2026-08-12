# Out-of-Africa-Earth Emergence — Probe 1 Findings (2026-08-11)

Sub-project B, probe-first cycle. **Question:** does the real Earth map's
geography (niche separation) let the invention climb emerge — the one untried
lever the [ooa-climb findings](2026-08-02-ooa-climb-findings.md) named?

**Answer so far: the real map made emergence HARDER, not easier — but the
dominant blocker is a *worldgen* issue, not the culture-exclusion problem, and
it is fixable.** The "real geography helps culture" hypothesis is not yet
testable because the map currently cannot produce *any* tech.

## Method

Cheap, escalating probes on `scenarios/out-of-africa-earth.toml` (seed 318),
release build:
- `demo --ticks 20000 --report-every 4000` — narrate the invention race.
- `autopsy --ticks 20000 --window 1000 --tag founder` — cultural-vs-asocial
  strategy trajectory + invasion fitness.
- `examples/earth_probe` (throwaway) — terrain histogram of the `from_earth`
  field vs. the procedural field, founder-cell terrain, and an elevation-retune
  test.

## What the probes found

**1. Zero inventions ever fire (20 000 ticks).** Every species stays era 0,
`adopted=[]`, `learning=` empty — even the innovator lineage while it briefly
survived with *high* energy (nrg≈900). Energy was never the constraint.

**2. The dominant blocker is 0% Rock → no obsidian → tech-tree root
unreachable.** Terrain histogram of the real map vs. procedural:

| terrain | real Earth | procedural (seed 318) |
|---|---|---|
| Water | **71.9%** | 21.4% |
| Rock | **0.0%** | 9.5% |
| Taiga | 0.1% | 26.4% |
| Savanna | 10.1% | 10.6% |
| Forest | 8.7% | 8.7% |
| mean carrying-capacity / cell | **3.04** | 8.78 |
| mean cap / LAND cell | 10.78 | 11.16 |

`stone_tools` (era-1 root, prereq of the whole tree) requires **obsidian×2**;
obsidian comes only from **Rock** terrain. **0% Rock → no obsidian → stone_tools
is materially impossible → nothing downstream can ever fire**, regardless of
culture dynamics. All founders sit on Savanna with no Rock in their neighborhood.

**Cause:** the offline builder normalizes elevation `0 m → 0.35`,
`8850 m → 1.0`, so `ROCK_LINE = 0.78` maps to ~5850 m real altitude. Almost no
Earth cell reaches that at 256-res (bilinear-smoothed bedrock ETOPO), so Rock and
high country (Taiga) nearly vanish. All land compresses into elevation
0.35–0.50.

**Fix validated in principle:** re-normalizing with a lower elevation ceiling
restores Rock — 0% → 0.6% (4000 m) → 1.0% (3000 m) → **2.6% (2000 m)**. A proper
builder fix (re-normalize from raw ETOPO with a contrast/gamma curve, not the
lossy inversion this probe used) would do better and could target obsidian near
the East African Rift, where real obsidian is.

**3. The population collapses ~50×** — 1142 → ~22 agents by tick 20 000
(survivors energy-*rich*, mean energy 80 → 1385: classic low-density,
low-carrying-capacity signature). Earth is 72% ocean, so total habitable land
(and thus total forage) is ~⅓ of the procedural map, and agents dispersing into
water die. Per-land-cell capacity is healthy (10.78 ≈ proc 11.16) — it is the
*amount* and *fragmentation* of land, plus ocean mortality, not land quality.

**4. Culture is still competitively excluded** (secondary, but real):
`invasion_fitness mutant=cultural r=-0.521 EXCLUDED`; share-relative
r=-0.273 EXCLUDED — consistent with the prior 0/16-era-3 finding. This wall is
*downstream* of blockers 2 and 3: even if culture weren't excluded, 0% Rock
means 0 tech.

## Interpretation

The real map is **not yet a viable substrate for emergence**. It is a harsher,
sparser, materially-incomplete world than the tuned procedural map. Sub-project
B is therefore not (yet) "build a culture-protection mechanism." It is a
**staged** problem:

1. **Make tech materially possible** — re-tune the builder's elevation
   normalization so realistic mountains/Rock (→ obsidian) exist, ideally near the
   cradle (Rift volcanism). *Validated tractable.*
2. **Make the world demographically viable** — reduce ocean mortality and/or
   raise habitable-land forage (founder placement on larger contiguous land;
   possibly a carrying-capacity or ocean-avoidance lever). *Tractable.*
3. **Only then** face the culture-exclusion wall (competitor cap / culture floor
   / niche protection) — the hard open problem the prior findings doc names,
   which no single knob has cleared.

Stages 1–2 are worldgen/scenario engineering; stage 3 is the genuine research
risk. Making tech *possible* and the world *viable* does not guarantee the
era-3 climb — it only makes the culture question askable.

## Decision (open — for the human)

Three directions, materially different in scope/appetite:
- **(A) Fix the real map (stages 1–2), then re-probe culture.** Keeps the
  real-Earth showcase; concrete engineering; culture wall still awaits at stage 3.
- **(B) Pursue emergent inventions on the PROCEDURAL map instead** — where the
  world is stable (~3000) and discovery already fires (Stone Tools t797) — by
  building a real culture-protection mechanism. Decouples the research from the
  real-map aesthetics; still may not clear the ≥50%-era-3 bar.
- **(C) Accept the honest negative result and stop.** Real geography does not
  rescue emergence; seeding (`out-of-africa-saga`) remains the honest showcase;
  era-3 emergence stays an open problem.

## Reproduce

```sh
cargo build --release -p anabios-headless
./target/release/anabios-headless demo    --scenario scenarios/out-of-africa-earth.toml --seed 318 --ticks 20000 --report-every 4000
./target/release/anabios-headless autopsy  --scenario scenarios/out-of-africa-earth.toml --seed 318 --ticks 20000 --window 1000 --out runs/earth-autopsy-s318.csv --tag founder
cargo run --release -p anabios-core --example earth_probe   # throwaway terrain/retune probe
```
