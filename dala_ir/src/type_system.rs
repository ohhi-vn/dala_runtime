//! Type system for the Dala IR.
//!
//! The BEAM VM is dynamically typed, but for optimization purposes,
//! the IR tracks type information. Types are used for:
//! - Constant propagation
//! - Dead code elimination
//! - Specialization
//! - Register allocation hints
//!
//! The type system uses a lattice structure where Top is "any type"
//! and Bottom is "unreachable code". Types can be refined through
//! analysis and pattern matching.

use std::fmt;

/// The lattice of types in the IR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IRType {
    /// Any possible term (top of the lattice)
    Any,
    /// Bottom type (unreachable code)
    Bottom,
    /// Small integer (fixnum)
    SmallInt,
    /// Non-negative small integer
    NonNegInt,
    /// Float
    Float,
    /// Atom
    Atom,
    /// Boolean (true or false)
    Boolean,
    /// Nil (empty list)
    Nil,
    /// Cons cell (non-empty list)
    Cons,
    /// List (nil or cons)
    List,
    /// Tuple of known arity
    Tuple { arity: u32 },
    /// Map
    Map,
    /// Binary (heap or refc)
    Binary,
    /// Function/closure
    Fun { arity: u32 },
    /// PID
    Pid,
    /// Port
    Port,
    /// Reference
    Reference,
    /// Union of two types
    Union(Box<IRType>, Box<IRType>),
    /// A specific constant value
    Constant(ConstantValue),
}

/// A specific constant value that can be tracked through the IR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstantValue {
    /// A specific small integer
    Int(i64),
    /// A specific atom (by index)
    Atom(u32),
    /// Nil
    Nil,
    /// Boolean true
    True,
    /// Boolean false
    False,
}

/// The top type (any possible value).
pub const TOP: IRType = IRType::Any;

/// The bottom type (unreachable code).
pub const BOTTOM: IRType = IRType::Bottom;

/// Type lattice operations.
impl IRType {
    /// Compute the least upper bound (join) of two types.
    ///
    /// This is used when two control flow paths merge - the resulting
    /// type must accommodate both possibilities.
    pub fn join(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }

        match (self, other) {
            // Bottom is absorbed by any type
            (IRType::Bottom, t) | (t, IRType::Bottom) => t.clone(),

            // Any absorbs everything
            (IRType::Any, _) | (_, IRType::Any) => IRType::Any,

            // Same category unions
            (IRType::SmallInt, IRType::NonNegInt) | (IRType::NonNegInt, IRType::SmallInt) => {
                IRType::SmallInt
            }
            (IRType::Nil, IRType::Cons) | (IRType::Cons, IRType::Nil) => IRType::List,

            // Constant with general type
            (IRType::Constant(_), general) | (general, IRType::Constant(_)) => general.clone(),

            // Union types
            (IRType::Union(a, b), c) => {
                let ab = a.join(b);
                ab.join(c)
            }
            (c, IRType::Union(a, b)) => {
                let ab = a.join(b);
                c.join(&ab)
            }

            // Default: fall back to Any
            _ => IRType::Any,
        }
    }

    /// Compute the greatest lower bound (meet) of two types.
    ///
    /// This is used for type refinement - narrowing a type based on
    /// constraints (e.g., after a type test).
    pub fn meet(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }

        match (self, other) {
            // Bottom absorbs any meet
            (IRType::Bottom, _) | (_, IRType::Bottom) => IRType::Bottom,

            // Any is neutral for meet
            (IRType::Any, t) | (t, IRType::Any) => t.clone(),

            // Constant refinement
            (IRType::Constant(c), general) | (general, IRType::Constant(c)) => {
                if general.contains_constant(c) {
                    IRType::Constant(c.clone())
                } else {
                    IRType::Bottom
                }
            }

            // Subtype relationships
            (IRType::Nil, IRType::List) | (IRType::List, IRType::Nil) => IRType::Nil,
            (IRType::Cons, IRType::List) | (IRType::List, IRType::Cons) => IRType::Cons,
            (IRType::NonNegInt, IRType::SmallInt) | (IRType::SmallInt, IRType::NonNegInt) => {
                IRType::NonNegInt
            }

            // Default: check if compatible
            _ => {
                if self.contains(other) {
                    other.clone()
                } else if other.contains(self) {
                    self.clone()
                } else {
                    IRType::Bottom
                }
            }
        }
    }

    /// Check if this type contains a specific constant.
    fn contains_constant(&self, c: &ConstantValue) -> bool {
        match (self, c) {
            (IRType::SmallInt, ConstantValue::Int(_)) => true,
            (IRType::NonNegInt, ConstantValue::Int(i)) => *i >= 0,
            (IRType::Atom, ConstantValue::Atom(_)) => true,
            (IRType::Boolean, ConstantValue::True) | (IRType::Boolean, ConstantValue::False) => {
                true
            }
            (IRType::Nil, ConstantValue::Nil) => true,
            (IRType::True, ConstantValue::True) => true,
            (IRType::False, ConstantValue::False) => true,
            (IRType::Any, _) => true,
            (IRType::Union(a, b), _) => a.contains_constant(c) || b.contains_constant(c),
            _ => false,
        }
    }

    /// Check if this type is a subtype of another.
    pub fn contains(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        match (self, other) {
            (IRType::Any, _) => true,
            (_, IRType::Bottom) => true,
            (IRType::List, IRType::Nil) | (IRType::List, IRType::Cons) => true,
            (IRType::SmallInt, IRType::NonNegInt) => true,
            (IRType::Union(a, b), _) => a.contains(other) || b.contains(other),
            _ => false,
        }
    }

    /// Check if this type is definitely a small integer.
    pub fn is_definitely_small_int(&self) -> bool {
        matches!(self, IRType::SmallInt | IRType::NonNegInt)
    }

    /// Check if this type is definitely an atom.
    pub fn is_definitely_atom(&self) -> bool {
        matches!(
            self,
            IRType::Atom | IRType::Boolean | IRType::Constant(ConstantValue::Atom(_))
        )
    }

    /// Check if this type is definitely a tuple.
    pub fn is_definitely_tuple(&self) -> bool {
        matches!(self, IRType::Tuple { .. })
    }

    /// Check if this type is definitely a list.
    pub fn is_definitely_list(&self) -> bool {
        matches!(self, IRType::List | IRType::Cons | IRType::Nil)
    }

    /// Check if this type is definitely a map.
    pub fn is_definitely_map(&self) -> bool {
        matches!(self, IRType::Map)
    }

    /// Check if this type is definitely a float.
    pub fn is_definitely_float(&self) -> bool {
        matches!(self, IRType::Float)
    }

    /// Check if this type is definitely a function.
    pub fn is_definitely_fun(&self) -> bool {
        matches!(self, IRType::Fun { .. })
    }

    /// Check if this type is definitely a PID.
    pub fn is_definitely_pid(&self) -> bool {
        matches!(self, IRType::Pid)
    }
}

impl fmt::Display for IRType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IRType::Any => write!(f, "any"),
            IRType::Bottom => write!(f, "bottom"),
            IRType::SmallInt => write!(f, "smallint"),
            IRType::NonNegInt => write!(f, "nonnegint"),
            IRType::Float => write!(f, "float"),
            IRType::Atom => write!(f, "atom"),
            IRType::Boolean => write!(f, "boolean"),
            IRType::Nil => write!(f, "nil"),
            IRType::Cons => write!(f, "cons"),
            IRType::List => write!(f, "list"),
            IRType::Tuple { arity } => write!(f, "tuple({})", arity),
            IRType::Map => write!(f, "map"),
            IRType::Binary => write!(f, "binary"),
            IRType::Fun { arity } => write!(f, "fun({})", arity),
            IRType::Pid => write!(f, "pid"),
            IRType::Port => write!(f, "port"),
            IRType::Reference => write!(f, "reference"),
            IRType::Union(a, b) => write!(f, "({} ∪ {})", a, b),
            IRType::Constant(c) => match c {
                ConstantValue::Int(i) => write!(f, "const({})", i),
                ConstantValue::Atom(a) => write!(f, "const(atom:{})", a),
                ConstantValue::Nil => write!(f, "const(nil)"),
                ConstantValue::True => write!(f, "const(true)"),
                ConstantValue::False => write!(f, "const(false)"),
            },
        }
    }
}

impl Default for IRType {
    fn default() -> Self {
        IRType::Any
    }
}
