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

// Task 5 adds: use crate::genome::{GenomeSlot, GENOME_LEN};
// Task 5 adds: use crate::rng::Rng;

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
}
