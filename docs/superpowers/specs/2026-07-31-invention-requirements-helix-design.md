# Invention Requirements + Dual-Inheritance Helix — Design Spec

**Date:** 2026-07-31
**Status:** Implemented
**Depends on:** the invention tree (`invention/mod.rs`), TG1 gene↔tech coupling
(`GeneAffinity`, `gene_tech_coupling`), the trade-goods economy
(`resource.rs`, `resources_enabled`), and the Godot coevo bridge
(`anabios-godot/src/lib.rs`, `coevo.rs`).

## 1. Goal

Two moves, one theme — make dual inheritance theory (DIT) a first-class
mechanic *and* a legible picture:

1. **Enhance every invention's genetic and material requirements.** Before
   this change only 4 of 10 inventions carried a `GeneAffinity`, genetic
   gating was soft-only (affinity weighting + the IQ ceiling), and material
   baskets were thin (1–3 goods, barely era-scaled). Now the whole tree
   participates in gene-culture coevolution.
2. **Visualize dual inheritance as a double helix.** A live Godot panel
   (`helix_panel.gd`, toggle `[X]`) draws the genome and the memome as two
   winding strands with rungs for every coupling between them — the DIT
   picture (two inheritance channels, interacting) rendered literally.

## 2. Mechanics (anabios-core)

### 2.1 Affinities for all ten inventions

Every `Invention` now carries a `GeneAffinity` (`invention/mod.rs`). The
pairings wire three previously inert slots into the adaptive loop and tie the
tree to the DIT learning slots:

| Invention | Affinity slot | Rationale |
|---|---|---|
| Stone Tools | `IndividualLearning` | knapping is learned-by-doing |
| Fire | `Openness` | bold experimenters (TG1 original) |
| Farming | `Conscientiousness` | sedentary planning (TG1 original) |
| Metalworking | `Territoriality` *(was inert)* | weapons pay for ground-holders |
| Writing | `CommunicationStrength` (TG1 original) | literacy rewards communicators |
| Medicine | `CognitivePotential` (TG1 original) | cognition lineage reap its gift |
| Husbandry | `Agreeableness` | livestock tolerate patient keepers |
| Machinery | `ExploreVsExploit` *(was inert)* | machines reward routine exploiters |
| Electricity | `Openness` | the TG1 roadmap's proposed pairing |
| Nuclear Power | `MutationRate` | living with mutagenic power |

To make the tech→gene arm real for the six newly-coupled inventions, every
buff site gained a coupled variant reading the holder's genome:
`graze/food_energy/weapon/scavenge/speed/perception/lifespan/spread_multiplier_
coupled` and `flat_upkeep_coupled` (Nuclear income). All coupled multipliers
now take `&Genome` and resolve each invention's own affinity slot via
`coupled_held_genome` — with `gene_tech_coupling` off every one is bit-
identical to the plain `held_f32` form. `sense_all`/`integrate_all` gained a
trailing `gene_tech_coupling: bool` parameter (threaded from `tick::step`;
`false` in tests = identity).

### 2.2 Hard genetic prerequisites (`GeneReq`)

New static field `Invention::gene_req: Option<(slot, min)>` — a hard ceiling
in the same shape as the IQ gate, but heritable: **culture waits on the
genome**. Enforced at both acquisition sites: the discovery candidate filter
(`invention_step`) and the social-copy apprenticeship (`culture_step`), via
`gene_permits(genome, k, enabled)`. Gated on a new opt-in scenario flag
`gene_requirements` (default off = bit-identical baselines; `World` field +
`FORMAT_VERSION` 22→23). Thresholds rise with era (Stone Tools free; Fire
Openness ≥ 0.30 … Nuclear CognitivePotential ≥ 0.65); the gate slot need not
equal the affinity slot (Machinery/Electricity/Nuclear gate on cognition-
adjacent slots while their affinity selects elsewhere).

### 2.3 Richer era-scaled material baskets

Baskets grew denser and era-scaled while respecting the economy invariants
(per-good cost ≤ `STOCK_TARGET` = 2.0, total ≤ `INVENTORY_BASE_CAP` = 12 —
locked by `material_baskets_fit_the_economy`): era 1 totals 2–3 (Fire now
wants hearth stones + fuelwood), era 2 totals 3–4, era 3 totals 3–5
(Husbandry: fodder + salt licks), era 4 totals 5–6 (Nuclear draws on all four
goods). Only consulted when `resources_enabled`.

### 2.4 Determinism invariants (held)

- All three flags off ⇒ byte-identical trajectories; the three golden suites
  (minimal/inventions/cognitive) moved only from serialized-layout growth and
  were regenerated deliberately with `UPDATE_HASHES=1`.
- No new RNG draws on any flag-off path; the gene gate is a pure filter.

## 3. Visualization — the dual-inheritance helix (Godot)

### 3.1 Bridge (`anabios-godot/src/lib.rs`)

- `invention_catalog()` extended: `materials`, `affinity {slot, coeff}`,
  `gene_req {slot, min}` per invention.
- `genome_slot_catalog()` — 50 slot display names (new
  `genome::SLOT_NAMES`, alignment unit-tested).
- `meme_channel_catalog()` — 20 channel names.
- `helix_snapshot()` — view-only, on-demand: population mean per genome slot
  (50), per meme channel (20), and per-invention holder−nonholder affinity
  differential (10). Never mutates the world; zero cost while the panel is
  hidden.

### 3.2 Panel (`game/scripts/helix_panel.gd`, toggle `[X]`)

A `Control` with custom `_draw()` in the coevo panel's instrument style:

- **Left strand (genes):** the 12 coupling-relevant slots (10 affinity slots
  + SocialLearning, InnateTechnique, EnvAffinity), node size/fill = live
  population mean.
- **Right strand (memes):** skill, DIT technique, the 10 inventions, the 2
  practices; node size/fill = live adoption level.
- **Rungs (base pairs):** solid = affinity, colored by the live selection
  differential (green: holders carry more of the gene — tech selecting
  genome; red: the reverse; gray idle), thickness ∝ |Δ|; dashed violet =
  hard `GeneReq`; thin blue = DIT learning arms (InnateTechnique /
  IndividualLearning / SocialLearning ↔ technique channel).
- Labels sit in fixed side columns with connector lines so the winding
  backbone never swings a node into its own label.

Registered in `main.gd`, documented in the `[H]` legend, and capturable via
`ANABIOS_HELIX=1` in `debug_capture.gd`. New flagship scenario
`scenarios/gene-requirements.toml` turns on all four flags.

## 4. Evidence

- Unit: `affinity_table_is_well_formed`, `gene_req_table_is_well_formed`,
  `material_baskets_fit_the_economy`, coupled-multiplier identity/monotonicity
  across all buff sites, `slot_names_align_with_the_enum`.
- Integration (`tests/inventions.rs`): `discovery_is_blocked_below_the_gene_
  gate` (a Territoriality-high / Conscientiousness-zero lineage invents
  Metalworking but never Farming — until the gene is lifted),
  `discovery_gene_gate_is_identity_when_flag_off`,
  `spread_respects_the_gene_gate`.
- Golden suites regenerated (layout-only moves); `cargo test --workspace`
  green; screenshot capture of the helix over `tech-gene-coupling.toml`
  (tick ~4000) shows live strands and a negative-differential rung on
  Fire↔Openness.

## 5. Open follow-ups

- TG3/TG4 remain: the helix is a state snapshot, not a time series — the
  lead–lag view (which strand moved first) is still unbuilt.
- Meme channels are unclamped by design (inherit jitter can push them just
  outside [0,1]); the panel shows raw values and clamps only the node fill.
