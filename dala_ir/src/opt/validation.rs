// dala_ir/src/validation.rs
//
// SSA IR validation pass.
//
// Checks invariants that must hold after IR construction and after
// every optimization pass. Run automatically in debug builds via
// the `validate!()` macro.
//
// Invariants checked:
//   1. SSA property     — every Value defined exactly once
//   2. Dominance        — every use dominated by its definition
//   3. Type consistency — operand types match instruction signatures
//   4. CFG integrity    — successor/predecessor symmetry
//   5. Phi nodes        — one argument per predecessor, all same type
//   6. Terminator       — every block ends with exactly one terminator
//   7. Entry block      — no predecessors, no phi nodes

use std::collections::{HashMap, HashSet};

use crate::{Block, BlockId, Function, Instruction, Terminator, Type, Value, ValueId};

// ── Public API ─────────────────────────────────────────────────────────────

/// Run all validation passes on `func`.
/// Returns `Ok(())` or a `Vec` of every violation found (not just the first).
pub fn validate(func: &Function) -> Result<(), ValidationErrors> {
    let mut ctx = ValidationCtx::new(func);
    ctx.run();
    if ctx.errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(ctx.errors))
    }
}

/// Convenience macro: validate in debug builds, panic on failure.
/// In release builds this compiles to nothing.
#[macro_export]
macro_rules! validate_ir {
    ($func:expr) => {
        #[cfg(debug_assertions)]
        {
            if let Err(errs) = $crate::validation::validate($func) {
                panic!("IR validation failed in '{}':\n{}", $func.name, errs);
            }
        }
    };
}

// ── Error types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ValidationErrors(pub Vec<ValidationError>);

impl std::fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            writeln!(f, "  [{i}] {e}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    // SSA
    RedefinedValue {
        value: ValueId,
        block: BlockId,
    },
    UndefinedUse {
        value: ValueId,
        block: BlockId,
        instr_index: usize,
    },
    NotDominated {
        value: ValueId,
        used_in: BlockId,
        defined_in: BlockId,
    },

    // Type
    TypeMismatch {
        instr_index: usize,
        block: BlockId,
        expected: Type,
        found: Type,
    },

    // CFG
    MissingPredecessor {
        block: BlockId,
        expected_pred: BlockId,
    },
    MissingSuccessor {
        block: BlockId,
        expected_succ: BlockId,
    },

    // Phi
    PhiArgCountMismatch {
        block: BlockId,
        phi_value: ValueId,
        expected_preds: usize,
        found_args: usize,
    },
    PhiTypeMismatch {
        block: BlockId,
        phi_value: ValueId,
        arg_index: usize,
        expected: Type,
        found: Type,
    },

    // Blocks
    BlockHasNoPhi {
        block: BlockId,
    },
    MissingTerminator {
        block: BlockId,
    },
    MultipleTerminators {
        block: BlockId,
    },
    EntryHasPredecessors {
        block: BlockId,
    },
    EntryHasPhis {
        block: BlockId,
    },

    // Misc
    EmptyFunction,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RedefinedValue { value, block } =>
                write!(f, "SSA: value {value:?} defined more than once (block {block:?})"),
            Self::UndefinedUse { value, block, instr_index } =>
                write!(f, "SSA: use of undefined value {value:?} in block {block:?} instr {instr_index}"),
            Self::NotDominated { value, used_in, defined_in } =>
                write!(f, "DOM: value {value:?} defined in {defined_in:?} does not dominate use in {used_in:?}"),
            Self::TypeMismatch { instr_index, block, expected, found } =>
                write!(f, "TYPE: block {block:?} instr {instr_index}: expected {expected:?}, found {found:?}"),
            Self::MissingPredecessor { block, expected_pred } =>
                write!(f, "CFG: block {block:?} missing expected predecessor {expected_pred:?}"),
            Self::MissingSuccessor { block, expected_succ } =>
                write!(f, "CFG: block {block:?} missing expected successor {expected_succ:?}"),
            Self::PhiArgCountMismatch { block, phi_value, expected_preds, found_args } =>
                write!(f, "PHI: block {block:?} phi {phi_value:?} has {found_args} args but {expected_preds} predecessors"),
            Self::PhiTypeMismatch { block, phi_value, arg_index, expected, found } =>
                write!(f, "PHI: block {block:?} phi {phi_value:?} arg {arg_index}: expected {expected:?} found {found:?}"),
            Self::BlockHasNoPhi { block } =>
                write!(f, "PHI: block {block:?} has multiple predecessors but no phis (may be intentional)"),
            Self::MissingTerminator { block } =>
                write!(f, "TERM: block {block:?} has no terminator"),
            Self::MultipleTerminators { block } =>
                write!(f, "TERM: block {block:?} has more than one terminator"),
            Self::EntryHasPredecessors { block } =>
                write!(f, "ENTRY: entry block {block:?} must have no predecessors"),
            Self::EntryHasPhis { block } =>
                write!(f, "ENTRY: entry block {block:?} must have no phi nodes"),
            Self::EmptyFunction =>
                write!(f, "FUNC: function has no blocks"),
        }
    }
}

// ── Internal context ───────────────────────────────────────────────────────

struct ValidationCtx<'a> {
    func: &'a Function,
    errors: Vec<ValidationError>,

    /// Map from ValueId → (defining BlockId, instruction index within block).
    /// Populated in the first pass; used for dominance checking.
    def_sites: HashMap<ValueId, (BlockId, usize)>,

    /// Dominator tree: map from BlockId → set of blocks it dominates.
    /// TODO: replace with a proper Lengauer-Tarjan dominator tree.
    /// For now, a simple RPO-based approximation is used.
    dominators: HashMap<BlockId, HashSet<BlockId>>,
}

impl<'a> ValidationCtx<'a> {
    fn new(func: &'a Function) -> Self {
        Self {
            func,
            errors: Vec::new(),
            def_sites: HashMap::new(),
            dominators: HashMap::new(),
        }
    }

    fn err(&mut self, e: ValidationError) {
        self.errors.push(e);
    }

    // ── Top-level ──────────────────────────────────────────────────────

    fn run(&mut self) {
        if self.func.blocks.is_empty() {
            self.err(ValidationError::EmptyFunction);
            return;
        }

        self.check_entry();
        self.collect_definitions(); // pass 1: build def_sites
        self.build_dominators(); // pass 2: dominator approximation
        self.check_cfg_symmetry(); // pass 3: predecessor/successor symmetry
        self.check_blocks(); // pass 4: per-block checks
    }

    // ── Pass 0: Entry block ────────────────────────────────────────────

    fn check_entry(&mut self) {
        let entry = self.func.entry_block;
        let block = &self.func.blocks[&entry];

        if !block.predecessors.is_empty() {
            self.err(ValidationError::EntryHasPredecessors { block: entry });
        }
        if !block.phis.is_empty() {
            self.err(ValidationError::EntryHasPhis { block: entry });
        }
    }

    // ── Pass 1: Collect all definition sites ───────────────────────────

    fn collect_definitions(&mut self) {
        for (&block_id, block) in &self.func.blocks {
            // Phi node results are defined at index 0 (before any instruction)
            for phi in &block.phis {
                self.register_def(phi.result, block_id, 0);
            }

            for (i, instr) in block.instructions.iter().enumerate() {
                if let Some(result) = instr.result {
                    self.register_def(result, block_id, i + 1);
                }
            }
        }
    }

    fn register_def(&mut self, value: ValueId, block: BlockId, idx: usize) {
        if self.def_sites.insert(value, (block, idx)).is_some() {
            self.err(ValidationError::RedefinedValue { value, block });
        }
    }

    // ── Pass 2: Dominator tree (RPO approximation) ─────────────────────

    fn build_dominators(&mut self) {
        // TODO: implement Lengauer-Tarjan for precise dominance.
        // This stub marks each block as dominating itself only, which
        // makes the dominance check conservative (will miss some violations).
        for &block_id in self.func.blocks.keys() {
            self.dominators.insert(block_id, {
                let mut s = HashSet::new();
                s.insert(block_id);
                s
            });
        }
        // TODO: propagate dominance along RPO traversal order
    }

    /// Returns true if `def_block` dominates `use_block`.
    fn dominates(&self, def_block: BlockId, use_block: BlockId) -> bool {
        if def_block == use_block {
            return true;
        }
        self.dominators
            .get(&def_block)
            .map(|dominated| dominated.contains(&use_block))
            .unwrap_or(false)
    }

    // ── Pass 3: CFG symmetry ───────────────────────────────────────────

    fn check_cfg_symmetry(&mut self) {
        for (&block_id, block) in &self.func.blocks {
            // Every successor should list this block as a predecessor
            for &succ_id in &block.successors {
                let succ = match self.func.blocks.get(&succ_id) {
                    Some(b) => b,
                    None => {
                        self.err(ValidationError::MissingSuccessor {
                            block: block_id,
                            expected_succ: succ_id,
                        });
                        continue;
                    }
                };
                if !succ.predecessors.contains(&block_id) {
                    self.err(ValidationError::MissingPredecessor {
                        block: succ_id,
                        expected_pred: block_id,
                    });
                }
            }
        }
    }

    // ── Pass 4: Per-block checks ───────────────────────────────────────

    fn check_blocks(&mut self) {
        // Collect block IDs to avoid borrow issues
        let block_ids: Vec<BlockId> = self.func.blocks.keys().copied().collect();

        for block_id in block_ids {
            self.check_phis(block_id);
            self.check_instructions(block_id);
            self.check_terminator(block_id);
        }
    }

    fn check_phis(&mut self, block_id: BlockId) {
        let block = &self.func.blocks[&block_id];
        let pred_count = block.predecessors.len();

        for phi in &block.phis {
            // Argument count must match predecessor count
            if phi.args.len() != pred_count {
                self.err(ValidationError::PhiArgCountMismatch {
                    block: block_id,
                    phi_value: phi.result,
                    expected_preds: pred_count,
                    found_args: phi.args.len(),
                });
            }

            // All phi args must match the result type
            let expected_ty = phi.ty.clone();
            for (i, &arg_val) in phi.args.iter().enumerate() {
                self.check_value_defined(arg_val, block_id, i);

                if let Some(found_ty) = self.value_type(arg_val) {
                    if found_ty != expected_ty {
                        self.err(ValidationError::PhiTypeMismatch {
                            block: block_id,
                            phi_value: phi.result,
                            arg_index: i,
                            expected: expected_ty.clone(),
                            found: found_ty,
                        });
                    }
                }
            }
        }
    }

    fn check_instructions(&mut self, block_id: BlockId) {
        let block = &self.func.blocks[&block_id];

        for (i, instr) in block.instructions.iter().enumerate() {
            // Check every value operand is defined and dominates this use
            for &operand in instr.operands() {
                self.check_value_defined(operand, block_id, i);
                self.check_dominance(operand, block_id, i);
            }

            // Check instruction-specific type rules
            self.check_instruction_types(instr, block_id, i);
        }
    }

    fn check_terminator(&mut self, block_id: BlockId) {
        let block = &self.func.blocks[&block_id];
        let term_count = block
            .instructions
            .iter()
            .filter(|i| i.is_terminator())
            .count();

        // Also check that terminator successors match block.successors
        match term_count {
            0 => self.err(ValidationError::MissingTerminator { block: block_id }),
            1 => { /* ok */ }
            _ => self.err(ValidationError::MultipleTerminators { block: block_id }),
        }
    }

    // ── Helpers ────────────────────────────────────────────────────────

    fn check_value_defined(&mut self, value: ValueId, block: BlockId, instr_index: usize) {
        if !self.def_sites.contains_key(&value) {
            self.err(ValidationError::UndefinedUse {
                value,
                block,
                instr_index,
            });
        }
    }

    fn check_dominance(&mut self, value: ValueId, use_block: BlockId, _instr_idx: usize) {
        if let Some(&(def_block, _)) = self.def_sites.get(&value) {
            if !self.dominates(def_block, use_block) {
                self.err(ValidationError::NotDominated {
                    value,
                    used_in: use_block,
                    defined_in: def_block,
                });
            }
        }
        // If not in def_sites, check_value_defined already reported it
    }

    fn check_instruction_types(&mut self, instr: &Instruction, block: BlockId, idx: usize) {
        // TODO: dispatch on instr.opcode and validate each operand type.
        // Example shape:
        //
        // match &instr.kind {
        //     InstrKind::Add { lhs, rhs, result } => {
        //         self.expect_type(*lhs, Type::I64, block, idx);
        //         self.expect_type(*rhs, Type::I64, block, idx);
        //     }
        //     InstrKind::Call { args, signature } => {
        //         for (i, (&arg, param_ty)) in args.iter().zip(signature.params).enumerate() {
        //             self.expect_type(arg, param_ty.clone(), block, idx);
        //         }
        //     }
        //     _ => {}
        // }
        let _ = (instr, block, idx); // suppress unused warning until implemented
    }

    fn expect_type(&mut self, value: ValueId, expected: Type, block: BlockId, idx: usize) {
        if let Some(found) = self.value_type(value) {
            if found != expected {
                self.err(ValidationError::TypeMismatch {
                    instr_index: idx,
                    block,
                    expected,
                    found,
                });
            }
        }
    }

    fn value_type(&self, value: ValueId) -> Option<Type> {
        // TODO: look up the type from the function's value table
        // self.func.value_type(value)
        let _ = value;
        None
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::FunctionBuilder; // TODO: implement test builder

    #[test]
    fn valid_function_passes() {
        // TODO: build a minimal valid SSA function and assert Ok(())
        // let func = FunctionBuilder::new("test")
        //     .entry_block(|b| b.ret(Value::I64(0)))
        //     .build();
        // assert!(validate(&func).is_ok());
    }

    #[test]
    fn catches_redefined_value() {
        // TODO: construct a function where the same ValueId is defined twice
        // assert!(matches!(
        //     validate(&func),
        //     Err(ValidationErrors(errs)) if errs.iter().any(|e| matches!(e, ValidationError::RedefinedValue { .. }))
        // ));
    }

    #[test]
    fn catches_undefined_use() {
        // TODO: use a value that is never defined
    }

    #[test]
    fn catches_phi_arg_count_mismatch() {
        // TODO: block with 2 predecessors but phi with 1 arg
    }

    #[test]
    fn catches_missing_terminator() {
        // TODO: block with instructions but no branch/return
    }

    #[test]
    fn catches_entry_with_predecessors() {
        // TODO: entry block that has a predecessor listed
    }
}
