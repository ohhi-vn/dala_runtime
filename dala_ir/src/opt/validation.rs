// SSA IR validation pass.
//
// Checks invariants that must hold after IR construction and after
// every optimization pass. Run automatically in debug builds via
// the `validate!()` macro.

use std::collections::{HashMap, HashSet};

use crate::function::{BasicBlock, IRFunction};
use crate::instruction::{IRInst, IRInstKind, Label};
use crate::value::IRValueId;
use crate::{BlockId, IRValue};

// ── Public API ─────────────────────────────────────────────────────────────

/// Run all validation passes on `func`.
/// Returns `Ok(())` or a `Vec` of every violation found.
pub fn validate(func: &IRFunction) -> Result<(), ValidationErrors> {
    let mut ctx = ValidationCtx::new(func);
    ctx.run();
    if ctx.errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(ctx.errors))
    }
}

/// Convenience macro: validate in debug builds, panic on failure.
#[macro_export]
macro_rules! validate_ir {
    ($func:expr) => {
        #[cfg(debug_assertions)]
        {
            if let Err(errs) = $crate::opt::validation::validate($func) {
                panic!("IR validation failed in '{}':\n{}", $func.full_name(), errs);
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
    /// A value is defined more than once (SSA violation).
    RedefinedValue { value: IRValueId, block: BlockId },
    /// A value is used but never defined.
    UndefinedUse {
        value: IRValueId,
        block: BlockId,
        instr_index: usize,
    },
    /// A use is not dominated by its definition.
    NotDominated {
        value: IRValueId,
        used_in: BlockId,
        defined_in: BlockId,
    },
    /// Block has no terminator.
    MissingTerminator { block: BlockId },
    /// Block has more than one terminator.
    MultipleTerminators { block: BlockId },
    /// Entry block has predecessors.
    EntryHasPredecessors { block: BlockId },
    /// Function has no blocks.
    EmptyFunction,
    /// Successor/predecessor mismatch.
    MissingPredecessor {
        block: BlockId,
        expected_pred: BlockId,
    },
    MissingSuccessor {
        block: BlockId,
        expected_succ: BlockId,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RedefinedValue { value, block } => write!(
                f,
                "SSA: value {:?} defined more than once (block {:?})",
                value, block
            ),
            Self::UndefinedUse {
                value,
                block,
                instr_index,
            } => write!(
                f,
                "SSA: use of undefined value {:?} in block {:?} instr {}",
                value, block, instr_index
            ),
            Self::NotDominated {
                value,
                used_in,
                defined_in,
            } => write!(
                f,
                "DOM: value {:?} defined in {:?} does not dominate use in {:?}",
                value, defined_in, used_in
            ),
            Self::MissingTerminator { block } => {
                write!(f, "TERM: block {:?} has no terminator", block)
            }
            Self::MultipleTerminators { block } => {
                write!(f, "TERM: block {:?} has more than one terminator", block)
            }
            Self::EntryHasPredecessors { block } => write!(
                f,
                "ENTRY: entry block {:?} must have no predecessors",
                block
            ),
            Self::EmptyFunction => write!(f, "FUNC: function has no blocks"),
            Self::MissingPredecessor {
                block,
                expected_pred,
            } => write!(
                f,
                "CFG: block {:?} missing expected predecessor {:?}",
                block, expected_pred
            ),
            Self::MissingSuccessor {
                block,
                expected_succ,
            } => write!(
                f,
                "CFG: block {:?} missing expected successor {:?}",
                block, expected_succ
            ),
        }
    }
}

// ── Internal context ───────────────────────────────────────────────────────

struct ValidationCtx<'a> {
    func: &'a IRFunction,
    errors: Vec<ValidationError>,
    /// Map from IRValueId → defining BlockId.
    def_sites: HashMap<IRValueId, BlockId>,
    /// Dominator approximation: block → set of blocks it dominates.
    dominators: HashMap<BlockId, HashSet<BlockId>>,
}

impl<'a> ValidationCtx<'a> {
    fn new(func: &'a IRFunction) -> Self {
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

    fn run(&mut self) {
        if self.func.blocks.is_empty() {
            self.err(ValidationError::EmptyFunction);
            return;
        }
        self.check_entry();
        self.collect_definitions();
        self.build_dominators();
        self.check_cfg_symmetry();
        self.check_blocks();
    }

    fn check_entry(&mut self) {
        let entry = self.func.entry_block;
        let block = &self.func.blocks[entry.0];
        if !block.predecessors.is_empty() {
            self.err(ValidationError::EntryHasPredecessors { block: entry });
        }
    }

    fn collect_definitions(&mut self) {
        for (block_id, block) in self.func.blocks.iter().enumerate() {
            let bid = BlockId(block_id);
            for inst in &block.instructions {
                if let Some(result) = inst.result {
                    if self.def_sites.insert(result, bid).is_some() {
                        self.err(ValidationError::RedefinedValue {
                            value: result,
                            block: bid,
                        });
                    }
                }
            }
        }
    }

    fn build_dominators(&mut self) {
        // Simple approximation: each block dominates itself.
        // TODO: implement Lengauer-Tarjan for precise dominance.
        for (block_id, _) in self.func.blocks.iter().enumerate() {
            let bid = BlockId(block_id);
            let mut s = HashSet::new();
            s.insert(bid);
            self.dominators.insert(bid, s);
        }
    }

    fn dominates(&self, def_block: BlockId, use_block: BlockId) -> bool {
        if def_block == use_block {
            return true;
        }
        self.dominators
            .get(&def_block)
            .map(|d| d.contains(&use_block))
            .unwrap_or(false)
    }

    fn check_cfg_symmetry(&mut self) {
        for (block_id, block) in self.func.blocks.iter().enumerate() {
            let bid = BlockId(block_id);
            for succ in &block.successors {
                let succ_id = BlockId(succ.0 as usize);
                if succ_id.0 < self.func.blocks.len() {
                    let succ_block = &self.func.blocks[succ_id.0];
                    if !succ_block.predecessors.contains(&Label(bid.0 as u32)) {
                        self.err(ValidationError::MissingPredecessor {
                            block: succ_id,
                            expected_pred: bid,
                        });
                    }
                }
            }
        }
    }

    fn check_blocks(&mut self) {
        for (block_id, block) in self.func.blocks.iter().enumerate() {
            let bid = BlockId(block_id);
            self.check_instructions(bid, block);
            self.check_terminator(bid, block);
        }
    }

    fn check_instructions(&mut self, block_id: BlockId, block: &BasicBlock) {
        for (i, inst) in block.instructions.iter().enumerate() {
            for &operand in &inst.operands {
                self.check_value_defined(operand, block_id, i);
                self.check_dominance(operand, block_id);
            }
        }
    }

    fn check_terminator(&mut self, block_id: BlockId, block: &BasicBlock) {
        let term_count = block
            .instructions
            .iter()
            .filter(|i| is_terminator(&i.kind))
            .count();
        match term_count {
            0 => self.err(ValidationError::MissingTerminator { block: block_id }),
            1 => {}
            _ => self.err(ValidationError::MultipleTerminators { block: block_id }),
        }
    }

    fn check_value_defined(&mut self, value: IRValueId, block: BlockId, instr_index: usize) {
        if !self.def_sites.contains_key(&value) {
            self.err(ValidationError::UndefinedUse {
                value,
                block,
                instr_index,
            });
        }
    }

    fn check_dominance(&mut self, value: IRValueId, use_block: BlockId) {
        if let Some(&def_block) = self.def_sites.get(&value) {
            if !self.dominates(def_block, use_block) {
                self.err(ValidationError::NotDominated {
                    value,
                    used_in: use_block,
                    defined_in: def_block,
                });
            }
        }
    }
}

/// Check whether an instruction kind is a block terminator.
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

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::IRFunction;
    use crate::instruction::{IRInst, IRInstKind, Label, SideEffects};
    use crate::value::IRValueId;

    fn make_function_with_block(inst_kinds: Vec<IRInstKind>) -> IRFunction {
        let mut func = IRFunction::new(0, 0, 1);
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
    fn valid_function_passes() {
        // A minimal valid function: define a value then return it.
        let mut func = IRFunction::new(0, 0, 0);
        let entry = func.entry_block;
        let block = func.get_block_mut(entry);
        // Define a constant value
        block.instructions.push(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 42 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        // Return that value
        block.instructions.push(IRInst {
            kind: IRInstKind::Ret {
                value: IRValueId(0),
            },
            result: None,
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        let result = validate(&func);
        assert!(
            result.is_ok(),
            "valid function should pass validation, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn catches_missing_terminator() {
        let func = make_function_with_block(vec![]);
        let result = validate(&func);
        assert!(result.is_err());
        let errs = result.unwrap_err().0;
        assert!(
            errs.iter()
                .any(|e| matches!(e, ValidationError::MissingTerminator { .. }))
        );
    }

    #[test]
    fn catches_multiple_terminators() {
        let func = make_function_with_block(vec![
            IRInstKind::Ret {
                value: IRValueId(0),
            },
            IRInstKind::Ret {
                value: IRValueId(0),
            },
        ]);
        let result = validate(&func);
        assert!(result.is_err());
        let errs = result.unwrap_err().0;
        assert!(
            errs.iter()
                .any(|e| matches!(e, ValidationError::MultipleTerminators { .. }))
        );
    }

    #[test]
    fn catches_empty_function() {
        let func = IRFunction::new(0, 0, 0);
        // Clear the entry block to make it truly empty
        let mut func = func;
        func.blocks.clear();
        let result = validate(&func);
        assert!(result.is_err());
        let errs = result.unwrap_err().0;
        assert!(
            errs.iter()
                .any(|e| matches!(e, ValidationError::EmptyFunction))
        );
    }

    #[test]
    fn catches_entry_with_predecessors() {
        let mut func = make_function_with_block(vec![IRInstKind::Ret {
            value: IRValueId(0),
        }]);
        let entry = func.entry_block;
        func.get_block_mut(entry).predecessors.push(Label(99));
        let result = validate(&func);
        assert!(result.is_err());
        let errs = result.unwrap_err().0;
        assert!(
            errs.iter()
                .any(|e| matches!(e, ValidationError::EntryHasPredecessors { .. }))
        );
    }
}
