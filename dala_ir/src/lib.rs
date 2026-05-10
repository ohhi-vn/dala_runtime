//! Dala IR - SSA Intermediate Representation for BEAM bytecode.
//!
//! This crate provides the bridge between raw BEAM instructions and
//! optimized native code generation. The key design principle is:
//!
//!   BEAM bytecode → Dala SSA IR → Optimization → Machine code
//!
//! Why a separate IR layer?
//! - BEAM opcodes are stack/register oriented and hard to optimize directly
//! - SSA form enables dead code elimination, constant propagation, inlining,
//!   register allocation, and loop optimization
//! - This is exactly what modern JITs (V8, SpiderMonkey, GraalVM) do
//!
//! The IR is designed to be:
//! - Typed (every value has a known Type)
//! - SSA (each variable assigned exactly once)
//! - Control-flow structured (basic blocks, branches, switches)
//! - Memory-model aware (explicit load/store for BEAM heap)

pub mod builder;
pub mod constant;
pub mod function;
pub mod instruction;
pub mod layout;
pub mod module;
pub mod opt;
pub mod type_system;
pub mod value;

// Re-exports
pub use builder::IRBuilder;
pub use function::IRFunction;
pub use instruction::{IRInst, IRInstKind};
pub use module::IRModule;
pub use type_system::{IRType, TypeKind};
pub use value::{IRValue, IRValueId};

/// The IR context - owns all IR data for a compilation unit.
pub struct IRContext {
    /// The module being compiled
    pub module: IRModule,
    /// Functions defined in this context
    pub functions: Vec<IRFunction>,
    /// Constant pool
    pub constants: Vec<IRValue>,
    /// Type arena
    pub types: Vec<IRType>,
}

impl IRContext {
    /// Create a new IR context.
    pub fn new() -> Self {
        Self {
            module: IRModule::new(),
            functions: Vec::new(),
            constants: Vec::new(),
            types: Vec::new(),
        }
    }

    /// Create a new function in this context.
    pub fn create_function(&mut self, name: String, ty: IRType) -> IRFunctionId {
        let id = IRFunctionId(self.functions.len());
        self.functions.push(IRFunction::new(name, ty));
        id
    }

    /// Get a function by ID.
    pub fn get_function(&self, id: IRFunctionId) -> &IRFunction {
        &self.functions[id.0]
    }

    /// Get a mutable function by ID.
    pub fn get_function_mut(&mut self, id: IRFunctionId) -> &mut IRFunction {
        &mut self.functions[id.0]
    }

    /// Create a new type in the type arena.
    pub fn create_type(&mut self, ty: IRType) -> TypeId {
        let id = TypeId(self.types.len());
        self.types.push(ty);
        id
    }

    /// Get a type by ID.
    pub fn get_type(&self, id: TypeId) -> &IRType {
        &self.types[id.0]
    }
}

/// Unique identifier for an IR function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IRFunctionId(pub usize);

/// Unique identifier for an IR value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub usize);

/// Unique identifier for a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub usize);

/// Unique identifier for a basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

/// Unique identifier for an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstId(pub usize);
