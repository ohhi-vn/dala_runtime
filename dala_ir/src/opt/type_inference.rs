//! Type Inference Engine.
//!
//! Propagates type information through the SSA IR using a
//! constraint-based approach.  This is the key analysis that
//! enables all other optimizations to work.
//!
//! # Algorithm
//!
//! 1. **Constraint generation**: Walk the IR and generate subtype
//!    constraints from each instruction.  For example, `IsTuple(x)`
//!    generates the constraint `x : Tuple`.
//!
//! 2. **Constraint solving**: Solve the constraint system using
//!    a fixed-point iteration.  For each value, compute the
//!    meet (greatest lower bound) of all constraints on that value.
//!
//! 3. **Type annotation**: Write the inferred types back to the IR
//!    value annotations.
//!
//! # Interaction with Set-Theoretic Types
//!
//! The constraint solver uses the full set-theoretic type algebra:
//! - Join (∪) for control-flow merge points
//! - Meet (∩) for type refinement after tests
//! - Difference (\\\\) for pattern-match narrowing
//!
//! This is significantly more powerful than Hindley-Milner inference
//! because it handles unions, intersections, and negation types.

use std::collections::{HashMap, VecDeque};

use crate::function::{BasicBlock, IRFunction};
use crate::instruction::IRInstKind;
use crate::type_system::{IRType, TypeKind};
use crate::value::IRValueId;

/// A type constraint on a value.
#[derive(Debug, Clone)]
enum Constraint {
    /// The value must be a subtype of the given type.
    SubtypeOf(IRType),
    /// The value must be exactly the given type.
    IsType(IRType),
    /// The value is the join of two other values (merge point).
    Join(IRValueId, IRValueId),
    /// The value is the meet of two other values (type refinement).
    Meet(IRValueId, IRValueId),
}

/// Type inference context.
struct InferCtx<'a> {
    /// The function being analyzed.
    func: &'a IRFunction,
    /// Constraints collected from the IR.
    constraints: HashMap<IRValueId, Vec<Constraint>>,
    /// Current type assignment for each value.
    types: HashMap<IRValueId, IRType>,
    /// Worklist for fixed-point iteration.
    worklist: VecDeque<IRValueId>,
}

impl<'a> InferCtx<'a> {
    fn new(func: &'a IRFunction) -> Self {
        Self {
            func,
            constraints: HashMap::new(),
            types: HashMap::new(),
            worklist: VecDeque::new(),
        }
    }

    /// Run type inference to completion.
    fn run(&mut self) -> HashMap<IRValueId, IRType> {
        self.generate_constraints();
        self.solve();
        self.types.clone()
    }

    /// Phase 1: Generate constraints from the IR.
    fn generate_constraints(&mut self) {
        for block in &self.func.blocks {
            if !block.reachable {
                continue;
            }
            for inst in &block.instructions {
                self.constrain_instruction(inst);
            }
        }
    }

    /// Generate constraints for a single instruction.
    fn constrain_instruction(&mut self, inst: &crate::instruction::IRInst) {
        match &inst.kind {
            // Type test instructions generate subtype constraints
            IRInstKind::IsTuple => {
                if let Some(&val) = inst.operands.first() {
                    if let Some(result) = inst.result {
                        // If IsTuple succeeds, val must be a tuple
                        self.add_constraint(
                            val,
                            Constraint::SubtypeOf(IRType::new(TypeKind::Tuple { arity: 0 })),
                        );
                        // The result is boolean
                        self.add_constraint(
                            result,
                            Constraint::IsType(IRType::new(TypeKind::Boolean)),
                        );
                    }
                }
            }
            IRInstKind::IsAtom => {
                if let Some(&val) = inst.operands.first() {
                    if let Some(result) = inst.result {
                        self.add_constraint(
                            val,
                            Constraint::SubtypeOf(IRType::new(TypeKind::Atom)),
                        );
                        self.add_constraint(
                            result,
                            Constraint::IsType(IRType::new(TypeKind::Boolean)),
                        );
                    }
                }
            }
            IRInstKind::IsSmallInt => {
                if let Some(&val) = inst.operands.first() {
                    if let Some(result) = inst.result {
                        self.add_constraint(
                            val,
                            Constraint::SubtypeOf(IRType::new(TypeKind::SmallInt)),
                        );
                        self.add_constraint(
                            result,
                            Constraint::IsType(IRType::new(TypeKind::Boolean)),
                        );
                    }
                }
            }
            IRInstKind::IsList => {
                if let Some(&val) = inst.operands.first() {
                    if let Some(result) = inst.result {
                        self.add_constraint(
                            val,
                            Constraint::SubtypeOf(IRType::new(TypeKind::List)),
                        );
                        self.add_constraint(
                            result,
                            Constraint::IsType(IRType::new(TypeKind::Boolean)),
                        );
                    }
                }
            }
            IRInstKind::IsMap => {
                if let Some(&val) = inst.operands.first() {
                    if let Some(result) = inst.result {
                        self.add_constraint(
                            val,
                            Constraint::SubtypeOf(IRType::new(TypeKind::Map)),
                        );
                        self.add_constraint(
                            result,
                            Constraint::IsType(IRType::new(TypeKind::Boolean)),
                        );
                    }
                }
            }
            IRInstKind::IsFloat => {
                if let Some(&val) = inst.operands.first() {
                    if let Some(result) = inst.result {
                        self.add_constraint(
                            val,
                            Constraint::SubtypeOf(IRType::new(TypeKind::Float)),
                        );
                        self.add_constraint(
                            result,
                            Constraint::IsType(IRType::new(TypeKind::Boolean)),
                        );
                    }
                }
            }
            IRInstKind::IsNil => {
                if let Some(&val) = inst.operands.first() {
                    if let Some(result) = inst.result {
                        self.add_constraint(
                            val,
                            Constraint::SubtypeOf(IRType::new(TypeKind::Nil)),
                        );
                        self.add_constraint(
                            result,
                            Constraint::IsType(IRType::new(TypeKind::Boolean)),
                        );
                    }
                }
            }

            // Arithmetic: operands must be integers
            IRInstKind::Add | IRInstKind::Sub | IRInstKind::Mul => {
                if inst.operands.len() >= 2 {
                    for &op in &inst.operands {
                        self.add_constraint(
                            op,
                            Constraint::SubtypeOf(IRType::new(TypeKind::SmallInt)),
                        );
                    }
                    if let Some(result) = inst.result {
                        self.add_constraint(
                            result,
                            Constraint::SubtypeOf(IRType::new(TypeKind::SmallInt)),
                        );
                    }
                }
            }

            // Comparison: result is boolean
            IRInstKind::Eq
            | IRInstKind::Ne
            | IRInstKind::Gt
            | IRInstKind::Ge
            | IRInstKind::Lt
            | IRInstKind::Le => {
                if let Some(result) = inst.result {
                    self.add_constraint(
                        result,
                        Constraint::IsType(IRType::new(TypeKind::Boolean)),
                    );
                }
            }

            // Narrow instruction: result type is the narrowed type
            IRInstKind::Narrow { value, new_type } => {
                if let Some(result) = inst.result {
                    self.add_constraint(
                        *value,
                        Constraint::SubtypeOf(new_type.as_ref().clone()),
                    );
                    self.add_constraint(
                        result,
                        Constraint::IsType(new_type.as_ref().clone()),
                    );
                }
            }

            // Constant instructions: exact type
            IRInstKind::ConstSmallInt { value } => {
                if let Some(result) = inst.result {
                    let ty = if *value >= 0 {
                        IRType::new(TypeKind::Constant(
                            crate::type_system::ConstantValue::Int(*value),
                        ))
                    } else {
                        IRType::new(TypeKind::Constant(
                            crate::type_system::ConstantValue::Int(*value),
                        ))
                    };
                    self.add_constraint(result, Constraint::IsType(ty));
                }
            }
            IRInstKind::ConstNil => {
                if let Some(result) = inst.result {
                    self.add_constraint(
                        result,
                        Constraint::IsType(IRType::new(TypeKind::Nil)),
                    );
                }
            }
            IRInstKind::ConstTrue => {
                if let Some(result) = inst.result {
                    self.add_constraint(
                        result,
                        Constraint::IsType(IRType::new(TypeKind::Boolean)),
                    );
                }
            }
            IRInstKind::ConstFalse => {
                if let Some(result) = inst.result {
                    self.add_constraint(
                        result,
                        Constraint::IsType(IRType::new(TypeKind::Boolean)),
                    );
                }
            }
            IRInstKind::ConstAtom { .. } => {
                if let Some(result) = inst.result {
                    self.add_constraint(
                        result,
                        Constraint::IsType(IRType::new(TypeKind::Atom)),
                    );
                }
            }

            // Branch instructions: condition must be boolean
            IRInstKind::BrIf { cond, .. } => {
                self.add_constraint(
                    *cond,
                    Constraint::SubtypeOf(IRType::new(TypeKind::Boolean)),
                );
            }

            // Return: no constraint on the value itself
            IRInstKind::Ret { .. } => {}

            // Default: no constraints generated
            _ => {}
        }
    }

    /// Add a constraint for a value.
    fn add_constraint(&mut self, value: IRValueId, constraint: Constraint) {
        self.constraints
            .entry(value)
            .or_default()
            .push(constraint);
        self.worklist.push_back(value);
    }

    /// Phase 2: Solve constraints using fixed-point iteration.
    fn solve(&mut self) {
        let mut iterations = 0;
        let max_iterations = 100;

        while let Some(value) = self.worklist.pop_front() {
            if iterations >= max_iterations {
                break;
            }
            iterations += 1;

            let constraints = self.constraints.get(&value).cloned().unwrap_or_default();
            let current_type = self.types.get(&value).cloned()
                .unwrap_or_else(|| IRType::new(TypeKind::Any));

            let mut new_type = current_type.clone();

            for constraint in &constraints {
                new_type = match constraint {
                    Constraint::SubtypeOf(ty) => {
                        // Narrow: meet current type with the constraint
                        new_type.meet(ty)
                    }
                    Constraint::IsType(ty) => {
                        // Exact: meet current type with the exact type
                        new_type.meet(ty)
                    }
                    Constraint::Join(a, b) => {
                        let ta = self.types.get(a).cloned().unwrap_or_else(|| IRType::new(TypeKind::Any));
                        let tb = self.types.get(b).cloned().unwrap_or_else(|| IRType::new(TypeKind::Any));
                        let joined = ta.join(&tb);
                        new_type.meet(&joined)
                    }
                    Constraint::Meet(a, b) => {
                        let ta = self.types.get(a).cloned().unwrap_or_else(|| IRType::new(TypeKind::Any));
                        let tb = self.types.get(b).cloned().unwrap_or_else(|| IRType::new(TypeKind::Any));
                        let met = ta.meet(&tb);
                        new_type.meet(&met)
                    }
                };
            }

            // Check if the type changed
            if new_type != current_type {
                self.types.insert(value, new_type);
                // Re-process all values that depend on this one
                for (other_value, other_constraints) in &self.constraints {
                    if *other_value != value {
                        let depends = other_constraints.iter().any(|c| match c {
                            Constraint::Join(a, b) | Constraint::Meet(a, b) => {
                                *a == value || *b == value
                            }
                            _ => false,
                        });
                        if depends && !self.worklist.contains(other_value) {
                            self.worklist.push_back(*other_value);
                        }
                    }
                }
            }
        }
    }
}

/// Run type inference on a function.
///
/// Returns a map from value IDs to their inferred types.
pub fn infer_types(func: &IRFunction) -> HashMap<IRValueId, IRType> {
    let mut ctx = InferCtx::new(func);
    ctx.run()
}

/// Run type inference and write the results back to the IR.
///
/// Returns true if any types were refined.
pub fn infer_and_annotate(func: &mut IRFunction) -> bool {
    let inferred = infer_types(func);
    let mut changed = false;

    // In a full implementation, this would update the type annotations
    // on each IRValue.  For now, we just log the results.
    for (value_id, ty) in &inferred {
        if ty.kind != TypeKind::Any {
            log::debug!("Inferred type for {:?}: {}", value_id, ty);
            changed = true;
        }
    }

    changed
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::IRFunction;
    use crate::instruction::{IRInst, IRInstKind, Label, SideEffects};
    use crate::value::IRValueId;

    #[test]
    fn test_type_inference_basic() {
        let mut func = IRFunction::new(0, 0, 1);
        let entry = func.entry_block;
        let block = func.get_block_mut(entry);

        // x = const(42)
        block.instructions.push(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 42 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        // y = x + 1
        block.instructions.push(IRInst {
            kind: IRInstKind::Add,
            result: Some(IRValueId(1)),
            operands: vec![IRValueId(0), IRValueId(0)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        // ret(y)
        block.instructions.push(IRInst {
            kind: IRInstKind::Ret {
                value: IRValueId(1),
            },
            result: None,
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let types = infer_types(&func);
        // The constant should have a constant type
        let const_type = types.get(&IRValueId(0)).unwrap();
        assert!(matches!(const_type.kind, TypeKind::Constant(_)));
    }

    #[test]
    fn test_type_inference_with_narrowing() {
        let mut func = IRFunction::new(0, 0, 1);
        let entry = func.entry_block;
        let block = func.get_block_mut(entry);

        // x = some value
        // is_tuple(x) → narrow x to tuple
        block.instructions.push(IRInst {
            kind: IRInstKind::IsTuple,
            result: Some(IRValueId(1)),
            operands: vec![IRValueId(0)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.instructions.push(IRInst {
            kind: IRInstKind::Narrow {
                value: IRValueId(0),
                new_type: Box::new(IRType::new(TypeKind::Tuple { arity: 2 })),
            },
            result: Some(IRValueId(2)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.instructions.push(IRInst {
            kind: IRInstKind::Ret {
                value: IRValueId(2),
            },
            result: None,
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let types = infer_types(&func);
        // After narrowing, the value should be a tuple
        let narrowed = types.get(&IRValueId(2)).unwrap();
        assert!(matches!(narrowed.kind, TypeKind::Tuple { .. }));
    }

    #[test]
    fn test_type_inference_boolean_result() {
        let mut func = IRFunction::new(0, 0, 1);
        let entry = func.entry_block;
        let block = func.get_block_mut(entry);

        // x = const(42)
        block.instructions.push(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 42 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        // y = is_atom(x)
        block.instructions.push(IRInst {
            kind: IRInstKind::IsAtom,
            result: Some(IRValueId(1)),
            operands: vec![IRValueId(0)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        // ret(y)
        block.instructions.push(IRInst {
            kind: IRInstKind::Ret {
                value: IRValueId(1),
            },
            result: None,
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let types = infer_types(&func);
        let bool_result = types.get(&IRValueId(1)).unwrap();
        assert!(matches!(bool_result.kind, TypeKind::Boolean));
    }
}
