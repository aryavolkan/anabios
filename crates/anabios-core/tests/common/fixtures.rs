//! Inline scenario fixtures. The scenario schema defaults every feature knob
//! ON; these keep the pre-flip configurations of files the suites depend on.
//! `OPT_OUT_ALL` lists every knob explicitly, so a fixture's meaning cannot
//! drift when a future knob is added — add it here too.
#![allow(dead_code)]

/// Every feature knob at its PRE-FLIP default: off for all of them, except
/// `practices_enabled`, which already defaulted on before the schema flip (a
/// fixture must replay the world as it was, and a file that never mentioned
/// the knob ran with practices on). Appended AFTER a file's own top-level
/// keys; TOML rejects duplicate keys, so a base that already sets one of
/// these must go through `with_opt_outs`, which drops the duplicates.
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
practices_enabled = true
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
    let set: std::collections::HashSet<&str> =
        base.lines().filter_map(|l| l.split('=').next()).map(str::trim).collect();
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
    with_opt_outs(include_str!("minimal.pre-flip.toml"))
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn with_opt_outs_covers_every_knob_and_keeps_explicit_values() {
        let s =
            with_opt_outs("name = \"t\"\nseed = 1\nwar_enabled = true\n[[agents]]\ncount = 1\n");
        assert!(s.contains("war_enabled = true") && !s.contains("war_enabled = false"));
        assert!(s.contains("territory_enabled = false"));
        assert!(s.find("territory_enabled").unwrap() < s.find("[[agents]]").unwrap());
        let parsed = anabios_core::scenario::Scenario::parse_toml(&s).expect("parses");
        assert!(parsed.war_enabled && !parsed.territory_enabled && parsed.season_period == 0);
        assert!(parsed.practices_enabled, "practices_enabled must replay its pre-flip on-default");
    }
}
