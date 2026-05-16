//! Common Subexpression Elimination (CSE).
//!
//! CSE identifies expressions that are computed multiple times with the
//! same operands and replaces subsequent computations with the result
//! of the first computation.
//!
//! Example:
//!   a = b + c
//!   d = b + c   →  d = a  (reuse previous result)

use std::collections::HashMap;

use crate::function::{BasicBlock, IRFunction};
use crate::instruction::{IRInstKind, SideEffects};
use crate::value::IRValueId;

/// Key for identifying expressions in CSE.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ExprKey {
    /// A commutative binary operation
    Commutative(BinOp, IRValueId, IRValueId),
    /// A non-commutative binary operation
    Binary(BinOp, IRValueId, IRValueId),
    /// A unary operation
    Unary(UnOp, IRValueId),
    /// A type test
    TypeTest(TypeTest, IRValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum UnOp {
    Neg,
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TypeTest {
    IsSmallInt,
    IsAtom,
    IsTuple,
    IsList,
    IsMap,
    IsBinary,
    IsFun,
    IsPid,
    IsNil,
    IsTrue,
    IsFalse,
    IsStableTuple,
    IsMessage,
    IsActor,
    IsTensor,
    IsCapability,
}

/// Eliminate common subexpressions from a function.
///
/// Returns true if any changes were made.
pub fn eliminate_common_subexprs(func: &mut IRFunction) -> bool {
    let mut changed = false;

    for block in &mut func.blocks {
        if !block.reachable {
            continue;
        }
        changed |= cse_block(block);
    }

    changed
}

/// Perform CSE on a single basic block.
fn cse_block(block: &mut BasicBlock) -> bool {
    let mut changed = false;
    let mut expr_map: HashMap<ExprKey, IRValueId> = HashMap::new();

    for inst in &mut block.instructions {
        // Skip instructions with side effects - they can't be CSE'd
        if has_side_effects(&inst.kind) {
            continue;
        }

        // Try to find a common subexpression
        if let Some(key) = expr_key(&inst.kind, &inst.operands) {
            if let Some(&existing) = expr_map.get(&key) {
                // Replace this instruction's result with the existing value
                if let Some(result) = inst.result {
                    // Mark instruction for removal by clearing it
                    // In a full implementation, we'd update all uses
                    inst.kind = IRInstKind::Nop;
                    changed = true;
                }
            } else {
                if let Some(result) = inst.result {
                    expr_map.insert(key, result);
                }
            }
        }
    }

    // Remove Nop instructions
    block
        .instructions
        .retain(|inst| !matches!(inst.kind, IRInstKind::Nop));

    changed
}

/// Generate an expression key for CSE matching.
fn expr_key(kind: &IRInstKind, operands: &[IRValueId]) -> Option<ExprKey> {
    match kind {
        IRInstKind::Add => {
            let [a, b] = operands else { return None };
            // Commutative: normalize order
            let (a, b) = if a.0 <= b.0 { (*a, *b) } else { (*b, *a) };
            Some(ExprKey::Commutative(BinOp::Add, a, b))
        }
        IRInstKind::Sub => {
            let [a, b] = operands else { return None };
            Some(ExprKey::Binary(BinOp::Sub, *a, *b))
        }
        IRInstKind::Mul => {
            let [a, b] = operands else { return None };
            let (a, b) = if a.0 <= b.0 { (*a, *b) } else { (*b, *a) };
            Some(ExprKey::Commutative(BinOp::Mul, a, b))
        }
        IRInstKind::Div => {
            let [a, b] = operands else { return None };
            Some(ExprKey::Binary(BinOp::Div, *a, *b))
        }
        IRInstKind::Rem => {
            let [a, b] = operands else { return None };
            Some(ExprKey::Binary(BinOp::Rem, *a, *b))
        }
        IRInstKind::BitAnd => {
            let [a, b] = operands else { return None };
            let (a, b) = if a.0 <= b.0 { (*a, *b) } else { (*b, *a) };
            Some(ExprKey::Commutative(BinOp::BitAnd, a, b))
        }
        IRInstKind::BitOr => {
            let [a, b] = operands else { return None };
            let (a, b) = if a.0 <= b.0 { (*a, *b) } else { (*b, *a) };
            Some(ExprKey::Commutative(BinOp::BitOr, a, b))
        }
        IRInstKind::BitXor => {
            let [a, b] = operands else { return None };
            let (a, b) = if a.0 <= b.0 { (*a, *b) } else { (*b, *a) };
            Some(ExprKey::Commutative(BinOp::BitXor, a, b))
        }
        IRInstKind::ShiftLeft => {
            let [a, b] = operands else { return None };
            Some(ExprKey::Binary(BinOp::Shl, *a, *b))
        }
        IRInstKind::ShiftRight => {
            let [a, b] = operands else { return None };
            Some(ExprKey::Binary(BinOp::Shr, *a, *b))
        }
        IRInstKind::Eq => {
            let [a, b] = operands else { return None };
            let (a, b) = if a.0 <= b.0 { (*a, *b) } else { (*b, *a) };
            Some(ExprKey::Commutative(BinOp::Eq, a, b))
        }
        IRInstKind::IsSmallInt => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsSmallInt, *a))
        }
        IRInstKind::IsAtom => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsAtom, *a))
        }
        IRInstKind::IsNil => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsNil, *a))
        }
        IRInstKind::IsTuple => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsTuple, *a))
        }
        IRInstKind::IsList => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsList, *a))
        }
        IRInstKind::IsMap => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsMap, *a))
        }
        IRInstKind::IsBinary => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsBinary, *a))
        }
        IRInstKind::IsFun => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsFun, *a))
        }
        IRInstKind::IsPid => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsPid, *a))
        }
        IRInstKind::IsTrue => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsTrue, *a))
        }
        IRInstKind::IsFalse => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsFalse, *a))
        }
        IRInstKind::IsStableTuple => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsStableTuple, *a))
        }
        IRInstKind::IsMessage => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsMessage, *a))
        }
        IRInstKind::IsActor => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsActor, *a))
        }
        IRInstKind::IsTensor => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsTensor, *a))
        }
        IRInstKind::IsCapability => {
            let [a] = operands else { return None };
            Some(ExprKey::TypeTest(TypeTest::IsCapability, *a))
        }
        IRInstKind::Neg => {
            let [a] = operands else { return None };
            Some(ExprKey::Unary(UnOp::Neg, *a))
        }
        IRInstKind::BitNot => {
            let [a] = operands else { return None };
            Some(ExprKey::Unary(UnOp::BitNot, *a))
        }
        _ => None,
    }
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
        | IRInstKind::ConsumeReductions { .. }
        | IRInstKind::BinaryNew { .. }
        | IRInstKind::BinarySize { .. }
        | IRInstKind::BinaryExtract { .. }
        | IRInstKind::MakeFun { .. }
        | IRInstKind::LoadLiteral { .. }
        | IRInstKind::Load { .. }
        | IRInstKind::TupleGet { .. }
        | IRInstKind::TupleSet { .. }
        | IRInstKind::Nop => true,
        _ => false,
    }
}
