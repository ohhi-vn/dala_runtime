//! Export table - maps (module, function, arity) to compiled code pointers.

use crate::CodePtr;
use dashmap::DashMap;

/// Key for export table lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExportKey {
    /// Module name (atom index)
    pub module: u64,
    /// Function name (atom index)
    pub function: u64,
    /// Arity
    pub arity: u32,
}

/// A single export table entry.
#[derive(Debug, Clone)]
pub struct ExportEntry {
    /// The compiled code pointer
    pub code_ptr: CodePtr,
}

impl ExportEntry {
    /// Create a new export entry.
    pub fn new(code_ptr: CodePtr) -> Self {
        Self { code_ptr }
    }
}

/// The global export table.
///
/// Uses a concurrent hash map for lock-free reads during execution.
/// Writes (code loading) are less frequent and can use the DashMap's
/// internal sharding.
#[derive(Default)]
pub struct ExportTable {
    entries: DashMap<ExportKey, ExportEntry>,
}

impl ExportTable {
    /// Create a new empty export table.
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Register a function export.
    pub fn register(&self, module: u64, function: u64, arity: u32, code_ptr: CodePtr) {
        let key = ExportKey {
            module,
            function,
            arity,
        };
        let entry = ExportEntry::new(code_ptr);
        self.entries.insert(key, entry);
    }

    /// Look up a function by (module, function, arity).
    pub fn lookup(&self, module: u64, function: u64, arity: u32) -> Option<CodePtr> {
        let key = ExportKey {
            module,
            function,
            arity,
        };
        self.entries.get(&key).map(|entry| entry.code_ptr)
    }

    /// Remove an export.
    pub fn remove(&self, module: u64, function: u64, arity: u32) -> bool {
        let key = ExportKey {
            module,
            function,
            arity,
        };
        self.entries.remove(&key).is_some()
    }

    /// Get the number of registered exports.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the export table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all exports for a given module.
    pub fn module_exports(&self, module: u64) -> Vec<(u64, u32, CodePtr)> {
        self.entries
            .iter()
            .filter(|entry| entry.key().module == module)
            .map(|entry| {
                let key = *entry.key();
                (key.function, key.arity, entry.code_ptr)
            })
            .collect()
    }
}
