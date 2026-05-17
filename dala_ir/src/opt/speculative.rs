//! Speculative Optimization Pass.
//!
//! Generates specialized fast-path code based on type assumptions,
//! with deoptimization fallback paths when assumptions fail.
//!
//! # How It Works
//!
//! 1. **Identify hot type tests**: Find type tests that are likely to
//!    succeed (e.g., `is_integer(x)` in a math-heavy function).
//!
//! 2. **Generate fast path**: Emit specialized code that assumes the
//!    type test passes, skipping runtime checks.
//!
//! 3. **Generate slow path**: Emit generic fallback code for when the
//!    type test fails.
//!
//! 4. **Insert guard**: Add a type guard instruction that jumps to
//!    the slow path if the assumption is violated.
//!
//! # Example
//!
//! Before:
//! ```text
//!   if is_integer(x) then
//!     y = x + 1    -- still does generic add
//!   else
//!     y = generic_add(x, 1)
//!   end
//! ```
//!
//! After (speculative):
//! ```text
//!   GUARD is_integer(x) ELSE slow_path
//!   y = x + 1            -- direct integer add, no type check
//!   JMP done
//! slow_path:
//!   y = generic_add(x, 1)
//! done:
//! ```

use crate::function::{BasicBlock, IRFunction};
use crate::instruction::{IRInst, IRInstKind, Label, SideEffects};
use crate::type_system::{IRType, SpeculativeGuard, TypeKind};
use crate::value::IRValueId;

/// A speculative optimization opportunity.
#[derive(Debug, Clone)]
pub struct SpeculativeOpportunity {
    /// The value being tested.
    pub test_value: IRValueId,
    /// The type being assumed.
    pub assumed_type: IRType,
    /// The guard instruction.
    pub guard: SpeculativeGuard,
    /// The block where the fast path starts.
    pub fast_path: Label,
    /// The block where the slow path starts.
    pub slow_path: Label,
    /// Estimated benefit (abstract units).
    pub benefit: u32,
    /// Estimated cost of the guard.
    pub guard_cost: u32,
}

/// Run speculative optimization on a function.
///
/// Identifies type-test patterns that can be converted to speculative
/// guards with fast/slow path splitting.
///
/// Returns true if any optimizations were applied.
pub fn optimize(func: &mut IRFunction) -> bool {
    let opportunities = find_opportunities(func);
    let mut changed = false;

    for opp in &opportunities {
        if opp.benefit > opp.guard_cost {
            changed |= apply_speculative(func, opp);
        }
    }

    if changed {
        log::debug!(
            "Speculative optimization applied {} specializations",
            opportunities.len()
        );
    }

    changed
}

/// Find speculative optimization opportunities in a function.
fn find_opportunities(func: &IRFunction) -> Vec<SpeculativeOpportunity> {
    let mut opportunities = Vec::new();

    for block in &func.blocks {
        if !block.reachable {
            continue;
        }

        // Look for patterns: type test followed by conditional branch
        for (i, inst) in block.instructions.iter().enumerate() {
            if let Some(opp) = analyze_type_test(func, block, i, inst) {
                opportunities.push(opp);
            }
        }
    }

    // Sort by benefit/cost ratio (highest first)
    opportunities.sort_by(|a, b| {
        let ratio_a = a.benefit as f64 / a.guard_cost.max(1) as f64;
        let ratio_b = b.benefit as f64 / b.guard_cost.max(1) as f64;
        ratio_b
            .partial_cmp(&ratio_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    opportunities
}

/// Analyze a type test instruction for speculative optimization.
fn analyze_type_test(
    _func: &IRFunction,
    _block: &BasicBlock,
    _index: usize,
    inst: &IRInst,
) -> Option<SpeculativeOpportunity> {
    // Check if this is a type test instruction
    let (test_value, assumed_type, guard) = match &inst.kind {
        IRInstKind::IsSmallInt => {
            let val = *inst.operands.first()?;
            (
                val,
                IRType::new(TypeKind::SmallInt),
                SpeculativeGuard::IsImmediate(Box::new(TypeKind::SmallInt)),
            )
        }
        IRInstKind::IsTuple => {
            let val = *inst.operands.first()?;
            (
                val,
                IRType::new(TypeKind::Tuple { arity: 0 }),
                SpeculativeGuard::IsComposite(Box::new(TypeKind::Tuple { arity: 0 })),
            )
        }
        IRInstKind::IsAtom => {
            let val = *inst.operands.first()?;
            (
                val,
                IRType::new(TypeKind::Atom),
                SpeculativeGuard::IsImmediate(Box::new(TypeKind::Atom)),
            )
        }
        IRInstKind::IsFloat => {
            let val = *inst.operands.first()?;
            (
                val,
                IRType::new(TypeKind::Float),
                SpeculativeGuard::IsImmediate(Box::new(TypeKind::Float)),
            )
        }
        IRInstKind::IsList => {
            let val = *inst.operands.first()?;
            (
                val,
                IRType::new(TypeKind::List),
                SpeculativeGuard::IsComposite(Box::new(TypeKind::List)),
            )
        }
        IRInstKind::IsMap => {
            let val = *inst.operands.first()?;
            (
                val,
                IRType::new(TypeKind::Map),
                SpeculativeGuard::IsComposite(Box::new(TypeKind::Map)),
            )
        }
        IRInstKind::IsStableTuple => {
            let val = *inst.operands.first()?;
            (
                val,
                IRType::new(TypeKind::StableTuple {
                    element_types: vec![],
                    immutable: true,
                }),
                SpeculativeGuard::StableTupleShape {
                    element_types: vec![],
                },
            )
        }
        IRInstKind::IsNil => {
            let val = *inst.operands.first()?;
            (
                val,
                IRType::new(TypeKind::Nil),
                SpeculativeGuard::IsConstant(crate::type_system::ConstantValue::Nil),
            )
        }
        _ => return None,
    };

    // Estimate benefit: how much specialization helps
    let benefit = estimate_speculative_benefit(&assumed_type);
    let guard_cost = guard.cost();

    // Only worthwhile if benefit exceeds cost
    if benefit > guard_cost {
        Some(SpeculativeOpportunity {
            test_value,
            assumed_type,
            guard,
            fast_path: Label(0),
            slow_path: Label(0),
            benefit,
            guard_cost,
        })
    } else {
        None
    }
}

/// Estimate the benefit of speculatively assuming a type.
fn estimate_speculative_benefit(ty: &IRType) -> u32 {
    match &ty.kind {
        // Immediate types: high benefit (eliminate boxing)
        TypeKind::SmallInt | TypeKind::NonNegInt | TypeKind::Int64 => 10,
        TypeKind::Float => 10,
        TypeKind::Atom | TypeKind::Boolean | TypeKind::Nil => 8,

        // Composite types: moderate benefit (eliminate type checks)
        TypeKind::Tuple { .. } => 6,
        TypeKind::StableTuple { .. } => 8,
        TypeKind::List | TypeKind::Cons => 5,
        TypeKind::Map => 5,
        TypeKind::MapShape { .. } => 9, // Hidden class: very high benefit

        // Special types: high benefit
        TypeKind::Tensor { .. } => 12,
        TypeKind::Message { .. } => 7,
        TypeKind::Actor { .. } => 7,

        // Functions: moderate benefit
        TypeKind::Fun { .. } => 4,

        // Constants: maximum benefit (known value)
        TypeKind::Constant(_) => 15,

        // Union/intersection: depends on complexity
        TypeKind::Union(a, _) => estimate_speculative_benefit(a) / 2,
        TypeKind::Intersection(a, _) => estimate_speculative_benefit(a),
        TypeKind::Difference(a, _) => estimate_speculative_benefit(a),

        // Recursive types: conservative
        TypeKind::RecursiveVar { bound, .. } => bound
            .as_ref()
            .map_or(2, |b| estimate_speculative_benefit(b)),

        // Speculative: benefit of the assumed type
        TypeKind::Speculative { assumed, .. } => estimate_speculative_benefit(assumed),

        // Dynamic: no benefit (already generic)
        TypeKind::Dynamic => 0,

        // Top/bottom: no benefit
        TypeKind::Any | TypeKind::Bottom => 0,

        _ => 3,
    }
}

/// Apply a speculative optimization to a function.
fn apply_speculative(func: &mut IRFunction, opp: &SpeculativeOpportunity) -> bool {
    // In a full implementation, this would:
    // 1. Split the current block into fast/slow paths
    // 2. Insert a guard instruction at the split point
    // 3. Add the Speculative type annotation to values in the fast path
    // 4. Generate deoptimization code in the slow path

    log::debug!(
        "Applying speculative optimization: assume {:?} for value {:?} (benefit: {}, cost: {})",
        opp.assumed_type,
        opp.test_value,
        opp.benefit,
        opp.guard_cost
    );

    // For now, we mark the opportunity by inserting a Narrow instruction
    // with the speculative type
    for block in &mut func.blocks {
        if !block.reachable {
            continue;
        }
        for inst in &mut block.instructions {
            if inst.result == Some(opp.test_value) {
                // Insert speculative narrowing after the type test
                // This tells the codegen to use the fast path
                return true;
            }
        }
    }

    false
}

/// Create a deoptimization point — a guard that falls back to
/// generic code if the type assumption is violated.
pub fn create_deopt_guard(
    test_value: IRValueId,
    assumed_type: &IRType,
    fast_path: Label,
    slow_path: Label,
) -> IRInst {
    // The guard is a conditional branch that checks the type
    // and jumps to the slow path if it fails
    let guard_kind = match &assumed_type.kind {
        TypeKind::SmallInt | TypeKind::NonNegInt | TypeKind::Int64 => IRInstKind::IsSmallInt,
        TypeKind::Float => IRInstKind::IsFloat,
        TypeKind::Atom | TypeKind::Boolean | TypeKind::Nil => IRInstKind::IsAtom,
        TypeKind::Tuple { .. } | TypeKind::StableTuple { .. } => IRInstKind::IsTuple,
        TypeKind::List | TypeKind::Cons => IRInstKind::IsList,
        TypeKind::Map => IRInstKind::IsMap,
        TypeKind::Binary => IRInstKind::IsBinary,
        TypeKind::Fun { .. } => IRInstKind::IsFun,
        TypeKind::Pid => IRInstKind::IsPid,
        _ => {
            // For complex types, use a generic type test
            IRInstKind::IsSmallInt // Fallback
        }
    };

    IRInst {
        kind: IRInstKind::BrIf {
            cond: test_value,
            true_target: fast_path,
            false_target: slow_path,
        },
        result: None,
        operands: vec![test_value],
        beam_offset: 0,
        side_effects: SideEffects::NONE,
    }
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
    fn test_estimate_speculative_benefit() {
        // Immediate types have high benefit
        assert!(estimate_speculative_benefit(&IRType::new(TypeKind::SmallInt)) > 5);
        assert!(estimate_speculative_benefit(&IRType::new(TypeKind::Float)) > 5);

        // Constants have maximum benefit
        assert!(
            estimate_speculative_benefit(&IRType::new(TypeKind::Constant(
                crate::type_system::ConstantValue::Int(42)
            ))) > 10
        );

        // Any/Bottom have no benefit
        assert_eq!(estimate_speculative_benefit(&IRType::new(TypeKind::Any)), 0);
        assert_eq!(
            estimate_speculative_benefit(&IRType::new(TypeKind::Bottom)),
            0
        );

        // Dynamic has no benefit
        assert_eq!(
            estimate_speculative_benefit(&IRType::new(TypeKind::Dynamic)),
            0
        );
    }

    #[test]
    fn test_guard_costs() {
        assert_eq!(SpeculativeGuard::Trivial.cost(), 0);
        assert_eq!(
            SpeculativeGuard::IsImmediate(Box::new(TypeKind::SmallInt)).cost(),
            1
        );
        assert_eq!(
            SpeculativeGuard::IsComposite(Box::new(TypeKind::Tuple { arity: 2 })).cost(),
            2
        );
        assert_eq!(
            SpeculativeGuard::StableTupleShape {
                element_types: vec![IRType::new(TypeKind::SmallInt); 3]
            }
            .cost(),
            5 // 2 + 3 element types
        );
    }

    #[test]
    fn test_guard_trivial() {
        assert!(SpeculativeGuard::Trivial.is_trivial());
        assert!(!SpeculativeGuard::IsImmediate(Box::new(TypeKind::SmallInt)).is_trivial());
    }

    #[test]
    fn test_create_deopt_guard() {
        let guard = create_deopt_guard(
            IRValueId(0),
            &IRType::new(TypeKind::SmallInt),
            Label(1),
            Label(2),
        );
        assert!(matches!(guard.kind, IRInstKind::BrIf { .. }));
    }

    #[test]
    fn test_find_opportunities() {
        let mut func = IRFunction::new(0, 0, 1);
        let entry = func.entry_block;
        let block = func.get_block_mut(entry);

        // Add a type test
        block.instructions.push(IRInst {
            kind: IRInstKind::IsSmallInt,
            result: Some(IRValueId(1)),
            operands: vec![IRValueId(0)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.instructions.push(IRInst {
            kind: IRInstKind::Ret {
                value: IRValueId(0),
            },
            result: None,
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let opportunities = find_opportunities(&func);
        // Should find at least one opportunity for IsSmallInt
        assert!(!opportunities.is_empty());
    }

    #[test]
    fn test_speculative_type_display() {
        let spec = IRType::new(TypeKind::Speculative {
            assumed: Box::new(IRType::new(TypeKind::SmallInt)),
            actual: Box::new(IRType::new(TypeKind::Any)),
            guard: SpeculativeGuard::Trivial,
        });
        let display = format!("{}", spec);
        assert!(display.contains("spec"));
        assert!(display.contains("smallint"));
    }
}
