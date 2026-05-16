//! Constant representation in the IR.
//!
//! Constants are values known at compile time. Tracking them through
//! the IR enables constant propagation, dead code elimination, and
//! other optimizations.

use crate::type_system::{ConstantValue, IRType, TypeKind};

/// A constant value in the IR.
#[derive(Debug, Clone, PartialEq)]
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
                    IRType::new(TypeKind::NonNegInt)
                } else {
                    IRType::new(TypeKind::SmallInt)
                }
            }
            Constant::Atom(_) => IRType::new(TypeKind::Atom),
            Constant::Nil => IRType::new(TypeKind::Nil),
            Constant::True => IRType::new(TypeKind::Boolean),
            Constant::False => IRType::new(TypeKind::Boolean),
            Constant::Float(_) => IRType::new(TypeKind::Float),
            Constant::Tuple(elements) => IRType::new(TypeKind::Tuple {
                arity: elements.len() as u32,
            }),
            Constant::List(_) => IRType::new(TypeKind::List),
            Constant::Binary(_) => IRType::new(TypeKind::Binary),
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
        if val { Constant::True } else { Constant::False }
    }
}

impl From<Constant> for ConstantValue {
    fn from(c: Constant) -> Self {
        match c {
            Constant::Int(i) => ConstantValue::Int(i),
            Constant::Atom(a) => ConstantValue::Atom(a),
            Constant::Nil => ConstantValue::Nil,
            Constant::True => ConstantValue::True,
            Constant::False => ConstantValue::False,
            Constant::Float(f) => ConstantValue::Float(f.to_bits()),
            Constant::Tuple(_) | Constant::List(_) | Constant::Binary(_) => ConstantValue::Nil,
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
            ConstantValue::Int(_) => IRType::new(TypeKind::SmallInt),
            ConstantValue::Atom(_) => IRType::new(TypeKind::Atom),
            ConstantValue::Nil => IRType::new(TypeKind::Nil),
            ConstantValue::True => IRType::new(TypeKind::Boolean),
            ConstantValue::False => IRType::new(TypeKind::Boolean),
            ConstantValue::Float(_) => IRType::new(TypeKind::Float),
        }
    }
}
