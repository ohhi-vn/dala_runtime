//! BEAM chunk types and parsing utilities.
//!
//! BEAM files are organized into "chunks" - each chunk has a 4-byte
//! ID, a 4-byte size, and the chunk data. This module defines the
//! chunk types and provides utilities for working with them.

use std::io::{Read, Seek};

use crate::error::{BeamError, Result};
use crate::reader::BeamReader;
#[derive(Debug, Clone)]
pub struct BeamChunk {
    /// The chunk ID (4 bytes)
    pub id: String,
    /// The chunk data
    pub data: Vec<u8>,
}

impl BeamChunk {
    /// Read a chunk from a reader.
    pub fn read<R: Read + Seek>(reader: &mut BeamReader<R>) -> Result<Option<Self>> {
        match reader.read_chunk()? {
            Some((id, data)) => Ok(Some(BeamChunk { id, data })),
            None => Ok(None),
        }
    }

    /// Parse the chunk data as a sequence of 32-bit integers.
    pub fn as_u32_slice(&self) -> Result<Vec<u32>> {
        if self.data.len() % 4 != 0 {
            return Err(BeamError::FormatError(format!(
                "Chunk {} has non-aligned size: {}",
                self.id,
                self.data.len()
            )));
        }

        let mut result = Vec::with_capacity(self.data.len() / 4);
        for chunk in self.data.chunks_exact(4) {
            let val = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            result.push(val);
        }
        Ok(result)
    }

    /// Parse the chunk data as a sequence of bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

/// Chunk IDs used in BEAM files.
pub mod chunk_ids {
    pub const ATOM: &str = "Atom";
    pub const CODE: &str = "Code";
    pub const EXPT: &str = "ExpT";
    pub const LITT: &str = "LitT";
    pub const LOCL: &str = "LocL";
    pub const LINE: &str = "Line";
    pub const ATTR: &str = "Attr";
    pub const CSTS: &str = "CSts";
    pub const STRT: &str = "StrT";
    pub const IMPT: &str = "ImpT";
    pub const FOR1: &str = "FOR1";
    pub const BEAM: &str = "BEAM";
}

/// Section types within a BEAM chunk.
#[derive(Debug, Clone)]
pub enum BeamSection {
    /// Atom table section
    Atoms { atoms: Vec<String> },
    /// Code section
    Code { functions: Vec<CodeEntry> },
    /// Export table section
    Exports { exports: Vec<ExportEntry> },
    /// Literal table section
    Literals { literals: Vec<u8> },
    /// Line info section
    LineInfo { entries: Vec<LineEntry> },
    /// Unknown section (preserved for forward compatibility)
    Unknown { id: String, data: Vec<u8> },
}

/// A code entry in the CODE chunk.
#[derive(Debug, Clone)]
pub struct CodeEntry {
    /// Function name atom index
    pub name: u32,
    /// Arity
    pub arity: u32,
    /// Entry label
    pub label: u32,
    /// Number of instructions
    pub num_instructions: u32,
    /// Instruction data (raw)
    pub instructions: Vec<u8>,
}

/// An export table entry.
#[derive(Debug, Clone)]
pub struct ExportEntry {
    /// Function name atom index
    pub name: u32,
    /// Arity
    pub arity: u32,
    /// Code label
    pub label: u32,
}

/// A line number entry.
#[derive(Debug, Clone)]
pub struct LineEntry {
    /// Location (file atom index or 0)
    pub location: u32,
    /// Line number
    pub line: u32,
}
