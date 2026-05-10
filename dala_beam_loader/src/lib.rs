//! BEAM module loader - parses .beam files into IR modules.
//!
//! This crate handles loading and parsing of BEAM (Erlang Abstract EMulation)
//! binary files. It reads the standard BEAM format chunks and converts them
//! into an intermediate representation suitable for compilation.

mod bytecode;
mod chunk;
mod error;
mod reader;

pub use bytecode::*;
pub use chunk::BeamChunk;
pub use error::{BeamError, Result};
pub use reader::BeamReader;

use std::collections::HashMap;
use std::io::{Read, Seek};

/// A loaded BEAM module.
#[derive(Debug, Clone)]
pub struct BeamModule {
    /// Module name
    pub name: String,
    /// Functions in this module, keyed by (name, arity)
    pub functions: HashMap<(String, u32), BeamFunction>,
    /// Exported functions (name, arity, label)
    pub exports: Vec<(String, u32, u32)>,
    /// Atom table
    pub atoms: Vec<String>,
    /// Attributes
    pub attributes: Vec<(String, String)>,
    /// Compile info
    pub compile_info: Option<CompileInfo>,
}

/// Compile-time information from the BEAM file.
#[derive(Debug, Clone)]
pub struct CompileInfo {
    /// Source file name
    pub source_file: Option<String>,
    /// Compiler options
    pub options: Vec<String>,
}

impl BeamModule {
    /// Create a new empty module.
    pub fn new(name: String) -> Self {
        Self {
            name,
            functions: HashMap::new(),
            exports: Vec::new(),
            atoms: Vec::new(),
            attributes: Vec::new(),
            compile_info: None,
        }
    }

    /// Get a function by name and arity.
    pub fn get_function(&self, name: &str, arity: u32) -> Option<&BeamFunction> {
        self.functions.get(&(name.to_string(), arity))
    }

    /// Get all exported functions.
    pub fn exported_functions(&self) -> &[(String, u32, u32)] {
        &self.exports
    }

    /// Get the number of functions.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
}

/// Load a BEAM module from a file path.
pub fn load_beam_file(path: &str) -> Result<BeamModule> {
    let mut reader = BeamReader::from_file(path)?;
    reader.read_module()
}

/// Load a BEAM module from a byte slice.
pub fn load_beam_bytes(data: &[u8]) -> Result<BeamModule> {
    let cursor = std::io::Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    reader.read_module()
}

/// Load a BEAM module from a reader.
pub fn load_beam<R: Read + Seek>(reader: R) -> Result<BeamModule> {
    let mut reader = BeamReader { reader, pos: 0 };
    reader.read_module()
}
