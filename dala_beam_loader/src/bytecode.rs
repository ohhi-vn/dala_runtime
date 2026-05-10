//! BEAM bytecode types - data structures for parsed BEAM instructions.
//!
//! These types represent the intermediate form of BEAM bytecode
//! between parsing and IR translation.

/// A BEAM register reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeamRegister {
    X(u32),
    Y(u32),
    F(u32),
}

/// A BEAM operand - can be a register, label, integer, float, or atom index.
#[derive(Debug, Clone, PartialEq)]
pub enum BeamOperand {
    Register(BeamRegister),
    Label(u32),
    Integer(i64),
    Float(f64),
    AtomIndex(u32),
}

/// A single BEAM instruction.
#[derive(Debug, Clone)]
pub struct BeamInstruction {
    pub opcode: u32,
    pub operands: Vec<BeamOperand>,
    pub line: Option<u32>,
}

/// A BEAM function (parsed from the CODE chunk).
#[derive(Debug, Clone)]
pub struct BeamFunction {
    pub name: String,
    pub arity: u32,
    pub label: u32,
    pub code: Vec<BeamInstruction>,
}
