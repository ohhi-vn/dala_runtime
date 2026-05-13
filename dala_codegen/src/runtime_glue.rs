//! Runtime glue - provides function signatures and pointers for runtime calls.
//!
//! NOTE: This is a simplified stub. Full implementation would declare
//! Cranelift function references for runtime calls.

/// Runtime function IDs that the code generator can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeFuncId {
    Alloc,
    ShouldYield,
    ConsumeReductions,
    BifDispatch,
    Throw,
    Send,
    Receive,
    LoadLiteral,
    MakeFun,
    BinaryNew,
    BinarySize,
    BinaryExtract,
    ListCons,
    ListHead,
    ListTail,
    MapGet,
    MapPut,
    TupleElement,
    Raise,
    Apply,
}

/// Runtime glue provides function references for runtime calls.
pub struct RuntimeGlue;

impl RuntimeGlue {
    pub fn new() -> Self {
        Self
    }

    /// Declare all runtime functions in the given Cranelift module.
    pub fn declare_all(&mut self, _module: &mut ()) {
        // Stub: would declare Cranelift function signatures
    }
}

impl Default for RuntimeGlue {
    fn default() -> Self {
        Self::new()
    }
}
