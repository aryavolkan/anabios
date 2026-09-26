# Territory pull diagnosis — why `inside_territory` stays < 80% and why a stronger pull made it worse

**Date:** 2026-09-25 · **Branch/HEAD:** `claude/territory-habitat-collision` @ `36cacc2` (TERRITORY_PULL = 2.5)
**Scope:** diagnosis only. All instrumentation below was throwaway; the worktree was restored to HEAD
(`git status --short` empty, `git diff` empty) at the end.

This is §1–§10 of the original diagnosis, moved here from a gitignored
scratch location so the code, design spec, and findings doc can cite a
committed source of record. The appendices (throwaway instrument source and
temporary env-knob diffs, both already reverted/deleted) are omitted; §1
and §9's env-knob note say so inline.

## TL;DR

* **The aggregate number is a class mix.** Split by class, HEAD is: **Land 99.6–100 % inside** (every
  seed it survives), **Water 38–98 %**, **Air 8–15 % in all 7 seeds it survives**. Seeds that end
  pure-Air read ~10 %, mixed seeds 52–73 %. Round 3's "worse" (1/8 → 0/8) is mostly *composition*
  (3 pure-Air seeds instead of 2, Water survival 6/8 → 3/8), plus Air never being held at all.
* **Root cause (mechanism): the territory pull cannot win against the steering it is added to, and when
  it is made to win the territory starves its members.** Three measured parts:
  1. **The pull is a fixed-size vector (≤ `TERRITORY_PULL·Territoriality` ≈ 1.25) added to an
     unnormalized, evolvable program intent.** Evolved programs feed sensor values (energy,
     distances clamped at 1e6) into `MoveToward*`; by 6–10 k ticks Air's median intent is
     **57–1040** (p90 up to 1e6) at PULL 2.5, versus ≈ 1 at PULL 0 or 1.0. After normalization the
     pull carries < 1 % of the direction: outside Air agents have **cos(desired, home) ≈ 0.0**
     (−0.07…+0.11), and the pull out-weighs the rest in only 2–17 % of them. A stronger pull made
     escape pay more, so selection inflated the intent faster. That is the "stronger pull, fewer
     at home" signature.
  2. **The pull is zero out to r and only ramps beyond r**, while the territory interior is grazed
     out (biomass under agents inside 0.00–0.34 vs outside 0.17–3.16). Any agent whose forage or herd
     steer points outward (typical: cos(other, home) −0.2 to −0.87) settles on a **stall shell at
     d ≈ r·(1 + |cos| / (PULL·terr))**, which is *outside r by construction*. Measured median d/r of
     non-inflated outside agents is 1.1–1.9, which matches that prediction. Territoriality is also
     selected down where the pull binds (Water 0.09–0.32, Air 0.16–0.37 in some seeds).
  3. **Capacity.** If (1) and (2) are fixed at the current radius law (K = 12, R_MAX = 256, about
     452 unit² per member, and less when co-located genome-species fragments overlap), containment
     jumps to 95–97 % at 1 k ticks, and then **5/8 seeds go fully extinct** by 10 k ticks. The range
     cannot feed the population it holds, so escaping is adaptive.
* **Smallest fix:** (a) in `decide_all`, **cap the intent built so far at unit length whenever the
  territory pull is non-zero** (6 lines, gated, no RNG); (b) in `territory_pull`, **ramp from r/2 to
  full strength at r** instead of r → 2r, so the stall shell falls inside r; (c) **enlarge the
  per-capita range** (`TERRITORY_K` 12 → 24, `TERRITORY_R_MAX` 256 → 512) so a binding range can feed
  its members. (a) alone is a pure bug fix but only reaches 1/8. (a)+(b) at K = 12 is lethal.
  **(a)+(b)+(c) was validated at 6/8 seeds ≥ 80 %, 0 extinctions, and Air alive in 8/8 seeds.**
  Caveat: R_MAX = 512 on a 1024 torus means a full-size range covers most of the world (see §8).
* Hypotheses: **H4 confirmed (dominant)**. **H5 partly** (the metric is composition-dominated but not
  broken; there are no unset rows). **H2 is secondary**, mostly through radius fragmentation, not
  stale centres. **H1 is minor and Water-only**. **H3 is a symptom, not a cause.** Two new causes were
  found: **H6, pull swamped by evolved intent magnitude**, and **H7, carrying capacity of the K√n
  range**.

---

## 1. Instruments (throwaway; deleted or reverted afterwards)

* `crates/anabios-core/tests/zz_territory_diag.rs` (deleted afterwards). It has two `#[ignore]` tests:
  * `zz_territory_diag`: seeds 1–3 (env `DIAG_SEEDS`), flagship scenario, 20 k ticks, with checkpoints
    at 1k/3k/6k/10k/15k/19k/20k. It prints:
    * live species (per `Territory.class`), `next_species_id`, and the share of agents in species born
      within the last 200 / 1000 ticks;
    * for **outside** agents: the d/r histogram, centre validity for the agent's class, class mismatch,
      on-coast (4-neighbour of invalid terrain), gate-blocked (vel = 0 with desired ≠ 0), pull masked
      by `habitat::pull_allowed`, centre moved > 0.25 r in the last step, young species,
      cos(desired, home), and Territoriality < 0.15;
    * mean plant biomass under agents, inside versus outside their own territory;
    * per species with ≥ 10 members: centre validity, r, % inside, RMS spread about the centre and
      about the current member mean, the centre→mean lag, % inside if the centre were at the mean,
      and the best attainable % inside for a perfectly placed centre at the same r;
    * species switches per species step, and whether a switch left the agent outside;
    * **a move decomposition.** At each checkpoint it records every agent's territory pull and
      separation steer, steps once, and reads the final pre-normalization `world.actions[i].move_*`.
      "program+other" = total − pull − sep. It reports |pull| vs |program+other| and cos(·, home).
      For non-inflated agents (|other| ≤ 2) it also reports whether "other" ≈ `plant_direction`,
      cos(plant_dir, home), and the forage-mode share (energy ≤ 30).
  * `zz_probe_lite`: the same 8 seeds × 20 k ticks and the same `inside` metric as
    `territory_measurement_probe`. It reproduces the Round 3 probe line for line, and adds per-class
    inside %, mean outside d/r, mean Territoriality, live species, and a geometry line (own-species r
    vs a radius sized by the whole class population, and inside-any-same-class-territory).
* Temporary env knobs in `src/territory.rs` / `src/tick.rs` (reverted afterwards), so fix
  variants could run without recompiling: `ZZ_CAP` (unit-cap the intent when pulled), `ZZ_INNER`
  (free-roam fraction of r), `ZZ_K`, `ZZ_RMAX`. With all knobs unset, `zz_probe_lite` reproduced
  HEAD exactly (sanity run C0).
* Commands, all `--release`:
  `cargo test -p anabios-core --release --test zz_territory_diag zz_territory_diag -- --ignored --nocapture`
  and `... zz_probe_lite -- --ignored --nocapture`. Knob runs prefix env, e.g.
  `ZZ_CAP=1 ZZ_INNER=0.5 ZZ_K=24 ZZ_RMAX=512 cargo test ...`.
  TERRITORY_PULL 0.0 / 1.0 runs used a temporary edit of the constant, which was reverted.

## 2. Baseline split by class (probe-lite = official probe metric)

HEAD, TERRITORY_PULL = 2.5. The aggregate column is identical to Round 3's probe.

| seed | alive L/W/A | inside | **L** | **W** | **A** | outside mean d/r W / A | mean Terr W / A | live species |
|---|---|---|---|---|---|---|---|---|
| 1 | 0/0/300 | 9.3 | – | – | **9.3** | – / 3.34 | – / 0.35 | 39 |
| 2 | 0/525/299 | 68.0 | – | 98.1 | **15.1** | 1.54 / 2.61 | 0.41 / 0.53 | 22 |
| 3 | 675/525/0 | 72.8 | 99.9 | 38.1 | – | 1.89 / – | 0.32 / – | 146 |
| 4 | 675/0/300 | 71.8 | 100.0 | – | **8.3** | – / 4.52 | – / 0.33 | 44 |
| 5 | 0/0/299 | 12.0 | – | – | **12.0** | – / 3.62 | – / 0.62 | 48 |
| 6 | 0/522/300 | 51.8 | – | 75.3 | **11.0** | 2.11 / 4.45 | **0.09** / 0.47 | 65 |
| 7 | 674/0/300 | 72.9 | 99.6 | – | **13.0** | – / 2.26 | – / 0.39 | 11 |
| 8 | 0/0/300 | 11.3 | – | – | **11.3** | – / 1.38 | – / 0.50 | 8 |

TERRITORY_PULL = 1.0 (Round 2): Air 7.2–28.0 %, Water 25.2–98.3 %, Land 97.9–100 %, 1/8 ≥ 80 %
(seed 5, the only seed with all three classes).

**Reading:** Land is always held. It is a tight herd (per-species RMS spread 3–40 around its centre;
the starter program's well-fed branch steers toward the nearest agent), and it is terrain-bounded.
Air is never held under either constant. The aggregate is roughly Σ(class share × class inside %), so
it moves with which classes survive far more than with the pull.

## 3. Why Air ignores the pull: evolved intent magnitude swamps a fixed-size pull (H6)

In `decide_all` the pull is `+= pull` onto `action.move_*`. Only the direction of the sum is used
(`v / len`). The starter program emits unit vectors, but mutation reaches `MoveToward*` fed by
`SenseEnergy` or distance sensors (`SenseOtherDist` etc. are clamped at **1e6**). Once an Air
lineage's intent is 10²–10⁶, a ≤ 1.25 pull is invisible.

Magnitude of |program+other| (= total − pull − sep) over **all** Air agents:

| | PULL 0.0 | PULL 1.0 | PULL 2.5 (HEAD) |
|---|---|---|---|
| seed 1, t = 6k / 10k / 15k / 19k: median | 5.95 / 1.46 / 1.00 / 1.14 | 0.98 / 0.96 / 223 / 1.23 | **472 / 1024 / 1041 / 304** |
| seed 1: share with \|intent\| > 10 | 48 / 35 / 17 / 34 % | 26 / 26 / 52 / 48 % | **57 / 59 / 64 / 61 %** |
| seed 2, t = 6k / 10k / 15k / 19k: median | (extinct) | 1.00 / 1.00 / 1.00 / 1.00 | **2.12 / 57.2 / 68.2 / 138** |
| seed 2: share > 10 | – | 18 / 13 / 4 / 3 % | **47 / 87 / 79 / 77 %** |
| seed 3: median (Air mostly extinct at 2.5) | 1.00 throughout | 0.68–1.84 | 1e6 (n = 7–17) |

Outside Air agents at HEAD, from the one-step decomposition: |pull| median 0.52–1.29 against
|program+other| median 58–1132. The pull out-weighs the rest in **2–17 %** of them.
**cos(total, home) = −0.07…+0.11** in steady state (seed 1 at t = 6k–15k: 0.05 / −0.00 / 0.05;
seed 2 at t = 10k / 15k: −0.03 / 0.10). Even at d ≥ 2r, cos(total, home) is −0.09…+0.20. The pull
effectively does not exist for them. A stronger pull raises the payoff of escaping, and inflation
indeed runs faster at 2.5 than at 1.0 or 0.

Debug sample (F1 variant, seed 2, t = 3000, an Air agent at d = 2.48 r):
`pull = (−0.02, −0.94)`, `total = (0.63, −0.12)`. The program-side intent is (0.65, 0.81), which
points *away* from home (next section), so the pull only cancels the radial part.

## 4. Why non-inflated agents stall outside r: outward steer + a pull that starts at r (H4, H7)

* **Forage gradient** (mean plant biomass under agents, inside / outside own territory, HEAD):
  seed 1 t = 1k: L 0.34/1.99, A 0.10/2.44. Seed 2 t = 1k: L 0.06/0.19, W 0.08/0.65, A 0.17/2.39.
  Seed 3 t = 1k: L 0.24/3.16, W 0.18/0.99. Later checkpoints: inside ≈ 0.00–0.27, outside
  0.14–2.26. **The territory core is grazed out; food is outside.**
* **Non-pull intent of outside agents points outward:** cos(program+other, home) is Water −0.21…−0.57,
  Land −0.43…−0.87, Air −0.05…−0.52 (HEAD, t ≥ 3k). "other" matches `plant_direction` in only 0–20 %
  of them (up to 40 % once). Mostly it is the herd/neighbour branch plus personality (Neuroticism
  flees the nearest *other-species* agent, which in an 8–274-species crowd is usually a sibling
  toward the centre).
* **Stall shell.** With pull = ramp·P·terr, ramp = (d − r)/r, and an outward unit intent with
  cos = c < 0, the radial balance is at d* = r·(1 + |c|/(P·terr)). For P = 2.5, terr ≈ 0.5,
  |c| = 0.5–0.8 that gives d* = 1.4–1.64 r. Measured **median d/r of non-inflated outside agents:
  Water 1.14–1.88 (seed 1), 1.51–1.85 (seed 2), 1.49–1.83 (seed 3); Land 1.09–1.71.** The d/r
  histograms peak in the 1.25–2 bins (Water in seed 3 at t = 10k: 60 % in [1.5, 2)). These agents
  are exactly where the soft edge puts them, which is outside the metric's r.
* **Territoriality is selected down where the pull binds:** Water 0.50 → 0.21–0.32 (seed 3), 0.09
  (seed 6). Under the unit-cap variant Air goes to 0.24–0.37. At terr < 1/P = 0.4 the full pull (< 1)
  can no longer beat a unit outward intent at all. (Land, which is held for free by terrain and
  herding, went *up* to 0.72–0.88 in seed 3.)

## 5. Species churn, fragmentation, centres (H1, H2, H3, H5)

* **Fragmentation is heavy.** There are 8–274 live species for 300–1234 agents, and
  `next_species_id` reaches 594–1367 by 20k ticks. Per species step, 0.9–6.8 % of surviving agents
  switch species (mostly into brand-new species), and 53–100 % of switchers land outside their new
  territory (>95 % in seeds 1–2). The share of agents in species ≤ 1000 ticks old is 0.2–37 %
  (typically 3–15 %).
  * Inherited-centre lag is real but episodic. Seed 2 at t = 6k had Water splinters 396/436/296/273
    with lag 74–119 and inside 0–11 % versus 67–100 % if centred on their own mean. That checkpoint
    (37 % of agents in young species) read 22.8 % inside. Most checkpoints show < 10 % young species
    among outside agents.
  * The bigger effect is **radius fragmentation**. Co-located siblings each get r = 12√n_s, so the
    union range is far smaller than 12√N. Land in seed 3 had 115 species, mean own r 75, against 256
    for the class population. It is still 100 % inside only because it herds at RMS 3–20.
  * For Air, sizing r by the whole class population at its own centre only lifts HEAD from
    8–15 % to 14–51 % (`GEOM` lines), so fragmentation is not the Air problem.
* **H1 (centre on invalid terrain / coastline pinning) is Water-only and minor.** Among outside
  agents: centre invalid for the class 0–73 % (volatile; high when the ocean-spanning Water species'
  torus mean falls on land), on-coast 0–68 %, pull masked by terrain 0–59 %, gate-blocked 0–18 %,
  all Water. Land is always ~100 % inside, and Air has no terrain.
* **H3 (spread > r) is real for Air, but it is the *result* of the pull failing.** Air per-species
  RMS spread is 250–360 (world-uniform on the 1024 torus ≈ 418), and even a perfectly placed centre
  holds only 20–55 %. At PULL = 0 Air spreads the same way (RMS 300–345), so this is Air's natural
  nomadism, not centre placement. Centre lag (centre→member mean) is 17–170 and matters little next
  to that spread.
* **H5 (metric):** `unset = 0` at every checkpoint, and there is no class mismatch (a species' majority
  class always equals its members' class). The metric is honest per agent but **composition-dominated**
  (see §2). Recommendation: report per-class inside % in the probe.

## 6. Interventions (probe-lite, 8 seeds × 20k, flagship, PULL = 2.5 unless noted)

M1 = unit-cap the intent when the pull ≠ 0. M2 = free roam inside r/2, ramp to full pull at r.
K / R_MAX = radius law constants.

| config | ≥ 80 % | extinct seeds | classes alive (L/W/A seeds) | Air inside (surviving seeds) | Water inside |
|---|---|---|---|---|---|
| HEAD | **0/8** | 0 | 3 / 3 / 7 | 8.3–15.1 | 38.1–98.1 |
| PULL = 1.0 (Round 2) | 1/8 | 0 | 2 / 6 / 7 | 7.2–28.0 | 25.2–98.3 |
| M1 | 1/8 | 0 | 1 / 6 / 6 | 6.0–31.0 | 25.0–100 |
| M1 + M2 (K 12, R 256) | 2/8 | **5** (2, 4, 5, 6, 8) | 1 / 1 / 1 | 45.8 | 100 |
| K 24, R 512 only | 1/8 | 1 (7) | 2 / 3 / 6 | 6.0–52.0 | 52.6–84.0 |
| M1 + K 24, R 512 | 2/8 | 0 | 3 / 6 / 7 | 19.1–56.3 | 60.8–100 |
| **M1 + M2 + K 24, R 512** | **6/8** | **0** | 2 / 5 / **8** | 18.3–100 | 90.5–100 |
| M1 + M2 + K 24, R 256 | 3/8 | 1 (7) | 1 / 5 / 3 | 17.9–91.3 | 45.1–99.6 |
| M1 + M2 + K 18, R 384 | 3/8 | 2 (1, 2) | 3 / 2 / 4 | 27.7–100 | 52.8–57.7 |
| M1 + M2 + K 16, R 320 | 3/8 | 2 (3, 7) | 2 / 2 / 2 | 22.7–50.3 | 79.8–100 |

Per-seed inside % for **M1 + M2 + K 24, R 512**: 99.7 (L+W+A, 1499 alive) / 86.0 / 80.2 / 70.3 /
23.3 / 83.7 / 85.5 / 93.5. The two misses (seeds 4 and 5) are Air lineages whose Territoriality
evolved down to 0.16 / 0.19, which is the remaining, heritable escape route.

M1 + M2 at K = 12 time series (seeds 2 and 4): inside 95.6 / 97.0 % at t = 1k with biomass under
agents ≈ 0.0–0.2. Seed 2 then falls 803 → 332 (t = 3k) → 27 (t = 6k) → 0; seed 4 falls
925 → 120 → 9 → 0. This is what a binding K = 12 range does.

## 7. Hypothesis verdicts

| | verdict | key numbers |
|---|---|---|
| H1 centre on invalid terrain / coast pinning | **minor, Water-only** | centre-invalid 0–73 % (volatile), pull masked 0–59 %, on-coast 0–68 % among outside Water; irrelevant for Land (≈ 100 % in) and Air |
| H2 species churn / inherited stale centres | **secondary** | 8–274 live species, 1–7 %/step switches, 53–100 % of switchers land outside; episodic big lags (74–119); main bite is radius fragmentation (Land mean own r 75 vs 256) |
| H3 centre between disjoint patches / spread > R_MAX | **symptom** | Air RMS 250–360 ≈ world-uniform; also at PULL = 0; Water basin spread explains part of Water's misses |
| H4 pull competes with other steering | **confirmed, dominant** | outward intents cos −0.2…−0.87 → stall shell at median d/r 1.1–1.9 (predicted 1.4–1.64 r) |
| H5 misleading metric | **partly** | no unset rows; aggregate dominated by class composition (L ≈ 100, W 38–98, A 8–15) |
| **H6 (new)** pull swamped by evolved intent magnitude | **confirmed, dominant for Air** | Air median intent 57–1040 (p90 → 1e6) at PULL 2.5 vs ≈ 1 at 0/1.0; cos(total, home) ≈ 0 |
| **H7 (new)** K√n range below carrying capacity | **confirmed** | inside biomass ≈ 0; binding pull at K = 12 → 5/8 extinct; K = 24 / R 512 → 0 extinct |

## 8. Root cause

The pull is an **absolute-magnitude, outside-only** bias on an **unbounded, evolvable** intent. Its
effective strength is therefore set by whatever magnitude the programs happen to emit, and it is
zero exactly where the metric draws its line. With the range sized at K = 12 (≈ 452 unit² per
member, further shrunk by genome-species fragmentation), the range core is grazed out. Every
forager presses outward and sits on the stall shell beyond r. Lineages that escape the pull entirely
(Air by inflating intent magnitude; Water and Air by lowering Territoriality) do better, so a
stronger pull speeds up the escape instead of improving containment. When the escape routes are
closed (M1 + M2), the K = 12 range starves the population (5/8 extinct). The containment shortfall
is the ecology's correct response to a range that cannot feed its members, amplified by a pull whose
weight is not controlled.

## 9. Recommended fix

Smallest change set, everything gated behind `territory_enabled`, with no RNG and a fixed evaluation
order. Flag-off stays byte-identical: `territory_pull` / `radius_for` are only reached under the
flag, and the cap sits inside the existing `if territory_enabled` block.

**(a) Magnitude-robust pull.** `crates/anabios-core/src/tick.rs::decide_all`, the territory block
(currently lines 341–352):

```rust
if territory_enabled {
    if let Some(t) = territories.get(agents.species_id[i] as usize) {
        let pull = crate::territory::territory_pull(
            t,
            agents.position[i],
            agents.genome[i].get(crate::genome::GenomeSlot::Territoriality),
            ws,
        );
        if pull != Vec2::ZERO {
            // Past the free-roam zone the intent built so far counts as at
            // most a unit vote: evolved programs feed sensor values (energy,
            // distances clamped at 1e6) into MoveToward*, which otherwise
            // drowns a fixed-size pull after normalization.
            let v = Vec2::new(action.move_x, action.move_y);
            if v.is_finite() {
                let c = v.clamp_length_max(1.0);
                action.move_x = c.x;
                action.move_y = c.y;
            }
        }
        action.move_x += pull.x;
        action.move_y += pull.y;
    }
}
```

**(b) Full pull at the edge, not beyond it.** `crates/anabios-core/src/territory.rs::territory_pull`
(lines 57–63):

```rust
/// Members roam free inside `TERRITORY_FREE_FRAC · r`; the pull ramps from
/// there to full strength at `r`, so the stall point of an outward-steering
/// member lies inside the range rather than on a shell beyond it.
pub const TERRITORY_FREE_FRAC: f32 = 0.5;
...
let inner = TERRITORY_FREE_FRAC * t.r;
if dist <= inner {
    return Vec2::ZERO;
}
let ramp = ((dist - inner) / (t.r - inner)).min(1.0);
(d / dist) * (ramp * TERRITORY_PULL * terr)
```

**(c) Range sized to what it must feed.** `territory.rs` constants: `TERRITORY_K: 12.0 → 24.0`,
`TERRITORY_R_MAX: 256.0 → 512.0` (4× per-capita area).

Follow-ups a real implementation needs: update `pull_is_zero_inside_and_ramps_outside` (new
geometry) and `radius_follows_sqrt_members_and_clamps` (radius_for(100) = 240); re-pin
`HABITAT_GOLDEN` (flag-on only); amend design spec §4 (pull formula, constants); optionally add
per-class inside % to `territory_measurement_probe`.

**Predicted effect:**
* (a) alone removes the intent-inflation escape but not the stall shell (≈ 1/8).
* (a)+(b) at K = 12 holds ≥ 95 % but starves (most seeds extinct), so it must not ship without (c).
* (a)+(b)+(c) should reach ≥ 80 % on most seeds with no extinctions. The remaining misses should be
  lineages that evolved Territoriality < 1/PULL = 0.4, which is a legitimate heritable escape.

**Validated** (probe-lite above, and the official probe in §10):
**HEAD 0/8 → 6/8 ≥ 80 %, 0 extinctions, Air alive 8/8 (was 7/8), Water 5/8 (was 3/8), Land 2/8
(was 3/8), one seed with all three classes at 1499 alive.**

**Caveats / alternatives:**
* R_MAX = 512 on a 1024 torus lets a max-size range cover most of the world, which weakens the
  territory story for the big classes. R_MAX = 256 with K = 24 gave only 3/8 (1 extinct). The honest
  reading is that the flagship's populations (max_share 675/525/300 on a ~50 %-ocean 1024 world) need
  near-world-scale ranges to be fed.
* If world-scale ranges are unwanted, the alternatives are:
  * lower the populations (max_population / max_share) or raise forage productivity, so that a
    ≤ 256 range can feed them;
  * size r from the co-ranging lineage population rather than each genome-species fragment
    (addresses the fragmentation half of H7). This is a larger change.
* Or accept the target as unreachable for Air by design and report per-class numbers.
* Territoriality stays heritable, so the pull can still be evolved away where holding is costly. That
  is intended behaviour, not a bug, but it bounds any "≥ 80 % on every seed" target.
* Optional hygiene, not needed for the result: snap a brand-new species' centre to its own members'
  centroid on its first `territory_step` instead of EMA-ing from the parent's centre (removes the
  episodic 74–119-unit lags in §5).

## 10. Official probe: before / after

`cargo test -p anabios-core --release --test invariants territory_measurement_probe -- --ignored --nocapture`

**Before** (HEAD `36cacc2`; identical to the Round 3 findings doc, and reproduced exactly by probe-lite):

```
seed=1 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=9.3% water_cells_with_biomass=6938
seed=2 alive=824 land=0 water=525 air=299 violations=0 deep_overlaps=3 inside_territory=68.0% water_cells_with_biomass=10030
seed=3 alive=1200 land=675 water=525 air=0 violations=0 deep_overlaps=0 inside_territory=72.8% water_cells_with_biomass=5768
seed=4 alive=975 land=675 water=0 air=300 violations=0 deep_overlaps=38 inside_territory=71.8% water_cells_with_biomass=3264
seed=5 alive=299 land=0 water=0 air=299 violations=0 deep_overlaps=2 inside_territory=12.0% water_cells_with_biomass=811
seed=6 alive=822 land=0 water=522 air=300 violations=0 deep_overlaps=116 inside_territory=51.8% water_cells_with_biomass=10205
seed=7 alive=974 land=674 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=72.9% water_cells_with_biomass=6285
seed=8 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=11.3% water_cells_with_biomass=9427
```
→ 0/8 ≥ 80 %. Land alive 3/8, Water 3/8, Air 7/8.

**After** fix (a)+(b)+(c), driven by the temporary knobs `ZZ_CAP=1 ZZ_INNER=0.5 ZZ_K=24 ZZ_RMAX=512`
(same arithmetic as the §9 sketch). The run took 237 s:

```
seed=1 alive=1499 land=674 water=525 air=300 violations=0 deep_overlaps=3 inside_territory=99.7% water_cells_with_biomass=11949
seed=2 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=86.0% water_cells_with_biomass=10454
seed=3 alive=825 land=0 water=525 air=300 violations=0 deep_overlaps=12 inside_territory=80.2% water_cells_with_biomass=6148
seed=4 alive=825 land=0 water=525 air=300 violations=0 deep_overlaps=2 inside_territory=70.3% water_cells_with_biomass=6611
seed=5 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=23.3% water_cells_with_biomass=6476
seed=6 alive=824 land=0 water=524 air=300 violations=0 deep_overlaps=1 inside_territory=83.7% water_cells_with_biomass=5192
seed=7 alive=773 land=0 water=473 air=300 violations=0 deep_overlaps=5 inside_territory=85.5% water_cells_with_biomass=6287
seed=8 alive=974 land=674 water=0 air=300 violations=0 deep_overlaps=1 inside_territory=93.5% water_cells_with_biomass=7689
```

Result: **6/8 ≥ 80 %** (was 0/8). Violations stay 0. deep_overlaps are 0–12 (was 0–116). There are no
extinctions. Land is alive in 2/8 seeds, Water 5/8 (was 3/8), Air 8/8 (was 7/8), and seed 1 keeps all
three classes. The two misses, seeds 4 and 5, are Air lineages that evolved Territoriality to
0.16 / 0.19, below the 1/PULL = 0.4 threshold. The prediction held: the misses are heritable escape,
not mechanism failure.
