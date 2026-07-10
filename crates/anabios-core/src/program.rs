//! Evolvable behavior program.
//!
//! Each agent owns a `Program` — a small expression tree stored in postfix
//! order. Evaluation runs a stack machine: leaves push values, operators pop
//! and push, output nodes pop and write to an `ActionRegister`. No recursion
//! anywhere, so weird mutants cannot blow the stack.
//!
//! Nodes that reference world systems not yet implemented (pheromone field,
//! meme transmission) evaluate to zero / no-op in M4 but remain in the
//! grammar so structural mutation can produce them. Later milestones wire
//! them up by editing only the evaluator.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::genome::GENOME_LEN;
use crate::rng::Rng;

/// Hard cap on program node count. Programs exceeding this are truncated.
pub const PROGRAM_MAX_NODES: usize = 64;
/// Bounded inline storage for the `SmallVec`.
pub const PROGRAM_INLINE: usize = 16;

/// Mutation probabilities applied during reproduction.
pub const POINT_MUTATE_PROB: f32 = 0.04;
pub const INSERT_NODE_PROB: f32 = 0.03;
pub const DELETE_NODE_PROB: f32 = 0.03;
pub const SUBTREE_REPLACE_PROB: f32 = 0.02;

/// Sigma for Gaussian perturbation of `Const(f32)` values.
pub const CONST_SIGMA: f32 = 0.1;

/// Number of pheromone channels (design §3.6). Wired by M13.
pub const PHEROMONE_CHANNELS: usize = 4;
/// Number of meme/broadcast channels (design §3.1). Wired by M14.
pub const MEME_CHANNELS: usize = 8;
/// Sentinel in `ActionRegister.target_id` meaning "no action target".
pub const NO_TARGET: u32 = u32::MAX;

/// AST node. Operators reference operands implicitly via the postfix
/// evaluation stack, so the same `Program` struct works for any topology
/// without explicit child indices.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Node {
    // Inputs — push one value onto the stack
    SenseEnergy,
    SenseAge,
    SenseGenome(u8),
    SenseNearestDistance,
    SenseNearestDirX,
    SenseNearestDirY,
    SensePlantDirX,
    SensePlantDirY,
    SenseLocalBiomass,
    SenseMeme(u8),
    Const(f32),

    // Operators — pop N, push 1
    Add,
    Sub,
    Mul,
    Min,
    Max,
    Neg,
    Tanh,
    ThresholdGt(f32),
    IfThenElse,
    Lerp,

    // Outputs — pop 1, write to action register, push nothing
    MoveTowardX,
    MoveTowardY,
    MoveAwayX,
    MoveAwayY,
    Feed,
    Mate,
    FireWeapon,
    EmitPheromone(u8),
    Broadcast(u8),
    Idle,

    // M11 inputs — appended at the END of the enum on purpose. Serde/bincode
    // encodes enum variants by positional index, so new variants MUST go last
    // to keep the serialized bytes of existing agent programs (and thus the
    // golden-tick state hashes) stable. Logical kinds live in `node_kind`.
    SenseSameDist,
    SenseSameDirX,
    SenseSameDirY,
    SenseOtherDist,
    SenseOtherDirX,
    SenseOtherDirY,
    SenseRelSize,
    SenseRelEnergy,
    SenseCrowding,
}

/// What an agent wants to do this tick, produced by the evaluator.
#[derive(Debug, Clone, Copy)]
pub struct ActionRegister {
    pub move_x: f32,
    pub move_y: f32,
    pub feed_intent: f32,
    pub mate_intent: f32,
    pub fire_intent: f32,
    pub emit_intent: [f32; PHEROMONE_CHANNELS],
    pub broadcast_intent: [f32; MEME_CHANNELS],
    /// Agent this action is directed at (combat/share target), derived from
    /// the nearest-neighbor sense. `NO_TARGET` when there is no neighbor.
    pub target_id: u32,
}

impl Default for ActionRegister {
    fn default() -> Self {
        Self {
            move_x: 0.0,
            move_y: 0.0,
            feed_intent: 0.0,
            mate_intent: 0.0,
            fire_intent: 0.0,
            emit_intent: [0.0; PHEROMONE_CHANNELS],
            broadcast_intent: [0.0; MEME_CHANNELS],
            target_id: NO_TARGET,
        }
    }
}

/// One agent's behavior program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub nodes: SmallVec<[Node; PROGRAM_INLINE]>,
}

impl Program {
    pub fn empty() -> Self {
        Self { nodes: SmallVec::new() }
    }

    /// Construct from a node slice. Truncates to `PROGRAM_MAX_NODES`.
    pub fn from_slice(nodes: &[Node]) -> Self {
        let mut sv: SmallVec<[Node; PROGRAM_INLINE]> = SmallVec::new();
        for &n in nodes.iter().take(PROGRAM_MAX_NODES) {
            sv.push(n);
        }
        Self { nodes: sv }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Stack inputs consumed by a node.
    pub fn arity(node: Node) -> usize {
        match node {
            Node::SenseEnergy
            | Node::SenseAge
            | Node::SenseGenome(_)
            | Node::SenseNearestDistance
            | Node::SenseNearestDirX
            | Node::SenseNearestDirY
            | Node::SensePlantDirX
            | Node::SensePlantDirY
            | Node::SenseLocalBiomass
            | Node::SenseMeme(_)
            | Node::SenseSameDist
            | Node::SenseSameDirX
            | Node::SenseSameDirY
            | Node::SenseOtherDist
            | Node::SenseOtherDirX
            | Node::SenseOtherDirY
            | Node::SenseRelSize
            | Node::SenseRelEnergy
            | Node::SenseCrowding
            | Node::Const(_) => 0,
            Node::Add | Node::Sub | Node::Mul | Node::Min | Node::Max => 2,
            Node::Neg | Node::Tanh | Node::ThresholdGt(_) => 1,
            Node::IfThenElse | Node::Lerp => 3,
            Node::MoveTowardX
            | Node::MoveTowardY
            | Node::MoveAwayX
            | Node::MoveAwayY
            | Node::Feed
            | Node::Mate
            | Node::FireWeapon
            | Node::EmitPheromone(_)
            | Node::Broadcast(_)
            | Node::Idle => 1,
        }
    }

    /// Whether the node writes to the action register and pushes nothing.
    pub fn is_output(node: Node) -> bool {
        matches!(
            node,
            Node::MoveTowardX
                | Node::MoveTowardY
                | Node::MoveAwayX
                | Node::MoveAwayY
                | Node::Feed
                | Node::Mate
                | Node::FireWeapon
                | Node::EmitPheromone(_)
                | Node::Broadcast(_)
                | Node::Idle
        )
    }

    /// Stable discriminant per node kind, grouping parameterized variants
    /// (e.g. all `Const(_)` share one kind). Used by codex novelty tracking.
    pub fn node_kind(node: Node) -> u8 {
        match node {
            Node::SenseEnergy => 0,
            Node::SenseAge => 1,
            Node::SenseGenome(_) => 2,
            Node::SenseNearestDistance => 3,
            Node::SenseNearestDirX => 4,
            Node::SenseNearestDirY => 5,
            Node::SensePlantDirX => 6,
            Node::SensePlantDirY => 7,
            Node::SenseLocalBiomass => 8,
            Node::SenseMeme(_) => 9,
            Node::Const(_) => 10,
            Node::Add => 11,
            Node::Sub => 12,
            Node::Mul => 13,
            Node::Min => 14,
            Node::Max => 15,
            Node::Neg => 16,
            Node::Tanh => 17,
            Node::ThresholdGt(_) => 18,
            Node::IfThenElse => 19,
            Node::Lerp => 20,
            Node::MoveTowardX => 21,
            Node::MoveTowardY => 22,
            Node::MoveAwayX => 23,
            Node::MoveAwayY => 24,
            Node::Feed => 25,
            Node::Mate => 26,
            Node::FireWeapon => 27,
            Node::EmitPheromone(_) => 28,
            Node::Broadcast(_) => 29,
            Node::Idle => 30,
            Node::SenseSameDist => 31,
            Node::SenseSameDirX => 32,
            Node::SenseSameDirY => 33,
            Node::SenseOtherDist => 34,
            Node::SenseOtherDirX => 35,
            Node::SenseOtherDirY => 36,
            Node::SenseRelSize => 37,
            Node::SenseRelEnergy => 38,
            Node::SenseCrowding => 39,
        }
    }
}

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

/// Library of starter programs. Founders use index 0 (`starter_grazer`).
pub fn starter_library() -> &'static [fn() -> Program] {
    &[starter_grazer, starter_stalker, starter_pack_hunter, starter_sentinel, starter_herd]
}

/// Per-agent inputs read by the evaluator. Caller fills this each tick.
#[derive(Debug, Clone, Copy)]
pub struct EvalContext<'a> {
    pub energy: f32,
    pub age: u32,
    pub genome: &'a crate::genome::Genome,
    pub nearest_distance: f32,
    pub nearest_dir: glam::Vec2,
    pub plant_dir: glam::Vec2,
    pub local_biomass: f32,
    pub same_distance: f32,
    pub same_dir: glam::Vec2,
    pub other_distance: f32,
    pub other_dir: glam::Vec2,
    pub rel_size: f32,
    pub rel_energy: f32,
    pub crowding: f32,
}

/// Evaluate `program` against `ctx`. Returns the populated action register.
/// `scratch` is the value stack — caller owns and reuses it across agents.
/// Underflow is silent: a node whose arity exceeds the current stack
/// depth is skipped, keeping malformed mutants safe.
pub fn evaluate(program: &Program, ctx: EvalContext, scratch: &mut Vec<f32>) -> ActionRegister {
    scratch.clear();
    let mut action = ActionRegister::default();

    for node in program.nodes.iter().copied() {
        let arity = Program::arity(node);
        if scratch.len() < arity {
            continue;
        }
        match node {
            Node::SenseEnergy => scratch.push(ctx.energy),
            Node::SenseAge => scratch.push(ctx.age as f32),
            Node::SenseGenome(slot) => {
                let s = (slot as usize).min(GENOME_LEN - 1);
                scratch.push(ctx.genome.0[s]);
            }
            Node::SenseNearestDistance => scratch.push(ctx.nearest_distance.min(1e6)),
            Node::SenseNearestDirX => scratch.push(ctx.nearest_dir.x),
            Node::SenseNearestDirY => scratch.push(ctx.nearest_dir.y),
            Node::SensePlantDirX => scratch.push(ctx.plant_dir.x),
            Node::SensePlantDirY => scratch.push(ctx.plant_dir.y),
            Node::SenseLocalBiomass => scratch.push(ctx.local_biomass),
            Node::SenseSameDist => scratch.push(ctx.same_distance.min(1e6)),
            Node::SenseSameDirX => scratch.push(ctx.same_dir.x),
            Node::SenseSameDirY => scratch.push(ctx.same_dir.y),
            Node::SenseOtherDist => scratch.push(ctx.other_distance.min(1e6)),
            Node::SenseOtherDirX => scratch.push(ctx.other_dir.x),
            Node::SenseOtherDirY => scratch.push(ctx.other_dir.y),
            Node::SenseRelSize => scratch.push(ctx.rel_size),
            Node::SenseRelEnergy => scratch.push(ctx.rel_energy),
            Node::SenseCrowding => scratch.push(ctx.crowding),
            Node::SenseMeme(_) => scratch.push(0.0),
            Node::Const(v) => scratch.push(v),

            Node::Add => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a + b);
            }
            Node::Sub => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a - b);
            }
            Node::Mul => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a * b);
            }
            Node::Min => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a.min(b));
            }
            Node::Max => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a.max(b));
            }
            Node::Neg => {
                let a = scratch.pop().unwrap();
                scratch.push(-a);
            }
            Node::Tanh => {
                let a = scratch.pop().unwrap();
                let e2x = crate::mathf::expf(2.0 * a);
                scratch.push((e2x - 1.0) / (e2x + 1.0));
            }
            Node::ThresholdGt(thr) => {
                let a = scratch.pop().unwrap();
                scratch.push(if a > thr { 1.0 } else { 0.0 });
            }
            Node::IfThenElse => {
                // Stack (bottom→top) is cond, then, else; we pop in reverse.
                let else_v = scratch.pop().unwrap();
                let then_v = scratch.pop().unwrap();
                let cond = scratch.pop().unwrap();
                scratch.push(if cond > 0.0 { then_v } else { else_v });
            }
            Node::Lerp => {
                // Stack (bottom→top) is t, a, b. Result: a + (b-a)*t.
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                let t = scratch.pop().unwrap().clamp(0.0, 1.0);
                scratch.push(a + (b - a) * t);
            }

            Node::MoveTowardX => action.move_x += scratch.pop().unwrap(),
            Node::MoveTowardY => action.move_y += scratch.pop().unwrap(),
            Node::MoveAwayX => action.move_x -= scratch.pop().unwrap(),
            Node::MoveAwayY => action.move_y -= scratch.pop().unwrap(),
            Node::Feed => action.feed_intent += scratch.pop().unwrap(),
            Node::Mate => action.mate_intent += scratch.pop().unwrap(),
            Node::FireWeapon => action.fire_intent += scratch.pop().unwrap(),
            Node::EmitPheromone(ch) => {
                let v = scratch.pop().unwrap();
                action.emit_intent[(ch as usize).min(PHEROMONE_CHANNELS - 1)] += v;
            }
            Node::Broadcast(ch) => {
                let v = scratch.pop().unwrap();
                action.broadcast_intent[(ch as usize).min(MEME_CHANNELS - 1)] += v;
            }
            Node::Idle => {
                scratch.pop();
            }
        }
    }

    action
}

/// Random node drawn from the full grammar. Used by structural mutation.
pub fn random_node(rng: &mut Rng) -> Node {
    match rng.index(20) {
        0 => Node::SenseEnergy,
        1 => Node::SenseAge,
        2 => Node::SenseGenome(rng.index(GENOME_LEN) as u8),
        3 => Node::SenseNearestDistance,
        4 => Node::SenseNearestDirX,
        5 => Node::SenseNearestDirY,
        6 => Node::SensePlantDirX,
        7 => Node::SensePlantDirY,
        8 => Node::SenseLocalBiomass,
        9 => Node::Const(rng.f32_range(-1.0, 1.0)),
        10 => Node::Add,
        11 => Node::Sub,
        12 => Node::Mul,
        13 => Node::Max,
        14 => Node::Tanh,
        15 => Node::IfThenElse,
        16 => Node::MoveTowardX,
        17 => Node::MoveTowardY,
        18 => Node::Feed,
        _ => Node::Mate,
    }
}

/// Per-node Gaussian perturbation: `Const` and `ThresholdGt` get nudged;
/// other nodes are swapped with a fresh random node at `POINT_MUTATE_PROB`.
pub fn point_mutate(program: &mut Program, rng: &mut Rng) {
    for node in program.nodes.iter_mut() {
        if rng.f32_unit() >= POINT_MUTATE_PROB {
            continue;
        }
        *node = match *node {
            Node::Const(v) => Node::Const((v + rng.gaussian(0.0, CONST_SIGMA)).clamp(-2.0, 2.0)),
            Node::ThresholdGt(v) => {
                Node::ThresholdGt((v + rng.gaussian(0.0, CONST_SIGMA)).clamp(-2.0, 2.0))
            }
            _ => random_node(rng),
        };
    }
}

/// Structural mutation: insert, delete, and/or subtree-replace one node each
/// with the corresponding probability. Keeps `[1, PROGRAM_MAX_NODES]`.
pub fn structural_mutate(program: &mut Program, rng: &mut Rng) {
    if program.nodes.len() < PROGRAM_MAX_NODES && rng.f32_unit() < INSERT_NODE_PROB {
        let pos = if program.nodes.is_empty() { 0 } else { rng.index(program.nodes.len()) };
        program.nodes.insert(pos, random_node(rng));
    }
    if program.nodes.len() > 1 && rng.f32_unit() < DELETE_NODE_PROB {
        let pos = rng.index(program.nodes.len());
        program.nodes.remove(pos);
    }
    if !program.nodes.is_empty() && rng.f32_unit() < SUBTREE_REPLACE_PROB {
        let pos = rng.index(program.nodes.len());
        program.nodes[pos] = random_node(rng);
    }
    while program.nodes.len() > PROGRAM_MAX_NODES {
        program.nodes.pop();
    }
}

/// Single-point crossover: take parent A's prefix and parent B's suffix at
/// a random split, then apply point + structural mutation.
pub fn crossover_and_mutate(a: &Program, b: &Program, rng: &mut Rng) -> Program {
    let max_len = a.len().max(b.len());
    let split = if max_len == 0 { 0 } else { rng.index(max_len + 1) };
    let mut nodes: SmallVec<[Node; PROGRAM_INLINE]> = SmallVec::new();
    for &n in a.nodes.iter().take(split) {
        if nodes.len() < PROGRAM_MAX_NODES {
            nodes.push(n);
        }
    }
    for &n in b.nodes.iter().skip(split) {
        if nodes.len() < PROGRAM_MAX_NODES {
            nodes.push(n);
        }
    }
    let mut child = Program { nodes };
    point_mutate(&mut child, rng);
    structural_mutate(&mut child, rng);
    child
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_grazer_is_bounded() {
        let g = starter_grazer();
        assert!(!g.is_empty());
        assert!(g.len() <= PROGRAM_MAX_NODES);
    }

    #[test]
    fn arity_matches_eval_contract() {
        assert_eq!(Program::arity(Node::Const(0.0)), 0);
        assert_eq!(Program::arity(Node::Add), 2);
        assert_eq!(Program::arity(Node::IfThenElse), 3);
        assert_eq!(Program::arity(Node::MoveTowardX), 1);
    }

    #[test]
    fn is_output_matches_action_nodes() {
        assert!(Program::is_output(Node::MoveTowardX));
        assert!(Program::is_output(Node::Mate));
        assert!(Program::is_output(Node::Idle));
        assert!(!Program::is_output(Node::Const(0.0)));
        assert!(!Program::is_output(Node::Add));
    }

    #[test]
    fn from_slice_truncates_to_max() {
        let huge = vec![Node::Const(0.0); 200];
        let p = Program::from_slice(&huge);
        assert_eq!(p.len(), PROGRAM_MAX_NODES);
    }

    use crate::genome::Genome;

    fn dummy_ctx(genome: &Genome) -> EvalContext<'_> {
        EvalContext {
            energy: 30.0,
            age: 100,
            genome,
            nearest_distance: 5.0,
            nearest_dir: glam::Vec2::new(1.0, 0.0),
            plant_dir: glam::Vec2::new(0.0, 1.0),
            local_biomass: 8.0,
            same_distance: f32::INFINITY,
            same_dir: glam::Vec2::ZERO,
            other_distance: f32::INFINITY,
            other_dir: glam::Vec2::ZERO,
            rel_size: 0.0,
            rel_energy: 0.0,
            crowding: 0.0,
        }
    }

    #[test]
    fn empty_program_yields_zero_action() {
        let p = Program::empty();
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.move_x, 0.0);
        assert_eq!(a.move_y, 0.0);
    }

    #[test]
    fn const_plus_move_writes_action_register() {
        let p = Program::from_slice(&[Node::Const(1.0), Node::MoveTowardX]);
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.move_x, 1.0);
    }

    #[test]
    fn arithmetic_chain_works() {
        // Compute 1 + 2 * 3 = 7, then MoveTowardX of that.
        let p = Program::from_slice(&[
            Node::Const(1.0),
            Node::Const(2.0),
            Node::Const(3.0),
            Node::Mul,
            Node::Add,
            Node::MoveTowardX,
        ]);
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.move_x, 7.0);
    }

    #[test]
    fn ifthenelse_semantics() {
        // Stack order: cond, then, else
        let p = Program::from_slice(&[
            Node::Const(1.0),  // cond (truthy)
            Node::Const(42.0), // then
            Node::Const(99.0), // else
            Node::IfThenElse,
            Node::MoveTowardX,
        ]);
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.move_x, 42.0);
    }

    #[test]
    fn underflow_is_safe() {
        let p = Program::from_slice(&[Node::Add, Node::MoveTowardX]);
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let _ = evaluate(&p, dummy_ctx(&g), &mut stack);
    }

    #[test]
    fn grazer_heads_toward_plant() {
        let p = starter_grazer();
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert!(a.move_y > 0.0, "grazer should head +y toward plant_dir: {:?}", a);
    }

    #[test]
    fn grazer_mates_when_well_fed() {
        let p = starter_grazer();
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let ctx = EvalContext { energy: 50.0, ..dummy_ctx(&g) };
        let a = evaluate(&p, ctx, &mut stack);
        assert!(a.mate_intent > 0.0);
    }

    #[test]
    fn point_mutate_preserves_length() {
        let mut rng = Rng::from_seed(7);
        let mut p = starter_grazer();
        let len = p.len();
        for _ in 0..50 {
            point_mutate(&mut p, &mut rng);
        }
        assert_eq!(p.len(), len);
    }

    #[test]
    fn structural_mutate_stays_in_bounds() {
        let mut rng = Rng::from_seed(11);
        let mut p = starter_grazer();
        for _ in 0..1000 {
            structural_mutate(&mut p, &mut rng);
            assert!(!p.is_empty());
            assert!(p.len() <= PROGRAM_MAX_NODES);
        }
    }

    #[test]
    fn crossover_with_identical_parents_stays_in_bounds() {
        let mut rng = Rng::from_seed(13);
        let p = starter_grazer();
        for _ in 0..200 {
            let c = crossover_and_mutate(&p, &p, &mut rng);
            assert!(c.len() <= PROGRAM_MAX_NODES);
        }
    }

    #[test]
    fn crossover_is_deterministic() {
        let p = starter_grazer();
        let mut r1 = Rng::from_seed(99);
        let mut r2 = Rng::from_seed(99);
        let c1 = crossover_and_mutate(&p, &p, &mut r1);
        let c2 = crossover_and_mutate(&p, &p, &mut r2);
        assert_eq!(c1, c2);
    }

    #[test]
    fn fire_emit_broadcast_write_intents() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let p = Program::from_slice(&[
            Node::Const(0.8),
            Node::FireWeapon,
            Node::Const(0.5),
            Node::EmitPheromone(2),
            Node::Const(0.9),
            Node::Broadcast(1),
        ]);
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.fire_intent, 0.8);
        assert_eq!(a.emit_intent[2], 0.5);
        assert_eq!(a.broadcast_intent[1], 0.9);
        assert_eq!(a.target_id, NO_TARGET);
    }

    #[test]
    fn out_of_range_channels_clamp() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let p = Program::from_slice(&[
            Node::Const(1.0),
            Node::EmitPheromone(250), // clamps to last pheromone channel
            Node::Const(1.0),
            Node::Broadcast(250), // clamps to last meme channel
        ]);
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.emit_intent[PHEROMONE_CHANNELS - 1], 1.0);
        assert_eq!(a.broadcast_intent[MEME_CHANNELS - 1], 1.0);
    }

    #[test]
    fn fire_intent_accumulates_and_idle_discards() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        // Two FireWeapon outputs accumulate (+=), and an Idle output writes
        // no intent while still draining its stack value.
        let p = Program::from_slice(&[
            Node::Const(0.3),
            Node::FireWeapon,
            Node::Const(0.4),
            Node::FireWeapon,
            Node::Const(99.0),
            Node::Idle,
        ]);
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert!((a.fire_intent - 0.7).abs() < 1e-6, "0.3 + 0.4 = 0.7, got {}", a.fire_intent);
        assert_eq!(a.emit_intent[0], 0.0);
        assert_eq!(a.broadcast_intent[0], 0.0);
        assert_eq!(a.feed_intent, 0.0);
    }

    #[test]
    fn social_starters_are_bounded_and_evaluable() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        for make in [starter_stalker, starter_pack_hunter, starter_sentinel, starter_herd] {
            let p = make();
            assert!(!p.is_empty());
            assert!(p.len() <= PROGRAM_MAX_NODES);
            // Must evaluate without panicking against a populated context.
            let _ = evaluate(&p, dummy_ctx(&g), &mut stack);
        }
    }

    #[test]
    fn herd_moves_toward_same_species() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let ctx = EvalContext { same_dir: glam::Vec2::new(1.0, 0.0), ..dummy_ctx(&g) };
        let a = evaluate(&starter_herd(), ctx, &mut stack);
        assert!(a.move_x > 0.0, "herd should move toward same-species: {:?}", a);
    }

    #[test]
    fn starter_library_has_all_starters() {
        assert_eq!(starter_library().len(), 5);
    }

    #[test]
    fn new_sense_nodes_push_context_values() {
        let g = Genome::neutral();
        let ctx = EvalContext {
            same_distance: 6.0,
            same_dir: glam::Vec2::new(1.0, 0.0),
            other_distance: 3.0,
            other_dir: glam::Vec2::new(0.0, 1.0),
            rel_size: 2.0,
            rel_energy: 0.5,
            crowding: 4.0,
            ..dummy_ctx(&g)
        };
        let mut stack = Vec::new();
        let p = Program::from_slice(&[Node::SenseRelSize, Node::MoveTowardX]);
        let a = evaluate(&p, ctx, &mut stack);
        assert_eq!(a.move_x, 2.0);
        let p2 = Program::from_slice(&[Node::SenseCrowding, Node::MoveTowardY]);
        let a2 = evaluate(&p2, ctx, &mut stack);
        assert_eq!(a2.move_y, 4.0);
    }
}
