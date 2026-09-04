//! Save→load→step round-trip hardening: every opt-in subsystem must survive
//! serialization bit-identically, and the world must step identically after a
//! reload (the `#[serde(skip)]`-accumulator footgun — a skipped field that
//! feeds future ticks is invisible to `state_hash` yet breaks replay).
//!
//! One test per opt-in flag, warmed enough that the subsystem's state is
//! non-trivial before saving, each guarding that its scenario actually
//! enables the flag (so a scenario edit silently dropping it fails loudly).
//! These assert *self-consistency*, not pinned values — no golden hashes.
//! See `docs/determinism-contract.md` for the skip rules this suite enforces.

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;
use anabios_core::world::World;

fn roundtrip(src: &str, warm: u64, flag: fn(&World) -> bool, flag_name: &str) {
    let mut world = Scenario::parse_toml(src).expect("parse").instantiate();
    assert!(flag(&world), "scenario must enable {flag_name}");
    for _ in 0..warm {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "{flag_name}: load must restore identical state"
    );
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "{flag_name}: diverged after save→load→step — hidden non-serialized state feeding the sim?"
    );
}

macro_rules! roundtrip_tests {
    ($($name:ident: $file:literal, $warm:literal, $flag:expr, $flag_name:literal;)*) => {
        $(
            #[test]
            fn $name() {
                roundtrip(include_str!($file), $warm, $flag, $flag_name);
            }
        )*
    };
}

roundtrip_tests! {
    env_period_roundtrip:
        "../../../scenarios/experiments/dit-env-slow.toml", 300, |w: &World| w.env_period > 0, "env_period";
    biome_adaptation_roundtrip:
        "../../../scenarios/biome-adaptation.toml", 400, |w: &World| w.biome_adaptation, "biome_adaptation";
    terrain_habitat_roundtrip:
        "../../../scenarios/geographic-trade.toml", 400, |w: &World| w.terrain_habitat, "terrain_habitat";
    inventions_roundtrip:
        "../../../scenarios/inventions.toml", 500, |w: &World| w.inventions_enabled, "inventions_enabled";
    gene_requirements_roundtrip:
        "../../../scenarios/experiments/gene-requirements.toml", 500, |w: &World| w.gene_requirements, "gene_requirements";
    cognition_roundtrip:
        "../../../scenarios/cognitive-coevolution.toml", 400, |w: &World| w.cognition_enabled, "cognition_enabled";
    living_biome_roundtrip:
        "../../../scenarios/living-sandbox-coevolution.toml", 400, |w: &World| w.living_biome, "living_biome";
    season_period_roundtrip:
        "../../../scenarios/sandbox-large.toml", 300, |w: &World| w.season_period > 0, "season_period";
    climate_drift_roundtrip:
        "../../../scenarios/experiments/drifting-climate.toml", 400, |w: &World| w.climate_drift_rate > 0.0, "climate_drift_rate";
    nutrient_fertility_roundtrip:
        "../../../scenarios/foraging-selection.toml", 400, |w: &World| w.nutrient_variation && w.soil_fertility, "nutrient_variation+soil_fertility";
    resources_roundtrip:
        "../../../scenarios/biome-trade.toml", 400, |w: &World| w.resources_enabled, "resources_enabled";
    disasters_roundtrip:
        "../../../scenarios/disturbance.toml", 400, |w: &World| w.disasters_enabled, "disasters_enabled";
    war_roundtrip:
        "../../../scenarios/war.toml", 600, |w: &World| w.war_enabled, "war_enabled";
    settlement_roundtrip:
        "../../../scenarios/settlement.toml", 400, |w: &World| w.settlement_enabled, "settlement_enabled";
    dimorphism_roundtrip:
        "../../../scenarios/dimorphism.toml", 400, |w: &World| w.sexual_dimorphism_enabled, "sexual_dimorphism_enabled";
    domestication_roundtrip:
        "../../../scenarios/domestication.toml", 400, |w: &World| w.domestication_enabled, "domestication_enabled";
    knowledge_roundtrip:
        "../../../scenarios/knowledge-ratchet.toml", 300, |w: &World| w.knowledge_enabled, "knowledge_enabled";
    affect_roundtrip:
        "../../../scenarios/affect-social.toml", 300, |w: &World| w.affect_enabled, "affect_enabled";
    practices_roundtrip:
        "../../../scenarios/experiments/o1-lever-practices-off.toml", 300, |w: &World| !w.practices_enabled && w.cognition_enabled, "practices_enabled(off variant)";
    payoff_biased_learning_roundtrip:
        "../../../scenarios/experiments/o2-payoff-biased-learning.toml", 300, |w: &World| w.payoff_biased_learning, "payoff_biased_learning";
    repro_biased_learning_roundtrip:
        "../../../scenarios/experiments/o3-repro-biased-learning.toml", 300, |w: &World| w.repro_biased_learning, "repro_biased_learning";
    unilateral_trade_roundtrip:
        "../../../scenarios/unilateral-trade.toml", 400, |w: &World| w.unilateral_trade && w.conserve_goods_on_death, "unilateral_trade+conserve_goods_on_death";
    basic_needs_roundtrip:
        "../../../scenarios/basic-needs.toml", 600, |w: &World| w.basic_needs_enabled, "basic_needs_enabled";
}

/// The strongest single guard: grand-theater warms every subsystem at once.
#[test]
fn grand_theater_everything_on_roundtrip() {
    roundtrip(
        include_str!("../../../scenarios/grand-theater.toml"),
        300,
        |w: &World| {
            w.env_period > 0
                && w.climate_drift_rate > 0.0
                && w.season_period > 0
                && w.biome_adaptation
                && w.living_biome
                && w.nutrient_variation
                && w.soil_fertility
                && w.disasters_enabled
                && w.terrain_habitat
                && w.resources_enabled
                && w.settlement_enabled
                && w.inventions_enabled
                && w.gene_tech_coupling
                && w.cognition_enabled
                && w.war_enabled
        },
        "grand-theater(everything-on)",
    );
}
