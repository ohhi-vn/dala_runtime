//! Intrinsics — special runtime functions callable from compiled code.
//!
//! Intrinsics are functions that the compiler knows about and can
//! generate inline code for, rather than making a regular function call.
//!
//! NOTE: This is a simplified stub. Full implementation would use
//! Cranelift's FunctionBuilder to emit inline code.

/// An intrinsic function that the code generator can emit inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    /// Get the current process pointer.
    GetProcess,
    /// Get the current reductions count.
    GetReductions,
    /// Set the reductions count.
    SetReductions,
    /// Check if we should yield.
    ShouldYield,
    /// Get heap pointer.
    GetHeapPtr,
    /// Set heap pointer.
    SetHeapPtr,
    /// Get stack pointer.
    GetStackPtr,
    /// Set stack pointer.
    SetStackPtr,
    /// GC write barrier.
    GcBarrier,
    /// Type test: is_small_int.
    IsSmallInt,
    /// Type test: is_atom.
    IsAtom,
    /// Type test: is_tuple.
    IsTuple,
    /// Type test: is_list.
    IsList,
    /// Type test: is_float.
    IsFloat,
    /// Type test: is_map.
    IsMap,
    /// Type test: is_binary.
    IsBinary,
    /// Type test: is_fun.
    IsFun,
    /// Type test: is_pid.
    IsPid,
    /// Type test: is_port.
    IsPort,
    /// Tuple element access.
    TupleElement,
    /// Map get.
    MapGet,
    /// Map put.
    MapPut,
    /// Binary construction.
    BinaryNew,
    /// Binary matching.
    BinaryMatch,
    /// List construction (cons).
    ListCons,
    /// List head.
    ListHead,
    /// List tail.
    ListTail,
    /// Raise an exception.
    Raise,
    /// Error out.
    Error,
    /// Throw.
    Throw,
    /// Apply a function.
    Apply,
    /// Send a message.
    Send,
    /// Receive a message.
    Receive,
    /// Unreachable code marker.
    Unreachable,
}

impl Intrinsic {
    /// Check if this intrinsic can be inlined.
    pub fn is_inlineable(&self) -> bool {
        matches!(
            self,
            Intrinsic::IsSmallInt
                | Intrinsic::IsAtom
                | Intrinsic::IsTuple
                | Intrinsic::IsList
                | Intrinsic::IsFloat
                | Intrinsic::IsMap
                | Intrinsic::IsBinary
                | Intrinsic::IsFun
                | Intrinsic::IsPid
                | Intrinsic::IsPort
                | Intrinsic::GetReductions
                | Intrinsic::ShouldYield
                | Intrinsic::GetHeapPtr
                | Intrinsic::GetStackPtr
                | Intrinsic::ListHead
                | Intrinsic::ListTail
                | Intrinsic::TupleElement
        )
    }

    /// Check if this intrinsic may trigger GC.
    pub fn may_gc(&self) -> bool {
        matches!(
            self,
            Intrinsic::BinaryNew
                | Intrinsic::MapPut
                | Intrinsic::ListCons
                | Intrinsic::Apply
                | Intrinsic::Raise
                | Intrinsic::Error
                | Intrinsic::Throw
        )
    }

    /// Check if this intrinsic may yield.
    pub fn may_yield(&self) -> bool {
        matches!(
            self,
            Intrinsic::Send | Intrinsic::Receive | Intrinsic::Apply | Intrinsic::ShouldYield
        )
    }
}

/// Emit an intrinsic as inline code in the current block.
///
/// NOTE: Stub implementation. Full version would use Cranelift FunctionBuilder.
pub fn emit_intrinsic(_builder: &mut (), _intrinsic: Intrinsic, _args: &[()]) -> Option<()> {
    None
}
