//! Internal prelude used across the crate.

pub(crate) use glam::Vec2;

/// Wrap a position into the bounded toroidal world `[0, size)` along each axis.
/// Inputs outside the range, including negative values, are normalized.
#[inline]
pub(crate) fn wrap_torus(pos: Vec2, size: Vec2) -> Vec2 {
    Vec2::new(pos.x.rem_euclid(size.x), pos.y.rem_euclid(size.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_torus_keeps_positive_in_range() {
        let wrapped = wrap_torus(Vec2::new(1024.5, -0.1), Vec2::splat(1024.0));
        assert!(wrapped.x >= 0.0 && wrapped.x < 1024.0);
        assert!(wrapped.y >= 0.0 && wrapped.y < 1024.0);
        assert!((wrapped.x - 0.5).abs() < 1e-5);
        assert!((wrapped.y - 1023.9).abs() < 1e-3);
    }
}
