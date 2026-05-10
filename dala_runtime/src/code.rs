//! Code management - stores compiled function pointers and module metadata.
//!
//! This module manages the mapping from (Module, Function, Arity) tuples
//! to their compiled implementations. It supports both AOT-compiled and
//! interpreted code paths, enabling mixed execution modes.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::term::Term;

/// A function pointer - represents a compiled or interpreted function.
///
/// The signature of all BEAM functions is:
///   fn(proc: &mut Process, args: &[Term]) -> Result<Term, Exception>
///
/// For AOT-compiled code, this is a raw function pointer.
/// For interpreted code, this wraps the bytecode interpreter entry.
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct CodePtr {
    ptr: usize,
}

impl CodePtr {
    /// Create a null code pointer.
    pub const fn null() -> Self {
        Self { ptr: 0 }
    }

    /// Check if this is a null pointer.
    pub fn is_null(self) -> bool {
        self.ptr == 0
    }

    /// Create a CodePtr from a raw function pointer.
    pub fn from_raw(ptr: usize) -> Self {
        Self { ptr }
    }

    /// Get the raw pointer value.
    pub fn as_usize(self) -> usize {
        self.ptr
    }
}

unsafe impl Send for CodePtr {}
unsafe impl Sync for CodePtr {}

/// The type signature for compiled BEAM functions.
pub type CompiledFn = unsafe extern "C" fn(proc: &mut super::Process, args: *const Term) -> Term;

/// Metadata for a compiled function.
#[derive(Debug, Clone)]
pub struct FunctionEntry {
    /// The compiled function pointer
    pub code: CodePtr,
    /// Whether this is AOT-compiled (false = interpreted)
    pub is_aot: bool,
    /// The module this function belongs to
    pub module: u64,
    /// The function name (atom index)
    pub function: u64,
    /// The arity
    pub arity: u32,
    /// Source file for debugging
    pub file: u64,
    /// Line number for debugging
    pub line: u32,
}

/// A module's code table.
#[derive(Debug, Default)]
pub struct ModuleCode {
    /// Functions indexed by (function_name_atom, arity)
    pub functions: HashMap<(u64, u32), FunctionEntry>,
    /// Module name (atom index)
    pub module: u64,
    /// Export table (function, arity) pairs
    pub exports: Vec<(u64, u32)>,
}

impl ModuleCode {
    /// Create a new module code table.
    pub fn new(module: u64) -> Self {
        Self {
            functions: HashMap::new(),
            module,
            exports: Vec::new(),
        }
    }

    /// Add a function entry to this module.
    pub fn add_function(&mut self, entry: FunctionEntry) {
        self.functions.insert((entry.function, entry.arity), entry);
    }

    /// Look up a function by name and arity.
    pub fn lookup(&self, function: u64, arity: u32) -> Option<&FunctionEntry> {
        self.functions.get(&(function, arity))
    }

    /// Add an export.
    pub fn add_export(&mut self, function: u64, arity: u32) {
        self.exports.push((function, arity));
    }
}

/// Global code registry - maps module names to their code tables.
pub struct CodeRegistry {
    modules: RwLock<HashMap<u64, ModuleCode>>,
}

impl CodeRegistry {
    /// Create a new empty code registry.
    pub fn new() -> Self {
        Self {
            modules: RwLock::new(HashMap::new()),
        }
    }

    /// Register a module's code.
    pub fn register_module(&self, module: u64, code: ModuleCode) {
        let mut modules = self.modules.write().unwrap();
        modules.insert(module, code);
    }

    /// Look up a function in the global registry.
    pub fn lookup(&self, module: u64, function: u64, arity: u32) -> Option<FunctionEntry> {
        let modules = self.modules.read().unwrap();
        modules
            .get(&module)
            .and_then(|m| m.lookup(function, arity))
            .cloned()
    }

    /// Get all registered module names.
    pub fn modules(&self) -> Vec<u64> {
        let modules = self.modules.read().unwrap();
        modules.keys().copied().collect()
    }
}

impl Default for CodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A lazy function reference - resolved at first call.
///
/// This enables hot code loading: when a module is updated, the
/// export table entries can be atomically swapped to point to the
/// new code without stopping the world.
#[repr(C)]
pub struct LazyFnRef {
    /// The resolved function pointer
    code: RwLock<CodePtr>,
    /// Module name (atom index)
    module: u64,
    /// Function name (atom index)
    function: u64,
    /// Arity
    arity: u32,
}

impl LazyFnRef {
    /// Create a new lazy function reference.
    pub fn new(module: u64, function: u64, arity: u32) -> Self {
        Self {
            code: RwLock::new(CodePtr::null()),
            module,
            function,
            arity,
        }
    }

    /// Get the current function pointer, resolving if necessary.
    pub fn get(&self) -> CodePtr {
        *self.code.read().unwrap()
    }

    /// Set the function pointer (used during code loading).
    pub fn set(&self, ptr: CodePtr) {
        *self.code.write().unwrap() = ptr;
    }

    /// Check if this reference has been resolved.
    pub fn is_resolved(&self) -> bool {
        !self.code.read().unwrap().is_null()
    }
}
