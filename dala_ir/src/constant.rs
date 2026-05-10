//! Constant representation in the IR.
//!
//! Constants are values known at compile time. Tracking them through
//! the IR enables constant propagation, dead code elimination, and
//! other optimizations.

use crate::type_system::{ConstantValue, IRType};

/// A constant value in the IR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Constant {
    /// A small integer constant.
    Int(i64),
    /// An atom constant (by index in the atom table).
    Atom(u32),
    /// The nil constant [].
    Nil,
    /// The boolean true.
    True,
    /// The boolean false.
    False,
    /// A float constant.
    Float(f64),
    /// A tuple constant (for constant propagation of small tuples).
    Tuple(Vec<Constant>),
    /// A list constant.
    List(Vec<Constant>),
    /// A binary constant.
    Binary(Vec<u8>),
}

impl Constant {
    /// Get the IR type of this constant.
    pub fn ir_type(&self) -> IRType {
        match self {
            Constant::Int(i) => {
                if *i >= 0 {
                    IRType::NonNegInt
                } else {
                    IRType::SmallInt
                }
            }
            Constant::Atom(_) => IRType::Atom,
            Constant::Nil => IRType::Nil,
            Constant::True => IRType::True,
            Constant::False => IRType::False,
            Constant::Float(_) => IRType::Float,
            Constant::Tuple(elements) => IRType::Tuple {
                arity: elements.len() as u32,
            },
            Constant::List(_) => IRType::List,
            Constant::Binary(_) => IRType::Binary,
        }
    }

    /// Try to interpret this constant as an integer.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Constant::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to interpret this constant as an atom index.
    pub fn as_atom_index(&self) -> Option<u32> {
        match self {
            Constant::Atom(a) => Some(*a),
            _ => None,
        }
    }

    /// Try to interpret this constant as a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Constant::True => Some(true),
            Constant::False => Some(false),
            _ => None,
        }
    }

    /// Try to interpret this constant as a float.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Constant::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Check if this is a compile-time known value.
    pub fn is_known(&self) -> bool {
        true // All constants are known by definition
    }
}

impl From<i64> for Constant {
    fn from(val: i64) -> Self {
        Constant::Int(val)
    }
}

impl From<bool> for Constant {
    fn from(val: bool) -> Self {
        if val {
            Constant::True
        } else {
            Constant::False
        }
    }
}

impl From<f64> for Constant {
    fn from(val: f64) -> Self {
        Constant::Float(val)
    }
}

impl From<ConstantValue> for IRType {
    fn from(cv: ConstantValue) -> Self {
        match cv {
            ConstantValue::Int(_) => IRType::SmallInt,
            ConstantValue::Atom(_) => IRType::Atom,
            ConstantValue::Nil => IRType::Nil,
            ConstantValue::True => IRType::True,
            ConstantValue::False => IRType::False,
        }
    }
}
