//! Dispatch manager - handles module dispatch, hot code loading, and tracing.

mod export_table;
mod hot_code;

pub use export_table::ExportTable;
pub use hot_code::HotCodeManager;

use dashmap::DashMap;
use parking_lot::RwLock;

use dala_ir::{IRFunction, IRModule};
use dala_runtime::{CodePtr, CodeRegistry};

use crate::hot_code::LazyFnRef;

use std::sync::Arc;

/// A compiled module ready for execution.
#[derive(Debug, Clone)]
pub struct CompiledModule {
    /// Module name (atom index)
    pub name: u64,
    /// The compiled code entries
    pub exports: Vec<ExportEntry>,
    /// The IR module (for recompilation)
    pub ir_module: IRModule,
    /// Code generation metadata
    pub metadata: ModuleMetadata,
}

/// Metadata about a compiled module.
#[derive(Debug, Clone, Default)]
pub struct ModuleMetadata {
    /// Source file name
    pub source_file: Option<String>,
    /// Compiler options used
    pub compiler_options: Vec<String>,
    /// Code size in bytes
    pub code_size: usize,
}

/// An export table entry for a compiled function.
#[derive(Debug, Clone)]
pub struct ExportEntry {
    /// Function name (atom index)
    pub function: u64,
    /// Arity
    pub arity: u32,
    /// The compiled code pointer
    pub code_ptr: CodePtr,
    /// Lazy reference for hot code upgrade
    pub lazy_ref: LazyFnRef,
}

/// The dispatch manager coordinates module loading and function dispatch.
pub struct DispatchManager {
    /// Registered modules
    modules: DashMap<u64, Arc<CompiledModule>>,
    /// Export table for global function lookup
    export_table: ExportTable,
    /// Hot code upgrade manager
    hot_code: HotCodeManager,
    /// Code registry for runtime code management
    code_registry: CodeRegistry,
}

impl DispatchManager {
    /// Create a new dispatch manager.
    pub fn new() -> Self {
        Self {
            modules: DashMap::new(),
            export_table: ExportTable::new(),
            hot_code: HotCodeManager::new(),
            code_registry: CodeRegistry::new(),
        }
    }

    /// Register a compiled module.
    pub fn register_module(&self, module: CompiledModule) -> u64 {
        let name = module.name;
        self.modules.insert(name, Arc::new(module.clone()));

        // Register exports
        for export in &module.exports {
            self.export_table
                .register(module.name, export.function, export.arity, export.code_ptr);
        }

        name
    }

    /// Look up a function by module, name, and arity.
    pub fn lookup_function(&self, module: u64, function: u64, arity: u32) -> Option<CodePtr> {
        self.export_table.lookup(module, function, arity)
    }

    /// Hot-replace a module's code.
    pub fn hot_replace(&self, module: CompiledModule) -> Result<(), HotCodeError> {
        let old_module = self.modules.get(&module.name);

        // Validate that the new module has the same exports
        if let Some(old) = old_module {
            let old_exports: Vec<_> = old.exports.iter().map(|e| (e.function, e.arity)).collect();
            let new_exports: Vec<_> = module
                .exports
                .iter()
                .map(|e| (e.function, e.arity))
                .collect();

            if old_exports != new_exports {
                return Err(HotCodeError::ExportMismatch);
            }
        }

        // Register the new module
        let name = self.register_module(module.clone());

        // Update lazy references atomically
        self.hot_code.update_module(name, module);

        Ok(())
    }

    /// Get the code registry.
    pub fn code_registry(&self) -> &CodeRegistry {
        &self.code_registry
    }
}

/// Error types for hot code operations.
#[derive(Debug, thiserror::Error)]
pub enum HotCodeError {
    #[error("export mismatch - cannot hot-replace module")]
    ExportMismatch,
    #[error("module not found: {0}")]
    ModuleNotFound(u64),
    #[error("compilation error: {0}")]
    CompilationError(String),
}

impl Default for DispatchManager {
    fn default() -> Self {
        Self::new()
    }
}
