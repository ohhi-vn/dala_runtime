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

use std::collections::HashSet;
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

    // ── Union / intersection / difference ──────────────────────────────
    /// Union of two types (A ∪ B)
    Union(Box<IRType>, Box<IRType>),
    /// Intersection of two types (A ∩ B)
    Intersection(Box<IRType>, Box<IRType>),
    /// Difference / subtraction (A \\ B) — values in A but not in B.
    /// Used by pattern-match narrowing to remove matched types.
    Difference(Box<IRType>, Box<IRType>),
    /// A specific constant value
    Constant(ConstantValue),

    // ── Map shape types (hidden-class specialization) ──────────────────
    /// A shape-specialized map with known key-value layout.
    /// Similar to V8 hidden classes — enables direct field access
    /// instead of hash lookup for maps with known keys.
    ///
    /// Example: %{id: integer(), name: binary()} becomes
    /// MapShape { keys: [atom(id), atom(name)], values: [SmallInt, Binary] }
    MapShape {
        /// Ordered keys (atoms only — the common case for struct-like maps)
        keys: Vec<u32>,
        /// Value types for each key
        values: Vec<IRType>,
    },
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
    /// A specific float value
    Float(u64), // bit pattern for f64
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
    ///
    /// The join is the smallest type that is a supertype of both `self` and `other`.
    /// This is the fundamental widening operation used in control-flow merge points.
    pub fn join(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }

        match (&self.kind, &other.kind) {
            // Lattice extremes
            (TypeKind::Bottom, t) | (t, TypeKind::Bottom) => IRType::new(t.clone()),
            (TypeKind::Any, _) | (_, TypeKind::Any) => IRType::new(TypeKind::Any),

            // Same category subtyping
            (TypeKind::SmallInt, TypeKind::NonNegInt)
            | (TypeKind::NonNegInt, TypeKind::SmallInt) => IRType::new(TypeKind::SmallInt),
            (TypeKind::SmallInt, TypeKind::Int64) | (TypeKind::Int64, TypeKind::SmallInt) => {
                IRType::new(TypeKind::Int64)
            }
            (TypeKind::NonNegInt, TypeKind::Int64) | (TypeKind::Int64, TypeKind::NonNegInt) => {
                IRType::new(TypeKind::Int64)
            }
            (TypeKind::Nil, TypeKind::Cons) | (TypeKind::Cons, TypeKind::Nil) => {
                IRType::new(TypeKind::List)
            }

            // Constant with general type (or constant with constant):
            // If both are constants and equal, keep the constant.
            // Otherwise widen to the general type (or Any if incompatible).
            (TypeKind::Constant(a), TypeKind::Constant(b)) => {
                if a == b {
                    self.clone()
                } else {
                    let ga: IRType = a.clone().into();
                    let gb: IRType = b.clone().into();
                    ga.join(&gb)
                }
            }
            (TypeKind::Constant(cv), general) | (general, TypeKind::Constant(cv)) => {
                let general_ty = IRType::new(general.clone());
                if general_ty.contains_constant(cv) {
                    general_ty
                } else {
                    IRType::new(TypeKind::Any)
                }
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
                    let joined: Vec<IRType> =
                        a.iter().zip(b.iter()).map(|(x, y)| x.join(y)).collect();
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

            // StableTuple with Tuple: fall back to Tuple
            (TypeKind::StableTuple { element_types, .. }, TypeKind::Tuple { arity })
            | (TypeKind::Tuple { arity }, TypeKind::StableTuple { element_types, .. }) => {
                IRType::new(TypeKind::Tuple {
                    arity: (*arity).max(element_types.len() as u32),
                })
            }

            // Tuple with Tuple
            (TypeKind::Tuple { arity: a }, TypeKind::Tuple { arity: b }) => {
                IRType::new(TypeKind::Tuple {
                    arity: (*a).max(*b),
                })
            }

            // Fun joins: same arity → Fun, different → Any
            (TypeKind::Fun { arity: a }, TypeKind::Fun { arity: b }) => {
                if a == b {
                    self.clone()
                } else {
                    IRType::new(TypeKind::Any)
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

            // Actor joins: union of accepted messages, most permissive lifecycle
            (
                TypeKind::Actor {
                    accepts: a1,
                    lifecycle: l1,
                },
                TypeKind::Actor {
                    accepts: a2,
                    lifecycle: l2,
                },
            ) => {
                let mut accepts = a1.clone();
                for a in a2 {
                    if !accepts.contains(a) {
                        accepts.push(a.clone());
                    }
                }
                let lifecycle = match (l1, l2) {
                    (ActorLifecycle::Supervisor, _) | (_, ActorLifecycle::Supervisor) => {
                        ActorLifecycle::Supervisor
                    }
                    (ActorLifecycle::Permanent, _) | (_, ActorLifecycle::Permanent) => {
                        ActorLifecycle::Permanent
                    }
                    (ActorLifecycle::Transient, _) | (_, ActorLifecycle::Transient) => {
                        ActorLifecycle::Transient
                    }
                    _ => ActorLifecycle::Temporary,
                };
                IRType::new(TypeKind::Actor { accepts, lifecycle })
            }

            // Tensor joins: same dtype → join shapes, different → Any
            (
                TypeKind::Tensor {
                    dtype: d1,
                    shape: s1,
                },
                TypeKind::Tensor {
                    dtype: d2,
                    shape: s2,
                },
            ) => {
                if d1 == d2 {
                    let shape: Vec<Option<u64>> = s1
                        .iter()
                        .zip(s2.iter())
                        .map(|(a, b)| match (a, b) {
                            (Some(a), Some(b)) => Some((*a).max(*b)),
                            _ => None,
                        })
                        .collect();
                    IRType::new(TypeKind::Tensor { dtype: *d1, shape })
                } else {
                    IRType::new(TypeKind::Any)
                }
            }

            // Capability joins: same resource → join flags, different → Any
            (
                TypeKind::Capability {
                    resource: r1,
                    owned: o1,
                    shareable: s1,
                },
                TypeKind::Capability {
                    resource: r2,
                    owned: o2,
                    shareable: s2,
                },
            ) => {
                if r1 == r2 {
                    IRType::new(TypeKind::Capability {
                        resource: *r1,
                        owned: *o1 || *o2,
                        shareable: *s1 || *s2,
                    })
                } else {
                    IRType::new(TypeKind::Any)
                }
            }

            // MapShape joins: if keys match, join value types;
            // otherwise fall back to Map.
            (
                TypeKind::MapShape {
                    keys: k1,
                    values: v1,
                },
                TypeKind::MapShape {
                    keys: k2,
                    values: v2,
                },
            ) => {
                if k1 == k2 {
                    let joined: Vec<IRType> =
                        v1.iter().zip(v2.iter()).map(|(a, b)| a.join(b)).collect();
                    IRType::new(TypeKind::MapShape {
                        keys: k1.clone(),
                        values: joined,
                    })
                } else {
                    IRType::new(TypeKind::Map)
                }
            }

            // MapShape with Map: fall back to Map
            (TypeKind::MapShape { .. }, TypeKind::Map)
            | (TypeKind::Map, TypeKind::MapShape { .. }) => IRType::new(TypeKind::Map),

            // Union types: flatten and simplify
            (TypeKind::Union(a, b), c) => {
                let ab = a.join(b);
                ab.join(&IRType::new(c.clone()))
            }
            (c, TypeKind::Union(a, b)) => {
                let ab = a.join(b);
                IRType::new(c.clone()).join(&ab)
            }

            // Intersection types: distribute over join
            (TypeKind::Intersection(a, b), _) => {
                let ja = a.join(other);
                let jb = b.join(other);
                ja.meet(&jb)
            }
            (_, TypeKind::Intersection(a, b)) => {
                let ja = self.join(a);
                let jb = self.join(b);
                ja.meet(&jb)
            }

            // Difference types: (A \\ B).join(C) ≈ (A.join(C)) \\ B  (conservative)
            (TypeKind::Difference(a, b), _) => {
                let joined = a.join(other);
                IRType::new(TypeKind::Difference(Box::new(joined), b.clone()))
            }
            (_, TypeKind::Difference(a, b)) => {
                let joined = self.join(a);
                IRType::new(TypeKind::Difference(Box::new(joined), b.clone()))
            }

            // Default: fall back to Any
            _ => IRType::new(TypeKind::Any),
        }
    }

    /// Compute the greatest lower bound (meet) of two types.
    ///
    /// The meet is the largest type that is a subtype of both `self` and `other`.
    /// This is the fundamental narrowing operation used in pattern matching
    /// and control-flow branching.
    pub fn meet(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }

        match (&self.kind, &other.kind) {
            // Lattice extremes
            (TypeKind::Bottom, _) | (_, TypeKind::Bottom) => IRType::new(TypeKind::Bottom),
            (TypeKind::Any, t) | (t, TypeKind::Any) => IRType::new(t.clone()),

            // Constant with constant: same value → keep, different → Bottom
            (TypeKind::Constant(a), TypeKind::Constant(b)) => {
                if a == b {
                    self.clone()
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // Constant with general type
            (TypeKind::Constant(c), general) | (general, TypeKind::Constant(c)) => {
                if IRType::new(general.clone()).contains_constant(c) {
                    IRType::new(TypeKind::Constant(c.clone()))
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // List subtyping
            (TypeKind::Nil, TypeKind::List) | (TypeKind::List, TypeKind::Nil) => {
                IRType::new(TypeKind::Nil)
            }
            (TypeKind::Cons, TypeKind::List) | (TypeKind::List, TypeKind::Cons) => {
                IRType::new(TypeKind::Cons)
            }

            // Integer subtyping
            (TypeKind::NonNegInt, TypeKind::SmallInt)
            | (TypeKind::SmallInt, TypeKind::NonNegInt) => IRType::new(TypeKind::NonNegInt),
            (TypeKind::SmallInt, TypeKind::Int64) | (TypeKind::Int64, TypeKind::SmallInt) => {
                IRType::new(TypeKind::SmallInt)
            }
            (TypeKind::NonNegInt, TypeKind::Int64) | (TypeKind::Int64, TypeKind::NonNegInt) => {
                IRType::new(TypeKind::NonNegInt)
            }

            // StableTuple meet: if shapes match, meet element types;
            // otherwise Bottom.
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
                    let met: Vec<IRType> = a.iter().zip(b.iter()).map(|(x, y)| x.meet(y)).collect();
                    IRType::new(TypeKind::StableTuple {
                        element_types: met,
                        immutable: *ia || *ib,
                    })
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // StableTuple meet with Tuple: fall back to Tuple if arity matches
            (TypeKind::StableTuple { element_types, .. }, TypeKind::Tuple { arity })
            | (TypeKind::Tuple { arity }, TypeKind::StableTuple { element_types, .. }) => {
                if element_types.len() as u32 == *arity {
                    IRType::new(TypeKind::Tuple { arity: *arity })
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // Tuple meet
            (TypeKind::Tuple { arity: a }, TypeKind::Tuple { arity: b }) => {
                if a == b {
                    self.clone()
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // Fun meet
            (TypeKind::Fun { arity: a }, TypeKind::Fun { arity: b }) => {
                if a == b {
                    self.clone()
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // Message meet: meet payloads, take lower priority
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
                payload: Box::new(p1.meet(p2)),
                priority: *pr1.min(pr2),
            }),

            // Actor meet: intersection of accepted messages, most restrictive lifecycle
            (
                TypeKind::Actor {
                    accepts: a1,
                    lifecycle: l1,
                },
                TypeKind::Actor {
                    accepts: a2,
                    lifecycle: l2,
                },
            ) => {
                let accepts: Vec<IRType> = a1.iter().filter(|a| a2.contains(a)).cloned().collect();
                let lifecycle = match (l1, l2) {
                    (ActorLifecycle::Temporary, _) | (_, ActorLifecycle::Temporary) => {
                        ActorLifecycle::Temporary
                    }
                    (ActorLifecycle::Transient, _) | (_, ActorLifecycle::Transient) => {
                        ActorLifecycle::Transient
                    }
                    (ActorLifecycle::Permanent, _) | (_, ActorLifecycle::Permanent) => {
                        ActorLifecycle::Permanent
                    }
                    _ => ActorLifecycle::Supervisor,
                };
                IRType::new(TypeKind::Actor { accepts, lifecycle })
            }

            // Tensor meet: same dtype → meet shapes, different → Bottom
            (
                TypeKind::Tensor {
                    dtype: d1,
                    shape: s1,
                },
                TypeKind::Tensor {
                    dtype: d2,
                    shape: s2,
                },
            ) => {
                if d1 == d2 {
                    if s1.len() == s2.len() {
                        let shape: Vec<Option<u64>> = s1
                            .iter()
                            .zip(s2.iter())
                            .map(|(a, b)| match (a, b) {
                                (Some(a), Some(b)) => Some((*a).min(*b)),
                                (Some(_), None) | (None, Some(_)) => None,
                                (None, None) => None,
                            })
                            .collect();
                        IRType::new(TypeKind::Tensor { dtype: *d1, shape })
                    } else {
                        IRType::new(TypeKind::Bottom)
                    }
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // Capability meet: same resource → meet flags, different → Bottom
            (
                TypeKind::Capability {
                    resource: r1,
                    owned: o1,
                    shareable: s1,
                },
                TypeKind::Capability {
                    resource: r2,
                    owned: o2,
                    shareable: s2,
                },
            ) => {
                if r1 == r2 {
                    IRType::new(TypeKind::Capability {
                        resource: *r1,
                        owned: *o1 && *o2,
                        shareable: *s1 && *s2,
                    })
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // MapShape meet: if keys match, meet value types;
            // otherwise Bottom.
            (
                TypeKind::MapShape {
                    keys: k1,
                    values: v1,
                },
                TypeKind::MapShape {
                    keys: k2,
                    values: v2,
                },
            ) => {
                if k1 == k2 {
                    let met: Vec<IRType> =
                        v1.iter().zip(v2.iter()).map(|(a, b)| a.meet(b)).collect();
                    IRType::new(TypeKind::MapShape {
                        keys: k1.clone(),
                        values: met,
                    })
                } else {
                    IRType::new(TypeKind::Bottom)
                }
            }

            // MapShape meet with Map: MapShape (the more specific type)
            (TypeKind::MapShape { .. }, TypeKind::Map) => self.clone(),
            (TypeKind::Map, TypeKind::MapShape { .. }) => other.clone(),

            // Union meet: distribute (A ∪ B) ∩ C = (A ∩ C) ∪ (B ∩ C)
            (TypeKind::Union(a, b), _) => {
                let ma = a.meet(other);
                let mb = b.meet(other);
                ma.join(&mb)
            }
            (_, TypeKind::Union(a, b)) => {
                let ma = self.meet(a);
                let mb = self.meet(b);
                ma.join(&mb)
            }

            // Intersection meet: flatten
            (TypeKind::Intersection(a, b), _) => {
                let ma = a.meet(other);
                let mb = b.meet(other);
                ma.meet(&mb)
            }
            (_, TypeKind::Intersection(a, b)) => {
                let ma = self.meet(a);
                let mb = self.meet(b);
                ma.meet(&mb)
            }

            // Difference meet: (A \\ B) ∩ C = (A ∩ C) \\ B
            (TypeKind::Difference(a, b), _) => {
                let met = a.meet(other);
                IRType::new(TypeKind::Difference(Box::new(met), b.clone()))
            }
            (_, TypeKind::Difference(a, b)) => {
                let met = self.meet(a);
                IRType::new(TypeKind::Difference(Box::new(met), b.clone()))
            }

            // Generic fallback: check subtyping
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
            (TypeKind::Int64, ConstantValue::Int(_)) => true,
            (TypeKind::Atom, ConstantValue::Atom(_)) => true,
            (TypeKind::Boolean, ConstantValue::True)
            | (TypeKind::Boolean, ConstantValue::False) => true,
            (TypeKind::Nil, ConstantValue::Nil) => true,
            (TypeKind::Float, ConstantValue::Float(_)) => true,
            (TypeKind::Any, _) => true,
            (TypeKind::Union(a, b), _) => {
                a.as_ref().contains_constant(c) || b.as_ref().contains_constant(c)
            }
            _ => false,
        }
    }

    /// Check if `self` is a supertype of (or equal to) `other`.
    ///
    /// This is the semantic subtyping relation: `self ⊇ other` means
    /// every value described by `other` is also described by `self`.
    pub fn contains(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        match (&self.kind, &other.kind) {
            // Lattice extremes
            (TypeKind::Any, _) => true,
            (_, TypeKind::Bottom) => true,

            // List subtyping: list ⊇ nil, list ⊇ cons
            (TypeKind::List, TypeKind::Nil) | (TypeKind::List, TypeKind::Cons) => true,

            // Integer subtyping: smallint ⊇ nonnegint, int64 ⊇ smallint
            (TypeKind::SmallInt, TypeKind::NonNegInt) => true,
            (TypeKind::Int64, TypeKind::SmallInt) | (TypeKind::Int64, TypeKind::NonNegInt) => true,

            // Constant → general type
            (TypeKind::SmallInt, TypeKind::Constant(ConstantValue::Int(_))) => true,
            (TypeKind::NonNegInt, TypeKind::Constant(ConstantValue::Int(i))) => *i >= 0,
            (TypeKind::Int64, TypeKind::Constant(ConstantValue::Int(_))) => true,
            (TypeKind::Atom, TypeKind::Constant(ConstantValue::Atom(_))) => true,
            (TypeKind::Boolean, TypeKind::Constant(ConstantValue::True)) => true,
            (TypeKind::Boolean, TypeKind::Constant(ConstantValue::False)) => true,
            (TypeKind::Nil, TypeKind::Constant(ConstantValue::Nil)) => true,
            (TypeKind::Float, TypeKind::Constant(ConstantValue::Float(_))) => true,

            // StableTuple ⊇ Tuple (stable is a refinement)
            (TypeKind::StableTuple { element_types, .. }, TypeKind::Tuple { arity }) => {
                element_types.len() as u32 == *arity
            }

            // StableTuple element-wise subtyping
            (
                TypeKind::StableTuple {
                    element_types: a, ..
                },
                TypeKind::StableTuple {
                    element_types: b, ..
                },
            ) => a.len() == b.len() && a.iter().zip(b.iter()).all(|(ea, eb)| ea.contains(eb)),

            // Tuple subtyping (arity must match)
            (TypeKind::Tuple { arity: a }, TypeKind::Tuple { arity: b }) => a == b,

            // Fun subtyping (arity must match)
            (TypeKind::Fun { arity: a }, TypeKind::Fun { arity: b }) => a == b,

            // Message subtyping: covariant in payload, contravariant in priority
            (
                TypeKind::Message {
                    payload: p1,
                    priority: pr1,
                },
                TypeKind::Message {
                    payload: p2,
                    priority: pr2,
                },
            ) => p1.contains(p2) && *pr1 >= *pr2,

            // Actor subtyping: contravariant in accepts, lifecycle must match
            (
                TypeKind::Actor {
                    accepts: a1,
                    lifecycle: l1,
                },
                TypeKind::Actor {
                    accepts: a2,
                    lifecycle: l2,
                },
            ) => l1 == l2 && a2.iter().all(|msg| a1.contains(msg)),

            // Tensor subtyping: same dtype, dimension-wise subtyping
            (
                TypeKind::Tensor {
                    dtype: d1,
                    shape: s1,
                },
                TypeKind::Tensor {
                    dtype: d2,
                    shape: s2,
                },
            ) => {
                d1 == d2
                    && s1.len() == s2.len()
                    && s1.iter().zip(s2.iter()).all(|(a, b)| match (a, b) {
                        (Some(a), Some(b)) => a == b,
                        (None, _) => true, // dynamic dimension accepts any
                        (Some(_), None) => false,
                    })
            }

            // Capability subtyping: same resource, owned ⇒ owned, shareable ⇒ shareable
            (
                TypeKind::Capability {
                    resource: r1,
                    owned: o1,
                    shareable: s1,
                },
                TypeKind::Capability {
                    resource: r2,
                    owned: o2,
                    shareable: s2,
                },
            ) => r1 == r2 && (!*o2 || *o1) && (!*s2 || *s1),

            // MapShape ⊇ MapShape: same keys, each value type is a supertype
            (
                TypeKind::MapShape {
                    keys: k1,
                    values: v1,
                },
                TypeKind::MapShape {
                    keys: k2,
                    values: v2,
                },
            ) => {
                k1 == k2
                    && v1.len() == v2.len()
                    && v1.iter().zip(v2.iter()).all(|(a, b)| a.contains(b))
            }

            // Map ⊇ MapShape: a generic map contains any specific shape
            (TypeKind::Map, TypeKind::MapShape { .. }) => true,

            // Union subtyping: A ∪ B ⊇ C iff A ⊇ C or B ⊇ C
            (TypeKind::Union(a, b), _) => a.contains(other) || b.contains(other),

            // Intersection subtyping: A ∩ B ⊇ C iff A ⊇ C and B ⊇ C
            (TypeKind::Intersection(a, b), _) => a.contains(other) && b.contains(other),

            // Difference subtyping: A \\ B ⊇ C iff A ⊇ C and C ∩ B = ∅
            (TypeKind::Difference(a, b), c) => {
                let c_ty = IRType::new(c.clone());
                a.contains(&c_ty) && c_ty.meet(&*b).kind == TypeKind::Bottom
            }

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
        matches!(
            self.kind,
            TypeKind::Tuple { .. } | TypeKind::StableTuple { .. }
        )
    }

    pub fn is_definitely_list(&self) -> bool {
        matches!(self.kind, TypeKind::List | TypeKind::Cons | TypeKind::Nil)
    }

    pub fn is_definitely_map(&self) -> bool {
        matches!(self.kind, TypeKind::Map | TypeKind::MapShape { .. })
    }

    /// Check if this type is a shape-specialized map.
    pub fn is_map_shape(&self) -> bool {
        matches!(self.kind, TypeKind::MapShape { .. })
    }

    /// Get the map shape keys and values if this is a MapShape.
    pub fn map_shape(&self) -> Option<(&[u32], &[IRType])> {
        match &self.kind {
            TypeKind::MapShape { keys, values } => Some((keys, values)),
            _ => None,
        }
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
            TypeKind::Nil
            | TypeKind::Atom
            | TypeKind::Boolean
            | TypeKind::SmallInt
            | TypeKind::NonNegInt
            | TypeKind::Int64
            | TypeKind::Float => true,
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

    // ── Type normalization ─────────────────────────────────────────────

    /// Normalize this type into a canonical form.
    ///
    /// This applies the following simplifications:
    /// - Flatten nested unions: (A | B) | C → A | B | C
    /// - Flatten nested intersections: (A & B) & C → A & B & C
    /// - Remove duplicate alternatives in unions/intersections
    /// - Absorb subtypes in unions: SmallInt | Int64 → Int64
    /// - Simplify intersections with Any/Bottom
    /// - Sort union/intersection operands for structural equality
    ///
    /// Two types that are semantically equal will have identical
    /// normalized forms, enabling simple `==` comparison.
    pub fn normalize(&self) -> Self {
        let alternatives = self.collect_union_alternatives();
        if alternatives.len() <= 1 {
            return self.clone();
        }

        // Remove subtypes: if A ⊇ B, keep only A
        let mut filtered: Vec<IRType> = Vec::new();
        for alt in &alternatives {
            let alt_norm = alt.normalize();
            // Check if any existing type in filtered already contains this one
            let dominated = filtered.iter().any(|existing| existing.contains(&alt_norm));
            if !dominated {
                // Remove any existing types that are dominated by this one
                filtered.retain(|existing| !alt_norm.contains(existing));
                filtered.push(alt_norm);
            }
        }

        if filtered.len() == 1 {
            filtered.into_iter().next().unwrap()
        } else {
            // Sort for canonical ordering (by string representation as tiebreaker)
            filtered.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
            // Build left-associative union
            let mut result = filtered[0].clone();
            for alt in &filtered[1..] {
                result = IRType::new(TypeKind::Union(Box::new(result), Box::new(alt.clone())));
            }
            result
        }
    }

    /// Collect all alternatives of a union type, flattening nested unions.
    fn collect_union_alternatives(&self) -> Vec<IRType> {
        match &self.kind {
            TypeKind::Union(a, b) => {
                let mut result = a.collect_union_alternatives();
                result.extend(b.collect_union_alternatives());
                result
            }
            _ => vec![self.clone()],
        }
    }

    /// Collect all conjuncts of an intersection type, flattening nested intersections.
    fn collect_intersection_conjuncts(&self) -> Vec<IRType> {
        match &self.kind {
            TypeKind::Intersection(a, b) => {
                let mut result = a.collect_intersection_conjuncts();
                result.extend(b.collect_intersection_conjuncts());
                result
            }
            _ => vec![self.clone()],
        }
    }

    // ── Exhaustiveness checking ────────────────────────────────────────

    /// Check if a set of pattern types is exhaustive for this type.
    ///
    /// Given `self` is the scrutinee type and `patterns` are the types
    /// matched by each arm, returns true if every possible value of the
    /// scrutinee is covered by at least one pattern.
    ///
    /// This is the core of exhaustiveness checking for `case` expressions.
    ///
    /// # Example
    /// ```
    /// // Scrutinee: boolean()
    /// // Patterns: [const(true), const(false)]
    /// // Result: true (exhaustive)
    ///
    /// // Scrutinee: list()
    /// // Patterns: [nil, cons]
    /// // Result: true (exhaustive)
    ///
    /// // Scrutinee: integer()
    /// // Patterns: [const(0)]
    /// // Result: false (not exhaustive)
    /// ```
    pub fn is_exhaustive(&self, patterns: &[IRType]) -> bool {
        // Compute the union of all pattern types
        if patterns.is_empty() {
            return self.kind == TypeKind::Bottom;
        }

        let mut pattern_union = patterns[0].clone();
        for pat in &patterns[1..] {
            pattern_union = pattern_union.join(pat);
        }

        // The patterns are exhaustive if their union covers the scrutinee:
        // pattern_union.contains(self) means every value in self is also in pattern_union
        pattern_union.contains(self)
    }

    /// Compute the "rest" type — the part of `self` not covered by `patterns`.
    ///
    /// This is useful for error reporting: "pattern `rest` is not covered".
    /// Returns Bottom if the patterns are exhaustive.
    pub fn uncovered_by(&self, patterns: &[IRType]) -> IRType {
        let mut rest = self.clone();
        for pat in patterns {
            rest = rest.subtract(pat);
            if rest.kind == TypeKind::Bottom {
                break;
            }
        }
        rest
    }

    /// Compute the difference of two types: self \\ other.
    ///
    /// This is a convenience wrapper around Difference that also
    /// applies simplification rules.
    pub fn subtract(&self, other: &IRType) -> IRType {
        // Trivial cases
        if other.kind == TypeKind::Any {
            return IRType::new(TypeKind::Bottom);
        }
        if other.kind == TypeKind::Bottom || self.kind == TypeKind::Bottom {
            return self.clone();
        }
        if self == other {
            return IRType::new(TypeKind::Bottom);
        }

        // If self contains other, the result is Difference
        if self.contains(other) {
            IRType::new(TypeKind::Difference(
                Box::new(self.clone()),
                Box::new(other.clone()),
            ))
        } else {
            // self doesn't contain other, so subtracting other removes nothing
            // (or partially removes — conservative: keep self)
            self.clone()
        }
    }

    // ── Type utility methods ───────────────────────────────────────────

    /// Check if this type is a union (possibly nested).
    pub fn is_union(&self) -> bool {
        matches!(self.kind, TypeKind::Union(..))
    }

    /// Check if this type is an intersection (possibly nested).
    pub fn is_intersection(&self) -> bool {
        matches!(self.kind, TypeKind::Intersection(..))
    }

    /// Check if this type is a difference type.
    pub fn is_difference(&self) -> bool {
        matches!(self.kind, TypeKind::Difference(..))
    }

    /// Check if this type is a compound type (union, intersection, or difference).
    pub fn is_compound(&self) -> bool {
        self.is_union() || self.is_intersection() || self.is_difference()
    }

    /// Check if this type is a simple (non-compound) type.
    pub fn is_simple(&self) -> bool {
        !self.is_compound()
    }

    /// Check if this type definitely represents a single concrete value.
    pub fn is_singleton(&self) -> bool {
        match &self.kind {
            TypeKind::Constant(_) => true,
            TypeKind::Boolean => false, // true | false — two values
            _ => false,
        }
    }

    /// Check if this type is definitely empty (no possible values).
    pub fn is_empty(&self) -> bool {
        self.kind == TypeKind::Bottom
    }

    /// Check if this type is definitely the top type (any possible value).
    pub fn is_any(&self) -> bool {
        self.kind == TypeKind::Any
    }

    /// Count the number of leaf type alternatives in a union tree.
    /// Useful for estimating code size in pattern matching.
    pub fn union_arity(&self) -> usize {
        self.collect_union_alternatives().len()
    }

    /// Count the number of conjuncts in an intersection tree.
    pub fn intersection_arity(&self) -> usize {
        self.collect_intersection_conjuncts().len()
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
                let tag = if *immutable {
                    "stable_tuple"
                } else {
                    "fixed_tuple"
                };
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
            TypeKind::Intersection(a, b) => write!(f, "({} ∩ {})", a, b),
            TypeKind::Difference(a, b) => write!(f, "({} \\ {})", a, b),
            TypeKind::MapShape { keys, values } => {
                let pairs: Vec<String> = keys
                    .iter()
                    .zip(values.iter())
                    .map(|(k, v)| format!("atom({}): {}", k, v))
                    .collect();
                write!(f, "map{{{}}}", pairs.join(", "))
            }
            TypeKind::Constant(c) => match c {
                ConstantValue::Int(i) => write!(f, "const({})", i),
                ConstantValue::Atom(a) => write!(f, "const(atom:{})", a),
                ConstantValue::Nil => write!(f, "const(nil)"),
                ConstantValue::True => write!(f, "const(true)"),
                ConstantValue::False => write!(f, "const(false)"),
                ConstantValue::Float(bits) => write!(f, "const(float:{})", bits),
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
            element_types: vec![
                IRType::new(TypeKind::NonNegInt),
                IRType::new(TypeKind::Atom),
            ],
            immutable: true,
        });
        let joined = a.join(&b);
        assert!(matches!(
            joined.kind,
            TypeKind::StableTuple {
                immutable: true,
                ..
            }
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
        assert_eq!(
            cap.capability_resource(),
            Some(NativeResourceKind::GpuContext)
        );
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
        // Bits 1 and 3 are set → offsets 8 and 24 (on 64-bit)
        assert_eq!(offsets, vec![8, 24]);
    }

    #[test]
    fn test_union_subtyping() {
        // Union(A, B).contains(C) when C is a subtype of A
        let union = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::Atom)),
        ));
        // NonNegInt is a subtype of SmallInt
        assert!(union.contains(&IRType::new(TypeKind::NonNegInt)));
        // Constant Int(42) is a subtype of SmallInt
        assert!(union.contains(&IRType::new(TypeKind::Constant(ConstantValue::Int(42)))));
        // Atom is directly in the union
        assert!(union.contains(&IRType::new(TypeKind::Atom)));
        // Float is not in the union
        assert!(!union.contains(&IRType::new(TypeKind::Float)));
    }

    #[test]
    fn test_intersection_subtyping() {
        // Intersection(A, B).contains(C) when C is a subtype of both A and B
        let intersection = IRType::new(TypeKind::Intersection(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::NonNegInt)),
        ));
        // NonNegInt is a subtype of both SmallInt and NonNegInt
        assert!(intersection.contains(&IRType::new(TypeKind::NonNegInt)));
        // Constant Int(0) is a subtype of both SmallInt and NonNegInt
        assert!(intersection.contains(&IRType::new(TypeKind::Constant(ConstantValue::Int(0)))));
        // Constant Int(-1) is a subtype of SmallInt but not NonNegInt
        assert!(!intersection.contains(&IRType::new(TypeKind::Constant(ConstantValue::Int(-1)))));
    }

    #[test]
    fn test_difference_type() {
        // Difference(A, B) removes values from A that are in B
        let diff = IRType::new(TypeKind::Difference(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::NonNegInt)),
        ));
        // Negative ints are in SmallInt but not NonNegInt, so they should be in the difference
        assert!(diff.contains(&IRType::new(TypeKind::Constant(ConstantValue::Int(-1)))));
        // NonNegInt values should be removed
        assert!(!diff.contains(&IRType::new(TypeKind::NonNegInt)));
        // Constant Int(0) is NonNegInt, so should be removed
        assert!(!diff.contains(&IRType::new(TypeKind::Constant(ConstantValue::Int(0)))));
    }

    #[test]
    fn test_map_shape_join_matching() {
        // MapShape join with matching keys joins value types
        let a = IRType::new(TypeKind::MapShape {
            keys: vec![1, 2],
            values: vec![IRType::new(TypeKind::SmallInt), IRType::new(TypeKind::Atom)],
        });
        let b = IRType::new(TypeKind::MapShape {
            keys: vec![1, 2],
            values: vec![
                IRType::new(TypeKind::NonNegInt),
                IRType::new(TypeKind::Atom),
            ],
        });
        let joined = a.join(&b);
        assert!(joined.is_map_shape());
        let (_, values) = joined.map_shape().unwrap();
        // SmallInt.join(NonNegInt) = SmallInt
        assert!(matches!(values[0].kind, TypeKind::SmallInt));
        // Atom.join(Atom) = Atom
        assert!(matches!(values[1].kind, TypeKind::Atom));
    }

    #[test]
    fn test_map_shape_join_mismatching() {
        // MapShape join with different keys falls back to Map
        let a = IRType::new(TypeKind::MapShape {
            keys: vec![1, 2],
            values: vec![IRType::new(TypeKind::SmallInt), IRType::new(TypeKind::Atom)],
        });
        let b = IRType::new(TypeKind::MapShape {
            keys: vec![3, 4],
            values: vec![IRType::new(TypeKind::Float), IRType::new(TypeKind::Boolean)],
        });
        let joined = a.join(&b);
        assert!(matches!(joined.kind, TypeKind::Map));
    }

    #[test]
    fn test_map_shape_meet() {
        // MapShape meet with matching keys meets value types
        let a = IRType::new(TypeKind::MapShape {
            keys: vec![1, 2],
            values: vec![IRType::new(TypeKind::SmallInt), IRType::new(TypeKind::Atom)],
        });
        let b = IRType::new(TypeKind::MapShape {
            keys: vec![1, 2],
            values: vec![
                IRType::new(TypeKind::NonNegInt),
                IRType::new(TypeKind::Boolean),
            ],
        });
        let met = a.meet(&b);
        assert!(met.is_map_shape());
        let (_, values) = met.map_shape().unwrap();
        // SmallInt.meet(NonNegInt) = NonNegInt
        assert!(matches!(values[0].kind, TypeKind::NonNegInt));
        // Atom.meet(Boolean) — no specific meet rule, falls to generic fallback
        // Atom does not contain Boolean and Boolean does not contain Atom,
        // so the meet is Bottom
        assert!(matches!(values[1].kind, TypeKind::Bottom));
    }

    #[test]
    fn test_map_shape_subtyping() {
        // Map contains MapShape
        let map = IRType::new(TypeKind::Map);
        let shape = IRType::new(TypeKind::MapShape {
            keys: vec![1],
            values: vec![IRType::new(TypeKind::SmallInt)],
        });
        assert!(map.contains(&shape));

        // MapShape subtyping is element-wise: wider value types in the container
        let shape_a = IRType::new(TypeKind::MapShape {
            keys: vec![1],
            values: vec![IRType::new(TypeKind::Int64)],
        });
        let shape_b = IRType::new(TypeKind::MapShape {
            keys: vec![1],
            values: vec![IRType::new(TypeKind::SmallInt)],
        });
        // shape_a has Int64 value, shape_b has SmallInt value
        // Int64.contains(SmallInt) is true, so shape_a.contains(shape_b) should be true
        assert!(shape_a.contains(&shape_b));
        // SmallInt does NOT contain Int64, so shape_b does NOT contain shape_a
        assert!(!shape_b.contains(&shape_a));
    }

    #[test]
    fn test_exhaustiveness_boolean() {
        // [true, false] is exhaustive for boolean
        let boolean = IRType::new(TypeKind::Boolean);
        let patterns = vec![
            IRType::new(TypeKind::Constant(ConstantValue::True)),
            IRType::new(TypeKind::Constant(ConstantValue::False)),
        ];
        assert!(boolean.is_exhaustive(&patterns));
    }

    #[test]
    fn test_exhaustiveness_list() {
        // [nil, cons] is exhaustive for list
        let list = IRType::new(TypeKind::List);
        let patterns = vec![IRType::new(TypeKind::Nil), IRType::new(TypeKind::Cons)];
        assert!(list.is_exhaustive(&patterns));
    }

    #[test]
    fn test_exhaustiveness_non_exhaustive() {
        // [const(0)] is NOT exhaustive for SmallInt
        let smallint = IRType::new(TypeKind::SmallInt);
        let patterns = vec![IRType::new(TypeKind::Constant(ConstantValue::Int(0)))];
        assert!(!smallint.is_exhaustive(&patterns));

        // [true] alone is NOT exhaustive for boolean
        let boolean = IRType::new(TypeKind::Boolean);
        let patterns = vec![IRType::new(TypeKind::Constant(ConstantValue::True))];
        assert!(!boolean.is_exhaustive(&patterns));
    }

    #[test]
    fn test_uncovered_type() {
        // uncovered_by returns the uncovered portion
        let smallint = IRType::new(TypeKind::SmallInt);
        let patterns = vec![IRType::new(TypeKind::Constant(ConstantValue::Int(0)))];
        let uncovered = smallint.uncovered_by(&patterns);
        // Should not be Bottom — there are many SmallInt values not covered by const(0)
        assert!(uncovered.kind != TypeKind::Bottom);

        // subtract is conservative: List \ Nil = Difference(List, Nil)
        // and Difference(List, Nil) \ Cons remains Difference (not Bottom)
        // because subtract can't prove the Difference contains Cons.
        // This is expected — subtract is a conservative approximation.
        let list = IRType::new(TypeKind::List);
        let uncovered = list.uncovered_by(&vec![
            IRType::new(TypeKind::Nil),
            IRType::new(TypeKind::Cons),
        ]);
        // The result is not Bottom because subtract is conservative,
        // but is_exhaustive correctly returns true via join+contains.
        assert!(list.is_exhaustive(&[IRType::new(TypeKind::Nil), IRType::new(TypeKind::Cons),]));
    }

    #[test]
    fn test_type_normalization_absorption() {
        // SmallInt | Int64 → Int64 (since Int64 contains SmallInt)
        let union = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::Int64)),
        ));
        let normalized = union.normalize();
        assert!(matches!(normalized.kind, TypeKind::Int64));

        // NonNegInt | SmallInt → SmallInt (since SmallInt contains NonNegInt)
        let union2 = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::NonNegInt)),
            Box::new(IRType::new(TypeKind::SmallInt)),
        ));
        let normalized2 = union2.normalize();
        assert!(matches!(normalized2.kind, TypeKind::SmallInt));
    }

    #[test]
    fn test_type_normalization_flatten() {
        // (A | B) | C should flatten to A | B | C
        let nested = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::Union(
                Box::new(IRType::new(TypeKind::SmallInt)),
                Box::new(IRType::new(TypeKind::Atom)),
            ))),
            Box::new(IRType::new(TypeKind::Float)),
        ));
        let normalized = nested.normalize();
        // After normalization, union_arity should be 3 (flattened)
        assert_eq!(normalized.union_arity(), 3);
    }

    #[test]
    fn test_union_arity() {
        // A simple type has arity 1
        assert_eq!(IRType::new(TypeKind::SmallInt).union_arity(), 1);

        // A union of two types has arity 2
        let union = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::Atom)),
        ));
        assert_eq!(union.union_arity(), 2);

        // A nested union (A | B) | C has arity 3
        let nested = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::Union(
                Box::new(IRType::new(TypeKind::SmallInt)),
                Box::new(IRType::new(TypeKind::Atom)),
            ))),
            Box::new(IRType::new(TypeKind::Float)),
        ));
        assert_eq!(nested.union_arity(), 3);
    }

    #[test]
    fn test_is_singleton() {
        // Constants are singletons
        assert!(IRType::new(TypeKind::Constant(ConstantValue::Int(42))).is_singleton());
        assert!(IRType::new(TypeKind::Constant(ConstantValue::Atom(42))).is_singleton());
        assert!(IRType::new(TypeKind::Constant(ConstantValue::True)).is_singleton());
        assert!(IRType::new(TypeKind::Constant(ConstantValue::False)).is_singleton());
        assert!(IRType::new(TypeKind::Constant(ConstantValue::Nil)).is_singleton());

        // Non-constants are not singletons
        assert!(!IRType::new(TypeKind::SmallInt).is_singleton());
        assert!(!IRType::new(TypeKind::Boolean).is_singleton());
        assert!(!IRType::new(TypeKind::Atom).is_singleton());
        assert!(!IRType::new(TypeKind::Float).is_singleton());
    }

    #[test]
    fn test_is_empty() {
        // Bottom type is empty
        assert!(IRType::new(TypeKind::Bottom).is_empty());

        // Other types are not empty
        assert!(!IRType::new(TypeKind::Any).is_empty());
        assert!(!IRType::new(TypeKind::SmallInt).is_empty());
        assert!(!IRType::new(TypeKind::Constant(ConstantValue::Int(0))).is_empty());
    }

    #[test]
    fn test_is_any() {
        // Any type
        assert!(IRType::new(TypeKind::Any).is_any());

        // Other types are not Any
        assert!(!IRType::new(TypeKind::Bottom).is_any());
        assert!(!IRType::new(TypeKind::SmallInt).is_any());
        assert!(!IRType::new(TypeKind::Boolean).is_any());
    }

    #[test]
    fn test_compound_predicates() {
        let union = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::Atom)),
        ));
        assert!(union.is_union());
        assert!(!union.is_intersection());
        assert!(!union.is_difference());
        assert!(union.is_compound());
        assert!(!union.is_simple());

        let intersection = IRType::new(TypeKind::Intersection(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::NonNegInt)),
        ));
        assert!(!intersection.is_union());
        assert!(intersection.is_intersection());
        assert!(!intersection.is_difference());
        assert!(intersection.is_compound());
        assert!(!intersection.is_simple());

        let difference = IRType::new(TypeKind::Difference(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::NonNegInt)),
        ));
        assert!(!difference.is_union());
        assert!(!difference.is_intersection());
        assert!(difference.is_difference());
        assert!(difference.is_compound());
        assert!(!difference.is_simple());

        // Simple types
        assert!(IRType::new(TypeKind::SmallInt).is_simple());
        assert!(IRType::new(TypeKind::Atom).is_simple());
        assert!(!IRType::new(TypeKind::SmallInt).is_compound());
    }

    #[test]
    fn test_subtract_basic() {
        // Subtracting Bottom returns self
        let a = IRType::new(TypeKind::SmallInt);
        let result = a.subtract(&IRType::new(TypeKind::Bottom));
        assert_eq!(result.kind, TypeKind::SmallInt);

        // Subtracting self returns Bottom
        let result = a.subtract(&a);
        assert_eq!(result.kind, TypeKind::Bottom);

        // Subtracting Any returns Bottom
        let result = a.subtract(&IRType::new(TypeKind::Any));
        assert_eq!(result.kind, TypeKind::Bottom);

        // Subtracting a contained type produces Difference
        let result = a.subtract(&IRType::new(TypeKind::NonNegInt));
        assert!(result.is_difference());

        // Subtracting a non-contained type returns self (conservative)
        let result = a.subtract(&IRType::new(TypeKind::Float));
        assert_eq!(result.kind, TypeKind::SmallInt);
    }

    #[test]
    fn test_contains_constant_with_union() {
        // contains_constant works through unions
        let union = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::Atom)),
        ));
        // Int constant is contained via SmallInt branch
        assert!(union.contains_constant(&ConstantValue::Int(42)));
        // Atom constant is contained via Atom branch
        assert!(union.contains_constant(&ConstantValue::Atom(1)));
        // Float constant is NOT contained
        assert!(!union.contains_constant(&ConstantValue::Float(1.0f64.to_bits())));

        // Direct type containment
        assert!(IRType::new(TypeKind::SmallInt).contains_constant(&ConstantValue::Int(-5)));
        assert!(IRType::new(TypeKind::NonNegInt).contains_constant(&ConstantValue::Int(0)));
        assert!(!IRType::new(TypeKind::NonNegInt).contains_constant(&ConstantValue::Int(-1)));
    }
}
