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
use crate::function::IRFunction;
use crate::instruction::{IRInst, IRInstKind};
use crate::type_system::IRType;
use crate::value::{IRValue, IRValueId};

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

        _ => None,
    }
}

/// Fold constant expressions: replace instructions whose operands are
/// all constants with a `ConstSmallInt` / `ConstNil` / etc. instruction.
///
/// Returns true if any instruction was replaced.
pub fn fold_constants(func: &mut IRFunction) -> bool {
    use crate::instruction::IRInstKind;

    let mut changed = false;
    let mut value_map: Vec<Option<Constant>> = vec![None; func.blocks.len() * 16];

    for block in &mut func.blocks {
        if !block.reachable {
            continue;
        }

        for inst in &mut block.instructions {
            // Resolve operands
            let resolved: Vec<Option<Constant>> = inst
                .operands
                .iter()
                .map(|&op| lookup_constant(&value_map, op))
                .collect();

            if inst.operands.len() == resolved.len() && resolved.iter().all(|c| c.is_some()) {
                if let Some(folded) = try_fold(inst, &resolved) {
                    // Replace the instruction with a constant load
                    match &folded {
                        Constant::Int(v) => {
                            inst.kind = IRInstKind::ConstSmallInt { value: *v };
                        }
                        Constant::Nil => {
                            inst.kind = IRInstKind::ConstNil;
                        }
                        Constant::True => {
                            inst.kind = IRInstKind::ConstTrue;
                        }
                        Constant::False => {
                            inst.kind = IRInstKind::ConstFalse;
                        }
                        Constant::Atom(a) => {
                            inst.kind = IRInstKind::ConstAtom { index: *a };
                        }
                        Constant::Float(f) => {
                            // Store float bits as a small-int constant for now
                            let bits = f.to_bits() as i64;
                            inst.kind = IRInstKind::ConstSmallInt { value: bits };
                        }
                        _ => continue,
                    }
                    inst.operands.clear();
                    inst.side_effects = crate::instruction::SideEffects::NONE;
                    if let Some(result) = inst.result {
                        if let Some(slot) = value_map.get_mut(result.0) {
                            *slot = Some(folded);
                        }
                    }
                    changed = true;
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
