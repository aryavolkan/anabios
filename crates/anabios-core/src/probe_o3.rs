//! O3 throwaway probe knobs (cultural niche construction levers).
//!
//! THIS MODULE IS A THROWAWAY MEASUREMENT PROBE — env-gated, off by default,
//! never to ship enabled on a default path. With no `ANABIOS_O3_*` env vars
//! set, every knob is an exact identity (0.0 gain / 1.0 multiplier) and the
//! sim is bit-identical to the unprobed build.
//!
//! Lever A — non-excludable niche construction: skilled Communicator agents
//! durably enrich the soil fertility of cells they successfully feed on
//! (`ANABIOS_O3_NICHE_GAIN`, per-graze increment; `ANABIOS_O3_NICHE_CAP`,
//! fertility ceiling; `ANABIOS_O3_NICHE_SKILL_MIN`, skill gate).
//!
//! Lever B — excludable resource tier: Communicator agents can reach a food
//! tier asocial foragers cannot (`ANABIOS_O3_TIER_MULT`, extra bite fraction).

use std::sync::OnceLock;

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Per-successful-graze fertility increment deposited by a skilled
/// Communicator agent. 0.0 (off) unless `ANABIOS_O3_NICHE_GAIN` is set.
pub fn niche_gain() -> f32 {
    static V: OnceLock<f32> = OnceLock::new();
    *V.get_or_init(|| env_f32("ANABIOS_O3_NICHE_GAIN", 0.0))
}

/// Ceiling for probe-deposited fertility (`ANABIOS_O3_NICHE_CAP`, default 3.0).
pub fn niche_cap() -> f32 {
    static V: OnceLock<f32> = OnceLock::new();
    *V.get_or_init(|| env_f32("ANABIOS_O3_NICHE_CAP", 3.0))
}

/// Minimum cultural skill before an agent's activity enriches soil
/// (`ANABIOS_O3_NICHE_SKILL_MIN`, default 0.5).
pub fn niche_skill_min() -> f32 {
    static V: OnceLock<f32> = OnceLock::new();
    *V.get_or_init(|| env_f32("ANABIOS_O3_NICHE_SKILL_MIN", 0.5))
}

/// Extra bite fraction available only to Communicator agents (the excludable
/// tier). 0.0 (off) unless `ANABIOS_O3_TIER_MULT` is set.
pub fn tier_mult() -> f32 {
    static V: OnceLock<f32> = OnceLock::new();
    *V.get_or_init(|| env_f32("ANABIOS_O3_TIER_MULT", 0.0))
}
