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

/// The kind of type in the IR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    /// Any possible term (top of the lattice)
    Any,
    /// Bottom type (unreachable code)
    Bottom,
    /// Small integer (fixnum)
    SmallInt,
    /// Non-negative small integer
    NonNegInt,
    /// 64-bit integer
    Int64,
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

/// The IR type representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IRType {
    /// The kind of this type
    pub kind: TypeKind,
}

/// The top type (any possible value).
pub const TOP: IRType = IRType {
    kind: TypeKind::Any,
};

/// The bottom type (unreachable code).
pub const BOTTOM: IRType = IRType {
    kind: TypeKind::Bottom,
};

impl IRType {
    /// Create a new IRType with the given kind.
    pub fn new(kind: TypeKind) -> Self {
        Self { kind }
    }

    /// Compute the least upper bound (join) of two types.
    ///
    /// This is used when two control flow paths merge - the resulting
    /// type must accommodate both possibilities.
    pub fn join(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }

        match (&self.kind, &other.kind) {
            // Bottom is absorbed by any type
            (TypeKind::Bottom, t) | (t, TypeKind::Bottom) => IRType::new(t.clone()),

            // Any absorbs everything
            (TypeKind::Any, _) | (_, TypeKind::Any) => IRType::new(TypeKind::Any),

            // Same category unions
            (TypeKind::SmallInt, TypeKind::NonNegInt)
            | (TypeKind::NonNegInt, TypeKind::SmallInt) => IRType::new(TypeKind::SmallInt),
            (TypeKind::Nil, TypeKind::Cons) | (TypeKind::Cons, TypeKind::Nil) => {
                IRType::new(TypeKind::List)
            }

            // Constant with general type
            (TypeKind::Constant(_), general) | (general, TypeKind::Constant(_)) => {
                IRType::new(general.clone())
            }

            // Union types
            (TypeKind::Union(a, b), c) => {
                let ab = a.join(b);
                ab.join(&IRType::new(c.clone()))
            }
            (c, TypeKind::Union(a, b)) => {
                let ab = a.join(b);
                IRType::new(c.clone()).join(&ab)
            }

            // Default: fall back to Any
            _ => IRType::new(TypeKind::Any),
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

        match (&self.kind, &other.kind) {
            // Bottom absorbs any meet
            (TypeKind::Bottom, _) | (_, TypeKind::Bottom) => IRType::new(TypeKind::Bottom),

            // Any is neutral for meet
            (TypeKind::Any, t) | (t, TypeKind::Any) => IRType::new(t.clone()),

            // Constant refinement
            (TypeKind::Constant(c), general) | (general, TypeKind::Constant(c)) => {
                if IRType::new(general.clone()).contains_constant(c) {
                    IRType::new(TypeKind::Constant(c.clone()))
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // Subtype relationships
            (TypeKind::Nil, TypeKind::List) | (TypeKind::List, TypeKind::Nil) => {
                IRType::new(TypeKind::Nil)
            }
            (TypeKind::Cons, TypeKind::List) | (TypeKind::List, TypeKind::Cons) => {
                IRType::new(TypeKind::Cons)
            }
            (TypeKind::NonNegInt, TypeKind::SmallInt)
            | (TypeKind::SmallInt, TypeKind::NonNegInt) => IRType::new(TypeKind::NonNegInt),

            // Default: check if compatible
            _ => {
                if self.contains(other) {
                    other.clone()
                } else if other.contains(self) {
                    self.clone()
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }
        }
    }

    /// Check if this type contains a specific constant.
    fn contains_constant(&self, c: &ConstantValue) -> bool {
        match (&self.kind, c) {
            (TypeKind::SmallInt, ConstantValue::Int(_)) => true,
            (TypeKind::NonNegInt, ConstantValue::Int(i)) => *i >= 0,
            (TypeKind::Atom, ConstantValue::Atom(_)) => true,
            (TypeKind::Boolean, ConstantValue::True)
            | (TypeKind::Boolean, ConstantValue::False) => true,
            (TypeKind::Nil, ConstantValue::Nil) => true,
            (TypeKind::Any, _) => true,
            (TypeKind::Union(a, b), _) => {
                a.as_ref().contains_constant(c) || b.as_ref().contains_constant(c)
            }
            _ => false,
        }
    }

    /// Check if this type is a subtype of another.
    pub fn contains(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        match (&self.kind, &other.kind) {
            (TypeKind::Any, _) => true,
            (_, TypeKind::Bottom) => true,
            (TypeKind::List, TypeKind::Nil) | (TypeKind::List, TypeKind::Cons) => true,
            (TypeKind::SmallInt, TypeKind::NonNegInt) => true,
            (TypeKind::Union(a, b), _) => a.as_ref().contains(other) || b.as_ref().contains(other),
            _ => false,
        }
    }

    /// Check if this type is definitely a small integer.
    pub fn is_definitely_small_int(&self) -> bool {
        matches!(self.kind, TypeKind::SmallInt | TypeKind::NonNegInt)
    }

    /// Check if this type is definitely an atom.
    pub fn is_definitely_atom(&self) -> bool {
        matches!(
            self.kind,
            TypeKind::Atom | TypeKind::Boolean | TypeKind::Constant(ConstantValue::Atom(_))
        )
    }

    /// Check if this type is definitely a tuple.
    pub fn is_definitely_tuple(&self) -> bool {
        matches!(self.kind, TypeKind::Tuple { .. })
    }

    /// Check if this type is definitely a list.
    pub fn is_definitely_list(&self) -> bool {
        matches!(self.kind, TypeKind::List | TypeKind::Cons | TypeKind::Nil)
    }

    /// Check if this type is definitely a map.
    pub fn is_definitely_map(&self) -> bool {
        matches!(self.kind, TypeKind::Map)
    }

    /// Check if this type is definitely a float.
    pub fn is_definitely_float(&self) -> bool {
        matches!(self.kind, TypeKind::Float)
    }

    /// Check if this type is definitely a function.
    pub fn is_definitely_fun(&self) -> bool {
        matches!(self.kind, TypeKind::Fun { .. })
    }

    /// Check if this type is definitely a PID.
    pub fn is_definitely_pid(&self) -> bool {
        matches!(self.kind, TypeKind::Pid)
    }
}

impl fmt::Display for IRType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TypeKind::Any => write!(f, "any"),
            TypeKind::Bottom => write!(f, "bottom"),
            TypeKind::SmallInt => write!(f, "smallint"),
            TypeKind::NonNegInt => write!(f, "nonnegint"),
            TypeKind::Int64 => write!(f, "int64"),
            TypeKind::Float => write!(f, "float"),
            TypeKind::Atom => write!(f, "atom"),
            TypeKind::Boolean => write!(f, "boolean"),
            TypeKind::Nil => write!(f, "nil"),
            TypeKind::Cons => write!(f, "cons"),
            TypeKind::List => write!(f, "list"),
            TypeKind::Tuple { arity } => write!(f, "tuple({})", arity),
            TypeKind::Map => write!(f, "map"),
            TypeKind::Binary => write!(f, "binary"),
            TypeKind::Fun { arity } => write!(f, "fun({})", arity),
            TypeKind::Pid => write!(f, "pid"),
            TypeKind::Port => write!(f, "port"),
            TypeKind::Reference => write!(f, "reference"),
            TypeKind::Union(a, b) => write!(f, "({} ∪ {})", a, b),
            TypeKind::Constant(c) => match c {
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
        IRType::new(TypeKind::Any)
    }
}
