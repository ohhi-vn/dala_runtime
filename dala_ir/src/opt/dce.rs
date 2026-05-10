//! Dead Code Elimination (DCE) - removes unreachable and unused code.
//!
//! DCE works in two phases:
//! 1. Mark phase: Walk from the entry block, marking all reachable blocks
//!    and used values.
//! 2. Sweep phase: Remove unreachable blocks and unused instructions.

use crate::function::{BasicBlock, IRFunction};
use crate::instruction::{IRInst, IRInstKind};
use crate::value::IRValueId;
use crate::BlockId;

/// Eliminate dead code from a function.
///
/// Returns true if any code was removed.
pub fn eliminate_dead_code(func: &mut IRFunction) -> bool {
    let mut changed = false;

    // Phase 1: Mark reachable blocks
    let reachable = mark_reachable(func);

    // Phase 2: Remove unreachable blocks
    changed |= remove_unreachable_blocks(func, &reachable);

    // Phase 3: Remove unused instructions within reachable blocks
    changed |= remove_unused_instructions(func);

    changed
}

/// Mark all reachable blocks from the entry block.
fn mark_reachable(func: &IRFunction) -> Vec<bool> {
    let mut reachable = vec![false; func.blocks.len()];
    let mut worklist = vec![func.entry_block];

    while let Some(block_id) = worklist.pop() {
        if reachable[block_id.0] {
            continue;
        }
        reachable[block_id.0] = true;

        let block = &func.blocks[block_id.0];
        for successor in &block.successors {
            if !reachable[successor.0] {
                worklist.push(*successor);
            }
        }
    }

    reachable
}

/// Remove unreachable blocks from the function.
fn remove_unreachable_blocks(func: &mut IRFunction, reachable: &[bool]) -> bool {
    let mut changed = false;

    for (i, &is_reachable) in reachable.iter().enumerate() {
        if !is_reachable {
            let block = &mut func.blocks[i];
            if block.reachable {
                block.reachable = false;
                block.instructions.clear();
                block.successors.clear();
                changed = true;
            }
        }
    }

    changed
}

/// Remove instructions whose results are never used.
fn remove_unused_instructions(func: &mut IRFunction) -> bool {
    let mut changed = false;

    // Collect all used values
    let mut used_values = vec![false; func.blocks.len() * 16]; // Estimate

    // First pass: mark all values used by operands
    for block in &func.blocks {
        if !block.reachable {
            continue;
        }
        for inst in &block.instructions {
            for &operand in &inst.operands {
                mark_used(operand, &mut used_values);
            }
            // Terminator instructions are always "used"
            if is_terminator(&inst.kind) {
                if let Some(result) = inst.result {
                    mark_used(result, &mut used_values);
                }
            }
        }
    }

    // Second pass: remove instructions with unused results
    for block in &mut func.blocks {
        if !block.reachable {
            continue;
        }
        block.instructions.retain(|inst| {
            if let Some(result) = inst.result {
                if is_used(result, &used_values) {
                    return true;
                }
                // Side-effecting instructions are kept even if result unused
                if has_side_effects(&inst.kind) {
                    return true;
                }
                changed = true;
                false
            } else {
                true // Instructions without results are always kept
            }
        });
    }

    changed
}

fn mark_used(value: IRValueId, used: &mut Vec<bool>) {
    let idx = value.0;
    if idx >= used.len() {
        used.resize(idx + 1, false);
    }
    used[idx] = true;
}

fn is_used(value: IRValueId, used: &[bool]) -> bool {
    value.0 < used.len() && used[value.0]
}

/// Check if an instruction kind has side effects.
fn has_side_effects(kind: &IRInstKind) -> bool {
    match kind {
        IRInstKind::Alloc { .. }
        | IRInstKind::Store { .. }
        | IRInstKind::SetReg { .. }
        | IRInstKind::SetStackPtr { .. }
        | IRInstKind::Br { .. }
        | IRInstKind::BrIf { .. }
        | IRInstKind::Switch { .. }
        | IRInstKind::Ret { .. }
        | IRInstKind::Call { .. }
        | IRInstKind::TailCall { .. }
        | IRInstKind::CallBif { .. }
        | IRInstKind::Throw { .. }
        | IRInstKind::Catch { .. }
        | IRInstKind::CatchPop
        | IRInstKind::Send { .. }
        | IRInstKind::Recv { .. }
        | IRInstKind::GcSafe
        | IRInstKind::ConsumeReductions { .. } => true,
        _ => false,
    }
}

/// Check if an instruction is a terminator (ends a basic block).
fn is_terminator(kind: &IRInstKind) -> bool {
    matches!(
        kind,
        IRInstKind::Br { .. }
            | IRInstKind::BrIf { .. }
            | IRInstKind::Switch { .. }
            | IRInstKind::Ret { .. }
            | IRInstKind::TailCall { .. }
            | IRInstKind::Throw { .. }
    )
}
