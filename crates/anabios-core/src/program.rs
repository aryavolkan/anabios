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
}

/// What an agent wants to do this tick, produced by the evaluator.
#[derive(Debug, Clone, Copy, Default)]
pub struct ActionRegister {
    pub move_x: f32,
    pub move_y: f32,
    pub feed_intent: f32,
    pub mate_intent: f32,
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
}

/// Canned starter: basic herbivore that always heads toward plants and
/// mates when well-fed. Simple by design — evolution discovers more
/// sophisticated strategies via mutation.
///
/// The `decide()` wrapper normalizes the action register's move vector
/// to a unit direction, so the magnitude of the components doesn't
/// matter — only the sign / ratio.
pub fn starter_grazer() -> Program {
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

/// Library of starter programs. Founders use index 0.
pub fn starter_library() -> &'static [fn() -> Program] {
    &[starter_grazer]
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
            Node::FireWeapon | Node::EmitPheromone(_) | Node::Broadcast(_) | Node::Idle => {
                scratch.pop();
            }
        }
    }

    action
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
}
