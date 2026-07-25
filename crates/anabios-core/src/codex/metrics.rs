//! Pure metric kernels shared by the codex detectors (and, for
//! `west_east_meme_divergence`, the Godot coevo panel). No `World` access, no
//! detector state — just math over the inputs, so they're trivially testable
//! and reusable. Extracted verbatim from `codex/mod.rs`.

use super::*;

/// RMS distance (torus-aware) of `positions` from their coordinate mean, on a
/// torus of the given `world_size`. Returns 0.0 for fewer than 2 points.
pub fn species_spread(positions: &[glam::Vec2], world_size: f32) -> f32 {
    if positions.len() < 2 {
        return 0.0;
    }
    let n = positions.len() as f32;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    for p in positions {
        cx += p.x as f64;
        cy += p.y as f64;
    }
    let centroid = glam::Vec2::new((cx / n as f64) as f32, (cy / n as f64) as f32);
    let mut sumsq = 0.0f64;
    for p in positions {
        let d = crate::spatial::torus_distance(*p, centroid, world_size);
        sumsq += (d as f64) * (d as f64);
    }
    ((sumsq / n as f64).sqrt()) as f32
}

/// Pure ArmsRace test: is there a species whose weapon-damage mean rose across
/// a full window while a *different* species' armor mean also rose? Returns
/// `(weaponized_species, weapon_rise)`.
pub fn arms_race_signal(
    weapon_history: &BTreeMap<u32, VecDeque<f32>>,
    armor_history: &BTreeMap<u32, VecDeque<f32>>,
) -> Option<(u32, f32)> {
    let rise = |buf: &VecDeque<f32>| -> Option<f32> {
        if buf.len() < ARMS_WINDOW {
            return None;
        }
        let delta = buf.back()? - buf.front()?;
        (delta >= ARMS_MIN_DELTA).then_some(delta)
    };
    for (wsid, wbuf) in weapon_history.iter() {
        let Some(wrise) = rise(wbuf) else { continue };
        for (asid, abuf) in armor_history.iter() {
            if asid == wsid {
                continue;
            }
            if rise(abuf).is_some() {
                return Some((*wsid, wrise));
            }
        }
    }
    None
}

/// Histogram intersection of two normalized terrain distributions
/// (`Σ min(a_t, b_t)`): 1.0 identical, 0.0 disjoint. Zero slots contribute
/// nothing, so fixed-array iteration matches the former sparse-map version.
pub fn histogram_overlap(a: &[f32; TERRAIN_SLOTS], b: &[f32; TERRAIN_SLOTS]) -> f32 {
    let mut overlap = 0.0f32;
    for t in 0..TERRAIN_SLOTS {
        overlap += a[t].min(b[t]);
    }
    overlap
}

/// L2 distance between two meme vectors.
pub fn meme_l2(a: &[f32; MEME_CHANNELS], b: &[f32; MEME_CHANNELS]) -> f32 {
    let mut s = 0.0f32;
    for ch in 0..MEME_CHANNELS {
        let d = a[ch] - b[ch];
        s += d * d;
    }
    s.sqrt()
}

/// West/east spatial-half meme divergence kernel shared by the DialectFormed
/// detector and the Godot coevo metric. Splits `idxs` at centroid x `cx`,
/// computes per-half per-channel meme means (f32, ascending index order),
/// and returns their L2 distance. `None` when either half has fewer than
/// `min_half` members.
pub fn west_east_meme_divergence(
    idxs: &[usize],
    cx: f32,
    min_half: u32,
    sample: impl Fn(usize) -> (f32, [f32; MEME_CHANNELS]),
) -> Option<f32> {
    let mut west_mean = [0.0f32; MEME_CHANNELS];
    let mut east_mean = [0.0f32; MEME_CHANNELS];
    let mut wn = 0u32;
    let mut en = 0u32;
    for &i in idxs {
        let (x, meme) = sample(i);
        if x < cx {
            for (ch, w) in west_mean.iter_mut().enumerate() {
                *w += meme[ch];
            }
            wn += 1;
        } else {
            for (ch, e) in east_mean.iter_mut().enumerate() {
                *e += meme[ch];
            }
            en += 1;
        }
    }
    if wn < min_half || en < min_half {
        return None;
    }
    for w in west_mean.iter_mut() {
        *w /= wn as f32;
    }
    for e in east_mean.iter_mut() {
        *e /= en as f32;
    }
    Some(meme_l2(&west_mean, &east_mean))
}
