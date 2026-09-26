//! Species territories (territory/habitat/collision layer): a persistent
//! per-species range — centre, radius, locomotion class — maintained every
//! species step, and the soft-edge pull that keeps members roaming inside it.
//! Everything is gated on `World::territory_enabled`.

use serde::{Deserialize, Serialize};

use crate::habitat::Locomotion;
use crate::prelude::Vec2;

/// One species' territory. `r == 0.0` marks a row not yet initialized (no
/// member seen by `territory_step`), which applies no pull.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Territory {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
    pub class: Locomotion,
}

impl Territory {
    #[inline]
    pub fn is_set(&self) -> bool {
        self.r > 0.0
    }

    #[inline]
    pub fn centre(&self) -> Vec2 {
        Vec2::new(self.cx, self.cy)
    }
}
