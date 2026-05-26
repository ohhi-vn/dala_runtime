//! Edge case tests for dala_beam_loader.

use super::*;
use crate::chunk::chunk_ids;
use crate::chunk::{BeamSection, CodeEntry, ExportEntry, LineEntry};
use std::io::Cursor;

// ═══════════════════════════════════════════════════════════════════════════
// BeamRegister
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_register_x_variant() {
    let reg = BeamRegister::X(42);
    assert_eq!(reg, BeamRegister::X(42));
    assert_ne!(reg, BeamRegister::X(43));
}

#[test]
fn test_beam_register_y_variant() {
    let reg = BeamRegister::Y(7);
    assert_eq!(reg, BeamRegister::Y(7));
    assert_ne!(reg, BeamRegister::Y(8));
}

#[test]
fn test_beam_register_f_variant() {
    let reg = BeamRegister::F(3);
    assert_eq!(reg, BeamRegister::F(3));
    assert_ne!(reg, BeamRegister::F(4));
}

#[test]
fn test_beam_register_cross_variant_inequality() {
    // X, Y, F with same number should not be equal
    let x = BeamRegister::X(1);
    let y = BeamRegister::Y(1);
    let f = BeamRegister::F(1);
    assert_ne!(x, y);
    assert_ne!(y, f);
    assert_ne!(x, f);
}

#[test]
fn test_beam_register_copy() {
    let reg = BeamRegister::X(5);
    let reg_copy = reg;
    // Both should be usable (Copy trait)
    assert_eq!(reg, reg_copy);
}

#[test]
fn test_beam_register_zero_index() {
    let x = BeamRegister::X(0);
    let y = BeamRegister::Y(0);
    let f = BeamRegister::F(0);
    assert_eq!(x, BeamRegister::X(0));
    assert_eq!(y, BeamRegister::Y(0));
    assert_eq!(f, BeamRegister::F(0));
}

#[test]
fn test_beam_register_max_index() {
    let x = BeamRegister::X(u32::MAX);
    let y = BeamRegister::Y(u32::MAX);
    let f = BeamRegister::F(u32::MAX);
    assert_eq!(x, BeamRegister::X(u32::MAX));
    assert_eq!(y, BeamRegister::Y(u32::MAX));
    assert_eq!(f, BeamRegister::F(u32::MAX));
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamOperand
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_operand_register_variant() {
    let op = BeamOperand::Register(BeamRegister::X(1));
    assert_eq!(op, BeamOperand::Register(BeamRegister::X(1)));
    assert_ne!(op, BeamOperand::Register(BeamRegister::X(2)));
}

#[test]
fn test_beam_operand_label_variant() {
    let op = BeamOperand::Label(42);
    assert_eq!(op, BeamOperand::Label(42));
    assert_ne!(op, BeamOperand::Label(43));
}

#[test]
fn test_beam_operand_integer_variant() {
    let op = BeamOperand::Integer(-1);
    assert_eq!(op, BeamOperand::Integer(-1));
    assert_ne!(op, BeamOperand::Integer(0));
    assert_ne!(op, BeamOperand::Integer(1));
}

#[test]
fn test_beam_operand_float_variant() {
    let op = BeamOperand::Float(3.14);
    assert_eq!(op, BeamOperand::Float(3.14));
    assert_ne!(op, BeamOperand::Float(2.71));
}

#[test]
fn test_beam_operand_float_nan() {
    // NaN is not equal to itself in IEEE 754, which means PartialEq will return false
    let nan_op = BeamOperand::Float(f64::NAN);
    assert_ne!(nan_op, BeamOperand::Float(f64::NAN));
}

#[test]
fn test_beam_operand_float_zero_sign() {
    // +0.0 and -0.0 are equal in Rust's PartialEq for f64
    let pos_zero = BeamOperand::Float(0.0);
    let neg_zero = BeamOperand::Float(-0.0);
    assert_eq!(pos_zero, neg_zero);
}

#[test]
fn test_beam_operand_atom_index_variant() {
    let op = BeamOperand::AtomIndex(5);
    assert_eq!(op, BeamOperand::AtomIndex(5));
    assert_ne!(op, BeamOperand::AtomIndex(6));
}

#[test]
fn test_beam_operand_clone() {
    let op = BeamOperand::Register(BeamRegister::Y(3));
    let cloned = op.clone();
    assert_eq!(op, cloned);
}

#[test]
fn test_beam_operand_cross_variant_inequality() {
    let reg = BeamOperand::Register(BeamRegister::X(0));
    let label = BeamOperand::Label(0);
    let int = BeamOperand::Integer(0);
    let float = BeamOperand::Float(0.0);
    let atom = BeamOperand::AtomIndex(0);

    assert_ne!(reg, label);
    assert_ne!(reg, int);
    assert_ne!(reg, float);
    assert_ne!(reg, atom);
    assert_ne!(label, int);
    assert_ne!(label, float);
    assert_ne!(label, atom);
    assert_ne!(int, float);
    assert_ne!(int, atom);
    assert_ne!(float, atom);
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamInstruction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_instruction_creation_basic() {
    let inst = BeamInstruction {
        opcode: 0,
        operands: vec![BeamOperand::Register(BeamRegister::X(0))],
        line: None,
    };
    assert_eq!(inst.opcode, 0);
    assert_eq!(inst.operands.len(), 1);
    assert!(inst.line.is_none());
}

#[test]
fn test_beam_instruction_creation_with_line() {
    let inst = BeamInstruction {
        opcode: 1,
        operands: vec![],
        line: Some(42),
    };
    assert_eq!(inst.opcode, 1);
    assert_eq!(inst.operands.len(), 0);
    assert_eq!(inst.line, Some(42));
}

#[test]
fn test_beam_instruction_empty_operands() {
    let inst = BeamInstruction {
        opcode: 99,
        operands: vec![],
        line: None,
    };
    assert!(inst.operands.is_empty());
}

#[test]
fn test_beam_instruction_many_operands() {
    let operands = vec![
        BeamOperand::Register(BeamRegister::X(0)),
        BeamOperand::Register(BeamRegister::Y(1)),
        BeamOperand::Label(10),
        BeamOperand::Integer(42),
        BeamOperand::Float(1.5),
        BeamOperand::AtomIndex(3),
    ];
    let inst = BeamInstruction {
        opcode: 50,
        operands: operands.clone(),
        line: Some(100),
    };
    assert_eq!(inst.operands.len(), 6);
    assert_eq!(inst.operands[0], BeamOperand::Register(BeamRegister::X(0)));
    assert_eq!(inst.operands[5], BeamOperand::AtomIndex(3));
}

#[test]
fn test_beam_instruction_large_opcode() {
    let inst = BeamInstruction {
        opcode: u32::MAX,
        operands: vec![],
        line: None,
    };
    assert_eq!(inst.opcode, u32::MAX);
}

#[test]
fn test_beam_instruction_clone() {
    let inst = BeamInstruction {
        opcode: 5,
        operands: vec![BeamOperand::Integer(10)],
        line: Some(1),
    };
    let cloned = inst.clone();
    assert_eq!(inst.opcode, cloned.opcode);
    assert_eq!(inst.operands, cloned.operands);
    assert_eq!(inst.line, cloned.line);
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamFunction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_function_creation() {
    let func = BeamFunction {
        name: "test_func".to_string(),
        arity: 2,
        label: 5,
        code: vec![],
    };
    assert_eq!(func.name, "test_func");
    assert_eq!(func.arity, 2);
    assert_eq!(func.label, 5);
    assert!(func.code.is_empty());
}

#[test]
fn test_beam_function_empty_code() {
    let func = BeamFunction {
        name: "empty".to_string(),
        arity: 0,
        label: 0,
        code: vec![],
    };
    assert_eq!(func.code.len(), 0);
}

#[test]
fn test_beam_function_many_instructions() {
    let code: Vec<BeamInstruction> = (0..1000)
        .map(|i| BeamInstruction {
            opcode: (i % 256) as u32,
            operands: vec![BeamOperand::Integer(i as i64)],
            line: Some(i as u32),
        })
        .collect();
    let func = BeamFunction {
        name: "big_func".to_string(),
        arity: 1,
        label: 0,
        code,
    };
    assert_eq!(func.code.len(), 1000);
    assert_eq!(func.code[999].opcode, (999 % 256) as u32);
}

#[test]
fn test_beam_function_clone() {
    let func = BeamFunction {
        name: "clone_test".to_string(),
        arity: 3,
        label: 7,
        code: vec![BeamInstruction {
            opcode: 0,
            operands: vec![BeamOperand::Register(BeamRegister::X(0))],
            line: None,
        }],
    };
    let cloned = func.clone();
    assert_eq!(func.name, cloned.name);
    assert_eq!(func.arity, cloned.arity);
    assert_eq!(func.code.len(), cloned.code.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamModule
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_module_new() {
    let module = BeamModule::new("test_module".to_string());
    assert_eq!(module.name, "test_module");
    assert!(module.functions.is_empty());
    assert!(module.exports.is_empty());
    assert!(module.atoms.is_empty());
    assert!(module.attributes.is_empty());
    assert!(module.compile_info.is_none());
}

#[test]
fn test_beam_module_get_function_hit() {
    let mut module = BeamModule::new("test".to_string());
    let func = BeamFunction {
        name: "my_func".to_string(),
        arity: 2,
        label: 0,
        code: vec![],
    };
    module.functions.insert(("my_func".to_string(), 2), func);
    assert!(module.get_function("my_func", 2).is_some());
    assert_eq!(module.get_function("my_func", 2).unwrap().name, "my_func");
}

#[test]
fn test_beam_module_get_function_miss_wrong_name() {
    let mut module = BeamModule::new("test".to_string());
    module.functions.insert(
        ("foo".to_string(), 1),
        BeamFunction {
            name: "foo".to_string(),
            arity: 1,
            label: 0,
            code: vec![],
        },
    );
    assert!(module.get_function("bar", 1).is_none());
}

#[test]
fn test_beam_module_get_function_miss_wrong_arity() {
    let mut module = BeamModule::new("test".to_string());
    module.functions.insert(
        ("foo".to_string(), 1),
        BeamFunction {
            name: "foo".to_string(),
            arity: 1,
            label: 0,
            code: vec![],
        },
    );
    assert!(module.get_function("foo", 2).is_none());
}

#[test]
fn test_beam_module_get_function_empty() {
    let module = BeamModule::new("empty".to_string());
    assert!(module.get_function("anything", 0).is_none());
}

#[test]
fn test_beam_module_exported_functions() {
    let mut module = BeamModule::new("test".to_string());
    module.exports = vec![("foo".to_string(), 1, 10), ("bar".to_string(), 2, 20)];
    let exports = module.exported_functions();
    assert_eq!(exports.len(), 2);
    assert_eq!(exports[0], ("foo".to_string(), 1, 10));
    assert_eq!(exports[1], ("bar".to_string(), 2, 20));
}

#[test]
fn test_beam_module_exported_functions_empty() {
    let module = BeamModule::new("test".to_string());
    assert!(module.exported_functions().is_empty());
}

#[test]
fn test_beam_module_function_count_empty() {
    let module = BeamModule::new("test".to_string());
    assert_eq!(module.function_count(), 0);
}

#[test]
fn test_beam_module_function_count_non_empty() {
    let mut module = BeamModule::new("test".to_string());
    for i in 0..5 {
        module.functions.insert(
            (format!("f{}", i), i as u32),
            BeamFunction {
                name: format!("f{}", i),
                arity: i as u32,
                label: 0,
                code: vec![],
            },
        );
    }
    assert_eq!(module.function_count(), 5);
}

#[test]
fn test_beam_module_clone() {
    let mut module = BeamModule::new("original".to_string());
    module.atoms = vec!["atom1".to_string(), "atom2".to_string()];
    let cloned = module.clone();
    assert_eq!(module.name, cloned.name);
    assert_eq!(module.atoms, cloned.atoms);
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamChunk
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_chunk_as_u32_slice_aligned() {
    // 8 bytes = 2 u32 values, properly aligned
    let data = vec![0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
    let chunk = BeamChunk {
        id: "Test".to_string(),
        data,
    };
    let result = chunk.as_u32_slice().unwrap();
    assert_eq!(result, vec![1, 2]);
}

#[test]
fn test_beam_chunk_as_u32_slice_empty() {
    let chunk = BeamChunk {
        id: "Empty".to_string(),
        data: vec![],
    };
    let result = chunk.as_u32_slice().unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_beam_chunk_as_u32_slice_non_aligned() {
    // 5 bytes is not aligned to 4
    let chunk = BeamChunk {
        id: "Bad".to_string(),
        data: vec![0x00, 0x00, 0x00, 0x01, 0xFF],
    };
    assert!(chunk.as_u32_slice().is_err());
}

#[test]
fn test_beam_chunk_as_u32_slice_single() {
    let chunk = BeamChunk {
        id: "One".to_string(),
        data: vec![0x00, 0x00, 0x00, 0x2A],
    };
    let result = chunk.as_u32_slice().unwrap();
    assert_eq!(result, vec![42]);
}

#[test]
fn test_beam_chunk_as_bytes() {
    let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let chunk = BeamChunk {
        id: "Bytes".to_string(),
        data: data.clone(),
    };
    assert_eq!(chunk.as_bytes(), &data[..]);
}

#[test]
fn test_beam_chunk_as_bytes_empty() {
    let chunk = BeamChunk {
        id: "Empty".to_string(),
        data: vec![],
    };
    assert!(chunk.as_bytes().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamSection
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_section_atoms() {
    let section = BeamSection::Atoms {
        atoms: vec!["hello".to_string(), "world".to_string()],
    };
    match section {
        BeamSection::Atoms { atoms } => assert_eq!(atoms.len(), 2),
        _ => panic!("Expected Atoms variant"),
    }
}

#[test]
fn test_beam_section_code() {
    let section = BeamSection::Code {
        functions: vec![CodeEntry {
            name: 1,
            arity: 2,
            label: 3,
            num_instructions: 10,
            instructions: vec![0; 40],
        }],
    };
    match section {
        BeamSection::Code { functions } => assert_eq!(functions.len(), 1),
        _ => panic!("Expected Code variant"),
    }
}

#[test]
fn test_beam_section_exports() {
    let section = BeamSection::Exports {
        exports: vec![ExportEntry {
            name: 1,
            arity: 2,
            label: 5,
        }],
    };
    match section {
        BeamSection::Exports { exports } => assert_eq!(exports.len(), 1),
        _ => panic!("Expected Exports variant"),
    }
}

#[test]
fn test_beam_section_literals() {
    let section = BeamSection::Literals {
        literals: vec![1, 2, 3],
    };
    match section {
        BeamSection::Literals { literals } => assert_eq!(literals, vec![1, 2, 3]),
        _ => panic!("Expected Literals variant"),
    }
}

#[test]
fn test_beam_section_line_info() {
    let section = BeamSection::LineInfo {
        entries: vec![LineEntry {
            location: 1,
            line: 42,
        }],
    };
    match section {
        BeamSection::LineInfo { entries } => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].line, 42);
        }
        _ => panic!("Expected LineInfo variant"),
    }
}

#[test]
fn test_beam_section_unknown() {
    let section = BeamSection::Unknown {
        id: "FOO!".to_string(),
        data: vec![0x01, 0x02],
    };
    match section {
        BeamSection::Unknown { id, data } => {
            assert_eq!(id, "FOO!");
            assert_eq!(data, vec![0x01, 0x02]);
        }
        _ => panic!("Expected Unknown variant"),
    }
}

#[test]
fn test_beam_section_clone() {
    let section = BeamSection::Atoms {
        atoms: vec!["a".to_string()],
    };
    let cloned = section.clone();
    match cloned {
        BeamSection::Atoms { atoms } => assert_eq!(atoms, vec!["a".to_string()]),
        _ => panic!("Expected Atoms variant"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CodeEntry, ExportEntry, LineEntry
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_code_entry_creation() {
    let entry = CodeEntry {
        name: 1,
        arity: 2,
        label: 3,
        num_instructions: 100,
        instructions: vec![0u8; 200],
    };
    assert_eq!(entry.name, 1);
    assert_eq!(entry.arity, 2);
    assert_eq!(entry.label, 3);
    assert_eq!(entry.num_instructions, 100);
    assert_eq!(entry.instructions.len(), 200);
}

#[test]
fn test_code_entry_clone() {
    let entry = CodeEntry {
        name: 5,
        arity: 0,
        label: 0,
        num_instructions: 0,
        instructions: vec![],
    };
    let cloned = entry.clone();
    assert_eq!(entry.name, cloned.name);
    assert_eq!(entry.arity, cloned.arity);
    assert_eq!(entry.label, cloned.label);
    assert_eq!(entry.num_instructions, cloned.num_instructions);
}

#[test]
fn test_export_entry_creation() {
    let entry = ExportEntry {
        name: 10,
        arity: 3,
        label: 20,
    };
    assert_eq!(entry.name, 10);
    assert_eq!(entry.arity, 3);
    assert_eq!(entry.label, 20);
}

#[test]
fn test_export_entry_clone() {
    let entry = ExportEntry {
        name: 1,
        arity: 1,
        label: 1,
    };
    let cloned = entry.clone();
    assert_eq!(entry.name, cloned.name);
    assert_eq!(entry.arity, cloned.arity);
    assert_eq!(entry.label, cloned.label);
}

#[test]
fn test_line_entry_creation() {
    let entry = LineEntry {
        location: 5,
        line: 100,
    };
    assert_eq!(entry.location, 5);
    assert_eq!(entry.line, 100);
}

#[test]
fn test_line_entry_clone() {
    let entry = LineEntry {
        location: 0,
        line: 0,
    };
    let cloned = entry.clone();
    assert_eq!(entry.location, cloned.location);
    assert_eq!(entry.line, cloned.line);
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamError - Display formatting
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_error_io_error_display() {
    let err = BeamError::IoError("file not found".to_string());
    assert_eq!(format!("{}", err), "I/O error: file not found");
}

#[test]
fn test_beam_error_format_error_display() {
    let err = BeamError::FormatError("bad magic bytes".to_string());
    assert_eq!(format!("{}", err), "format error: bad magic bytes");
}

#[test]
fn test_beam_error_unexpected_eof_display() {
    let err = BeamError::UnexpectedEof;
    assert_eq!(format!("{}", err), "unexpected end of file");
}

#[test]
fn test_beam_error_unsupported_display() {
    let err = BeamError::Unsupported("new chunk type".to_string());
    assert_eq!(format!("{}", err), "unsupported feature: new chunk type");
}

#[test]
fn test_beam_error_debug() {
    let err = BeamError::IoError("test".to_string());
    let debug = format!("{:?}", err);
    assert!(debug.contains("IoError"));
}

#[test]
fn test_beam_error_clone() {
    let err = BeamError::FormatError("test".to_string());
    // Result<T> uses BeamError, verify it can be used in error paths
    let _result: Result<()> = Err(err);
}

// ═══════════════════════════════════════════════════════════════════════════
// chunk_ids - verify all constants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_chunk_id_atom() {
    assert_eq!(chunk_ids::ATOM, "Atom");
}

#[test]
fn test_chunk_id_code() {
    assert_eq!(chunk_ids::CODE, "Code");
}

#[test]
fn test_chunk_id_expt() {
    assert_eq!(chunk_ids::EXPT, "ExpT");
}

#[test]
fn test_chunk_id_litt() {
    assert_eq!(chunk_ids::LITT, "LitT");
}

#[test]
fn test_chunk_id_locl() {
    assert_eq!(chunk_ids::LOCL, "LocL");
}

#[test]
fn test_chunk_id_line() {
    assert_eq!(chunk_ids::LINE, "Line");
}

#[test]
fn test_chunk_id_attr() {
    assert_eq!(chunk_ids::ATTR, "Attr");
}

#[test]
fn test_chunk_id_csts() {
    assert_eq!(chunk_ids::CSTS, "CSts");
}

#[test]
fn test_chunk_id_strt() {
    assert_eq!(chunk_ids::STRT, "StrT");
}

#[test]
fn test_chunk_id_impt() {
    assert_eq!(chunk_ids::IMPT, "ImpT");
}

#[test]
fn test_chunk_id_for1() {
    assert_eq!(chunk_ids::FOR1, "FOR1");
}

#[test]
fn test_chunk_id_beam() {
    assert_eq!(chunk_ids::BEAM, "BEAM");
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamReader - error cases with empty reader
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_reader_read_u8_empty() {
    let data: Vec<u8> = vec![];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    assert!(reader.read_u8().is_err());
}

#[test]
fn test_beam_reader_read_u16_empty() {
    let data: Vec<u8> = vec![];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    assert!(reader.read_u16().is_err());
}

#[test]
fn test_beam_reader_read_u32_empty() {
    let data: Vec<u8> = vec![];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    assert!(reader.read_u32().is_err());
}

#[test]
fn test_beam_reader_read_u64_empty() {
    let data: Vec<u8> = vec![];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    assert!(reader.read_u64().is_err());
}

#[test]
fn test_beam_reader_read_bytes_partial() {
    let data: Vec<u8> = vec![0x01, 0x02];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    // Requesting more bytes than available should error
    assert!(reader.read_bytes(10).is_err());
}

#[test]
fn test_beam_reader_read_bytes_zero() {
    let data: Vec<u8> = vec![0x01, 0x02];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let result = reader.read_bytes(0).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_beam_reader_read_string_empty() {
    // Empty data means read_u8 fails immediately
    let data: Vec<u8> = vec![];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    assert!(reader.read_string().is_err());
}

#[test]
fn test_beam_reader_read_string_with_null() {
    // A null byte immediately means empty string
    let data: Vec<u8> = vec![0x00];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let result = reader.read_string().unwrap();
    assert_eq!(result, "");
}

#[test]
fn test_beam_reader_read_string_hello() {
    let data: Vec<u8> = vec![b'h', b'e', b'l', b'l', b'o', 0x00];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let result = reader.read_string().unwrap();
    assert_eq!(result, "hello");
}

#[test]
fn test_beam_reader_read_chunk_header_empty() {
    let data: Vec<u8> = vec![];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let result = reader.read_chunk_header().unwrap();
    assert!(result.is_none());
}

#[test]
fn test_beam_reader_read_chunk_header_partial_id() {
    // Only 2 bytes of the 4-byte ID — read_exact hits UnexpectedEof
    // which read_chunk_header converts to Ok(None)
    let data: Vec<u8> = vec![b'F', b'O'];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let result = reader.read_chunk_header().unwrap();
    assert!(result.is_none());
}

#[test]
fn test_beam_reader_read_chunk_header_no_size() {
    // 4-byte ID but no size bytes
    let data: Vec<u8> = vec![b'F', b'O', b'R', b'1'];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    assert!(reader.read_chunk_header().is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// BeamReader - position tracking
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_reader_position_initial() {
    let data: Vec<u8> = vec![0x01, 0x02, 0x03];
    let cursor = Cursor::new(data);
    let reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    assert_eq!(reader.position(), 0);
}

#[test]
fn test_beam_reader_position_after_read_u8() {
    let data: Vec<u8> = vec![0x01, 0x02, 0x03];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let _ = reader.read_u8().unwrap();
    assert_eq!(reader.position(), 1);
}

#[test]
fn test_beam_reader_position_after_read_u16() {
    let data: Vec<u8> = vec![0x00, 0x01, 0x00, 0x02];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let _ = reader.read_u16().unwrap();
    assert_eq!(reader.position(), 2);
}

#[test]
fn test_beam_reader_position_after_read_u32() {
    let data: Vec<u8> = vec![0x00, 0x00, 0x00, 0x01];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let _ = reader.read_u32().unwrap();
    assert_eq!(reader.position(), 4);
}

#[test]
fn test_beam_reader_position_after_read_u64() {
    let data: Vec<u8> = vec![0x00; 8];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let _ = reader.read_u64().unwrap();
    assert_eq!(reader.position(), 8);
}

#[test]
fn test_beam_reader_position_after_read_bytes() {
    let data: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04, 0x05];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let _ = reader.read_bytes(3).unwrap();
    assert_eq!(reader.position(), 3);
}

#[test]
fn test_beam_reader_position_after_multiple_reads() {
    let data: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let cursor = Cursor::new(data);
    let mut reader = BeamReader {
        reader: cursor,
        pos: 0,
    };
    let _ = reader.read_u8().unwrap(); // +1
    let _ = reader.read_u16().unwrap(); // +2
    let _ = reader.read_u32().unwrap(); // +4
    assert_eq!(reader.position(), 7);
}

// ═══════════════════════════════════════════════════════════════════════════
// load_beam_bytes with invalid data
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_load_beam_bytes_empty() {
    let result = load_beam_bytes(&[]);
    assert!(result.is_err());
}

#[test]
fn test_load_beam_bytes_too_short() {
    // Less than 4 bytes can't even have a FOR1 header
    let result = load_beam_bytes(&[0x01, 0x02]);
    assert!(result.is_err());
}

#[test]
fn test_load_beam_bytes_wrong_magic() {
    // "BAD!" instead of "FOR1"
    let data = b"BAD!";
    let result = load_beam_bytes(data);
    assert!(result.is_err());
}

#[test]
fn test_load_beam_bytes_for1_but_no_beam() {
    // FOR1 header but no BEAM header follows
    let data = vec![
        b'F', b'O', b'R', b'1', // FOR1
        0x00, 0x00, 0x00, 0x04, // size = 4
        b'B', b'A', b'D', b'!', // not BEAM
    ];
    let result = load_beam_bytes(&data);
    assert!(result.is_err());
}

#[test]
fn test_load_beam_bytes_for1_truncated_size() {
    // FOR1 header but truncated size field
    let data = vec![
        b'F', b'O', b'R', b'1', // FOR1
        0x00, 0x00, // only 2 bytes of size
    ];
    let result = load_beam_bytes(&data);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// CompileInfo
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_compile_info_creation() {
    let info = CompileInfo {
        source_file: Some("test.erl".to_string()),
        options: vec!["debug_info".to_string()],
    };
    assert_eq!(info.source_file, Some("test.erl".to_string()));
    assert_eq!(info.options, vec!["debug_info".to_string()]);
}

#[test]
fn test_compile_info_none_source() {
    let info = CompileInfo {
        source_file: None,
        options: vec![],
    };
    assert!(info.source_file.is_none());
    assert!(info.options.is_empty());
}

#[test]
fn test_compile_info_clone() {
    let info = CompileInfo {
        source_file: Some("mod.erl".to_string()),
        options: vec!["inline".to_string(), "no_copt".to_string()],
    };
    let cloned = info.clone();
    assert_eq!(info.source_file, cloned.source_file);
    assert_eq!(info.options, cloned.options);
}
