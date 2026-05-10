//! IR values - represent SSA values in the intermediate representation.
//!
//! Each value in the IR is either:
//! 1. An instruction result (computed by an IRInst)
//! 2. A constant (known at compile time)
//! 3. An argument (function parameter)
//!
//! Values are identified by ValueId and carry type information.

use crate::type_system::{ConstantValue, IRType};

/// An SSA value in the IR.
#[derive(Debug, Clone)]
pub enum IRValue {
    /// A constant value known at compile time.
    Constant {
        /// The constant value
        value: ConstantValue,
        /// The type of the constant
        ty: IRType,
    },
    /// An instruction result.
    InstResult {
        /// The instruction that produces this value
        inst: super::InstId,
        /// The result index (for instructions with multiple results)
        result_index: u32,
        /// The type of the value
        ty: IRType,
    },
    /// A function argument.
    Argument {
        /// The argument index
        index: u32,
        /// The type of the argument
        ty: IRType,
    },
    /// A placeholder value (used during construction, replaced later).
    Placeholder,
}

/// Identifier for an IR value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IRValueId(pub usize);

impl IRValue {
    /// Get the type of this value.
    pub fn ty(&self) -> &IRType {
        match self {
            IRValue::Constant { ty, .. } => ty,
            IRValue::InstResult { ty, .. } => ty,
            IRValue::Argument { ty, .. } => ty,
            IRValue::Placeholder => &IRType::Any,
        }
    }

    /// Check if this is a constant.
    pub fn is_constant(&self) -> bool {
        matches!(self, IRValue::Constant { .. })
    }

    /// Check if this is an instruction result.
    pub fn is_inst_result(&self) -> bool {
        matches!(self, IRValue::InstResult { .. })
    }

    /// Check if this is an argument.
    pub fn is_argument(&self) -> bool {
        matches!(self, IRValue::Argument { .. })
    }

    /// Try to extract a constant integer value.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            IRValue::Constant {
                value: ConstantValue::Int(i),
                ..
            } => Some(*i),
            _ => None,
        }
    }

    /// Try to extract a constant atom index.
    pub fn as_atom_index(&self) -> Option<u32> {
        match self {
            IRValue::Constant {
                value: ConstantValue::Atom(a),
                ..
            } => Some(*a),
            _ => None,
        }
    }

    /// Try to extract a constant boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            IRValue::Constant {
                value: ConstantValue::True,
                ..
            } => Some(true),
            IRValue::Constant {
                value: ConstantValue::False,
                ..
            } => Some(false),
            _ => None,
        }
    }
}

/// A use of a value in an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueUse {
    /// The value being used
    pub value_id: IRValueId,
    /// The instruction using this value
    pub user_inst: InstId,
}

/// A definition of a value (the instruction that produces it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueDef {
    /// The instruction defining this value
    pub inst_id: InstId,
    /// The result index
    pub result_index: u32,
}
