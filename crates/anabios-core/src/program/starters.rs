//! Starter program library (split from program.rs). Each builds a canonical
//! behavior kit; `starter_library` enumerates them.

use super::*;

/// Canned starter: basic herbivore that always heads toward plants and
/// mates when well-fed. Simple by design — evolution discovers more
/// sophisticated strategies via mutation.
///
/// The `decide()` wrapper normalizes the action register's move vector
/// to a unit direction, so the magnitude of the components doesn't
/// matter — only the sign / ratio.
pub fn starter_grazer() -> Program {
    // Strategy:
    //   well_fed = energy > 30
    //   move_x   = well_fed ? nearest_dir.x : plant_dir.x
    //   move_y   = well_fed ? nearest_dir.y : plant_dir.y
    //   mate_intent = energy > 35
    //
    // Stack order for IfThenElse: cond pushed first, then "then", then "else".
    // IfThenElse pops in reverse: else, then, cond; result = cond > 0 ? then : else.
    Program::from_slice(&[
        // x axis
        Node::SenseEnergy,
        Node::ThresholdGt(30.0), // cond: well_fed
        Node::SenseNearestDirX,  // then: mate-seek
        Node::SensePlantDirX,    // else: forage
        Node::IfThenElse,
        Node::MoveTowardX,
        // y axis
        Node::SenseEnergy,
        Node::ThresholdGt(30.0),
        Node::SenseNearestDirY,
        Node::SensePlantDirY,
        Node::IfThenElse,
        Node::MoveTowardY,
        // mate intent
        Node::SenseEnergy,
        Node::ThresholdGt(35.0),
        Node::Mate,
    ])
}

/// Stalker: approach the nearest other-species agent and fire a weapon when
/// within ~3 units. (`FireWeapon` is inert until M12 wires combat.)
pub fn starter_stalker() -> Program {
    Program::from_slice(&[
        Node::SenseOtherDirX,
        Node::MoveTowardX,
        Node::SenseOtherDirY,
        Node::MoveTowardY,
        // fire when other_dist < 3  ==  (-other_dist) > -3
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-3.0),
        Node::FireWeapon,
    ])
}

/// Pack hunter: approach prey, broadcast its presence on channel 0 when within
/// ~5 units, and fire when within ~3 units. (Broadcast/FireWeapon inert until
/// M14/M12.)
pub fn starter_pack_hunter() -> Program {
    Program::from_slice(&[
        Node::SenseOtherDirX,
        Node::MoveTowardX,
        Node::SenseOtherDirY,
        Node::MoveTowardY,
        // broadcast presence when other_dist < 5
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-5.0),
        Node::Broadcast(0),
        // fire when other_dist < 3
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-3.0),
        Node::FireWeapon,
    ])
}

/// Sentinel: flee from the nearest other-species agent and raise an alarm on
/// channel 1 when one is within ~8 units. (Broadcast inert until M14.)
pub fn starter_sentinel() -> Program {
    Program::from_slice(&[
        Node::SenseOtherDirX,
        Node::MoveAwayX,
        Node::SenseOtherDirY,
        Node::MoveAwayY,
        // alarm when other_dist < 8
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-8.0),
        Node::Broadcast(1),
    ])
}

/// Herd: move toward the nearest same-species neighbor (cohesion).
pub fn starter_herd() -> Program {
    Program::from_slice(&[
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
    ])
}

/// Marker: emit Marker pheromone (channel 3) each tick and cohere toward the
/// nearest same-species neighbor (herd), so the group clusters while marking.
pub fn starter_marker() -> Program {
    Program::from_slice(&[
        // deposit a strong marker every tick
        Node::Const(1.0),
        Node::EmitPheromone(3),
        // cohesion toward same-species
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
    ])
}

/// Communicator: broadcast a strong signal on channel 1 and cohere toward the
/// nearest same-species neighbor, so the meme propagates and sweeps the cluster
/// (population `meme[1]` rises from ~0 to dominant → MemeSweep).
pub fn starter_communicator() -> Program {
    Program::from_slice(&[
        Node::Const(1.0),
        Node::Broadcast(1),
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
    ])
}

/// Cooperator: share energy with kin (when kinship > 0.3) and cohere toward
/// the nearest same-species neighbor. `SenseKinship` pushes kinship onto the
/// stack, `ThresholdGt(0.3)` maps it to 1.0/0.0, `Share` pops that value and
/// adds it to `share_intent` (positive → altruistic transfer fires when
/// `Altruism > 0`). M15.
pub fn starter_cooperator() -> Program {
    Program::from_slice(&[
        Node::SenseKinship,
        Node::ThresholdGt(0.3),
        Node::Share,
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
    ])
}

/// Cultural cooperator (gene-culture experiment): broadcast a cooperation meme
/// on channel 2 and share with kin ONLY WHEN the received cooperation meme is
/// also high — i.e. sharing is gated on BOTH a cultural norm (`SenseMeme(2)`)
/// and genetic kinship (`SenseKinship`). `Mul` ANDs the two thresholded gates;
/// `Share` fires only when both are satisfied. The meme is a heritable,
/// culturally-transmitted "cooperation norm"; the `Communicator` module is the
/// genetically-encoded capacity to hold and spread it. Requires a Communicator.
pub fn starter_cultural_cooperator() -> Program {
    Program::from_slice(&[
        // forage toward plants + mate when well-fed (IDENTICAL to the asocial
        // control, so sharing is the only behavioural difference)
        Node::SensePlantDirX,
        Node::MoveTowardX,
        Node::SensePlantDirY,
        Node::MoveTowardY,
        Node::SenseEnergy,
        Node::ThresholdGt(35.0),
        Node::Mate,
        // broadcast the cooperation norm every tick (channel 2)
        Node::Const(1.0),
        Node::Broadcast(2),
        // NEED-BASED sharing: share only to a kin neighbour who is POORER than
        // me (rel_energy <= 0.7) when the norm is present and I have surplus.
        // gate = (meme>0.3)*(kin>0.3)*(own energy>40)*(neighbour poorer).
        // Targeting surplus to needy kin saves them from starvation (raising
        // inclusive fitness) instead of flattening energy across the group.
        Node::SenseMeme(2),
        Node::ThresholdGt(0.3),
        Node::SenseKinship,
        Node::ThresholdGt(0.3),
        Node::Mul,
        Node::SenseEnergy,
        Node::ThresholdGt(40.0),
        Node::Mul,
        // (1 - [rel_energy > 0.7]) == neighbour is poorer than me
        Node::Const(1.0),
        Node::SenseRelEnergy,
        Node::ThresholdGt(0.7),
        Node::Sub,
        Node::Mul,
        Node::Share,
    ])
}

/// Asocial control forager: forage toward plants + mate when well-fed. Same
/// ecology as `starter_cultural_cooperator` MINUS the broadcast/meme/share — the
/// clean control for the gene-culture experiment. No Communicator.
pub fn starter_asocial_forager() -> Program {
    Program::from_slice(&[
        Node::SensePlantDirX,
        Node::MoveTowardX,
        Node::SensePlantDirY,
        Node::MoveTowardY,
        Node::SenseEnergy,
        Node::ThresholdGt(35.0),
        Node::Mate,
    ])
}

/// Culture-prey (gene-culture experiment, alarm variant): forage + herd, and
/// broadcast an ALARM meme (channel 0) when a predator is near, then flee scaled
/// by the RECEIVED alarm — i.e. respond to a warning propagated by neighbours,
/// even for a predator not yet in the agent's own perception. Alarm information
/// is non-zero-sum: the sender loses nothing, the receiver gains a head start.
pub fn starter_culture_prey() -> Program {
    Program::from_slice(&[
        // forage
        Node::SensePlantDirX,
        Node::MoveTowardX,
        Node::SensePlantDirY,
        Node::MoveTowardY,
        // herd (cohesion → the alarm has a group to propagate through)
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
        // broadcast alarm when a predator (other species) is within ~12
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-12.0),
        Node::Broadcast(0),
        // flee from the predator, scaled by the received alarm meme (early warning)
        Node::SenseOtherDirX,
        Node::SenseMeme(0),
        Node::Mul,
        Node::MoveAwayX,
        Node::SenseOtherDirY,
        Node::SenseMeme(0),
        Node::Mul,
        Node::MoveAwayY,
        // mate when well-fed
        Node::SenseEnergy,
        Node::ThresholdGt(35.0),
        Node::Mate,
    ])
}

/// Asocial-prey control (alarm variant): identical forage + herd + mate, but
/// flees ONLY on the agent's OWN direct detection of a predator (no comms, no
/// warning propagation). Clean control vs `starter_culture_prey`. No Communicator.
pub fn starter_asocial_prey() -> Program {
    Program::from_slice(&[
        Node::SensePlantDirX,
        Node::MoveTowardX,
        Node::SensePlantDirY,
        Node::MoveTowardY,
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
        // flee scaled by OWN detection: gate = (other within 12)
        Node::SenseOtherDirX,
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-12.0),
        Node::Mul,
        Node::MoveAwayX,
        Node::SenseOtherDirY,
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-12.0),
        Node::Mul,
        Node::MoveAwayY,
        Node::SenseEnergy,
        Node::ThresholdGt(35.0),
        Node::Mate,
    ])
}

/// Cultural hunter (gene-culture experiment): an omnivore that grazes as a
/// fallback but, when the HUNT-TECHNIQUE meme (channel 4) is active, pursues the
/// nearest other-species agent and fires on it — "leap on prey". Broadcasts the
/// hunt meme so the technique spreads. The technique's PAYOFF (catching mobile
/// prey for a flesh bonus) is conditional on the agent's genetic speed
/// (Locomotor max_speed): fast agents catch prey, slow agents waste energy — so
/// the meme is adaptive only given the speed gene (gene-culture coupling).
pub fn starter_cultural_hunter() -> Program {
    Program::from_slice(&[
        // broadcast the hunt-technique meme (channel 4) — the shared technique
        Node::Const(1.0),
        Node::Broadcast(4),
        // "leap on prey": pursue the nearest other-species agent...
        Node::SenseOtherDirX,
        Node::MoveTowardX,
        Node::SenseOtherDirY,
        Node::MoveTowardY,
        // ...and fire when within ~3. Catching mobile prey requires genetic
        // speed > prey speed; slow hunters chase fruitlessly and starve.
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-3.0),
        Node::FireWeapon,
        // mate when well-fed
        Node::SenseEnergy,
        Node::ThresholdGt(35.0),
        Node::Mate,
    ])
}

/// Library of starter programs. Founders use index 0 (`starter_grazer`).
pub fn starter_library() -> &'static [fn() -> Program] {
    &[
        starter_grazer,
        starter_stalker,
        starter_pack_hunter,
        starter_sentinel,
        starter_herd,
        starter_marker,
        starter_communicator,
        starter_cooperator,
        starter_cultural_cooperator,
    ]
}
