//! BEAM file reader - low-level parsing of the BEAM binary format.
//!
//! The BEAM format is based on the "Abstract Format" defined in
//! EEP-46. It consists of a series of chunks, each identified by
//! a 4-byte ID and containing structured binary data.

use std::io::{Read, Seek, SeekFrom};

use crate::BeamModule;
use crate::bytecode::{BeamFunction, BeamOperand, BeamRegister};
use crate::error::{BeamError, Result};

/// A reader for BEAM files.
pub struct BeamReader<R: Read + Seek> {
    reader: R,
    /// Current position in the file
    pos: u64,
}

impl BeamReader<std::fs::File> {
    /// Create a new reader from a file path.
    pub fn from_file(path: &str) -> Result<Self> {
        let file = std::fs::File::open(path).map_err(|e| BeamError::IoError(e.to_string()))?;
        Ok(Self {
            reader: file,
            pos: 0,
        })
    }
}

impl<R: Read + Seek> BeamReader<R> {
    /// Get the current position.
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Read a single byte.
    pub fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.reader
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        self.pos += 1;
        Ok(buf[0])
    }

    /// Read a big-endian u16.
    pub fn read_u16(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.reader
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        self.pos += 2;
        Ok(u16::from_be_bytes(buf))
    }

    /// Read a big-endian u32.
    pub fn read_u32(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.reader
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        self.pos += 4;
        Ok(u32::from_be_bytes(buf))
    }

    /// Read a big-endian u64.
    pub fn read_u64(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.reader
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        self.pos += 8;
        Ok(u64::from_be_bytes(buf))
    }

    /// Read raw bytes.
    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.reader
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        self.pos += len as u64;
        Ok(buf)
    }

    /// Read a null-terminated string.
    pub fn read_string(&mut self) -> Result<String> {
        let mut bytes = Vec::new();
        loop {
            let b = self.read_u8()?;
            if b == 0 {
                break;
            }
            bytes.push(b);
        }
        String::from_utf8(bytes).map_err(|e| BeamError::FormatError(e.to_string()))
    }

    /// Read a chunk header (ID + size).
    pub fn read_chunk_header(&mut self) -> Result<Option<(String, u32)>> {
        let mut id_bytes = [0u8; 4];
        match self.reader.read_exact(&mut id_bytes) {
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(BeamError::IoError(e.to_string())),
        }
        self.pos += 4;

        let id = String::from_utf8_lossy(&id_bytes).to_string();
        let size = self.read_u32()?;

        Ok(Some((id, size)))
    }

    /// Read a complete chunk (header + data).
    pub fn read_chunk(&mut self) -> Result<Option<(String, Vec<u8>)>> {
        match self.read_chunk_header()? {
            Some((id, size)) => {
                let data = self.read_bytes(size as usize)?;
                Ok(Some((id, data)))
            }
            None => Ok(None),
        }
    }

    /// Read the complete module from the reader.
    pub fn read_module(&mut self) -> Result<BeamModule> {
        // Read FOR1 header (outer container)
        let header = self.read_bytes(4)?;
        if &header != b"FOR1" {
            return Err(BeamError::FormatError("Expected FOR1 header".to_string()));
        }

        let _total_size = self.read_u32()?;

        // Read BEAM header
        let beam_header = self.read_bytes(4)?;
        if &beam_header != b"BEAM" {
            return Err(BeamError::FormatError("Expected BEAM header".to_string()));
        }

        // Read all chunks
        let mut module = BeamModule::new(String::new());
        let mut atoms = Vec::new();

        loop {
            match self.read_chunk()? {
                Some((id, data)) => {
                    match id.as_str() {
                        "Atom" => {
                            atoms = self.parse_atom_chunk(&data)?;
                            module.atoms = atoms.clone();
                        }
                        "Code" => {
                            let functions = self.parse_code_chunk(&data, &atoms)?;
                            for (key, func) in functions {
                                module.functions.insert(key, func);
                            }
                        }
                        "ExpT" => {
                            let exports = self.parse_export_table(&data, &atoms)?;
                            module.exports = exports;
                        }
                        "LitT" => {
                            // Literal table - parsed for future use
                        }
                        "LocL" => {
                            // Local function table
                        }
                        "Line" => {
                            // Line number info
                        }
                        "Attr" => {
                            // Attributes
                        }
                        "StrT" | "ImpT" | "CSts" => {
                            // Optional chunks
                        }
                        _ => {
                            log::warn!("Unknown chunk type: {}", id);
                        }
                    }
                }
                None => break,
            }
        }

        Ok(module)
    }

    /// Parse the atom chunk.
    fn parse_atom_chunk(&mut self, data: &[u8]) -> Result<Vec<String>> {
        let mut cursor = std::io::Cursor::new(data);
        let count = {
            let mut buf = [0u8; 4];
            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            u32::from_be_bytes(buf) as usize
        };

        let mut atoms = Vec::with_capacity(count);
        for _ in 0..count {
            let atom = self.read_string_from_cursor(&mut cursor)?;
            atoms.push(atom);
        }

        Ok(atoms)
    }

    /// Parse the export table.
    fn parse_export_table(
        &mut self,
        data: &[u8],
        atoms: &[String],
    ) -> Result<Vec<(String, u32, u32)>> {
        let mut cursor = std::io::Cursor::new(data);
        let count = {
            let mut buf = [0u8; 4];
            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            u32::from_be_bytes(buf) as usize
        };

        let mut exports = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buf = [0u8; 4];

            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            let name_index = u32::from_be_bytes(buf);

            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            let arity = u32::from_be_bytes(buf);

            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            let label = u32::from_be_bytes(buf);

            let name = if name_index > 0 && (name_index as usize) <= atoms.len() {
                atoms[name_index as usize - 1].clone()
            } else {
                format!("atom_{}", name_index)
            };

            exports.push((name, arity, label));
        }

        Ok(exports)
    }

    /// Parse the code chunk.
    fn parse_code_chunk(
        &mut self,
        data: &[u8],
        atoms: &[String],
    ) -> Result<HashMap<(String, u32), BeamFunction>> {
        let mut cursor = std::io::Cursor::new(data);

        // Code chunk header
        let mut buf = [0u8; 4];
        cursor
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        let _sub_size = u32::from_be_bytes(buf);

        cursor
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        let _instruction_start = u32::from_be_bytes(buf);

        cursor
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        let num_functions = u32::from_be_bytes(buf) as usize;

        cursor
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        let _num_labels = u32::from_be_bytes(buf);

        cursor
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        let num_records = u32::from_be_bytes(buf) as usize;

        let mut functions: HashMap<(String, u32), crate::BeamFunction> = HashMap::new();

        for _ in 0..num_records {
            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            let name_index = u32::from_be_bytes(buf);

            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            let arity = u32::from_be_bytes(buf);

            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            let _label = u32::from_be_bytes(buf);

            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            let num_instructions = u32::from_be_bytes(buf) as usize;

            let name = if name_index > 0 && (name_index as usize) <= atoms.len() {
                atoms[name_index as usize - 1].clone()
            } else {
                format!("f{}", name_index)
            };

            let mut code = Vec::with_capacity(num_instructions);
            for _ in 0..num_instructions {
                cursor
                    .read_exact(&mut buf)
                    .map_err(|e| BeamError::IoError(e.to_string()))?;
                let opcode = u32::from_be_bytes(buf);

                cursor
                    .read_exact(&mut buf)
                    .map_err(|e| BeamError::IoError(e.to_string()))?;
                let num_operands = u32::from_be_bytes(buf) as usize;

                let mut operands = Vec::with_capacity(num_operands);
                for _ in 0..num_operands {
                    let op_byte = self.read_u8_from_cursor(&mut cursor)?;
                    match op_byte {
                        0 => {
                            cursor
                                .read_exact(&mut buf)
                                .map_err(|e| BeamError::IoError(e.to_string()))?;
                            let reg = u32::from_be_bytes(buf);
                            let reg_type = reg >> 24;
                            let reg_num = reg & 0xFFFFFF;
                            let beam_reg = match reg_type {
                                0 => BeamRegister::X(reg_num),
                                1 => BeamRegister::Y(reg_num),
                                2 => BeamRegister::F(reg_num),
                                _ => BeamRegister::X(reg_num),
                            };
                            operands.push(BeamOperand::Register(beam_reg));
                        }
                        1 => {
                            cursor
                                .read_exact(&mut buf)
                                .map_err(|e| BeamError::IoError(e.to_string()))?;
                            let label = u32::from_be_bytes(buf);
                            operands.push(BeamOperand::Label(label));
                        }
                        2 => {
                            cursor
                                .read_exact(&mut buf)
                                .map_err(|e| BeamError::IoError(e.to_string()))?;
                            let val = i32::from_be_bytes(buf) as i64;
                            operands.push(BeamOperand::Integer(val));
                        }
                        3 => {
                            cursor
                                .read_exact(&mut buf)
                                .map_err(|e| BeamError::IoError(e.to_string()))?;
                            let val = u64::from_be_bytes(buf);
                            let float_val = f64::from_bits(val);
                            operands.push(BeamOperand::Float(float_val));
                        }
                        4 => {
                            cursor
                                .read_exact(&mut buf)
                                .map_err(|e| BeamError::IoError(e.to_string()))?;
                            let atom_idx = u32::from_be_bytes(buf);
                            operands.push(BeamOperand::AtomIndex(atom_idx));
                        }
                        _ => {
                            return Err(BeamError::FormatError(format!(
                                "Unknown operand type: {}",
                                op_byte
                            )));
                        }
                    }
                }

                code.push(BeamInstruction {
                    opcode,
                    operands,
                    line: None,
                });
            }

            let func = BeamFunction {
                name: name.clone(),
                arity,
                label: 0,
                code,
            };

            functions.insert((name, arity), func);
        }

        Ok(functions)
    }

    /// Read a u32 from a cursor.
    fn read_u32_from_cursor(&self, cursor: &mut std::io::Cursor<&[u8]>) -> Result<u32> {
        let mut buf = [0u8; 4];
        cursor
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        Ok(u32::from_be_bytes(buf))
    }

    /// Read a string from a cursor.
    fn read_string_from_cursor(&self, cursor: &mut std::io::Cursor<&[u8]>) -> Result<String> {
        let mut bytes = Vec::new();
        loop {
            let mut buf = [0u8; 1];
            cursor
                .read_exact(&mut buf)
                .map_err(|e| BeamError::IoError(e.to_string()))?;
            if buf[0] == 0 {
                break;
            }
            bytes.push(buf[0]);
        }
        String::from_utf8(bytes).map_err(|e| BeamError::FormatError(e.to_string()))
    }

    /// Read a single byte from a cursor.
    fn read_u8_from_cursor(&self, cursor: &mut std::io::Cursor<&[u8]>) -> Result<u8> {
        let mut buf = [0u8; 1];
        cursor
            .read_exact(&mut buf)
            .map_err(|e| BeamError::IoError(e.to_string()))?;
        Ok(buf[0])
    }
}
