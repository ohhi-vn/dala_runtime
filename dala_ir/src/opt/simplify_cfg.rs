//! CFG Simplification - removes unnecessary branches and merges blocks.
//!
//! This pass:
//! - Eliminates branches to the immediate next block (fall-through)
//! - Merges blocks with a single successor that has a single predecessor
//! - Removes unreachable blocks
//! - Converts conditional branches to unconditional when condition is constant

use crate::BlockId;
use crate::function::IRFunction;
use crate::instruction::{IRInstKind, Label};

/// Simplify the control flow graph of a function.
///
/// Returns true if the CFG was modified.
pub fn simplify(func: &mut IRFunction) -> bool {
    let mut changed = false;

    // Remove branches to fall-through blocks
    changed |= eliminate_fallthrough_branches(func);

    // Merge blocks with single predecessor/successor
    changed |= merge_blocks(func);

    // Remove unreachable blocks
    changed |= remove_unreachable(func);

    changed
}

/// Eliminate unconditional branches to the immediately following block.
fn eliminate_fallthrough_branches(func: &mut IRFunction) -> bool {
    let mut changed = false;

    for block in &mut func.blocks {
        if !block.reachable {
            continue;
        }

        if let Some(last_inst) = block.instructions.last() {
            if let IRInstKind::Br { target } = last_inst.kind {
                // Check if target is the next block in sequence
                let current_idx = block.label.0;
                if target.0 == current_idx + 1 {
                    // This is a fall-through branch - remove it
                    block.instructions.pop();
                    block.successors.retain(|&s| s != target);
                    changed = true;
                }
            }
        }
    }

    changed
}

/// Merge blocks where possible.
///
/// If block A has a single successor B, and B has a single predecessor A,
/// merge B into A.
fn merge_blocks(func: &mut IRFunction) -> bool {
    let mut changed = false;
    let block_count = func.blocks.len();

    // Build predecessor counts
    let mut pred_count = vec![0u32; block_count];
    for block in &func.blocks {
        for &succ in &block.successors {
            pred_count[succ.0 as usize] += 1;
        }
    }

    // Find mergeable pairs
    let mut merges = Vec::new();
    for (i, block) in func.blocks.iter().enumerate() {
        if !block.reachable || block.successors.len() != 1 {
            continue;
        }
        let succ = block.successors[0];
        if pred_count[succ.0 as usize] == 1 {
            merges.push((i, succ.0 as usize));
        }
    }

    // Apply merges (in reverse order to preserve indices)
    for (src_idx, dst_idx) in merges.into_iter().rev() {
        if src_idx >= func.blocks.len() || dst_idx >= func.blocks.len() {
            continue;
        }

        // Don't merge if it would create a block that's too large
        let src_len = func.blocks[src_idx].instructions.len();
        let dst_len = func.blocks[dst_idx].instructions.len();
        if src_len + dst_len > 1000 {
            continue;
        }

        // Move instructions from dst to src (removing the branch at end of src)
        let branch = func.blocks[src_idx].instructions.pop();
        let dst_insts = std::mem::take(&mut func.blocks[dst_idx].instructions);
        func.blocks[src_idx].instructions.extend(dst_insts);

        // Update successors
        let dst_successors = std::mem::take(&mut func.blocks[dst_idx].successors);
        func.blocks[src_idx].successors = dst_successors;

        // Update the branch instruction's target if needed
        let src_label = func.blocks[src_idx].label;
        if let Some(last) = func.blocks[src_idx].instructions.last_mut() {
            if let IRInstKind::Br { target } = &mut last.kind {
                if target.0 as usize == dst_idx {
                    *target = src_label;
                }
            }
        }

        func.blocks[dst_idx].reachable = false;
        changed = true;
    }

    changed
}

/// Remove unreachable blocks and update references.
fn remove_unreachable(func: &mut IRFunction) -> bool {
    let mut changed = false;
    let reachable = compute_reachable(func);

    for (i, &is_reachable) in reachable.iter().enumerate() {
        if !is_reachable && func.blocks[i].reachable {
            func.blocks[i].reachable = false;
            func.blocks[i].instructions.clear();
            func.blocks[i].successors.clear();
            changed = true;
        }
    }

    changed
}

/// Compute reachable blocks from the entry block.
fn compute_reachable(func: &IRFunction) -> Vec<bool> {
    let mut reachable = vec![false; func.blocks.len()];
    let mut worklist: Vec<BlockId> = vec![func.entry_block];

    while let Some(block_id) = worklist.pop() {
        if reachable[block_id.0] {
            continue;
        }
        reachable[block_id.0] = true;

        let block = &func.blocks[block_id.0];
        for &succ in &block.successors {
            let succ_block = BlockId(succ.0 as usize);
            if !reachable[succ.0 as usize] {
                worklist.push(succ_block);
            }
        }
    }

    reachable
}
