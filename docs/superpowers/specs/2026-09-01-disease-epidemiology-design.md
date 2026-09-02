# Disease & Epidemiology — design (2026-09-01)

> One subsystem, opt-in per scenario: **pathogens spill over in crowded
> populations, spread by proximity, drain energy (mortality funnels through the
> existing starve path), and Medicine finally has a pressure to counter.**
> Flag: `disease_enabled`. New codex events: `EpidemicOutbreak` (id 60),
> `MedicineContainment` (id 61).

Roadmap item: H1 (Sep–Nov 2026), `ROADMAP.md` "Disease/epidemiology subsystem".
Following the repo's opt-in recipe (flag → scenario plumbing → tick stage →
codex detector → tests → determinism rehash), mirroring the knowledge-ratchet
(E14) / anthro-race (PR #141) skeletons.

## Motivation & evidence base

- **Corpus gap.** The 60 codex event types cover ecology (speciation, cycles,
  cascades), culture (dialects, memes, ratchets), economy (trade, markets),
  conflict (raids, war) and affect (panics, frenzies, grief) — **zero
  pathology**. Boom-bust in the e1.3 corpus is driven exclusively by predation
  and starvation; no run has ever exhibited disease dynamics because no
  substrate exists. This subsystem closes that hole with the smallest honest
  model.
- **Medicine has no counter-pressure.** Invention id 5 (Medicine, era 3)
  currently confers only a flat lifespan buff (`invention/mod.rs:220-228`,
  `MEDICINE_LIFESPAN = 0.50`). Nothing in the sim ever *justifies* medicine —
  its adoption is free-floating. A pathogen gives it a selective raison d'être,
  which is what `MedicineContainment` measures.
- **Scope discipline.** Per the E-arc scope-creep guardrail, this v1 is
  deliberately minimal: one column, one stage, two detectors. Immunity memory,
  carcass-borne transmission, and cross-species strain specialization are
  explicitly deferred (§Follow-ups).

## Design decisions

1. **SIS model, no immunity memory (v1).** Infection is a single per-agent
   intensity; recovered agents are immediately susceptible again. Epidemic
   *waves* emerge from population churn (death of the infected, birth of
   susceptibles) rather than an immunity clock — fewer columns, fewer
   parameters, and the boom-bust pathology story survives.
2. **Mortality via energy drain, not a new death path.** The stage subtracts
   `infection * DRAIN` from energy; death still funnels through
   `age::age_and_starve` (stage 7), so carcasses, scavenging, war attribution,
   and conserve-goods-on-death all keep working untouched. The disease stage
   runs as **6g**, after knowledge (6f) and before age+starve (7).
3. **Spillover is seeded by crowding.** Infection enters the world only where
   density is high — the classic epidemiological story, and it ties outbreaks
   to the *emergent* population boom rather than a scripted injector. No RNG
   is drawn for uncrowded agents.
4. **Medicine counters at both edges.** Holders transmit less (susceptibility
   down) and recover faster — so the containment arc (outbreak → adoption →
   resolution) is actually reachable, not just detectable.
5. **Inventions not required.** `disease_enabled` stands alone (outbreaks work
   in culture-free worlds; the containment event simply never fires). The
   showcase scenario layers `inventions_enabled` + an ape band on top.

## Components

### A — agent column

- `AgentBuffers.infection: Vec<f32>` (0.0..1.0) — infection intensity. Written
  in all three SoA write paths (`spawn`, `grow_one`, dead-slot reset in `kill`).
  Serialized (plain column, not `#[serde(skip)]` — it is path-dependent state).
  Zero/inert with the flag off; the schema change rehashes all goldens once
  (FORMAT_VERSION 34 → 35).

### B — tick stage `disease_step` (6g)

Order per tick, flag-gated early return with **zero RNG draws when off**:

1. **Recovery & drain** (alive, ascending id): if `infection[i] > 0`:
   - recover: `infection -= RECOVERY_RATE * (has Medicine ? MEDICINE_RECOVERY_MULT : 1)`,
     clamped to 0 (deterministic, no draw);
   - drain: `energy -= infection * DISEASE_DRAIN` (only while still infected).
2. **Spillover** (alive, ascending id): for an uninfected agent whose neighbor
   count within `SPILLOVER_RADIUS` (spatial-hash query) reaches
   `SPILLOVER_MIN_NEIGHBORS`, draw once from `world.rng`; hit with
   `SPILLOVER_P` ⇒ `infection = INFECTION_SEED`.
3. **Transmission** (two-pass, order-independent): for each shedding agent
   (`infection >= SHED_MIN`), query neighbors within `TRANSMISSION_RADIUS`;
   each *uninfected* neighbor is a candidate with
   `p = TRANSMIT_P * (neighbor has Medicine ? MEDICINE_TRANSMIT_MULT : 1)`.
   Candidates are collected into a `BTreeMap<target, best_p>` (dedup keeps the
   strongest source), then resolved in ascending-target order with one
   `world.rng` draw each: hit ⇒ `infection = INFECTION_SEED`.

Determinism notes: all iteration is ascending id / BTreeMap; neighbor queries
use the rebuilt-this-tick spatial hash (stage 1) with exact torus-distance
filtering; the draw *count* depends only on deterministic candidate sets.

### C — codex detectors

- Per-species `infected_count` (members with `infection >= SHED_MIN` — clinically
  sick/shedding) joins the fused aggregation pass (`SpeciesAgg`, added to
  `Default`/`build`/`reset`). The "infected fraction" is
  `infected_count / count` — **not** mean intensity (`infection_sum / count`),
  which measured ~0.06 during a real wave and never crossed threshold
  (caught by the first emergence run).
- **`EpidemicOutbreak` (60)** — per species with `count >= OUTBREAK_MIN_POP`:
  infected fraction `>= OUTBREAK_FRACTION` (0.25) ⇒ fire once and latch
  (`CodexState.epidemic_latched: BTreeSet<u32>`); re-arm when the fraction
  falls below `OUTBREAK_REARM` (0.125), so successive epidemic waves re-fire.
- **`MedicineContainment` (61)** — outbreak latched **and** infected fraction
  `< OUTBREAK_REARM` **and** Medicine adoption among the species
  (`invention_counts[MEDICINE] / count`) `>= 0.5` ⇒ fire once per wave, latch
  cleared. This is the counter-pressure receipt: the species that resolved its
  outbreak is the one carrying medicine.

**Measured nuance (2026-09-01 emergence run):** a culture that holds Medicine
*from t0* mostly **prevents** outbreaks (0.25× susceptibility, 3× recovery — a
seeded wave died at 1 infected), so `MedicineContainment` fires in the
adopt-during-wave arc (outbreak first, adoption rises mid-wave, wave
resolves), not in the pre-seeded demo. Both events are therefore expected to
be rarer than `EpidemicOutbreak` in the demo scenario — prevention is the
stronger effect, and that is the correct epidemiology.

### D — scenario

`scenarios/disease.toml`: two bands. A dense susceptible **grazer herd** (150
founders, tight cluster — the outbreak population; spillover fires in the
first ~25–70 ticks on most seeds) and a **medicine-bearing innovator band**
(60 founders, seeded with the full era-3 chain — medicine requires writing;
apes-only content policy, only ape archetypes invent). Viewer menu entry
optional (not added in v1); the scenario is smoke-covered by
`all_scenarios.rs` automatically.

## Parameters (in `disease.rs`, tuned so outbreaks are rare-but-reachable)

| const | value | meaning |
|---|---|---|
| `SPILLOVER_RADIUS` | 8.0 | crowding probe radius |
| `SPILLOVER_MIN_NEIGHBORS` | 6 | minimum crowd for zoonotic spillover |
| `SPILLOVER_P` | 0.0002 | per-tick spillover probability when crowded |
| `TRANSMISSION_RADIUS` | 4.0 | contact radius |
| `TRANSMIT_P` | 0.05 | per-contact per-tick transmission probability |
| `INFECTION_SEED` | 0.3 | initial intensity on infection |
| `SHED_MIN` | 0.1 | minimum intensity to shed |
| `RECOVERY_RATE` | 0.01 | per-tick deterministic recovery |
| `DISEASE_DRAIN` | 0.02 | per-tick energy drain at intensity 1.0 |
| `MEDICINE_RECOVERY_MULT` | 3.0 | recovery multiplier for Medicine holders |
| `MEDICINE_TRANSMIT_MULT` | 0.25 | susceptibility multiplier for Medicine holders |
| `OUTBREAK_MIN_POP` | 20 | minimum species count to outbreak-detect |
| `OUTBREAK_FRACTION` | 0.25 | infected-fraction outbreak threshold |
| `OUTBREAK_REARM` | 0.125 | re-arm/containment resolution threshold |

## Plumbing & tables checklist

- `scenario.rs`: `disease_enabled` flag (serde default false). No cross-flag
  requirement.
- `world.rs`: `disease_enabled` field (serde default), `World::new` default.
- `agent.rs`: `infection` column in `spawn`/`grow_one`/`kill`.
- `lib.rs`: `pub mod disease;`. `tick.rs`: stage 6g call.
- `codex/event.rs`: `EpidemicOutbreak = 60`, `MedicineContainment = 61`
  (append-only; discriminant-pinning test updated).
- `codex/agg.rs`: `infected_count` (+ Default/build/reset).
- `codex/mod.rs`: `mod disease;`, `CodexState.epidemic_latched`, gated call in
  `observe_all`.
- `codex/disease.rs`: the two detectors.
- `snapshot.rs`: FORMAT_VERSION 34 → 35 + changelog line.
- `anabios-headless/src/score.rs`: `ALL_EVENT_NAMES` += 2, `DEFAULT_CORPUS_NT`
  += 2 (at 0 → novelty bonus), `event_name` arms; `tests/sweep_csv.rs` column
  count 70 → 72.
- `game/scripts/codex_panel.gd`: `CHAPTER_NAMES`/`CHAPTER_COLORS` += 2.

## Tests

- `tests/disease.rs`: (a) handcrafted crowded world with the flag on ⇒
  `EpidemicOutbreak` fires; (b) flag off ⇒ zero infection, zero events; (c)
  medicine A/B: a holder cohort ends a seeded infection wave with materially
  lower peak infected fraction than a matched non-holder cohort; (d) negative
  test: sparse world (below `SPILLOVER_MIN_NEIGHBORS`) ⇒ no spillover, no
  events.
- `tests/determinism.rs`: golden rehash (`UPDATE_HASHES=1`) — schema change;
  behavior with flag off is byte-identical by construction (stage early-return,
  zero draws).
- `tests/save_load_roundtrip.rs`: add a disease-flag round-trip row.
- Sweep evidence: a `--archive runs/corpus-e1.3` sweep of `disease.toml` with
  `epidemic_outbreak` firing; corpus gets the two new columns at weight 0.

## Follow-ups (explicitly out of v1)

- Immunity memory (SIR/SIRS) — second column `immunity_timer`.
- Carcass-borne transmission via scavenging.
- Cross-species strain specialization / zoonotic jump events.
- Viewer disease overlay tint; deck in the scenario garden.

---

_Plan: `docs/superpowers/plans/2026-09-01-disease-epidemiology.md`._
