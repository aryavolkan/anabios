//! Cross-platform deterministic `f32` transcendentals.
//!
//! `f32::sin`, `f32::cos`, `f32::ln`, `f32::exp` are not IEEE 754
//! correctly-rounded — different libm implementations (glibc, musl,
//! macOS libm, Windows ucrt) produce different last-ulp results. For a
//! deterministic simulation that's a problem: the same seed yields
//! different state hashes on different operating systems.
//!
//! The `libm` crate is a pure-Rust port of the FreeBSD msun library
//! that produces bit-identical output regardless of the host libm.
//! Wrap every transcendental in this module and use the wrappers
//! throughout the simulation. `sqrt` is correctly rounded by IEEE 754
//! so the standard library's `f32::sqrt` is safe.

#[inline]
pub fn sinf(x: f32) -> f32 {
    libm::sinf(x)
}

#[inline]
pub fn cosf(x: f32) -> f32 {
    libm::cosf(x)
}

#[inline]
pub fn lnf(x: f32) -> f32 {
    libm::logf(x)
}

#[inline]
pub fn expf(x: f32) -> f32 {
    libm::expf(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_of_zero_is_zero() {
        assert_eq!(sinf(0.0), 0.0);
    }

    #[test]
    fn cos_of_zero_is_one() {
        assert_eq!(cosf(0.0), 1.0);
    }

    #[test]
    fn ln_of_e_is_one() {
        let r = lnf(std::f32::consts::E);
        assert!((r - 1.0).abs() < 1e-6);
    }
}
