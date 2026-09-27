//! Inline scenario fixtures. The scenario schema defaults every feature knob
//! ON; these keep the pre-flip configurations of files the suites depend on.
//! `OPT_OUT_ALL` lists every knob explicitly, so a fixture's meaning cannot
//! drift when a future knob is added — add it to `opt-out-all.toml` too (the
//! guard test at the bottom fails until you do).
#![allow(dead_code)]

/// Every feature knob at its PRE-FLIP default: off for all of them, except
/// `practices_enabled`, which already defaulted on before the schema flip (a
/// fixture must replay the world as it was, and a file that never mentioned
/// the knob ran with practices on). Appended AFTER a file's own top-level
/// keys; TOML rejects duplicate keys, so a base that already sets one of
/// these must go through `with_opt_outs`, which drops the duplicates.
///
/// The list lives in `opt-out-all.toml` beside this file so the
/// `anabios-headless` unit tests (which cannot reach this module) read the
/// same list; `tests::opt_out_all_turns_off_every_default_on_knob` fails if
/// a knob the schema defaults on is missing from it.
pub const OPT_OUT_ALL: &str = include_str!("opt-out-all.toml");

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

// Retired scenario files, kept verbatim (`<name>.pre-flip.toml`) because a
// suite asserts something no world shows within its horizon, or round-trips a
// lever no world turns on. Each helper replays the file's exact pre-flip
// configuration: `with_opt_outs` keeps the file's own flags and puts every
// other knob at its pre-flip default. A fixture no suite reads is deleted
// with its `.pre-flip.toml`.

pub const BIOME_STEP_INTERVAL_PRE_FLIP: &str = include_str!("biome-step-interval.pre-flip.toml");
/// `experiments/biome-step-interval.toml` as it was: 200 founders; only
/// `living_biome` on, with `biome_step_interval = 4`.
pub fn biome_step_interval_flag_off() -> String {
    with_opt_outs(BIOME_STEP_INTERVAL_PRE_FLIP)
}

pub const DIMORPHISM_PRE_FLIP: &str = include_str!("dimorphism.pre-flip.toml");
/// `dimorphism.toml` as it was: two 30-agent dimorphic morphs (0.3 / 0.7)
/// and 8 stalkers; only `sexual_dimorphism_enabled` on.
pub fn dimorphism_flag_off() -> String {
    with_opt_outs(DIMORPHISM_PRE_FLIP)
}

pub const DISEASE_PRE_FLIP: &str = include_str!("disease.pre-flip.toml");
/// `disease.toml` as it was: 150 grazers beside 60 innovators seeded through
/// Medicine; only `disease_enabled` and `inventions_enabled` on.
pub fn disease_flag_off() -> String {
    with_opt_outs(DISEASE_PRE_FLIP)
}

pub const DISTURBANCE_PRE_FLIP: &str = include_str!("disturbance.pre-flip.toml");
/// `disturbance.toml` as it was: two 40-grazer herds; only
/// `disasters_enabled` and `living_biome` on.
pub fn disturbance_flag_off() -> String {
    with_opt_outs(DISTURBANCE_PRE_FLIP)
}

pub const DIT_ENV_SLOW_PRE_FLIP: &str = include_str!("dit-env-slow.pre-flip.toml");
/// `experiments/dit-env-slow.toml` as it was: 120 founders; only the
/// `env_period = 400` lever on.
pub fn dit_env_slow_flag_off() -> String {
    with_opt_outs(DIT_ENV_SLOW_PRE_FLIP)
}

pub const DOMESTICATION_PRE_FLIP: &str = include_str!("domestication.pre-flip.toml");
/// `domestication.toml` as it was: 24 innovators beside a 40-agent wild herd;
/// only `inventions_enabled` and `domestication_enabled` on.
pub fn domestication_flag_off() -> String {
    with_opt_outs(DOMESTICATION_PRE_FLIP)
}

pub const DRIFTING_CLIMATE_PRE_FLIP: &str = include_str!("drifting-climate.pre-flip.toml");
/// `experiments/drifting-climate.toml` as it was: 120 founders; only the
/// `env_period = 400` and `climate_drift_rate = 0.00005` levers on.
pub fn drifting_climate_flag_off() -> String {
    with_opt_outs(DRIFTING_CLIMATE_PRE_FLIP)
}

pub const GENE_REQUIREMENTS_PRE_FLIP: &str = include_str!("gene-requirements.pre-flip.toml");
/// `experiments/gene-requirements.toml` as it was: 80 founders; only
/// `inventions_enabled`, `cognition_enabled`, `gene_tech_coupling`,
/// `gene_requirements` and `resources_enabled` on.
pub fn gene_requirements_flag_off() -> String {
    with_opt_outs(GENE_REQUIREMENTS_PRE_FLIP)
}

pub const GEOGRAPHIC_TRADE_PRE_FLIP: &str = include_str!("geographic-trade.pre-flip.toml");
/// `geographic-trade.toml` as it was: 962 terrain-affinity foragers at the
/// biome junction; only `terrain_habitat` and `resources_enabled` on.
pub fn geographic_trade_flag_off() -> String {
    with_opt_outs(GEOGRAPHIC_TRADE_PRE_FLIP)
}

pub const INVENTIONS_PRE_FLIP: &str = include_str!("inventions.pre-flip.toml");
/// `inventions.toml` as it was: 64 agents (innovators, traditionalists, an
/// acultural control), no seeded inventions; only `inventions_enabled` on.
pub fn inventions_flag_off() -> String {
    with_opt_outs(INVENTIONS_PRE_FLIP)
}

pub const KNOWLEDGE_RATCHET_PRE_FLIP: &str = include_str!("knowledge-ratchet.pre-flip.toml");
/// `knowledge-ratchet.toml` as it was: 80 innovators holding Writing from t0;
/// only `inventions_enabled` and `knowledge_enabled` on.
pub fn knowledge_ratchet_flag_off() -> String {
    with_opt_outs(KNOWLEDGE_RATCHET_PRE_FLIP)
}

pub const O1_LEVER_PRACTICES_OFF_PRE_FLIP: &str =
    include_str!("o1-lever-practices-off.pre-flip.toml");
/// `experiments/o1-lever-practices-off.toml` as it was: 1000 asocial vs 20
/// cultural foragers under grand-theater's pre-flip flag set (both levers
/// included) plus dimorphism and domestication, with `practices_enabled = false`.
pub fn o1_lever_practices_off_flag_off() -> String {
    with_opt_outs(O1_LEVER_PRACTICES_OFF_PRE_FLIP)
}

pub const O2_PAYOFF_BIASED_LEARNING_PRE_FLIP: &str =
    include_str!("o2-payoff-biased-learning.pre-flip.toml");
/// `experiments/o2-payoff-biased-learning.toml` as it was: the O1 world
/// (practices back on) with the `payoff_biased_learning` lever on.
pub fn o2_payoff_biased_learning_flag_off() -> String {
    with_opt_outs(O2_PAYOFF_BIASED_LEARNING_PRE_FLIP)
}

pub const O3_REPRO_BIASED_LEARNING_PRE_FLIP: &str =
    include_str!("o3-repro-biased-learning.pre-flip.toml");
/// `experiments/o3-repro-biased-learning.toml` as it was: the O1 flag set
/// (practices back on) with `repro_biased_learning` on, and 1000 omnivore
/// (diet-matched) vs 20 cultural foragers.
pub fn o3_repro_biased_learning_flag_off() -> String {
    with_opt_outs(O3_REPRO_BIASED_LEARNING_PRE_FLIP)
}

pub const UNILATERAL_TRADE_PRE_FLIP: &str = include_str!("unilateral-trade.pre-flip.toml");
/// `unilateral-trade.toml` as it was: five 320-grazer goods lineages; only
/// `resources_enabled`, `conserve_goods_on_death`, the `unilateral_trade`
/// lever and `living_biome` on.
pub fn unilateral_trade_flag_off() -> String {
    with_opt_outs(UNILATERAL_TRADE_PRE_FLIP)
}

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

    /// Top-level fields of `s` as serialized (knobs are scalars there).
    fn fields(s: &anabios_core::scenario::Scenario) -> serde_json::Map<String, serde_json::Value> {
        match serde_json::to_value(s).expect("serialize scenario") {
            serde_json::Value::Object(m) => m,
            v => panic!("scenario serialized to a non-object: {v}"),
        }
    }

    /// Scalar "on": `true`, or a non-zero number (`season_period`).
    fn is_on(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Bool(b) => *b,
            serde_json::Value::Number(n) => n.as_f64() != Some(0.0),
            _ => false,
        }
    }

    /// The experiment levers: off in the schema too, but listed in
    /// `OPT_OUT_ALL` so a fixture base cannot inherit one.
    const LEVERS: [&str; 4] =
        ["env_period", "climate_drift_rate", "payoff_biased_learning", "unilateral_trade"];

    /// The guard behind `OPT_OUT_ALL`'s "every knob" promise. The default-on
    /// set is read off the schema itself (a bare `name`/`seed` scenario,
    /// serialized), not from a hand-kept list, so a new `default_true` knob
    /// that `opt-out-all.toml` misses fails here instead of silently running
    /// every fixture (and the flag-off trajectory guards) with it on.
    #[test]
    fn opt_out_all_turns_off_every_default_on_knob() {
        use anabios_core::scenario::Scenario;
        let header = "name = \"g\"\nseed = 1\n";
        let schema = fields(&Scenario::parse_toml(header).expect("bare header parses"));
        let opted = fields(&Scenario::parse_toml(&with_opt_outs(header)).expect("opt-outs parse"));
        let default_on: Vec<&String> =
            schema.iter().filter(|(k, v)| *k != "seed" && is_on(v)).map(|(k, _)| k).collect();
        // 25 `default_true` knobs plus `season_period` today; a floor, so a
        // serialization change that hid them would not pass vacuously.
        assert!(default_on.len() >= 26, "found only {} default-on knobs", default_on.len());
        for key in &default_on {
            if *key == "practices_enabled" {
                // Its pre-flip default was already on; fixtures keep it.
                assert!(is_on(&opted[*key]), "practices_enabled must stay on");
            } else {
                assert!(
                    !is_on(&opted[*key]),
                    "`{key}` defaults on in the scenario schema but OPT_OUT_ALL leaves it on — \
                     add `{key} = false` (its pre-flip default) to tests/common/opt-out-all.toml"
                );
            }
        }
        for lever in LEVERS {
            assert!(!is_on(&schema[lever]) && !is_on(&opted[lever]), "lever {lever} must be off");
        }
        // Nothing but knobs in the list: each line is a default-on knob or a lever.
        let listed: Vec<&str> = OPT_OUT_ALL
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split('=').next().unwrap().trim())
            .collect();
        assert_eq!(listed.len(), default_on.len() + LEVERS.len(), "OPT_OUT_ALL: {listed:?}");
    }

    /// The Godot crate's export tests keep their own copy of the list (its
    /// `pre_flip_knobs_but_inventions!` macro, which leaves `inventions_enabled`
    /// to each fixture). Text-level check that it writes every other
    /// `OPT_OUT_ALL` line verbatim, so the two cannot drift apart.
    #[test]
    fn godot_pre_flip_knob_list_matches_opt_out_all() {
        let src = include_str!("../../../anabios-godot/src/lib.rs");
        let start =
            src.find("macro_rules! pre_flip_knobs_but_inventions").expect("Godot knob macro");
        let body = &src[start..start + src[start..].find("};").expect("macro end")];
        let godot: Vec<&str> = body
            .lines()
            .filter_map(|l| l.trim().strip_prefix('"')?.strip_suffix("\\n\","))
            .collect();
        let expected: Vec<&str> = OPT_OUT_ALL
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("inventions_enabled"))
            .collect();
        assert_eq!(godot, expected, "crates/anabios-godot/src/lib.rs knob list vs OPT_OUT_ALL");
    }
}
