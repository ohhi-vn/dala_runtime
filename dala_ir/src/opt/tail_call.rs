//! Tail Call Analysis Pass — identifies and marks tail-position calls.
//!
//! BEAM guarantees proper tail calls: a function's last action may call
//! another function (including itself) without growing the stack.  This is
//! fundamental to actor-loop correctness (e.g. a GenServer's `handle_call`
//! recursing forever must not overflow the stack).
//!
//! This pass walks every basic block of a function and looks for `Call`
//! instructions that appear in *tail position* — i.e. the last
//! side-effecting instruction before a terminator (`Ret`, `Throw`, or
//! another `TailCall`).  Those `Call` nodes are rewritten in-place to
//! `TailCall`, which the codegen later lowers to Cranelift's
//! `return_call` / `return_call_indirect`.
//!
//! Algorithm
//! ---------
//! For each block:
//!   1. Scan instructions from the end backwards.
//!   2. Skip the terminator (`Ret`, `Throw`, `TailCall`).
//!   3. If the preceding instruction is a `Call`, convert it to `TailCall`.
//!   4. If the preceding instruction is *not* a call (e.g. a computation
//!      whose result is returned), then the block has no tail call.
//!
//! This is a single-pass, local analysis — no fixpoint iteration needed.
//!
//! After the pass, the `simplify_cfg` pass may merge blocks so that
//! `TailCall` nodes that target the current function become visible as
//! loops to later passes.

use crate::function::{BasicBlock, IRFunction};
use crate::instruction::IRInstKind;

/// Run the tail-call analysis pass on `func`.
///
/// Returns `true` if any `Call` instruction was converted to `TailCall`.
pub fn analyze(func: &mut IRFunction) -> bool {
    let mut changed = false;

    // We iterate by index so we can mutate in-place.
    for idx in 0..func.blocks.len() {
        if !func.blocks[idx].reachable {
            continue;
        }
        changed |= analyze_block(&mut func.blocks[idx]);
    }

    if changed {
        log::debug!(
            "TailCall analysis: converted calls to tail calls in {}",
            func.full_name()
        );
    }

    changed
}

/// Analyse a single basic block for tail-position calls.
///
/// A `Call` is in tail position when it is the instruction immediately
/// preceding the block terminator (which must be a `Ret`, `Throw`, or
/// already a `TailCall`).  We also handle the case where the only
/// instruction after the `Call` is a `Ret` whose value is exactly the
/// `Call`'s result — this is the common pattern for tail-recursive
/// functions.
fn analyze_block(block: &mut BasicBlock) -> bool {
    let len = block.instructions.len();
    if len < 2 {
        return false;
    }

    // The last instruction must be a terminator.
    let terminator_idx = len - 1;
    if !is_terminator(&block.instructions[terminator_idx].kind) {
        return false;
    }

    // Walk backwards past any `Move` / `Nop` / `SetReg` that merely
    // shuttle the call result into the return register.
    let mut scan = terminator_idx;
    while scan > 0 {
        let prev = scan - 1;
        match &block.instructions[prev].kind {
            // These are transparent — they just move the call result.
            IRInstKind::Nop => {
                scan -= 1;
                continue;
            }
            IRInstKind::Call { func, args } => {
                // Found a call in tail position — convert it.
                block.instructions[prev].kind = IRInstKind::TailCall {
                    func: *func,
                    args: args.clone(),
                };
                return true;
            }
            // Any other instruction means the call (if any) is NOT in
            // tail position.
            _ => return false,
        }
    }

    false
}

/// Check whether an instruction kind is a block terminator.
fn is_terminator(kind: &IRInstKind) -> bool {
    matches!(
        kind,
        IRInstKind::Ret { .. }
            | IRInstKind::Throw { .. }
            | IRInstKind::TailCall { .. }
            | IRInstKind::Br { .. }
            | IRInstKind::BrIf { .. }
            | IRInstKind::Switch { .. }
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::{BasicBlock, IRFunction};
    use crate::instruction::{IRInst, IRInstKind, Label, SideEffects};
    use crate::value::{IRValue, IRValueId};

    /// Helper: build a minimal `IRFunction` with one block whose
    /// instructions are given as a `Vec<IRInstKind>`.  Each instruction
    /// is assigned a fresh `IRValueId` result (except terminators).
    fn make_function_with_block(inst_kinds: Vec<IRInstKind>) -> IRFunction {
        let mut func = IRFunction::new(0, 0, 1);
        // Clear the auto-created entry block instructions and replace.
        let entry_id = func.entry_block;
        let block = func.get_block_mut(entry_id);
        block.instructions.clear();

        for (i, kind) in inst_kinds.into_iter().enumerate() {
            let result = if is_terminator(&kind) {
                None
            } else {
                Some(IRValueId(i))
            };
            block.instructions.push(IRInst {
                kind,
                result,
                operands: vec![],
                beam_offset: 0,
                side_effects: SideEffects::NONE,
            });
        }

        func
    }

    #[test]
    fn test_tail_call_detected_before_ret() {
        // Simulate:  result = call(f, [arg]) ; ret(result)
        let mut func = make_function_with_block(vec![
            IRInstKind::Call {
                func: IRValueId(99),
                args: vec![IRValueId(0)],
            },
            IRInstKind::Ret {
                value: IRValueId(0),
            },
        ]);

        let changed = analyze(&mut func);
        assert!(changed, "tail call should be detected");

        let block = func.get_block(func.entry_block);
        match &block.instructions[0].kind {
            IRInstKind::TailCall { .. } => {} // good
            other => panic!("expected TailCall, got {:?}", other),
        }
    }

    #[test]
    fn test_non_tail_call_not_converted() {
        // Simulate:  result = call(f, [arg]) ; x = add(result, 1) ; ret(x)
        // The call is NOT in tail position because another instruction
        // follows it before the Ret.
        let mut func = make_function_with_block(vec![
            IRInstKind::Call {
                func: IRValueId(99),
                args: vec![IRValueId(0)],
            },
            IRInstKind::Add,
            IRInstKind::Ret {
                value: IRValueId(2),
            },
        ]);

        let changed = analyze(&mut func);
        assert!(!changed, "non-tail call should NOT be converted");

        let block = func.get_block(func.entry_block);
        assert!(matches!(
            block.instructions[0].kind,
            IRInstKind::Call { .. }
        ));
    }

    #[test]
    fn test_tail_call_before_throw() {
        // Tail call before a Throw terminator.
        let mut func = make_function_with_block(vec![
            IRInstKind::Call {
                func: IRValueId(99),
                args: vec![IRValueId(0)],
            },
            IRInstKind::Throw {
                reason: IRValueId(0),
            },
        ]);

        let changed = analyze(&mut func);
        assert!(changed);

        let block = func.get_block(func.entry_block);
        assert!(matches!(
            block.instructions[0].kind,
            IRInstKind::TailCall { .. }
        ));
    }

    #[test]
    fn test_recursive_tail_call_pattern() {
        // Simulate a classic recursive tail call:
        //   _ = self(arg)   // tail call
        //   ret _
        let mut func = make_function_with_block(vec![
            IRInstKind::Call {
                func: IRValueId(42), // "self"
                args: vec![IRValueId(0)],
            },
            IRInstKind::Ret {
                value: IRValueId(0),
            },
        ]);

        assert!(analyze(&mut func));

        // After conversion the first instruction must be a TailCall.
        let block = func.get_block(func.entry_block);
        match &block.instructions[0].kind {
            IRInstKind::TailCall { func, args } => {
                assert_eq!(*func, IRValueId(42));
                assert_eq!(*args, vec![IRValueId(0)]);
            }
            other => panic!("expected TailCall, got {:?}", other),
        }
    }

    #[test]
    fn test_deeply_scanned_nop_skip() {
        // Nop between call and ret should be skipped.
        let mut func = make_function_with_block(vec![
            IRInstKind::Call {
                func: IRValueId(10),
                args: vec![],
            },
            IRInstKind::Nop,
            IRInstKind::Ret {
                value: IRValueId(0),
            },
        ]);

        assert!(analyze(&mut func));

        let block = func.get_block(func.entry_block);
        assert!(matches!(
            block.instructions[0].kind,
            IRInstKind::TailCall { .. }
        ));
    }

    #[test]
    fn test_empty_block_no_panic() {
        let mut func = IRFunction::new(0, 0, 0);
        // Entry block with no instructions.
        let entry = func.entry_block;
        func.get_block_mut(entry).instructions.clear();
        let changed = analyze(&mut func);
        assert!(!changed);
    }

    #[test]
    fn test_single_instruction_no_panic() {
        let mut func = make_function_with_block(vec![IRInstKind::Ret {
            value: IRValueId(0),
        }]);
        let changed = analyze(&mut func);
        assert!(!changed);
    }
}
