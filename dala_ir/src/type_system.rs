//! Type system for the Dala IR.
//!
//! The BEAM VM is dynamically typed, but for optimization purposes,
//! the IR tracks type information. Types are used for:
//! - Constant propagation
//! - Dead code elimination
//! - Specialization
//! - Register allocation hints
//! - Mailbox optimization
//! - Pattern matching specialization
//! - AOT compilation
//!
//! The type system uses a lattice structure where Top is "any type"
//! and Bottom is "unreachable code". Types can be refined through
//! analysis and pattern matching.
//!
//! # Typed Runtime Metadata
//!
//! Beyond basic BEAM types, the Dala type system tracks:
//! - **Stable tuple shapes**: Fixed-layout tuples for fast access
//! - **Immutable markers**: Compiler-proven structural immutability
//! - **Binary layout metadata**: Known binary sizes and alignments
//! - **Message patterns**: Expected message shapes for mailbox specialization
//! - **Actor type hints**: Actor identity and supervision metadata
//! - **Tensor types**: Shape and dtype for AI workloads
//! - **Capability types**: Typed native resource handles

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Core Type Kinds
// ═══════════════════════════════════════════════════════════════════════════

/// The kind of type in the IR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    /// Any possible term (top of the lattice)
    Any,
    /// Bottom type (unreachable code)
    Bottom,

    // ── Immediate types ─────────────────────────────────────────────────
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

    // ── Composite types ─────────────────────────────────────────────────
    /// Cons cell (non-empty list)
    Cons,
    /// List (nil or cons)
    List,
    /// Tuple of known arity
    Tuple { arity: u32 },
    /// Tuple with stable (fixed, immutable) layout — enables compact
    /// native representation and fast field access without tagging.
    StableTuple {
        /// Element types in order
        element_types: Vec<IRType>,
        /// Structurally immutable after construction
        immutable: bool,
    },
    /// Map
    Map,
    /// Binary (heap or refc)
    Binary,
    /// Function/closure
    Fun { arity: u32 },

    // ── BEAM identity types ─────────────────────────────────────────────
    /// PID
    Pid,
    /// Port
    Port,
    /// Reference
    Reference,

    // ── Message types (for mailbox specialization) ─────────────────────
    /// A typed message pattern — describes the shape of messages an
    /// actor expects to receive.  Used by the mailbox optimization pass
    /// to generate fast-path matching and priority routing.
    Message {
        /// The payload type (e.g. a specific tuple shape)
        payload: Box<IRType>,
        /// Optional priority class for QoS-aware scheduling
        priority: MessagePriority,
    },

    // ── Actor types ─────────────────────────────────────────────────────
    /// Typed actor reference — carries the expected message protocol
    /// for this actor, enabling compile-time verification of sends.
    Actor {
        /// The message types this actor accepts
        accepts: Vec<IRType>,
        /// Actor lifecycle class (transient, permanent, temporary)
        lifecycle: ActorLifecycle,
    },

    // ── Tensor types (for AI workloads) ─────────────────────────────────
    /// Typed tensor — shape and element dtype for AI inference.
    /// Enables zero-copy interop with native ML frameworks.
    Tensor {
        /// Element data type
        dtype: TensorDtype,
        /// Static shape dimensions (None = dynamic)
        shape: Vec<Option<u64>>,
    },

    // ── Capability types (for native resources) ─────────────────────────
    /// A capability-typed native resource handle.  The capability
    /// describes what operations are permitted and tracks ownership.
    Capability {
        /// The kind of native resource
        resource: NativeResourceKind,
        /// Whether this handle is owned (responsible for cleanup)
        owned: bool,
        /// Whether this handle can be shared across actors
        shareable: bool,
    },

    // ── Union / intersection ────────────────────────────────────────────
    /// Union of two types
    Union(Box<IRType>, Box<IRType>),
    /// A specific constant value
    Constant(ConstantValue),
}

// ═══════════════════════════════════════════════════════════════════════════
// Supporting enums
// ═══════════════════════════════════════════════════════════════════════════

/// Message priority classes for QoS-aware mailbox routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MessagePriority {
    /// Low-priority background messages (e.g. telemetry)
    Low = 0,
    /// Normal messages (default)
    Normal = 1,
    /// High-priority messages (e.g. UI events, control signals)
    High = 2,
    /// Critical messages (e.g. supervision, fault recovery)
    Critical = 3,
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Actor lifecycle classes — determines supervision strategy and
/// restart behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorLifecycle {
    /// Process exits when its parent exits
    Transient,
    /// Process is always restarted on failure
    Permanent,
    /// Process is never restarted
    Temporary,
    /// Process is part of a supervision tree
    Supervisor,
}

/// Tensor element data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TensorDtype {
    /// 32-bit float (most common for inference)
    F32,
    /// 16-bit float (quantized models)
    F16,
    /// 64-bit float
    F64,
    /// 32-bit signed integer
    I32,
    /// 64-bit signed integer
    I64,
    /// 8-bit unsigned integer (quantized)
    U8,
    /// 1-bit (binary nets)
    Bool,
}

/// Native resource kinds for capability typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeResourceKind {
    /// GPU compute context (Metal, Vulkan, CUDA)
    GpuContext,
    /// Pre-compiled ML model (weights + graph)
    MlModel,
    /// GPU/NN buffer (tensor storage)
    TensorBuffer,
    /// File descriptor / I/O resource
    IoHandle,
    /// Network socket
    Socket,
    /// Shared memory region
    SharedMemory,
    /// Platform-specific UI surface
    UiSurface,
    /// Camera / microphone
    MediaDevice,
    /// Arbitrary opaque handle (fallback)
    Opaque,
}

// ═══════════════════════════════════════════════════════════════════════════
// Constant values
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// IRType
// ═══════════════════════════════════════════════════════════════════════════

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

    // ── Lattice operations ─────────────────────────────────────────────

    /// Compute the least upper bound (join) of two types.
    pub fn join(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }

        match (&self.kind, &other.kind) {
            (TypeKind::Bottom, t) | (t, TypeKind::Bottom) => IRType::new(t.clone()),
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

            // StableTuple joins: if shapes match, join element types;
            // otherwise fall back to regular Tuple or Any.
            (
                TypeKind::StableTuple {
                    element_types: a,
                    immutable: ia,
                },
                TypeKind::StableTuple {
                    element_types: b,
                    immutable: ib,
                },
            ) => {
                if a.len() == b.len() {
                    let joined: Vec<IRType> = a.iter().zip(b.iter()).map(|(x, y)| x.join(y)).collect();
                    IRType::new(TypeKind::StableTuple {
                        element_types: joined,
                        immutable: *ia && *ib,
                    })
                } else {
                    IRType::new(TypeKind::Tuple {
                        arity: a.len().max(b.len()) as u32,
                    })
                }
            }

            // Message joins: take the higher priority, join payloads
            (
                TypeKind::Message {
                    payload: p1,
                    priority: pr1,
                },
                TypeKind::Message {
                    payload: p2,
                    priority: pr2,
                },
            ) => IRType::new(TypeKind::Message {
                payload: Box::new(p1.join(p2)),
                priority: *pr1.max(pr2),
            }),

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
    pub fn meet(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }

        match (&self.kind, &other.kind) {
            (TypeKind::Bottom, _) | (_, TypeKind::Bottom) => IRType::new(TypeKind::Bottom),
            (TypeKind::Any, t) | (t, TypeKind::Any) => IRType::new(t.clone()),

            (TypeKind::Constant(c), general) | (general, TypeKind::Constant(c)) => {
                if IRType::new(general.clone()).contains_constant(c) {
                    IRType::new(TypeKind::Constant(c.clone()))
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            (TypeKind::Nil, TypeKind::List) | (TypeKind::List, TypeKind::Nil) => {
                IRType::new(TypeKind::Nil)
            }
            (TypeKind::Cons, TypeKind::List) | (TypeKind::List, TypeKind::Cons) => {
                IRType::new(TypeKind::Cons)
            }
            (TypeKind::NonNegInt, TypeKind::SmallInt)
            | (TypeKind::SmallInt, TypeKind::NonNegInt) => IRType::new(TypeKind::NonNegInt),

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

    // ── Type predicates ────────────────────────────────────────────────

    pub fn is_definitely_small_int(&self) -> bool {
        matches!(self.kind, TypeKind::SmallInt | TypeKind::NonNegInt)
    }

    pub fn is_definitely_atom(&self) -> bool {
        matches!(
            self.kind,
            TypeKind::Atom | TypeKind::Boolean | TypeKind::Constant(ConstantValue::Atom(_))
        )
    }

    pub fn is_definitely_tuple(&self) -> bool {
        matches!(self.kind, TypeKind::Tuple { .. } | TypeKind::StableTuple { .. })
    }

    pub fn is_definitely_list(&self) -> bool {
        matches!(self.kind, TypeKind::List | TypeKind::Cons | TypeKind::Nil)
    }

    pub fn is_definitely_map(&self) -> bool {
        matches!(self.kind, TypeKind::Map)
    }

    pub fn is_definitely_float(&self) -> bool {
        matches!(self.kind, TypeKind::Float)
    }

    pub fn is_definitely_fun(&self) -> bool {
        matches!(self.kind, TypeKind::Fun { .. })
    }

    pub fn is_definitely_pid(&self) -> bool {
        matches!(self.kind, TypeKind::Pid)
    }

    /// Check if this type is immutable (compiler-proven).
    pub fn is_immutable(&self) -> bool {
        match &self.kind {
            TypeKind::StableTuple { immutable, .. } => *immutable,
            TypeKind::Nil | TypeKind::Atom | TypeKind::Boolean | TypeKind::SmallInt
            | TypeKind::NonNegInt | TypeKind::Int64 | TypeKind::Float => true,
            TypeKind::Tuple { .. } | TypeKind::Cons | TypeKind::List => false,
            _ => false,
        }
    }

    /// Check if this type represents a message pattern.
    pub fn is_message(&self) -> bool {
        matches!(self.kind, TypeKind::Message { .. })
    }

    /// Check if this type represents an actor reference.
    pub fn is_actor(&self) -> bool {
        matches!(self.kind, TypeKind::Actor { .. })
    }

    /// Check if this type represents a tensor.
    pub fn is_tensor(&self) -> bool {
        matches!(self.kind, TypeKind::Tensor { .. })
    }

    /// Check if this type represents a capability.
    pub fn is_capability(&self) -> bool {
        matches!(self.kind, TypeKind::Capability { .. })
    }

    /// Get the message priority if this is a message type.
    pub fn message_priority(&self) -> Option<MessagePriority> {
        match &self.kind {
            TypeKind::Message { priority, .. } => Some(*priority),
            _ => None,
        }
    }

    /// Get the tensor shape if this is a tensor type.
    pub fn tensor_shape(&self) -> Option<&[Option<u64>]> {
        match &self.kind {
            TypeKind::Tensor { shape, .. } => Some(shape),
            _ => None,
        }
    }

    /// Get the native resource kind if this is a capability type.
    pub fn capability_resource(&self) -> Option<NativeResourceKind> {
        match &self.kind {
            TypeKind::Capability { resource, .. } => Some(*resource),
            _ => None,
        }
    }

    /// Get the stable tuple element types if this is a stable tuple.
    pub fn stable_tuple_elements(&self) -> Option<&[IRType]> {
        match &self.kind {
            TypeKind::StableTuple { element_types, .. } => Some(element_types),
            _ => None,
        }
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
            TypeKind::StableTuple {
                element_types,
                immutable,
            } => {
                let tag = if *immutable { "stable_tuple" } else { "fixed_tuple" };
                let elems: Vec<String> = element_types.iter().map(|t| t.to_string()).collect();
                write!(f, "{}({})", tag, elems.join(", "))
            }
            TypeKind::Map => write!(f, "map"),
            TypeKind::Binary => write!(f, "binary"),
            TypeKind::Fun { arity } => write!(f, "fun({})", arity),
            TypeKind::Pid => write!(f, "pid"),
            TypeKind::Port => write!(f, "port"),
            TypeKind::Reference => write!(f, "reference"),
            TypeKind::Message { payload, priority } => {
                write!(f, "msg<{}, {:?}>", payload, priority)
            }
            TypeKind::Actor { accepts, lifecycle } => {
                write!(f, "actor<{:?}, {} msg>", lifecycle, accepts.len())
            }
            TypeKind::Tensor { dtype, shape } => {
                let dims: Vec<String> = shape
                    .iter()
                    .map(|d| d.map(|v| v.to_string()).unwrap_or_else(|| "?".to_string()))
                    .collect();
                write!(f, "tensor<{:?}, [{}]>", dtype, dims.join(", "))
            }
            TypeKind::Capability {
                resource,
                owned,
                shareable,
            } => {
                let flags = match (*owned, *shareable) {
                    (true, true) => "owned+shared",
                    (true, false) => "owned",
                    (false, true) => "shared",
                    (false, false) => "borrowed",
                };
                write!(f, "cap<{:?}, {}>", resource, flags)
            }
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

// ═══════════════════════════════════════════════════════════════════════════
// Type descriptor — runtime type metadata for GC and AOT
// ═══════════════════════════════════════════════════════════════════════════

/// Runtime type descriptor emitted by the compiler for every heap-allocated
/// type.  Used by the GC for precise tracing and by the AOT backend for
/// layout computation and specialization.
#[derive(Debug, Clone)]
pub struct TypeDescriptor {
    /// Total allocation size in bytes (header included).
    pub alloc_size: u32,
    /// Bitmap: bit N is set if word N of the payload is a GC-traced pointer.
    pub pointer_map: u64,
    /// Compiler-proven structural immutability.
    pub immutable: bool,
    /// Optional compact native layout for SIR promotion.
    pub native_layout: Option<NativeLayout>,
    /// Whether this type can be promoted to the Stable Immutable Region.
    pub promotable_to_stable: bool,
}

/// Compact memory layout used after SIR promotion.
#[derive(Debug, Clone)]
pub struct NativeLayout {
    /// Field descriptors in declaration order.
    pub fields: Vec<NativeField>,
    /// Total size of the compact representation in bytes.
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct NativeField {
    pub offset: u32,
    pub kind: NativeFieldKind,
}

#[derive(Debug, Clone)]
pub enum NativeFieldKind {
    I64,
    F64,
    Ptr,
    Bytes { len: u32 },
}

impl TypeDescriptor {
    /// Returns an iterator over the byte offsets of pointer fields.
    pub fn pointer_offsets(&self) -> impl Iterator<Item = usize> + '_ {
        (0u32..64)
            .filter(move |&bit| self.pointer_map & (1 << bit) != 0)
            .map(|bit| bit as usize * std::mem::size_of::<usize>())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_tuple_join_matching() {
        let a = IRType::new(TypeKind::StableTuple {
            element_types: vec![IRType::new(TypeKind::SmallInt), IRType::new(TypeKind::Atom)],
            immutable: true,
        });
        let b = IRType::new(TypeKind::StableTuple {
            element_types: vec![IRType::new(TypeKind::NonNegInt), IRType::new(TypeKind::Atom)],
            immutable: true,
        });
        let joined = a.join(&b);
        assert!(matches!(
            joined.kind,
            TypeKind::StableTuple { immutable: true, .. }
        ));
    }

    #[test]
    fn test_stable_tuple_join_mismatch_arity() {
        let a = IRType::new(TypeKind::StableTuple {
            element_types: vec![IRType::new(TypeKind::SmallInt)],
            immutable: true,
        });
        let b = IRType::new(TypeKind::StableTuple {
            element_types: vec![IRType::new(TypeKind::SmallInt), IRType::new(TypeKind::Atom)],
            immutable: true,
        });
        let joined = a.join(&b);
        // Falls back to regular Tuple
        assert!(matches!(joined.kind, TypeKind::Tuple { arity: 2 }));
    }

    #[test]
    fn test_message_type_priority() {
        let msg = IRType::new(TypeKind::Message {
            payload: Box::new(IRType::new(TypeKind::Tuple { arity: 2 })),
            priority: MessagePriority::High,
        });
        assert_eq!(msg.message_priority(), Some(MessagePriority::High));
        assert!(msg.is_message());
    }

    #[test]
    fn test_tensor_type() {
        let t = IRType::new(TypeKind::Tensor {
            dtype: TensorDtype::F32,
            shape: vec![Some(1), Some(224), Some(224), Some(3)],
        });
        assert!(t.is_tensor());
        assert_eq!(t.tensor_shape().unwrap().len(), 4);
    }

    #[test]
    fn test_capability_type() {
        let cap = IRType::new(TypeKind::Capability {
            resource: NativeResourceKind::GpuContext,
            owned: true,
            shareable: false,
        });
        assert!(cap.is_capability());
        assert_eq!(cap.capability_resource(), Some(NativeResourceKind::GpuContext));
    }

    #[test]
    fn test_immutable_predicate() {
        assert!(IRType::new(TypeKind::Nil).is_immutable());
        assert!(IRType::new(TypeKind::SmallInt).is_immutable());
        assert!(IRType::new(TypeKind::Atom).is_immutable());
        assert!(!IRType::new(TypeKind::Cons).is_immutable());
        assert!(!IRType::new(TypeKind::Tuple { arity: 2 }).is_immutable());

        let stable = IRType::new(TypeKind::StableTuple {
            element_types: vec![IRType::new(TypeKind::SmallInt)],
            immutable: true,
        });
        assert!(stable.is_immutable());
    }

    #[test]
    fn test_message_join_takes_higher_priority() {
        let low = IRType::new(TypeKind::Message {
            payload: Box::new(IRType::new(TypeKind::SmallInt)),
            priority: MessagePriority::Low,
        });
        let high = IRType::new(TypeKind::Message {
            payload: Box::new(IRType::new(TypeKind::SmallInt)),
            priority: MessagePriority::High,
        });
        let joined = low.join(&high);
        assert_eq!(joined.message_priority(), Some(MessagePriority::High));
    }

    #[test]
    fn test_type_descriptor_pointer_offsets() {
        let desc = TypeDescriptor {
            alloc_size: 32,
            pointer_map: 0b0000_1010,
            immutable: true,
            native_layout: None,
            promotable_to_stable: true,
        };
        let offsets: Vec<usize> = desc.pointer_offsets().collect();
        assert_eq!(offsets, vec![8, 24]);
    }
}
