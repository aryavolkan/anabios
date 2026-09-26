# Scenario Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scenario files run the full engine unless they opt out, and the curated set shrinks from 81 files to twelve worlds organised by setting, with every reader (tests, Godot menu, decks, web atlas, docs) following.

**Architecture:** The flip happens in the `Scenario` serde schema only (`#[serde(default = "default_true")]` on 26 feature knobs, `season_period` defaulting to 2000); `World::new` stays all-off so the engine's flag-off guarantees and ~530 unit tests keep their meaning. Flag-off trajectory guards get explicit all-off inline fixtures BEFORE any golden is re-pinned, so "engine unchanged" is proven separately from "scenarios changed". Twelve TOML files replace 81; suites are re-targeted by ingredient with inline fixtures wherever a phenomenon no longer appears.

**Tech Stack:** Rust 2021 (serde/toml, criterion), Godot 4 GDScript, Node (web manifest), bash scripts; gates: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items`, `gdformat --check game/scripts/`, `gdlint game/scripts/`, `scripts/godot-smoke.sh`.

**Spec:** `docs/superpowers/specs/2026-09-26-scenario-consolidation-design.md`

## Global Constraints

- Engine defaults (`World::new`, `World::with_dims`) do not change; no engine constant or mechanism changes (spec §6).
- `FORMAT_VERSION` does not change (no serialized layout moves).
- Flag-off trajectory pins must not move: minimal@1000 `0xd1133dd8d119e894`, grand-theater@200 `0x56819428b6cd2bf0` (spec §3 rule 3).
- Four knobs stay off by default: `env_period`, `climate_drift_rate`, `payoff_biased_learning`, `unilateral_trade` (spec Decisions 6).
- No test suite is deleted or silently weakened; a vanished phenomenon keeps an inline fixture (spec §3 rule 2).
- After the change `scenarios/` contains exactly twelve files: `minimal`, `predator-prey`, `speciation`, `tribes`, `markets`, `habitat-territories`, `grand-theater`, `out-of-africa-saga`, `out-of-africa-earth`, `sandbox`, `riverlands`, `huge-steppe` (`.toml`), plus `scenarios/decks/README.md`.
- Never `git add -A` / `git add .`; stage explicit paths. Never `cd` into `/Users/aryasen/projects/anabios` (the main checkout); work in the branch's worktree.
- Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Coverage floor (anabios-core ≥ 95% lines) must hold; check with `RUSTFLAGS="--cfg coverage" cargo check -p anabios-core --tests` that the shortened horizons still compile.

---

## File structure

| Path | Responsibility |
|---|---|
| `crates/anabios-core/src/scenario.rs` | `Scenario` schema: knob defaults, doc comments, `ScenarioError` messages |
| `crates/anabios-core/tests/common/mod.rs` | shared helpers (unchanged); `assert_golden`, `assert_roundtrip`, `ticks` |
| `crates/anabios-core/tests/fixtures.rs` (new, `#[path]`-included module) | inline TOML fixtures: flag-off copies of minimal / grand-theater, retired experiment files, retired demo scenarios that keep a suite alive |
| `crates/anabios-core/tests/*.rs` | suites re-targeted per the table in Task 5 |
| `scenarios/*.toml` | the twelve worlds |
| `game/scripts/menu.gd` | twelve menu entries |
| `game/showcase/*.json` | deck pins (`_comment: "scenario=<world>"`) |
| `web/scripts/manifest.mjs` | atlas manifest: `off` list instead of `flags` |
| `showcase/replay.js` | re-recorded saga replay |
| `docs/scenarios.md`, `docs/determinism-contract.md` | rewritten table; two-layer default rule |

`tests/fixtures.rs` is shared by several test binaries the same way `common/mod.rs` is: each suite that needs it adds `#[path = "fixtures.rs"] mod fixtures;` (a sibling file, not a directory, so `cargo test` does not treat it as its own test binary — name it `fixtures.rs` under `tests/` would be compiled as a test target; therefore place it at `tests/common/fixtures.rs` and reference it as `mod common;` + `use common::fixtures;`). **Decision:** put the fixtures in `crates/anabios-core/tests/common/fixtures.rs` and add `pub mod fixtures;` to `tests/common/mod.rs`. All later tasks use `common::fixtures::NAME`.

---

### Task 1: Flip the scenario schema defaults

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs` (struct `Scenario`, lines 22–253; `ScenarioError`, lines ~856–880)
- Modify: `docs/determinism-contract.md`
- Test: `crates/anabios-core/src/scenario.rs` (`mod tests`)

**Interfaces:**
- Produces: `Scenario` whose 26 feature knobs default to `true`, `season_period` to `2000`; `pub(crate) fn default_season_period() -> u32`. `Scenario::parse_toml("name = \"x\"\nseed = 1\n")` now yields a full-stack scenario.

- [ ] **Step 1: Write the failing tests** (append inside `mod tests` in `scenario.rs`)

```rust
    #[test]
    fn bare_scenario_runs_the_full_stack_by_default() {
        let s = Scenario::parse_toml("name = \"bare\"\nseed = 1\n").expect("parse");
        // Every feature knob is on; the four experiment levers stay off.
        assert!(s.biome_adaptation && s.terrain_habitat && s.inventions_enabled);
        assert!(s.gene_tech_coupling && s.gene_requirements && s.cognition_enabled);
        assert!(s.affect_enabled && s.living_biome && s.nutrient_variation);
        assert!(s.soil_fertility && s.resources_enabled && s.conserve_goods_on_death);
        assert!(s.disasters_enabled && s.war_enabled && s.settlement_enabled);
        assert!(s.sexual_dimorphism_enabled && s.domestication_enabled);
        assert!(s.knowledge_enabled && s.practices_enabled && s.basic_needs_enabled);
        assert!(s.mate_seeking_enabled && s.territory_enabled && s.disease_enabled);
        assert!(s.anthro_race_enabled && s.repro_biased_learning);
        assert_eq!(s.season_period, 2000);
        assert_eq!(s.env_period, 0);
        assert_eq!(s.climate_drift_rate, 0.0);
        assert!(!s.payoff_biased_learning && !s.unilateral_trade);
        // The engine layer is untouched: a bare World is still all-off.
        let w = World::new(1);
        assert!(!w.territory_enabled && !w.affect_enabled && !w.cognition_enabled);
    }

    #[test]
    fn explicit_opt_out_still_wins() {
        let s = Scenario::parse_toml(
            "name = \"off\"\nseed = 1\nterritory_enabled = false\nseason_period = 0\n",
        )
        .expect("parse");
        assert!(!s.territory_enabled);
        assert_eq!(s.season_period, 0);
        let w = s.instantiate();
        assert!(!w.territory_enabled && w.affect_enabled);
    }

    #[test]
    fn opting_out_of_inventions_while_knowledge_stays_on_is_rejected() {
        let err = Scenario::parse_toml("name = \"k\"\nseed = 1\ninventions_enabled = false\n")
            .expect_err("knowledge_enabled defaults on and needs inventions");
        assert!(matches!(err, ScenarioError::KnowledgeNeedsInventions), "{err}");
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p anabios-core --lib scenario::tests::bare_scenario_runs_the_full_stack_by_default -- --nocapture`
Expected: FAIL on `assert!(s.biome_adaptation && ...)` (defaults are false today).

- [ ] **Step 3: Flip the defaults**

In `scenario.rs`, next to `default_true` (line 12) add:

```rust
/// Scenario-schema default for `season_period`: grand-theater's cycle. `0` opts out.
pub(crate) fn default_season_period() -> u32 {
    2000
}
```

For each of these 26 fields replace `#[serde(default)]` with `#[serde(default = "default_true")]`: `biome_adaptation`, `terrain_habitat`, `inventions_enabled`, `gene_tech_coupling`, `gene_requirements`, `cognition_enabled`, `affect_enabled`, `living_biome`, `nutrient_variation`, `soil_fertility`, `resources_enabled`, `conserve_goods_on_death`, `disasters_enabled`, `war_enabled`, `settlement_enabled`, `sexual_dimorphism_enabled`, `domestication_enabled`, `knowledge_enabled`, `basic_needs_enabled`, `mate_seeking_enabled`, `territory_enabled`, `disease_enabled`, `anthro_race_enabled`, `repro_biased_learning` (`practices_enabled` already uses `default_true`). For `season_period` use `#[serde(default = "default_season_period")]`. Leave `env_period`, `climate_drift_rate`, `payoff_biased_learning`, `unilateral_trade` on `#[serde(default)]`.

Rewrite each flipped field's doc comment: replace the leading "Opt-in:" / "Opt-in …" with "On by default (scenario schema):" and replace the trailing "`false` (default) …" sentence with "Set `false` to opt out; the engine's own default (`World::new`) stays off, so the flag-off byte-identity guarantee is unchanged." For the four levers, keep "Opt-in" and add "(experiment lever, stays off by default)".

Update the `KnowledgeNeedsInventions` message to: `"knowledge_enabled is on but inventions_enabled = false — knowledge accumulation tracks Writing-holding cultures, which don't exist without the invention tree; opt out of knowledge_enabled too"`. Update `InventionsDisabled` similarly ("starting_inventions given but inventions_enabled = false …").

- [ ] **Step 4: Run the scenario unit tests; fix expectations that assumed defaults-off**

Run: `cargo test -p anabios-core --lib scenario 2>&1 | grep -E 'test result|FAILED|panicked'`
Expected: the three new tests pass. Any pre-existing test that fails does so because it parsed a bare TOML and asserted a flag was off, or because instantiate now runs the full stack (e.g. founders relocated by the territory layer, `starting_inventions` now valid). For each failure: if it asserted a default, invert the assertion to the new default; if it needs the old behaviour, add the explicit opt-out line(s) to its TOML string. Do not change what the test proves.

- [ ] **Step 5: Run the whole core unit suite**

Run: `cargo test -p anabios-core --lib 2>&1 | grep -E 'test result|FAILED'`
Expected: `test result: ok`. (Only `scenario.rs` tests parse TOML; the rest build `World` directly.)

- [ ] **Step 6: Document the two default layers**

Append to `docs/determinism-contract.md` after the "trajectory guards" paragraph:

```markdown
## Two default layers

The **scenario schema** (`Scenario`, `scenarios/*.toml`) defaults to the full
engine: every feature knob is `true` and `season_period` is 2000 unless a file
opts out. Four experiment levers stay off by default (`env_period`,
`climate_drift_rate`, `payoff_biased_learning`, `unilateral_trade`). The
**engine** (`World::new`, `World::with_dims`) defaults to nothing: every
subsystem flag is `false`. Every "flag off ⇒ zero RNG draws, byte-identical"
guarantee in this document is stated and tested at the engine layer, and the
flag-off trajectory guards pin inline all-off fixtures, not the scenario files.
```

- [ ] **Step 7: Gates and commit**

Run: `cargo fmt --all --check && cargo clippy -p anabios-core --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc -p anabios-core --no-deps --document-private-items`
Expected: all clean.

```bash
git add crates/anabios-core/src/scenario.rs docs/determinism-contract.md
git commit -m "feat(scenario): every feature knob defaults on at the schema layer

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

(Integration tests are red after this commit — the goldens and the guards move — and are fixed by Tasks 2 and 5 before the branch is pushed.)

---

### Task 2: Flag-off fixtures for the trajectory guards

**Files:**
- Create: `crates/anabios-core/tests/common/fixtures.rs`
- Modify: `crates/anabios-core/tests/common/mod.rs` (add `pub mod fixtures;`)
- Modify: `crates/anabios-core/tests/determinism.rs` (guards use the fixtures)

**Interfaces:**
- Produces: `common::fixtures::OPT_OUT_ALL: &str` (a TOML block turning every knob off), `common::fixtures::MINIMAL_FLAG_OFF: &str`, `common::fixtures::GRAND_THEATER_FLAG_OFF: &str`, and `common::fixtures::with_opt_outs(base: &str) -> String`.

- [ ] **Step 1: Write the opt-out block and the fixture helper**

`crates/anabios-core/tests/common/fixtures.rs`:

```rust
//! Inline scenario fixtures. The scenario schema defaults every feature knob
//! ON; these keep the pre-flip configurations of files the suites depend on.
//! `OPT_OUT_ALL` lists every knob explicitly, so a fixture's meaning cannot
//! drift when a future knob is added — add it here too.
#![allow(dead_code)]

/// Every feature knob, off. Appended AFTER a file's own top-level keys; TOML
/// rejects duplicate keys, so a base that already sets one of these must go
/// through `with_opt_outs`, which drops the duplicates.
pub const OPT_OUT_ALL: &str = "
biome_adaptation = false
terrain_habitat = false
inventions_enabled = false
gene_tech_coupling = false
gene_requirements = false
cognition_enabled = false
affect_enabled = false
living_biome = false
season_period = 0
env_period = 0
climate_drift_rate = 0.0
nutrient_variation = false
soil_fertility = false
resources_enabled = false
conserve_goods_on_death = false
disasters_enabled = false
war_enabled = false
settlement_enabled = false
sexual_dimorphism_enabled = false
domestication_enabled = false
knowledge_enabled = false
practices_enabled = false
payoff_biased_learning = false
basic_needs_enabled = false
mate_seeking_enabled = false
territory_enabled = false
repro_biased_learning = false
unilateral_trade = false
anthro_race_enabled = false
disease_enabled = false
";

/// `base` with every knob it does NOT already set turned off. The opt-outs
/// are inserted before the first table header (`[climate]`, `[[agents]]`, …)
/// so they stay top-level keys.
pub fn with_opt_outs(base: &str) -> String {
    let set: std::collections::HashSet<&str> = base
        .lines()
        .filter_map(|l| l.split('=').next())
        .map(str::trim)
        .collect();
    let extra: String = OPT_OUT_ALL
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| !set.contains(l.split('=').next().unwrap().trim()))
        .map(|l| format!("{l}\n"))
        .collect();
    match base.find("\n[") {
        Some(i) => format!("{}\n{}{}", &base[..i], extra, &base[i..]),
        None => format!("{base}\n{extra}"),
    }
}

/// `scenarios/minimal.toml` as it was before the schema flip (all knobs off).
pub fn minimal_flag_off() -> String {
    with_opt_outs(include_str!("../../../../scenarios/minimal.toml"))
}

/// `scenarios/grand-theater.toml` as it was before the flip: its own explicit
/// flags (14 on) plus every other knob off.
pub fn grand_theater_flag_off() -> String {
    with_opt_outs(GRAND_THEATER_PRE_FLIP)
}

/// Verbatim copy of `scenarios/grand-theater.toml` at commit 16d9731 (the
/// merge of PR #173), i.e. the configuration the pin was computed on. Kept
/// inline because Task 3 rewrites the live file.
pub const GRAND_THEATER_PRE_FLIP: &str = include_str!("grand-theater.pre-flip.toml");
```

Then copy the file: `cp scenarios/grand-theater.toml crates/anabios-core/tests/common/grand-theater.pre-flip.toml` and likewise `cp scenarios/minimal.toml crates/anabios-core/tests/common/minimal.pre-flip.toml`, and make `minimal_flag_off()` include the copy instead of the live file (the live file may change in Task 3):

```rust
pub fn minimal_flag_off() -> String {
    with_opt_outs(include_str!("minimal.pre-flip.toml"))
}
```

Add `pub mod fixtures;` at the top of `tests/common/mod.rs`.

- [ ] **Step 2: Point the guards at the fixtures**

In `tests/determinism.rs` replace the two guard tests' sources:

```rust
#[test]
fn minimal_trajectory_unchanged_by_territory_substrate() {
    assert_trajectory(
        "minimal (all knobs off)",
        &common::fixtures::minimal_flag_off(),
        1000,
        MINIMAL_TRAJECTORY_AT_1000,
    );
}

#[test]
fn grand_theater_trajectory_unchanged_by_territory_substrate() {
    assert_trajectory(
        "grand-theater (pre-flip flags)",
        &common::fixtures::grand_theater_flag_off(),
        200,
        GRAND_THEATER_TRAJECTORY_AT_200,
    );
}
```

Rename the two tests to `minimal_flag_off_trajectory_is_pinned` and `grand_theater_pre_flip_trajectory_is_pinned`, and update the doc comment above `trajectory_hash` to say the pins are computed on the inline fixtures, not on the live files.

- [ ] **Step 3: Run the guards — they must pass with the UNCHANGED pins**

Run: `cargo test -p anabios-core --release --test determinism trajectory 2>&1 | grep -E '^test |test result|panicked|left|right'`
Expected: both `... ok`, pins `0xd1133dd8d119e894` and `0x56819428b6cd2bf0` untouched. If a pin moves, `with_opt_outs` missed a knob (compare against the field list in Task 1) — do NOT re-pin.

- [ ] **Step 4: Unit-test the helper**

Append to `fixtures.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn with_opt_outs_covers_every_knob_and_keeps_explicit_values() {
        let s = with_opt_outs("name = \"t\"\nseed = 1\nwar_enabled = true\n[[agents]]\ncount = 1\n");
        assert!(s.contains("war_enabled = true") && !s.contains("war_enabled = false"));
        assert!(s.contains("territory_enabled = false"));
        assert!(s.find("territory_enabled").unwrap() < s.find("[[agents]]").unwrap());
        let parsed = anabios_core::scenario::Scenario::parse_toml(&s).expect("parses");
        assert!(parsed.war_enabled && !parsed.territory_enabled && parsed.season_period == 0);
    }
}
```

(`common` is compiled into each test binary; this `mod tests` runs under `cargo test -p anabios-core --test determinism`.)

Run: `cargo test -p anabios-core --test determinism with_opt_outs -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/tests/common/fixtures.rs crates/anabios-core/tests/common/mod.rs crates/anabios-core/tests/common/minimal.pre-flip.toml crates/anabios-core/tests/common/grand-theater.pre-flip.toml crates/anabios-core/tests/determinism.rs
git commit -m "test(determinism): pin the flag-off trajectories on inline fixtures, not the live files

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: The twelve worlds

**Files:**
- Create: `scenarios/speciation.toml`, `scenarios/tribes.toml`, `scenarios/markets.toml`, `scenarios/sandbox.toml`
- Modify: `scenarios/minimal.toml`, `scenarios/predator-prey.toml`, `scenarios/grand-theater.toml`, `scenarios/habitat-territories.toml`, `scenarios/out-of-africa-saga.toml`, `scenarios/out-of-africa-earth.toml`, `scenarios/riverlands.toml`, `scenarios/huge-steppe.toml`
- Delete: every other `scenarios/*.toml`, the whole `scenarios/experiments/` directory

**Interfaces:**
- Produces: the twelve files, each parseable by `Scenario::parse_toml` and surviving `tests/all_scenarios.rs` (200 ticks).

Conventions for every world file: no feature knob is written unless it is an opt-out (`= false`) or an experiment lever the world deliberately uses (`env_period`, `climate_drift_rate`); every lineage in a multi-lineage world carries `max_share` (the shared `max_population` cap otherwise lets the first lineage to breed sterilise the rest — see `habitat-territories.toml`'s header); the header comment states the setting, the phenomena the world shows, and the files it absorbed.

- [ ] **Step 1: Drop explicit feature flags from the eight surviving files**

For each of `minimal`, `predator-prey`, `grand-theater`, `habitat-territories`, `out-of-africa-saga`, `out-of-africa-earth`, `riverlands`, `huge-steppe`: delete every top-level line matching the 26 flipped knobs set to `true`, and delete `season_period = 2000` where present (it is now the default; keep other values such as `season_period = 3000`/`5000`). Keep `env_period` / `climate_drift_rate` lines where present (grand-theater, the two out-of-africa files) — they are experiment levers the saga uses. Keep `[climate]`, `[world_map]`, sizes, caps, founders. In each header comment replace "Flags on: …" style text with "Runs the full stack (see docs/scenarios.md); opt-outs: none."

- [ ] **Step 2: Write `scenarios/predator-prey.toml`** (absorbs trophic-cascade, grazers-and-wolves, mammals-vs-reptiles, foraging-selection, biome-adaptation)

```toml
# Predator / prey: a grazer herd and its stalkers at the centre, a mammal
# grazer/pursuer pair to the north, a reptile ambusher/basker pair to the
# east, and a thin background of unspecialised foragers. Shows predation
# cycles and population crashes (PopCrash), the trophic cascade when the
# predators collapse, mammal-vs-reptile niche sorting on the drier east, and
# foraging / climate-affinity selection in the background stock. Runs the
# full stack. Absorbed: trophic-cascade, grazers-and-wolves,
# mammals-vs-reptiles, foraging-selection, biome-adaptation.
name = "predator-prey"
seed = 0
max_population = 2000

[climate]
moisture_bias = -0.1

[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 60.0 }
max_share = 0.25
[agents.traits]
size = 0.5
lifespan_bias = 1.0

[[agents]]
count = 10
archetype = "stalker"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 60.0 }
max_share = 0.08
[agents.traits]
size = 0.7
lifespan_bias = 1.0

[[agents]]
count = 80
archetype = "mammal_grazer"
placement = { kind = "cluster", center_x = 340.0, center_y = 300.0, radius = 70.0 }
max_share = 0.2
[agents.traits]
size = 0.4
lifespan_bias = 1.0
env_affinity = 0.5
reproduction_threshold = 0.5
mutation_rate = 0.3

[[agents]]
count = 14
archetype = "mammal_pursuer"
placement = { kind = "cluster", center_x = 340.0, center_y = 200.0, radius = 50.0 }
max_share = 0.06
[agents.traits]
size = 0.65
lifespan_bias = 1.0
env_affinity = 0.5
reproduction_threshold = 0.4

[[agents]]
count = 80
archetype = "reptile_basker"
placement = { kind = "cluster", center_x = 684.0, center_y = 340.0, radius = 70.0 }
max_share = 0.2
[agents.traits]
size = 0.55
lifespan_bias = 1.0
env_affinity = 0.8

[[agents]]
count = 24
archetype = "reptile_ambusher"
placement = { kind = "cluster", center_x = 684.0, center_y = 512.0, radius = 50.0 }
max_share = 0.06
[agents.traits]
size = 0.7
lifespan_bias = 1.0
env_affinity = 0.8

[[agents]]
count = 150
placement = { kind = "uniform" }
max_share = 0.15
[agents.traits]
lifespan_bias = 0.6
reproduction_threshold = 0.5
```

Before writing, open the old `predator-prey.toml` and copy its exact `radius` values for the two centre clusters (the summary above truncated them); use those instead of `60.0` if they differ.

- [ ] **Step 3: Write `scenarios/speciation.toml`** (absorbs divergent, convergent, cooperation, territories, dialects, gene-culture ×4)

```toml
# Speciation: one herbivore stock seeded as two body-size morphs at the west
# and east ends of the map, with a communicator, a scent-marker, a
# cooperator and a hunter-prey lineage folded in. Shows speciation from a
# founder stock (Divergence / Convergence events), dialect formation between
# the two communicator clusters, pheromone territories between the marker
# clusters, kin cooperation, and the gene-culture pairings (cultural vs
# asocial foragers, skilled foragers, fast vs slow hunters, cultured vs
# asocial prey). Runs the full stack. Absorbed: divergent, convergent,
# cooperation, territories, dialects, gene-culture, gene-culture-skill,
# gene-culture-hunt, gene-culture-alarm.
name = "speciation"
seed = 4242
max_population = 2000

[[agents]]
count = 60
placement = { kind = "cluster", center_x = 212.0, center_y = 512.0, radius = 60.0 }
max_share = 0.12
[agents.traits]
size = 0.2
basal_metabolism = 0.2
lifespan_bias = 0.7
reproduction_threshold = 0.4

[[agents]]
count = 60
placement = { kind = "cluster", center_x = 812.0, center_y = 512.0, radius = 60.0 }
max_share = 0.12
[agents.traits]
size = 0.8
basal_metabolism = 0.8
lifespan_bias = 0.7
reproduction_threshold = 0.4

[[agents]]
count = 24
archetype = "communicator"
placement = { kind = "cluster", center_x = 260.0, center_y = 300.0, radius = 40.0 }
max_share = 0.08
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 24
archetype = "communicator"
placement = { kind = "cluster", center_x = 764.0, center_y = 300.0, radius = 40.0 }
max_share = 0.08
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 30
archetype = "marker"
placement = { kind = "cluster", center_x = 300.0, center_y = 760.0, radius = 40.0 }
max_share = 0.06
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 30
archetype = "marker"
placement = { kind = "cluster", center_x = 720.0, center_y = 760.0, radius = 40.0 }
max_share = 0.06
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 30
archetype = "cultural_cooperator"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 40.0 }
max_share = 0.08
[agents.traits]
altruism = 1.0
basal_metabolism = 0.7
lifespan_bias = 1.0

[[agents]]
count = 30
archetype = "asocial_forager"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 40.0 }
max_share = 0.08
[agents.traits]
altruism = 0.0
basal_metabolism = 0.7
lifespan_bias = 1.0

[[agents]]
count = 40
archetype = "skilled_forager"
placement = { kind = "cluster", center_x = 512.0, center_y = 200.0, radius = 40.0 }
max_share = 0.08
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 40
archetype = "culture_prey"
placement = { kind = "cluster", center_x = 512.0, center_y = 820.0, radius = 40.0 }
max_share = 0.08
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 40
archetype = "asocial_prey"
placement = { kind = "cluster", center_x = 512.0, center_y = 820.0, radius = 40.0 }
max_share = 0.08
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 16
archetype = "fast_hunter"
placement = { kind = "cluster", center_x = 560.0, center_y = 820.0, radius = 30.0 }
max_share = 0.04
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 16
archetype = "slow_hunter"
placement = { kind = "cluster", center_x = 464.0, center_y = 820.0, radius = 30.0 }
max_share = 0.04
[agents.traits]
lifespan_bias = 1.0
```

- [ ] **Step 4: Write `scenarios/tribes.toml`** (absorbs inventions, tool-users, weapons, weapons-arena, weapons-arms-race, war, traditions, cognitive-coevolution, tech-gene-coupling, knowledge-ratchet, domestication, dimorphism, disease, anthro-race, affect ×5, basic-needs)

```toml
# Tribes: ape-tier omnivore lineages — innovators, traditionalists, a
# culture-bearing hunter band seeded with stone tools — among grazer herds,
# armoured and spined prey, and predator packs. Shows discovery and adoption
# of inventions (Discovery / Adoption / KnowledgeRatchet), traditions,
# IQ-tech coevolution, weapons and war (War / WarEnded), domestication pens,
# sexual dimorphism, epidemics (Epidemic / MedContain), the anthropogenic arms
# race, the affect layer's moods and play, and thirst / sleep. Runs the full
# stack. Absorbed: inventions, tool-users, weapons, weapons-arena,
# weapons-arms-race, war, traditions, cognitive-coevolution,
# tech-gene-coupling, knowledge-ratchet, domestication, dimorphism, disease,
# anthro-race, affect-play, affect-seeking, affect-showcase, affect-social,
# affect-threat, basic-needs.
name = "tribes"
seed = 60623
max_population = 1500

[[agents]]
count = 30
archetype = "innovator"
placement = { kind = "cluster", center_x = 340.0, center_y = 512.0, radius = 40.0 }
max_share = 0.12
starting_inventions = ["stone_tools"]
[agents.traits]
altruism = 0.3
basal_metabolism = 0.6
lifespan_bias = 1.0
openness = 1.0

[[agents]]
count = 30
archetype = "traditionalist"
placement = { kind = "cluster", center_x = 684.0, center_y = 512.0, radius = 40.0 }
max_share = 0.12
[agents.traits]
altruism = 0.3
basal_metabolism = 0.6
lifespan_bias = 1.0
openness = 0.05

[[agents]]
count = 24
archetype = "ape_hunter"
placement = { kind = "cluster", center_x = 400.0, center_y = 300.0, radius = 40.0 }
max_share = 0.1
starting_inventions = ["stone_tools"]
culture_bearer = true
[agents.traits]
altruism = 0.3
basal_metabolism = 0.6
lifespan_bias = 1.0

[[agents]]
count = 20
archetype = "asocial_forager"
placement = { kind = "cluster", center_x = 512.0, center_y = 300.0, radius = 40.0 }
max_share = 0.06
[agents.traits]
altruism = 0.0
basal_metabolism = 0.6
lifespan_bias = 1.0

[[agents]]
count = 30
archetype = "communicator"
placement = { kind = "cluster", center_x = 430.0, center_y = 700.0, radius = 40.0 }
max_share = 0.08
[agents.traits]
lifespan_bias = 1.0
openness = 1.0

[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "cluster", center_x = 620.0, center_y = 700.0, radius = 60.0 }
max_share = 0.14
[agents.traits]
size = 0.5
lifespan_bias = 1.0
vigilance = 0.5

[[agents]]
count = 24
archetype = "herd"
placement = { kind = "cluster", center_x = 700.0, center_y = 340.0, radius = 40.0 }
max_share = 0.06
[agents.traits]
size = 0.55
lifespan_bias = 1.0

[[agents]]
count = 20
placement = { kind = "cluster", center_x = 472.0, center_y = 512.0, radius = 30.0 }
max_share = 0.05
[agents.traits]
size = 0.5
lifespan_bias = 1.0
sexual_dimorphism = 0.3
mate_choosiness = 0.6

[[agents]]
count = 20
placement = { kind = "cluster", center_x = 552.0, center_y = 512.0, radius = 30.0 }
max_share = 0.05
[agents.traits]
size = 0.5
lifespan_bias = 1.0
sexual_dimorphism = 0.7
mate_choosiness = 0.6

[[agents]]
count = 10
archetype = "stalker"
placement = { kind = "cluster", center_x = 620.0, center_y = 700.0, radius = 60.0 }
max_share = 0.04
[agents.traits]
size = 0.7
lifespan_bias = 1.0

[[agents]]
count = 12
archetype = "pack_hunter"
placement = { kind = "cluster", center_x = 330.0, center_y = 700.0, radius = 40.0 }
max_share = 0.04
[agents.traits]
size = 0.7
lifespan_bias = 1.0

[[agents]]
count = 12
archetype = "fast_hunter"
placement = { kind = "cluster", center_x = 594.0, center_y = 512.0, radius = 30.0 }
max_share = 0.04
[agents.traits]
size = 0.65
lifespan_bias = 1.0
openness = 0.9

[[agents]]
count = 12
archetype = "spiner"
placement = { kind = "cluster", center_x = 700.0, center_y = 340.0, radius = 40.0 }
max_share = 0.04
[agents.traits]
size = 0.6
lifespan_bias = 1.0

[[agents]]
count = 12
archetype = "bruiser"
placement = { kind = "cluster", center_x = 330.0, center_y = 700.0, radius = 40.0 }
max_share = 0.04
[agents.traits]
size = 0.8
lifespan_bias = 1.0
```

- [ ] **Step 5: Write `scenarios/markets.toml`** (absorbs settlement, biome-trade, geographic-trade, trade-hubs, unilateral-trade)

First read the five `[[agents]]` blocks of the old `biome-trade.toml`: they differ in a per-lineage trait or `goods` key the summary above did not print. Copy those five blocks verbatim (halving each `count` to 160) into the file below where marked, then delete the old file.

```toml
# Markets: four terrain-affinity forager lineages founded at a biome junction
# (the settlement / geographic-trade founders) beside five goods-producing
# grazer lineages (the biome-trade founders). Shows home-range anchoring and
# settlements, resource harvesting, bilateral barter at the predetermined
# trade hubs, and the trade-flow / market events. Runs the full stack;
# `unilateral_trade` stays off (experiment lever) and is exercised by an
# inline fixture in tests/trade.rs. Absorbed: settlement, biome-trade,
# geographic-trade, trade-hubs, unilateral-trade.
name = "markets"
seed = 424242
max_population = 2200

[[agents]]
count = 156
archetype = "asocial_forager"
placement = { kind = "cluster", center_x = 816.0, center_y = 336.0, radius = 90.0 }
max_share = 0.14
[agents.traits]
reproduction_threshold = 0.2
terrain_affinity = 0.12

[[agents]]
count = 33
archetype = "asocial_forager"
placement = { kind = "cluster", center_x = 816.0, center_y = 336.0, radius = 90.0 }
max_share = 0.06
[agents.traits]
reproduction_threshold = 0.2
terrain_affinity = 0.37

[[agents]]
count = 98
archetype = "asocial_forager"
placement = { kind = "cluster", center_x = 816.0, center_y = 336.0, radius = 90.0 }
max_share = 0.1
[agents.traits]
reproduction_threshold = 0.2
terrain_affinity = 0.62

[[agents]]
count = 195
archetype = "asocial_forager"
placement = { kind = "cluster", center_x = 816.0, center_y = 336.0, radius = 90.0 }
max_share = 0.16
[agents.traits]
reproduction_threshold = 0.2
terrain_affinity = 0.87

# --- the five biome-trade goods lineages go here, copied verbatim from the
# --- old biome-trade.toml with count = 160 and max_share = 0.1 each ---
```

Copy the `radius` of the old settlement clusters too (replace `90.0` with the real value).

- [ ] **Step 6: Write `scenarios/sandbox.toml`** (absorbs sandbox-large, sandbox-coevolution, living-sandbox-coevolution)

```toml
# Sandbox: a 2048-unit world with an 8k cap and no staging — innovators,
# asocial foragers, cultural and skilled foragers, and two predator kinds
# scattered uniformly. The open-ended run for watching the tech tree, gene vs
# culture and long ecological cycles at scale. Runs the full stack.
# Absorbed: sandbox-large, sandbox-coevolution, living-sandbox-coevolution.
name = "sandbox"
seed = 7
world_size = 2048.0
biome_res = 256
hash_res = 128
season_period = 2500
max_population = 8000

[[agents]]
count = 500
archetype = "innovator"
placement = { kind = "uniform" }
max_share = 0.25

[[agents]]
count = 700
archetype = "asocial_forager"
placement = { kind = "uniform" }
max_share = 0.3

[[agents]]
count = 300
archetype = "cultural_forager"
placement = { kind = "uniform" }
max_share = 0.15

[[agents]]
count = 200
archetype = "skilled_forager"
placement = { kind = "uniform" }
max_share = 0.12

[[agents]]
count = 100
archetype = "stalker"
placement = { kind = "uniform" }
max_share = 0.1

[[agents]]
count = 50
archetype = "pack_hunter"
placement = { kind = "uniform" }
max_share = 0.08
```

- [ ] **Step 7: Fold `continental` into `riverlands`, `out-of-africa` into the saga**

`riverlands.toml` already has continental's climate block (`continentality`, `mountain_uplift`, `rain_shadow`, `river_threshold`, `sea_level`); add continental's founder stock as a third lineage: `count = 120`, no archetype, `placement = { kind = "cluster", center_x = 1492.0, center_y = 1020.0, radius = <copy from continental.toml> }`, `max_share = 0.05`, traits `lifespan_bias = 0.6`, `reproduction_threshold = 0.5`. Update its header to name continental as absorbed.

`out-of-africa-saga.toml` and `out-of-africa.toml` share founders except the saga's innovator block carries `starting_inventions`; the saga is the superset. Delete `out-of-africa.toml`; the saga's header names it as absorbed.

- [ ] **Step 8: Delete the rest**

```bash
cd scenarios
git rm -q affect-play.toml affect-seeking.toml affect-showcase.toml affect-social.toml affect-threat.toml anthro-race.toml basic-needs.toml biome-adaptation.toml biome-trade.toml cognitive-coevolution.toml continental.toml convergent.toml cooperation.toml dialects.toml dimorphism.toml disease.toml disturbance.toml divergent.toml domestication.toml foraging-selection.toml gene-culture-alarm.toml gene-culture-hunt.toml gene-culture-skill.toml gene-culture.toml geographic-trade.toml grazers-and-wolves.toml inventions.toml knowledge-ratchet.toml living-sandbox-coevolution.toml mammals-vs-reptiles.toml out-of-africa.toml sandbox-coevolution.toml sandbox-large.toml settlement.toml tech-gene-coupling.toml territories.toml tool-users.toml trade-hubs.toml traditions.toml trophic-cascade.toml unilateral-trade.toml war.toml weapons-arena.toml weapons-arms-race.toml weapons.toml
git rm -rq experiments
cd ..
ls scenarios   # expect the twelve .toml files + decks/
```

Before deleting, copy the TOML of every file that Task 5 names as a fixture into `crates/anabios-core/tests/common/` as `<name>.pre-flip.toml` (the list is in Task 5, Step 1) — do this copy first, in the same commit.

- [ ] **Step 9: Every world parses, instantiates and runs**

Run: `cargo test -p anabios-core --release --test all_scenarios every_scenario_parses_instantiates_and_runs -- --nocapture 2>&1 | tail -20`
Expected: PASS over exactly twelve files. A parse error means a knob line or a trait name is wrong; an instantiate panic in `nearest_valid` relocation means a founder cluster sits where its class has no habitat (move the cluster).

- [ ] **Step 10: Commit**

```bash
git add scenarios crates/anabios-core/tests/common/*.pre-flip.toml
git commit -m "feat(scenarios): twelve worlds by setting; retire the one-feature demos and experiments

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Menu, deck pins, atlas manifest, workflow references

**Files:**
- Modify: `game/scripts/menu.gd` (`SCENARIOS` array, lines 6–~230)
- Modify: `game/showcase/dialects.json`, `game/showcase/inventions.json` (`_comment` pin + file rename)
- Modify: `web/scripts/manifest.mjs` (flags → opt-outs)
- Modify: `.github/workflows/ci.yml` (nothing to change: `minimal`, `predator-prey` survive — verify only)
- Modify: `docs/scenarios.md` — Task 6

- [ ] **Step 1: Rewrite the Godot menu list**

Replace the whole `SCENARIOS` constant with these twelve entries. The `ground`/`body` modes are copied from the entries they replace (predator-prey 0/2, dialects 1/1 → speciation, inventions 0/1 → tribes, settlement 7/0 → markets, habitat 8/0, the grand and sandbox entries 0/0; riverlands and huge-steppe had no entry and take 0/0):

```gdscript
const SCENARIOS: Array[Dictionary] = [
	# Foundations
	{"label": "Minimal — 200 herbivores", "path": "res://../scenarios/minimal.toml", "ground": 0, "body": 0},
	{
		"label": "Predator / prey — cycles & cascades",
		"path": "res://../scenarios/predator-prey.toml",
		"ground": 0,
		"body": 2
	},
	{
		"label": "Speciation — morphs, dialects, kin",
		"path": "res://../scenarios/speciation.toml",
		"ground": 1,
		"body": 1
	},
	# Worlds
	{"label": "Tribes — tools, war, traditions", "path": "res://../scenarios/tribes.toml", "ground": 0, "body": 1},
	{"label": "Markets — settlements & trade", "path": "res://../scenarios/markets.toml", "ground": 7, "body": 0},
	{
		"label": "Habitat — land / sea / air territories",
		"path": "res://../scenarios/habitat-territories.toml",
		"ground": 8,
		"body": 0
	},
	{
		"label": "Grand theater — staged emergence",
		"path": "res://../scenarios/grand-theater.toml",
		"ground": 0,
		"body": 0
	},
	{
		"label": "Out of Africa — the saga",
		"path": "res://../scenarios/out-of-africa-saga.toml",
		"ground": 0,
		"body": 0
	},
	{
		"label": "Out of Africa — Earth (4096)",
		"path": "res://../scenarios/out-of-africa-earth.toml",
		"ground": 0,
		"body": 0
	},
	# Scale
	{"label": "Sandbox — 2048², 8k cap", "path": "res://../scenarios/sandbox.toml", "ground": 0, "body": 0},
	{"label": "Riverlands — 4096 continents", "path": "res://../scenarios/riverlands.toml", "ground": 0, "body": 0},
	{"label": "Huge steppe — 8192 scale test", "path": "res://../scenarios/huge-steppe.toml", "ground": 0, "body": 0},
]
```

(`gdformat` decides the one-line vs multi-line form; write them however it reformats them.)

Run: `gdformat game/scripts/menu.gd && gdlint game/scripts/menu.gd`
Expected: formatted, no problems.

- [ ] **Step 2: Re-pin the decks**

`git mv game/showcase/dialects.json game/showcase/speciation.json` and inside it change the `_comment` to contain `scenario=speciation`; `git mv game/showcase/inventions.json game/showcase/tribes.json` with `scenario=tribes`. Leave `predator-prey.json`, `out-of-africa-saga.json` and the `smoke-*` decks. Keep each deck's `seed` unless the validation sweep (Task 7) shows the pinned seed no longer produces the deck's chapters, in which case pick the best seed from the sweep and note it in the `_comment`.

Run: `cargo test -p anabios-core --release --test deck_scenarios 2>&1 | grep -E 'test result|panicked'`
Expected: PASS (every deck's pinned scenario resolves and survives 200 ticks).

- [ ] **Step 3: Atlas manifest shows opt-outs**

In `web/scripts/manifest.mjs` replace the `flags` computation:

```js
  // The schema turns every feature on; what a world OPTS OUT of is the
  // informative list. `= false` for a feature knob, or a lever the world
  // deliberately turns on (env_period / climate_drift_rate > 0).
  const off = [...top.matchAll(/^([a-z_]+_enabled|living_biome|terrain_habitat|biome_adaptation|nutrient_variation|soil_fertility|gene_tech_coupling|gene_requirements|conserve_goods_on_death|repro_biased_learning)\s*=\s*false/gm)]
    .map((m) => m[1].replace(/_enabled$/, ""));
  if (get("season_period") === "0") off.push("season");
  const levers = [];
  for (const k of ["env_period"]) if (Number(get(k)) > 0) levers.push("env");
  if (Number(get("climate_drift_rate")) > 0) levers.push("drift");
  for (const k of ["payoff_biased_learning", "unilateral_trade"]) if (get(k) === "true") levers.push(k);
```

and in the pushed object replace `flags,` with `off, levers,`. Then grep `web/src` for `.flags` and switch the chip renderer to show `off` (label "off: …") and `levers` (label "levers: …"); if nothing reads `flags`, only the manifest changes.

Run: `scripts/web.sh build && node -e 'const m=require("./web/scenarios/index.json"); console.log(m.scenarios?m.scenarios.length:m.length)'`
Expected: `12`.

- [ ] **Step 4: Workflow references**

`grep -n 'scenarios/' .github/workflows/*.yml` must list only `minimal`, `predator-prey`, `out-of-africa-saga` — all survivors. No change expected; if a deleted name appears, replace it with the absorbing world.

- [ ] **Step 5: Godot smoke**

Run: `cargo build -p anabios-godot && scripts/godot-smoke.sh scenes res://scenes/menu.tscn res://scenes/main.tscn 2>&1 | grep -iE 'SCRIPT ERROR|Parse Error' ; echo rc=$?`
Expected: no `SCRIPT ERROR` lines (grep rc=1 is the good outcome).

- [ ] **Step 6: Commit**

```bash
git add game/scripts/menu.gd game/showcase web/scripts/manifest.mjs
git commit -m "feat(viewer,web): twelve-world menu, deck pins, atlas manifest lists opt-outs

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Re-target the test suites, add fixtures, re-pin the goldens

**Files:**
- Modify: `crates/anabios-core/tests/common/fixtures.rs` (retired-file fixtures)
- Create: `crates/anabios-core/tests/common/<name>.pre-flip.toml` for each fixture (copied in Task 3 Step 8)
- Modify: every suite in the table below
- Modify: `crates/anabios-core/tests/determinism.rs` (goldens), `crates/anabios-core/tests/save_load_roundtrip.rs` (table)

**Interfaces:**
- Consumes: `common::fixtures::with_opt_outs`, the twelve world files.
- Produces: `common::fixtures::<NAME>_PRE_FLIP: &str` constants (verbatim retired files) and `common::fixtures::<name>_flag_off() -> String` helpers for each fixture the suites use.

**Rule of the task (spec §3):** for each suite, first point it at the absorbing world and run it. If every test passes, done. If a test fails because the phenomenon it asserts does not occur in the world within the suite's horizon (not because of a broken reference), give that test the retired file as an inline fixture via `with_opt_outs(<NAME>_PRE_FLIP)` — the fixture reproduces the exact pre-flip configuration — and leave a comment naming the absorbing world and the reason. A test that fails for a mechanical reason (renamed file, changed founder count in an assertion) is fixed mechanically.

- [ ] **Step 1: Fixture constants**

For each retired file the suites may need, add to `fixtures.rs` a verbatim include and a helper, following this pattern:

```rust
pub const INVENTIONS_PRE_FLIP: &str = include_str!("inventions.pre-flip.toml");
/// `inventions.toml` as it was: 64 agents, only `inventions_enabled` on.
pub fn inventions_flag_off() -> String {
    with_opt_outs(INVENTIONS_PRE_FLIP)
}
```

Provide the pair for: `inventions`, `knowledge-ratchet`, `cognitive-coevolution`, `tech-gene-coupling`, `domestication`, `disease`, `dimorphism`, `war`, `weapons-arms-race`, `traditions`, `tool-users`, `dialects`, `territories`, `cooperation`, `convergent`, `divergent`, `gene-culture`, `gene-culture-skill`, `gene-culture-hunt`, `gene-culture-alarm`, `trophic-cascade`, `disturbance`, `mammals-vs-reptiles`, `grazers-and-wolves`, `basic-needs`, `affect-play`, `affect-seeking`, `affect-showcase`, `affect-social`, `affect-threat`, `anthro-race`, `biome-trade`, `unilateral-trade`, `geographic-trade`, `settlement`, `biome-adaptation`, `foraging-selection`, `continental`, `sandbox-large`, `living-sandbox-coevolution`, and the seven experiments `biome-step-interval`, `dit-env-slow`, `drifting-climate`, `gene-requirements`, `o1-lever-practices-off`, `o2-payoff-biased-learning`, `o3-repro-biased-learning`. (Task 3 Step 8 copies these 47 files to `tests/common/*.pre-flip.toml` before deleting them; a fixture that ends up unused after this task is deleted again together with its `.pre-flip.toml`, so only fixtures a suite actually reads survive.)

Note `with_opt_outs` keeps a file's explicit flags, so an experiment fixture such as `o2-payoff-biased-learning` still has its 17 flags on and only the rest off — exactly its pre-flip meaning.

- [ ] **Step 2: Suite-by-suite re-targeting**

Apply the rule of the task to each row; the "world" column is the first thing to try.

| Suite | Old scenario(s) | World to try | Notes |
|---|---|---|---|
| `affect.rs` | affect-seeking, affect-threat | `minimal` (seeking), `tribes` (threat) | seeking's 200-uniform world is minimal's founders |
| `affect_play.rs` | affect-play | `tribes` | play needs juveniles; fixture likely |
| `affect_social.rs` | affect-social | `minimal` | |
| `all_scenarios.rs` | living-sandbox-coevolution, inventions | `sandbox`, `tribes` | rename `living_sandbox_smoke` → `sandbox_smoke`, `inventions_scenario_smoke` → `tribes_smoke` |
| `basic_needs.rs` | basic-needs | fixture (`basic_needs_flag_off`) | needs `climate.river_threshold = 60` on the 1024 map; no world has it |
| `cognition.rs`, `cognition_evolution.rs` | cognitive-coevolution | `tribes` | |
| `determinism.rs` | minimal, habitat, affect ×4, tech-gene-coupling, grand-theater | see Step 3 | |
| `dimorphism.rs` | dimorphism, minimal | `tribes` (two dimorphic morphs are in it), `minimal` | |
| `dims.rs` | minimal | `minimal` | |
| `disease.rs` | disease | `tribes` | |
| `domestication.rs` | domestication, inventions | `tribes` | |
| `emergence.rs` | convergent, cooperation, dialects, territories → `speciation`; disturbance, mammals-vs-reptiles, predator-prey, trophic-cascade → `predator-prey`; settlement → `markets`; tech-gene-coupling, tool-users, traditions, war, weapons-arms-race → `tribes`; minimal → `minimal` | | 17 tests; expect several fixtures |
| `gene_culture.rs` | gene-culture ×4 | `speciation` | |
| `grazers_wolves.rs` | grazers-and-wolves | `predator-prey` | |
| `invariants.rs` | habitat-territories | unchanged | |
| `inventions.rs` | inventions | `tribes` | 41 tests tuned to a 64-agent world; most will take the fixture |
| `knowledge.rs` | inventions, knowledge-ratchet | `tribes` | ratchet needs Writing seeded; fixture likely |
| `living_sandbox.rs` | living-sandbox-coevolution | `sandbox` | |
| `save_load_roundtrip.rs` | 34 files | see Step 4 | |
| `snapshot_size.rs` | huge-steppe | `huge-steppe` | |
| `substrate.rs` | affect-showcase, divergent, domestication, minimal, sandbox-large | `tribes`, `speciation`, `tribes`, `minimal`, `sandbox` | |
| `trade.rs` | biome-trade, geographic-trade, minimal, unilateral-trade | `markets`, `markets`, `minimal`, fixture (`unilateral_trade_flag_off`) | unilateral is a lever kept off |
| `worldgen.rs` | biome-adaptation, continental, foraging-selection, minimal | `predator-prey`, `riverlands`, `predator-prey`, `minimal` | worldgen asserts are about the map; continental's climate lives in riverlands |
| `deck_scenarios.rs` | deck pins | done in Task 4 | |
| `dit_boundary.rs` | (builds worlds programmatically) | unchanged | |

For each suite, after editing, run `cargo test -p anabios-core --release --test <suite> 2>&1 | grep -E '^test |test result|panicked'` and record in the commit message which tests moved to a fixture.

- [ ] **Step 3: `determinism.rs`**

- The thread-count identity test iterates scenario sources; replace the list with `minimal`, `tribes`, `habitat-territories`, `grand-theater` (all full-stack now) and keep 300 ticks.
- `GOLDEN` (minimal) and `HABITAT_GOLDEN`: regenerate after Steps 2 and 4 are green:

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --release --test determinism golden -- --nocapture 2>&1 | grep -E '\(0,|\(100,|\(1000,'`
Paste the printed triples into the two constants and add a dated comment: "Re-pinned 2026-09-26: the scenario schema now defaults every feature on; the flag-off engine is pinned separately by the `*_trajectory_is_pinned` guards, which did not move."
- The goldens in `affect.rs` / `affect_play.rs` / `affect_social.rs` / `cognition.rs` / `inventions.rs` (each has a `common::assert_golden` call) are re-pinned the same way with `UPDATE_HASHES=1 cargo test -p anabios-core --release --test <suite> golden -- --nocapture`, after their scenario source is settled (fixture or world).

- [ ] **Step 4: `save_load_roundtrip.rs` table**

Rewrite the `roundtrip_tests!` table to one row per world (twelve rows; keep each old row's warm-up tick count and flag predicate where the world still has that flag — e.g. `"../../../scenarios/tribes.toml", 420, |w| w.inventions_enabled && w.disease_enabled, "tribes"`), plus one row per experiment fixture using the fixture helper. Since the macro takes a `&str` source, add a second macro arm or a small loop for `String` sources:

```rust
#[test]
fn experiment_fixtures_roundtrip() {
    for (src, warm, what) in [
        (common::fixtures::biome_step_interval_flag_off(), 120, "biome_step_interval"),
        (common::fixtures::dit_env_slow_flag_off(), 100, "dit-env-slow"),
        (common::fixtures::drifting_climate_flag_off(), 100, "drifting-climate"),
        (common::fixtures::gene_requirements_flag_off(), 300, "gene-requirements"),
        (common::fixtures::o1_lever_practices_off_flag_off(), 300, "o1-lever-practices-off"),
        (common::fixtures::o2_payoff_biased_learning_flag_off(), 300, "o2-payoff-biased-learning"),
        (common::fixtures::o3_repro_biased_learning_flag_off(), 300, "o3-repro-biased-learning"),
    ] {
        let mut w = common::world(&src);
        common::run(&mut w, common::ticks(warm));
        common::assert_roundtrip_world(&mut w, what);
    }
}
```

(Copy each old row's warm-up value from the current table instead of the numbers above where they differ.)

Run: `cargo test -p anabios-core --release --test save_load_roundtrip 2>&1 | grep -E 'test result|panicked'`
Expected: PASS.

- [ ] **Step 5: Full gate**

Run, in order: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items`; `cargo test --workspace --lib`; `cargo test -p anabios-core --release` (the whole integration set; expect ~10 min); `RUSTFLAGS="--cfg coverage" cargo check -p anabios-core --tests`; `cargo bench --workspace --no-run`.
Expected: everything green. Delete any fixture (and its `.pre-flip.toml`) no suite references: `grep -L` each `_PRE_FLIP` name across `tests/`.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/tests
git commit -m "test: re-target the suites to the twelve worlds; fixtures for retired configurations; goldens re-pinned

<one line per suite: world or fixture, and why>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Replay, docs, memory

**Files:**
- Modify: `showcase/replay.js` (re-recorded)
- Modify: `docs/scenarios.md` (rewrite)
- Modify: memory `anabios-*` notes (outside the repo)

- [ ] **Step 1: Re-record the saga replay**

Run: `cargo build --release -p anabios-headless && ./target/release/anabios-headless record --scenario scenarios/out-of-africa-saga.toml --seed 318 --out showcase/replay.js | tee /tmp/record.log && grep -o 'state_hash=[^ ]*' /tmp/record.log`
Expected: a new `showcase/replay.js` and a `state_hash=0x…` line; keep the hash for the PR body. If seed 318 no longer yields the deck's chapters (Task 7 decides), record with the chosen seed and update `.github/workflows/showcase.yml` line 67 (`--seed 318`) and `game/showcase/out-of-africa-saga.json`'s pin to match.

Run: `scripts/web.sh test out-of-africa-saga 300 318 2>&1 | tail -3`
Expected: native and wasm fingerprints equal.

- [ ] **Step 2: Rewrite `docs/scenarios.md`**

Replace the file with: the intro (twelve worlds by setting; every file runs the full stack; how to opt out; the four levers), a twelve-row table `| World | Setting | Shows | Opt-outs / levers | Seed |`, a "Running" section (`scripts/emergence.sh view|run|sweep <world>`), and a "Retired experiments" appendix listing the seven experiment fixtures and the O1–O3, DIT and worldgen findings docs they came from, with the sentence "The retired one-feature demos live on as inline fixtures under `crates/anabios-core/tests/common/` where a suite still needs their exact configuration." Keep the phenomenon wording from the old rows for the absorbed files (the atlas manifest reads this file's table for its descriptions — keep the `` `world.toml` `` back-tick form in the first column).

Run: `node web/scripts/manifest.mjs && python3 -c "import json;d=json.load(open('web/scenarios/index.json'));print([s['description'][:40] for s in d['scenarios']])"`
Expected: twelve non-empty descriptions.

- [ ] **Step 3: Memory**

Update `~/.claude/projects/-Users-aryasen-projects-anabios/memory/anabios-showcase-decks.md` (deck pins now `speciation`/`tribes`), add a `anabios-scenario-consolidation.md` note (schema-default-on vs engine-off, the twelve worlds, where retired configs live), and index both in `MEMORY.md`.

- [ ] **Step 4: Commit**

```bash
git add showcase/replay.js docs/scenarios.md
git commit -m "docs(scenarios): twelve worlds; saga replay re-recorded

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Validation sweeps and founder tuning

**Files:**
- Modify: any of the twelve `scenarios/*.toml` (founders, `max_share`, `max_population`, placement only)
- Modify: `docs/scenarios.md` (per-world cost row; gaps)

**Interfaces:**
- Consumes: `./target/release/anabios-headless sweep --scenario <file> --seeds 8 --ticks <horizon> --out <dir>` (scorecard per seed) and `run --scenario <file> --ticks <n> --seed <s>` (one tally).

- [ ] **Step 1: Sweep every world**

```bash
mkdir -p /tmp/sweeps
for w in minimal predator-prey speciation tribes markets habitat-territories grand-theater out-of-africa-saga sandbox riverlands; do
  ./target/release/anabios-headless sweep --scenario scenarios/$w.toml --seeds 8 --ticks 5000 --out /tmp/sweeps/$w
done
# out-of-africa-earth and huge-steppe: 4 seeds x 2000 ticks (they are slow)
for w in out-of-africa-earth huge-steppe; do
  ./target/release/anabios-headless sweep --scenario scenarios/$w.toml --seeds 4 --ticks 2000 --out /tmp/sweeps/$w
done
```

Read each scorecard (the `sweep` output directory; `scripts/emergence.sh sweep <world>` prints the same table) and fill a table `| world | seeds alive at horizon | founder kinds alive (majority?) | headline detectors fired (seeds/8) |`.

- [ ] **Step 2: Tune founders where a world fails the bar**

The bar (spec §5): the world's headline phenomena fire on ≥ 5/8 seeds, and on ≥ 5/8 seeds no founder kind is extinct and the population is above half the founder count. Levers, in order: `max_share` (a kind that dies is usually starved by the shared cap — raise its share), founder `count`, cluster placement (a Water-locked or Land-locked founder cluster from the territory layer — move it onto its habitat), `max_population`. Never touch an engine constant; if no founder change reaches the bar, record the gap in `docs/scenarios.md` ("does not reliably show X; the fixture test in `tests/<suite>.rs` covers it") and move on.

After every founder change: `cargo test -p anabios-core --release --test all_scenarios --test deck_scenarios --test determinism` — a founder change moves that world's golden and roundtrip; re-pin with `UPDATE_HASHES=1` only at the end of the tuning loop for that world.

- [ ] **Step 3: Cost**

```bash
for w in minimal predator-prey speciation tribes markets habitat-territories grand-theater out-of-africa-saga sandbox riverlands; do
  /usr/bin/time -p ./target/release/anabios-headless run --scenario scenarios/$w.toml --ticks 2000 2>&1 | grep -E '^real' | sed "s/^/$w /"
done
```

Record `ms/tick` (= real × 1000 / 2000) per world in `docs/scenarios.md`'s table. Also run `cargo bench -p anabios-core --bench tick_bench -- territory` once and note the `off`/`on` pair in the PR body as the 10k-agent reference.

- [ ] **Step 4: Viewer captures**

Run `scripts/emergence.sh capture out-of-africa-saga` and `scripts/emergence.sh capture habitat-territories` (windowed; see the `running-the-viewer` skill for the pause/minimised-window pitfalls) and attach the two PNGs to the PR.

- [ ] **Step 5: Commit and PR**

```bash
git add scenarios docs/scenarios.md crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/save_load_roundtrip.rs
git commit -m "feat(scenarios): founders tuned to the validation bar; per-world cost recorded

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
git push -u origin claude/scenario-consolidation
gh pr create --title "Scenarios: full stack by default, twelve worlds by setting" --body-file <the PR body written from the spec's sections 1–5, the sweep table, the cost table, the replay state_hash, and the gates run>
```

The PR body ends with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.

---

## Self-review

- **Spec coverage:** §1 defaults → Task 1 (+ Task 2 for the guards); §2 twelve worlds → Task 3; §3 tests rules 1–6 → Tasks 2, 5; §4 readers (menu, decks, manifest, replay, docs, README, workflows) → Tasks 4, 6; §5 validation → Task 7; §6 out of scope → Task 7 Step 2 forbids engine changes; §7 rollout order → Tasks 1–7 commit order. FORMAT_VERSION untouched: no task edits `snapshot.rs`.
- **Placeholders:** Task 5 Step 1 lists the 47 fixture names explicitly; Task 4 Step 1 carries the real menu modes; Task 3 Step 5 tells the implementer to copy the five biome-trade blocks verbatim rather than inventing them (the summary in this plan did not carry their distinguishing keys) — that is a read-and-copy instruction, not a gap.
- **Type consistency:** `common::fixtures::with_opt_outs(&str) -> String`, `<name>_flag_off() -> String`, `<NAME>_PRE_FLIP: &str`, `OPT_OUT_ALL: &str`, `default_season_period() -> u32` are used with those exact names and types in Tasks 2, 5 and 6; `assert_trajectory(label, src: &str, ticks, pinned)` takes `&String` via deref as written in Task 2.
