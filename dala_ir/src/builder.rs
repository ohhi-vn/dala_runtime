//! IR Builder - constructs the SSA IR from BEAM bytecode.
//!
//! The IR builder translates BEAM instructions into SSA form.
//! It handles:
//! - Register allocation (BEAM uses virtual registers X0-X255, Y0-Y*)
//! - Control flow graph construction
//! - Type inference
//! - SSA value numbering

use indexmap::IndexMap;

use crate::function::{BasicBlock, IRFunction};
use crate::instruction::{IRInst, IRInstKind, Label, Reg, SideEffects};
use crate::type_system::{IRType, IRType as Ty, TypeId};
use crate::value::{IRValue, IRValueId};
use crate::IRFunctionId;

/// The IR builder constructs SSA IR from BEAM bytecode.
pub struct IRBuilder {
    /// The function being built
    pub function: IRFunction,

    /// Current block being filled
    current_block: BlockId,

    /// Value table for SSA construction
    values: Vec<IRValue>,

    /// Register mappings (BEAM register -> current SSA value)
    x_regs: [Option<IRValueId>; 256],
    y_regs: IndexMap<u32, IRValueId>,

    /// Next value ID
    next_value_id: usize,

    /// Pending blocks (for back-patching)
    pending_blocks: Vec<BlockId>,
}

impl IRBuilder {
    /// Create a new IR builder for a function.
    pub fn new(module: u64, name: u64, arity: u32) -> Self {
        let mut function = IRFunction::new(module, name, arity);
        let entry_block = function.entry_block;

        Self {
            function,
            current_block: entry_block,
            values: Vec::new(),
            x_regs: [None; 256],
            y_regs: IndexMap::new(),
            next_value_id: 0,
            pending_blocks: Vec::new(),
        }
    }

    /// Get the current block.
    pub fn current_block(&self) -> BlockId {
        self.current_block
    }

    /// Set the current block.
    pub fn set_current_block(&mut self, block: BlockId) {
        self.current_block = block;
    }

    /// Create a new basic block.
    pub fn create_block(&mut self) -> BlockId {
        self.function.create_block()
    }

    /// Seal the current block and start a new one.
    pub fn seal_block(&mut self) -> BlockId {
        let new_block = self.create_block();
        self.current_block = new_block;
        new_block
    }

    // ===== Value Management =====

    /// Add a value to the value table.
    fn add_value(&mut self, value: IRValue) -> IRValueId {
        let id = IRValueId(self.next_value_id);
        self.next_value_id += 1;
        self.values.push(value);
        id
    }

    /// Get a value by ID.
    pub fn get_value(&self, id: IRValueId) -> &IRValue {
        &self.values[id.0]
    }

    /// Create a constant value.
    pub fn constant(&mut self, value: crate::constant::Constant) -> IRValueId {
        let ty = value.ir_type();
        self.add_value(IRValue::Constant {
            value: value.into(),
            ty,
        })
    }

    /// Create a constant integer value.
    pub fn const_small_int(&mut self, val: i64) -> IRValueId {
        self.add_value(IRValue::Constant {
            value: crate::constant::Constant::Int(val),
            ty: IRType::SmallInt,
        })
    }

    /// Create a constant atom value.
    pub fn const_atom(&mut self, atom_idx: u32) -> IRValueId {
        self.add_value(IRValue::Constant {
            value: crate::constant::Constant::Atom(atom_idx),
            ty: IRType::Atom,
        })
    }

    /// Create a constant nil value.
    pub fn const_nil(&mut self) -> IRValueId {
        self.add_value(IRValue::Constant {
            value: crate::constant::Constant::Nil,
            ty: IRType::Nil,
        })
    }

    /// Create a constant true value.
    pub fn const_true(&mut self) -> IRValueId {
        self.add_value(IRValue::Constant {
            value: crate::constant::Constant::True,
            ty: IRType::Boolean,
        })
    }

    /// Create a constant false value.
    pub fn const_false(&mut self) -> IRValueId {
        self.add_value(IRValue::Constant {
            value: crate::constant::Constant::False,
            ty: IRType::Boolean,
        })
    }

    // ===== Register Operations =====

    /// Read an X register.
    pub fn get_x_reg(&mut self, idx: u32) -> IRValueId {
        if let Some(val_id) = self.x_regs[idx as usize] {
            val_id
        } else {
            // First read - create a placeholder that will be filled by the caller
            let val_id = self.add_value(IRValue::Placeholder);
            self.x_regs[idx as usize] = Some(val_id);
            val_id
        }
    }

    /// Write an X register.
    pub fn set_x_reg(&mut self, idx: u32, value: IRValueId) {
        self.x_regs[idx as usize] = Some(value);
    }

    /// Read a Y register (stack slot).
    pub fn get_y_reg(&mut self, idx: u32) -> IRValueId {
        if let Some(&val_id) = self.y_regs.get(&idx) {
            val_id
        } else {
            let val_id = self.add_value(IRValue::Placeholder);
            self.y_regs.insert(idx, val_id);
            val_id
        }
    }

    /// Write a Y register (stack slot).
    pub fn set_y_reg(&mut self, idx: u32, value: IRValueId) {
        self.y_regs.insert(idx, value);
    }

    // ===== Instruction Emission =====

    /// Emit an instruction with no result.
    pub fn emit(&mut self, kind: IRInstKind) -> InstId {
        self.emit_with_side_effects(kind, SideEffects::NONE)
    }

    /// Emit an instruction with side effects.
    pub fn emit_with_side_effects(&mut self, kind: IRInstKind, se: SideEffects) -> InstId {
        let inst = IRInst {
            kind,
            result: None,
            operands: Vec::new(),
            beam_offset: 0,
            side_effects: se,
        };
        let id = InstId(self.current_block_instructions().len());
        self.current_block_instructions().push(inst);
        id
    }

    /// Emit an instruction that produces a value.
    pub fn emit_with_result(&mut self, kind: IRInstKind, ty: TypeId) -> (InstId, IRValueId) {
        let result_id = self.add_value(IRValue::InstResult {
            inst: InstId(0), // Will be patched
            result_index: 0,
            ty: self.function.module.types[ty.0].clone(),
        });

        let inst = IRInst {
            kind,
            result: Some(result_id),
            operands: Vec::new(),
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        };

        let block = self.current_block_instructions();
        let inst_id = InstId(block.len());
        // Patch the result to point to the correct instruction
        if let IRValue::InstResult {
            inst: ref mut i, ..
        } = self.values[result_id.0]
        {
            *i = inst_id;
        }
        block.push(inst);

        (inst_id, result_id)
    }

    /// Emit an arithmetic instruction.
    pub fn emit_add(&mut self, a: IRValueId, b: IRValueId) -> IRValueId {
        let (_, result) =
            self.emit_with_result(IRInstKind::Add, self.values[a.0].ty().clone().into());
        self.instructions_mut().last_mut().unwrap().operands = vec![a, b];
        self.instructions_mut().last_mut().unwrap().side_effects = SideEffects::NONE;
        result
    }

    pub fn emit_sub(&mut self, a: IRValueId, b: IRValueId) -> IRValueId {
        let (_, result) =
            self.emit_with_result(IRInstKind::Sub, self.values[a.0].ty().clone().into());
        self.instructions_mut().last_mut().unwrap().operands = vec![a, b];
        self.instructions_mut().last_mut().unwrap().side_effects = SideEffects::NONE;
        result
    }

    pub fn emit_mul(&mut self, a: IRValueId, b: IRValueId) -> IRValueId {
        let (_, result) =
            self.emit_with_result(IRInstKind::Mul, self.values[a.0].ty().clone().into());
        self.instructions_mut().last_mut().unwrap().operands = vec![a, b];
        self.instructions_mut().last_mut().unwrap().side_effects = SideEffects::NONE;
        result
    }

    // ===== Control Flow =====

    /// Emit an unconditional branch.
    pub fn emit_br(&mut self, target: BlockId) {
        self.emit(IRInstKind::Br { target });
    }

    /// Emit a conditional branch.
    pub fn emit_br_if(&mut self, cond: IRValueId, true_target: BlockId, false_target: BlockId) {
        self.emit_with_side_effects(
            IRInstKind::BrIf {
                cond,
                true_target,
                false_target,
            },
            SideEffects {
                may_raise: false,
                calls: false,
                allocates: false,
                reads_heap: false,
                writes_heap: false,
                may_yield: false,
            },
        );
    }

    /// Emit a return instruction.
    pub fn emit_ret(&mut self, value: IRValueId) {
        self.emit_with_side_effects(IRInstKind::Ret { value }, SideEffects::NONE);
    }

    /// Emit a call instruction.
    pub fn emit_call(&mut self, func: IRValueId, args: &[IRValueId]) -> IRValueId {
        let (_, result) = self.emit_with_result(
            IRInstKind::Call {
                func,
                args: args.to_vec(),
            },
            TypeId(0), // Any return type
        );
        self.instructions_mut().last_mut().unwrap().side_effects = SideEffects {
            calls: true,
            may_raise: true,
            allocates: true,
            ..SideEffects::NONE
        };
        result
    }

    /// Emit a GC safepoint.
    pub fn emit_gc_safe(&mut self) {
        self.emit(IRInstKind::GcSafe);
    }

    // ===== Private helpers =====

    fn current_block_instructions(&mut self) -> &mut Vec<IRInst> {
        let block_id = self.current_block;
        &mut self.function.blocks[block_id.0].instructions
    }

    fn instructions_mut(&mut self) -> &mut Vec<IRInst> {
        let block_id = self.current_block;
        &mut self.function.blocks[block_id.0].instructions
    }
}

impl Default for IRBuilder {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}
