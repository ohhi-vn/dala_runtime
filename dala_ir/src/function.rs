//! IR Function representation.
//!
//! An IRFunction contains the compiled representation of a single
//! BEAM function in SSA form. It consists of:
//! - A list of basic blocks (the control flow graph)
//! - An entry block
//! - Parameter types
//! - The function signature (module, name, arity)

use crate::instruction::{IRInst, Reg, Label};
use crate::type_system::{IRType, TypeId};
use crate::value::{IRValueId, IRValue};

/// A basic block in the IR.
///
/// A basic block is a sequence of instructions with a single entry
/// point (no branches in) and a single exit point (no branches out
/// except at the end).
#[derive(Debug, Clone, Default)]
pub struct BasicBlock {
    /// Instructions in this block
    pub instructions: Vec<IRInst>,
    /// The block label
    pub label: Label,
    /// Predecessor blocks (blocks that branch to this one)
    pub predecessors: Vec<Label>,
    /// Successor blocks (blocks this one branches to)
    pub successors: Vec<Label>,
    /// Whether this block is reachable
    pub reachable: bool,
}

impl BasicBlock {
    /// Create a new basic block with the given label.
    pub fn new(label: Label) -> Self {
        Self {
            instructions: Vec::new(),
            label,
            predecessors: Vec::new(),
            successors: Vec::new(),
            reachable: true,
        }
    }

    /// Add an instruction to this block.
    pub fn push_inst(&mut self, inst: IRInst) {
        self.instructions.push(inst);
    }

    /// Get the terminator instruction (last instruction in the block).
    pub fn terminator(&self) -> Option<&IRInst> {
        self.instructions.last()
    }

    /// Check if this block is terminated.
    pub fn is_terminated(&self) -> bool {
        matches!(
            self.instructions.last().map(|i| &i.kind),
            Some(
                crate::instruction::IRInstKind::Br { .. }
                    | crate::instruction::IRInstKind::BrIf { .. }
                    | crate::instruction::IRInstKind::Switch { .. }
                    | crate::instruction::IRInstKind::Ret { .. }
                    | crate::instruction::IRInstKind::TailCall { .. }
                    | crate::instruction::IRInstKind::Throw { .. }
            )
        )
    }
}

/// An IR function - a single compiled BEAM function in SSA form.
#[derive(Debug, Clone)]
pub struct IRFunction {
    /// Module name (atom index)
    pub module: u64,
    /// Function name (atom index)
    pub name: u64,
    /// Arity
    pub arity: u32,
    /// Source file (atom index)
    pub file: u64,
    /// Source line
    pub line: u32,
    /// The basic blocks of this function
    pub blocks: Vec<BasicBlock>,
    /// The entry block ID
    pub entry_block: BlockId,
    /// Parameter types
    pub param_types: Vec<TypeId>,
    /// Return type
    pub return_type: TypeId,
    /// Local variable bindings (register -> value mapping)
    pub locals: Vec<(Reg, IRValueId)>,
    /// Whether this function has been fully compiled
    pub compiled: bool,
    /// Stack map entries for GC safepoints
    pub stack_maps: Vec<StackMapEntry>,
}

/// A stack map entry for GC.
#[derive(Debug, Clone, Copy)]
pub struct StackMapEntry {
    /// Instruction offset within the function
    pub instruction_offset: u32,
    /// Which registers are live at this point
    pub live_registers: u64, // bitmask of X registers
    /// Which stack slots are live
    pub live_stack_slots: u32,
}

impl IRFunction {
    /// Create a new IR function.
    pub fn new(module: u64, name: u64, arity: u32) -> Self {
        let mut func = Self {
            module,
            name,
            arity,
            file: 0,
            line: 0,
            blocks: Vec::new(),
            entry_block: BlockId(0),
            param_types: Vec::new(),
            return_type: TypeId(0),
            locals: Vec::new(),
            compiled: false,
            stack_maps: Vec::new(),
        };

        // Create the entry block
        func.entry_block = func.create_block();
        func
    }

    /// Create a new basic block and return its ID.
    pub fn create_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(BasicBlock::new(Label(id.0 as u32)));
        id
    }

    /// Get a basic block by ID.
    pub fn get_block(&self, id: BlockId) -> &BasicBlock {
        &self.blocks[id.0]
    }

    /// Get a mutable basic block by ID.
    pub fn get_block_mut(&mut self, id: BlockId) -> &mut BasicBlock {
        &mut self.blocks[id.0]
    }

    /// Get the number of basic blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Get the function's name as a string (requires atom table).
    pub fn name_str(&self) -> String {
        format!("f{}", self.name)
    }

    /// Get the function's fully qualified name.
    pub fn full_name(&self) -> String {
        format!("m{}.f{}/{}", self.module, self.name, self.arity)
    }

    /// Add a parameter type.
    pub fn add_param_type(&mut self, ty: TypeId) {
        self.param_types.push(ty);
    }

    /// Set the return type.
    pub fn set_return_type(&mut self, ty: TypeId) {
        self.return_type = ty;
    }

    /// Record a stack map entry for GC.
    pub fn add_stack_map(&mut self, offset: u32, live_regs: u64, live_stack: u32) {
        self.stack_maps.push(StackMapEntry {
            instruction_offset: offset,
            live_registers: live_regs,
            live_stack_slots: live_stack,
        });
    }
}

/// A function signature (for type checking calls).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionSignature {
    /// Parameter types
    pub params: Vec<TypeId>,
    /// Return type
    pub return_type: TypeId,
    /// Whether the function can raise exceptions
    pub throws: bool,
}
