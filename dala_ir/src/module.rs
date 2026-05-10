//! IR Module - represents a compiled BEAM module in IR form.
//!
//! A BEAM module (.beam file) is translated into an IRModule which
//! contains all the functions, exports, and metadata needed for
//! compilation to native code.

use indexmap::IndexMap;

use crate::function::IRFunction;
use crate::type_system::{IRType, TypeId};
use crate::IRFunctionId;

/// An IR module representing a compiled BEAM module.
#[derive(Debug, Clone)]
pub struct IRModule {
    /// Module name (atom index)
    pub name: u64,
    /// Functions in this module, indexed by function ID
    pub functions: IndexMap<(u64, u32), IRFunctionId>,
    /// Function definitions
    pub function_bodies: Vec<IRFunction>,
    /// Exported functions (name, arity) pairs
    pub exports: Vec<(u64, u32)>,
    /// Imports (module -> list of (function, arity))
    pub imports: IndexMap<u64, Vec<(u64, u32)>>,
    /// Attributes (module-level metadata)
    pub attributes: Vec<(u64, u64)>,
    /// Compile info
    pub compile_info: CompileInfo,
    /// Literal table
    pub literals: Vec<u64>,
    /// Line information for debugging
    pub line_info: Vec<(u32, u32)>, // (file_atom, line)
}

/// Compile-time information about a module.
#[derive(Debug, Clone, Default)]
pub struct CompileInfo {
    /// Source file name
    pub source_file: Option<String>,
    /// Compiler options
    pub options: Vec<String>,
    /// Compiler version
    pub version: Option<String>,
    /// Whether debug info is available
    pub debug_info: bool,
}

impl IRModule {
    /// Create a new IR module with the given name.
    pub fn new(name: u64) -> Self {
        Self {
            name,
            functions: IndexMap::new(),
            function_bodies: Vec::new(),
            exports: Vec::new(),
            imports: IndexMap::new(),
            attributes: Vec::new(),
            compile_info: CompileInfo::default(),
            literals: Vec::new(),
            line_info: Vec::new(),
        }
    }

    /// Add a function to this module.
    pub fn add_function(&mut self, name: u64, arity: u32) -> IRFunctionId {
        let id = IRFunctionId(self.function_bodies.len());
        let func = IRFunction::new(self.name, name, arity);
        self.functions.insert((name, arity), id);
        self.function_bodies.push(func);
        id
    }

    /// Get a function by name and arity.
    pub fn get_function(&self, name: u64, arity: u32) -> Option<IRFunctionId> {
        self.functions.get(&(name, arity)).copied()
    }

    /// Get a function body by ID.
    pub fn get_function_body(&self, id: IRFunctionId) -> &IRFunction {
        &self.function_bodies[id.0]
    }

    /// Get a mutable function body by ID.
    pub fn get_function_body_mut(&mut self, id: IRFunctionId) -> &mut IRFunction {
        &mut self.function_bodies[id.0]
    }

    /// Add an export.
    pub fn add_export(&mut self, name: u64, arity: u32) {
        self.exports.push((name, arity));
    }

    /// Add an import from another module.
    pub fn add_import(&mut self, module: u64, function: u64, arity: u32) {
        self.imports
            .entry(module)
            .or_default()
            .push((function, arity));
    }

    /// Add a literal to the literal table.
    pub fn add_literal(&mut self, value: u64) -> u32 {
        let idx = self.literals.len() as u32;
        self.literals.push(value);
        idx
    }

    /// Check if a function is exported.
    pub fn is_exported(&self, name: u64, arity: u32) -> bool {
        self.exports.contains(&(name, arity))
    }

    /// Get all exported functions.
    pub fn exported_functions(&self) -> &[(u64, u32)] {
        &self.exports
    }

    /// Get the number of functions in this module.
    pub fn function_count(&self) -> usize {
        self.function_bodies.len()
    }
}

/// A compilation unit - an IR module with its type context.
pub struct CompilationUnit {
    /// The module being compiled
    pub module: IRModule,
    /// Type context
    pub types: Vec<IRType>,
    /// Constant pool
    pub constants: Vec<u64>,
}

impl CompilationUnit {
    /// Create a new compilation unit from an IR module.
    pub fn new(module: IRModule) -> Self {
        Self {
            module,
            types: Vec::new(),
            constants: Vec::new(),
        }
    }

    /// Add a type and return its ID.
    pub fn add_type(&mut self, ty: IRType) -> TypeId {
        let id = TypeId(self.types.len());
        self.types.push(ty);
        id
    }

    /// Add a constant and return its index.
    pub fn add_constant(&mut self, value: u64) -> u32 {
        let idx = self.constants.len() as u32;
        self.constants.push(value);
        idx
    }
}
