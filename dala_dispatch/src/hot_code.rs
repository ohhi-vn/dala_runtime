//! Hot code upgrade manager - supports atomic module replacement.

use parking_lot::RwLock;

use crate::CodePtr;
use crate::CompiledModule;
use dashmap::DashMap;

/// A lazy function reference for hot code upgrade support.
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

    /// Get the current function pointer.
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

/// The hot code upgrade manager.
#[derive(Default)]
pub struct HotCodeManager {
    /// Mapping from module name to current compiled module
    modules: DashMap<u64, CompiledModule>,
}

impl HotCodeManager {
    /// Create a new hot code manager.
    pub fn new() -> Self {
        Self {
            modules: DashMap::new(),
        }
    }

    /// Update a module's code atomically.
    pub fn update_module(&self, module_name: u64, module: CompiledModule) {
        self.modules.insert(module_name, module.clone());
        for export in &module.exports {
            export.lazy_ref.set(export.code_ptr);
        }
    }

    /// Get the current version of a module.
    pub fn get_module(&self, module_name: u64) -> Option<CompiledModule> {
        self.modules.get(&module_name).map(|m| m.clone())
    }

    /// Check if a module has been loaded.
    pub fn has_module(&self, module_name: u64) -> bool {
        self.modules.contains_key(&module_name)
    }

    /// Remove a module.
    pub fn remove_module(&self, module_name: u64) -> bool {
        self.modules.remove(&module_name).is_some()
    }
}
