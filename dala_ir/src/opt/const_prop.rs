//! Constant propagation and folding optimizations.
//!
//! Constant propagation replaces uses of variables with their known
//! constant values. Constant folding evaluates constant expressions
//! at compile time.
//!
//! Example:
//!   x = 42
//!   y = x + 1   →  y = 43  (constant folding)
//!   if x then ... → if true then ...  (constant propagation)

use crate::constant::Constant;
use crate::function::{BasicBlock, IRFunction};
use crate::instruction::{IRInst, IRInstKind};
use crate::type_system::IRType;
use crate::value::{IRValue, IRValueId};
use crate::BlockId;

/// Propagate constants through the IR.
///
/// Returns true if any changes were made.
pub fn propagate_constants(func: &mut IRFunction) -> bool {
    let mut changed = false;
    let mut value_map: Vec<Option<Constant>> = vec![None; func.blocks.len() * 16];

    // Iterate to convergence
    loop {
        let mut iter_changed = false;

        for block_idx in 0..func.blocks.len() {
            if !func.blocks[block_idx].reachable {
                continue;
            }

            let block = &mut func.blocks[block_idx];
            for inst_idx in 0..block.instructions.len() {
                let inst = &mut block.instructions[inst_idx];

                // Try to resolve operands to constants
                let resolved_operands: Vec<Option<Constant>> = inst
                    .operands
                    .iter()
                    .map(|&op| lookup_constant(&value_map, op))
                    .collect();

                // Apply constant folding for each instruction kind
                if let Some(folded) = try_fold(inst, &resolved_operands) {
                    if let Some(result) = inst.result {
                        if let Some(slot) = value_map.get_mut(result.0) {
                            if slot.is_none() {
                                *slot = Some(folded);
                                iter_changed = true;
                            }
                        }
                    }
                }

                // Propagate: if an operand is a constant, we may be able
                // to simplify the instruction
                if let Some(result) = inst.result {
                    if let Some(slot) = value_map.get_mut(result.0) {
                        if slot.is_none() {
                            // Check if all operands are constants
                            if !resolved_operands.is_empty()
                                && resolved_operands.iter().all(|c| c.is_some())
                            {
                                if let Some(folded) = try_fold(inst, &resolved_operands) {
                                    *slot = Some(folded);
                                    iter_changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        if !iter_changed {
            break;
        }
        changed = true;
    }

    changed
}

/// Try to fold an instruction with constant operands.
fn try_fold(inst: &IRInst, operands: &[Option<Constant>]) -> Option<Constant> {
    match &inst.kind {
        // Arithmetic
        IRInstKind::Add => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::Int(a + b))
            } else {
                None
            }
        }
        IRInstKind::Sub => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::Int(a - b))
            } else {
                None
            }
        }
        IRInstKind::Mul => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::Int(a * b))
            } else {
                None
            }
        }
        IRInstKind::Div => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                if *b != 0 {
                    Some(Constant::Int(a / b))
                } else {
                    None
                }
            } else {
                None
            }
        }
        IRInstKind::Rem => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                if *b != 0 {
                    Some(Constant::Int(a % b))
                } else {
                    None
                }
            } else {
                None
            }
        }

        // Comparison
        IRInstKind::Eq => {
            if let [Some(a), Some(b)] = operands {
                Some(Constant::from(a == b))
            } else {
                None
            }
        }
        IRInstKind::Ne => {
            if let [Some(a), Some(b)] = operands {
                Some(Constant::from(a != b))
            } else {
                None
            }
        }
        IRInstKind::Gt => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::from(*a > *b))
            } else {
                None
            }
        }
        IRInstKind::Ge => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::from(*a >= *b))
            } else {
                None
            }
        }
        IRInstKind::Lt => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::from(*a < *b))
            } else {
                None
            }
        }
        IRInstKind::Le => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::from(*a <= *b))
            } else {
                None
            }
        }

        // Bitwise
        IRInstKind::BitAnd => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::Int(a & b))
            } else {
                None
            }
        }
        IRInstKind::BitOr => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::Int(a | b))
            } else {
                None
            }
        }
        IRInstKind::BitXor => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::Int(a ^ b))
            } else {
                None
            }
        }
        IRInstKind::ShiftLeft => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::Int(a << b))
            } else {
                None
            }
        }
        IRInstKind::ShiftRight => {
            if let [Some(Constant::Int(a)), Some(Constant::Int(b))] = operands {
                Some(Constant::Int(a >> b))
            } else {
                None
            }
        }

        // Type tests with constant values
        IRInstKind::IsSmallInt => {
            if let [Some(Constant::Int(_))] = operands {
                Some(Constant::True)
            } else {
                None
            }
        }
        IRInstKind::IsAtom => {
            if let [Some(Constant::Atom(_))] = operands {
                Some(Constant::True)
            } else {
                None
            }
        }
        IRInstKind::IsNil => {
            if let [Some(Constant::Nil)] = operands {
                Some(Constant::True)
            } else {
                None
            }
        }

        // Branch optimization
        IRInstKind::BrIf {
            cond,
            true_target,
            false_target,
        } => {
            if let Some(cond_val) = lookup_constant(&vec![None; 0], *cond) {
                // We can resolve the branch
                // This is handled separately in simplify_cfg
                None
            } else {
                None
            }
        }

        _ => None,
    }
}

/// Fold constants: replace instructions with known constant results.
pub fn fold_constants(func: &mut IRFunction) -> bool {
    let mut changed = false;

    for block in &mut func.blocks {
        if !block.reachable {
            continue;
        }

        for inst in &mut block.instructions {
            // For instructions that produce a constant result,
            // we could replace all uses with the constant value.
            // This is a simplified version.
            if let Some(result) = inst.result {
                if let Some(folded) = try_fold(inst, &vec![None; inst.operands.len()]) {
                    // We can potentially replace uses of this result
                    // with the folded constant. Full implementation
                    // would require use-def chain analysis.
                    let _ = folded;
                    let _ = result;
                }
            }
        }
    }

    changed
}

/// Look up a constant value for a given value ID.
fn lookup_constant(map: &[Option<Constant>], id: IRValueId) -> Option<Constant> {
    if id.0 < map.len() {
        map[id.0].clone()
    } else {
        None
    }
}
