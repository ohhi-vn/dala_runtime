//! Atom table - manages interned atom strings.
//!
//! Atoms are interned string constants that are compared by index.
//! The BEAM VM has a global atom table that maps atom IDs to string names.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::RwLock;

/// The global atom table.
pub struct AtomTable {
    names: RwLock<Vec<&'static str>>,
    indices: RwLock<HashMap<&'static str, u32>>,
}

impl AtomTable {
    /// Create a new atom table with standard BEAM atoms pre-loaded.
    pub fn new() -> Self {
        let standard_atoms: &[&str] = &[
            "nil",
            "true",
            "false",
            "undefined",
            "error",
            "exit",
            "throw",
            "ok",
            "after",
            "normal",
            "shutdown",
            "badmatch",
            "case_clause",
            "if_clause",
            "try_clause",
            "badarg",
            "badarith",
            "badbool",
            "function_clause",
            "match",
            "multifile",
            "no_local",
            "on_load",
            "noconnection",
            "call_from_c",
            "file",
            "line",
            "badmap",
            "badkey",
        ];

        let mut names = Vec::with_capacity(256);
        let mut indices = HashMap::new();

        for (idx, &name) in standard_atoms.iter().enumerate() {
            names.push(name);
            indices.insert(name, idx as u32);
        }

        Self {
            names: RwLock::new(names),
            indices: RwLock::new(indices),
        }
    }

    /// Look up or insert an atom by name. Returns the atom index.
    pub fn lookup_or_insert(&self, name: &str) -> u32 {
        {
            let indices = self.indices.read().unwrap();
            if let Some(&idx) = indices.get(name) {
                return idx;
            }
        }

        let mut indices = self.indices.write().unwrap();
        let mut names = self.names.write().unwrap();

        if let Some(&idx) = indices.get(name) {
            return idx;
        }

        let idx = names.len() as u32;
        let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
        names.push(leaked);
        indices.insert(leaked, idx);
        idx
    }

    /// Look up an atom by name.
    pub fn lookup(&self, name: &str) -> Option<u32> {
        let indices = self.indices.read().unwrap();
        indices.get(name).copied()
    }

    /// Get the name for an atom index.
    pub fn get_name(&self, index: u32) -> Option<&'static str> {
        let names = self.names.read().unwrap();
        names.get(index as usize).copied()
    }

    /// Get the number of atoms.
    pub fn len(&self) -> usize {
        let names = self.names.read().unwrap();
        names.len()
    }

    /// Check if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for AtomTable {
    fn default() -> Self {
        Self::new()
    }
}

static ATOM_TABLE: OnceLock<AtomTable> = OnceLock::new();

/// Get a reference to the global atom table.
pub fn get_atom_table() -> &'static AtomTable {
    ATOM_TABLE.get_or_init(AtomTable::new)
}

/// Look up or insert an atom by name.
pub fn atom(name: &str) -> u32 {
    get_atom_table().lookup_or_insert(name)
}

/// Get the name for an atom index.
pub fn get_name(index: u32) -> Option<&'static str> {
    get_atom_table().get_name(index)
}
