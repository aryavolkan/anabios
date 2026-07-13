//! Pure per-tick co-evolution metric helpers. Each takes compact slices over
//! the *live* agents (already filtered/parallel) and returns one scalar. Kept
//! free of Godot and `World` types so they unit-test in isolation.

use anabios_core::culture::{technique_match, TECH_CHANNEL};
use anabios_core::genome::{Genome, GenomeSlot, GENOME_LEN};
use anabios_core::program::MEME_CHANNELS;

/// Minimum members per spatial half for a species to count toward dialect
/// divergence. Mirrors `DIALECT_MIN_HALF` in `anabios_core::codex`.
const DIALECT_MIN_HALF: usize = 3;

/// Fraction of `flags` that are true, in `[0,1]`. Empty slice returns 0.0.
pub(crate) fn frac_true(flags: &[bool]) -> f32 {
    if flags.is_empty() {
        return 0.0;
    }
    flags.iter().filter(|&&b| b).count() as f32 / flags.len() as f32
}

/// Mean of one genome slot over all `genomes`. Empty returns 0.0.
pub(crate) fn mean_slot(genomes: &[Genome], slot: GenomeSlot) -> f32 {
    if genomes.is_empty() {
        return 0.0;
    }
    genomes.iter().map(|g| g.get(slot)).sum::<f32>() / genomes.len() as f32
}

/// Mean of meme channel `ch` over agents where `keep[i]` is true. No kept
/// agents (or bad channel) returns 0.0.
pub(crate) fn mean_channel_over(memes: &[[f32; MEME_CHANNELS]], keep: &[bool], ch: usize) -> f32 {
    if ch >= MEME_CHANNELS {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut n = 0u32;
    for (m, &k) in memes.iter().zip(keep) {
        if k {
            sum += m[ch];
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

/// Mean `technique_match(meme[TECH], opt)` over kept agents. No kept agents
/// returns 0.0.
pub(crate) fn mean_tech_match(memes: &[[f32; MEME_CHANNELS]], keep: &[bool], opt: f32) -> f32 {
    let mut sum = 0.0;
    let mut n = 0u32;
    for (m, &k) in memes.iter().zip(keep) {
        if k {
            sum += technique_match(m[TECH_CHANNEL], opt);
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

/// Mean per-slot variance across `genomes` (summed variance over the 50 slots
/// divided by 50). Empty returns 0.0. A cheap scalar for genetic spread.
pub(crate) fn genetic_diversity(genomes: &[Genome]) -> f32 {
    if genomes.is_empty() {
        return 0.0;
    }
    let n = genomes.len() as f32;
    let mut total_var = 0.0;
    for slot in 0..GENOME_LEN {
        let mut mean = 0.0;
        for g in genomes {
            mean += g.0[slot];
        }
        mean /= n;
        let mut var = 0.0;
        for g in genomes {
            let d = g.0[slot] - mean;
            var += d * d;
        }
        total_var += var / n;
    }
    total_var / GENOME_LEN as f32
}

/// Maximum, over Communicator-bearing species, of the west/east per-channel
/// mean-meme L2 distance — the same kernel the `DialectFormed` detector uses,
/// aggregated to one scalar. A species contributes only if each half (split at
/// its members' mean x) has at least `DIALECT_MIN_HALF` members. None qualify
/// returns 0.0.
pub(crate) fn species_max_meme_divergence(
    memes: &[[f32; MEME_CHANNELS]],
    species: &[u32],
    xs: &[f32],
    comm: &[bool],
) -> f32 {
    use std::collections::BTreeMap;
    // Group live indices by species; note which species have a communicator.
    let mut members: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut has_comm: BTreeMap<u32, bool> = BTreeMap::new();
    for i in 0..memes.len() {
        members.entry(species[i]).or_default().push(i);
        let e = has_comm.entry(species[i]).or_insert(false);
        *e = *e || comm[i];
    }
    let mut best = 0.0f32;
    for (sid, idxs) in members.iter() {
        if !has_comm.get(sid).copied().unwrap_or(false) {
            continue;
        }
        let cx = idxs.iter().map(|&i| xs[i]).sum::<f32>() / idxs.len() as f32;
        let (mut west, mut east): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        for &i in idxs {
            if xs[i] < cx {
                west.push(i);
            } else {
                east.push(i);
            }
        }
        if west.len() < DIALECT_MIN_HALF || east.len() < DIALECT_MIN_HALF {
            continue;
        }
        let mut wm = [0.0f32; MEME_CHANNELS];
        let mut em = [0.0f32; MEME_CHANNELS];
        for &i in &west {
            for ch in 0..MEME_CHANNELS {
                wm[ch] += memes[i][ch];
            }
        }
        for &i in &east {
            for ch in 0..MEME_CHANNELS {
                em[ch] += memes[i][ch];
            }
        }
        let (wn, en) = (west.len() as f32, east.len() as f32);
        let mut l2 = 0.0f32;
        for ch in 0..MEME_CHANNELS {
            let d = wm[ch] / wn - em[ch] / en;
            l2 += d * d;
        }
        best = best.max(l2.sqrt());
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::genome::{Genome, GenomeSlot, GENOME_LEN};
    use anabios_core::program::MEME_CHANNELS;

    fn genome_with(slot: GenomeSlot, v: f32) -> Genome {
        let mut a = [0.0f32; GENOME_LEN];
        a[slot as usize] = v;
        Genome(a)
    }

    #[test]
    fn frac_true_counts_and_bounds() {
        assert_eq!(frac_true(&[]), 0.0);
        assert_eq!(frac_true(&[true, false, false, false]), 0.25);
        assert_eq!(frac_true(&[true, true]), 1.0);
    }

    #[test]
    fn mean_slot_averages_named_slot() {
        let gs = [
            genome_with(GenomeSlot::SocialLearning, 0.2),
            genome_with(GenomeSlot::SocialLearning, 0.8),
        ];
        assert!((mean_slot(&gs, GenomeSlot::SocialLearning) - 0.5).abs() < 1e-6);
        assert_eq!(mean_slot(&[], GenomeSlot::SocialLearning), 0.0);
    }

    #[test]
    fn mean_channel_respects_keep_mask() {
        let mut a = [0.0f32; MEME_CHANNELS];
        a[5] = 1.0;
        let mut b = [0.0f32; MEME_CHANNELS];
        b[5] = 0.0;
        let memes = [a, b];
        // Only the first agent is a communicator, so mean over kept = 1.0.
        assert_eq!(mean_channel_over(&memes, &[true, false], 5), 1.0);
        // No communicators returns 0.0, not NaN.
        assert_eq!(mean_channel_over(&memes, &[false, false], 5), 0.0);
        // Bad channel returns 0.0.
        assert_eq!(mean_channel_over(&memes, &[true, true], 99), 0.0);
    }

    #[test]
    fn genetic_diversity_zero_for_identical_and_positive_for_spread() {
        let same = [genome_with(GenomeSlot::Size, 0.5), genome_with(GenomeSlot::Size, 0.5)];
        assert_eq!(genetic_diversity(&same), 0.0);
        let spread = [genome_with(GenomeSlot::Size, 0.0), genome_with(GenomeSlot::Size, 1.0)];
        assert!(genetic_diversity(&spread) > 0.0);
    }

    #[test]
    fn divergence_needs_comm_and_min_half() {
        // Two species. Species 7 has 3 west (meme0=0) + 3 east (meme0=1) comms.
        let lo = [0.0f32; MEME_CHANNELS];
        let mut hi = [0.0f32; MEME_CHANNELS];
        hi[0] = 1.0;
        let memes = vec![lo, lo, lo, hi, hi, hi];
        let species = vec![7u32; 6];
        let xs = vec![0.0, 1.0, 2.0, 10.0, 11.0, 12.0]; // mean x = 6 -> 3 west, 3 east
        let comm = vec![true; 6];
        let d = species_max_meme_divergence(&memes, &species, &xs, &comm);
        assert!((d - 1.0).abs() < 1e-6, "expected L2 ~1.0, got {d}");
        // No communicators returns 0.0.
        let none = vec![false; 6];
        assert_eq!(species_max_meme_divergence(&memes, &species, &xs, &none), 0.0);
    }
}
