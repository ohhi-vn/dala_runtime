//! Runtime glue - provides function signatures and pointers for runtime calls.

use cranelift::ir::FuncRef;
use cranelift::prelude::*;

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
pub struct RuntimeGlue {
    funcs: Vec<Option<FuncRef>>,
}

impl RuntimeGlue {
    pub fn new() -> Self {
        Self { funcs: Vec::new() }
    }

    fn make_runtime_sig(num_args: usize, num_rets: usize) -> Signature {
        let mut sig = Signature::new(cranelift::isa::CallConv::SystemV);
        sig.params.push(AbiParam::new(types::I64));
        for _ in 0..num_args {
            sig.params.push(AbiParam::new(types::I64));
        }
        for _ in 0..num_rets {
            sig.returns.push(AbiParam::new(types::I64));
        }
        sig
    }

    /// Declare all runtime functions in the given Cranelift module.
    pub fn declare_all(&mut self, module: &mut cranelift_module::Module) {
        let names = [
            (RuntimeFuncId::Alloc, "dala_alloc", 2, 1),
            (RuntimeFuncId::ShouldYield, "dala_should_yield", 1, 1),
            (
                RuntimeFuncId::ConsumeReductions,
                "dala_consume_reductions",
                2,
                1,
            ),
            (RuntimeFuncId::BifDispatch, "dala_bif_dispatch", 3, 1),
            (RuntimeFuncId::Throw, "dala_throw", 2, 0),
            (RuntimeFuncId::Send, "dala_send", 3, 1),
            (RuntimeFuncId::Receive, "dala_receive", 2, 1),
            (RuntimeFuncId::LoadLiteral, "dala_load_literal", 2, 1),
            (RuntimeFuncId::MakeFun, "dala_make_fun", 4, 1),
            (RuntimeFuncId::BinaryNew, "dala_binary_new", 3, 1),
            (RuntimeFuncId::BinarySize, "dala_binary_size", 1, 1),
            (RuntimeFuncId::BinaryExtract, "dala_binary_extract", 4, 1),
            (RuntimeFuncId::ListCons, "dala_list_cons", 3, 1),
            (RuntimeFuncId::ListHead, "dala_list_head", 2, 1),
            (RuntimeFuncId::ListTail, "dala_list_tail", 2, 1),
            (RuntimeFuncId::MapGet, "dala_map_get", 3, 1),
            (RuntimeFuncId::MapPut, "dala_map_put", 4, 1),
            (RuntimeFuncId::TupleElement, "dala_tuple_element", 3, 1),
            (RuntimeFuncId::Raise, "dala_raise", 2, 0),
            (RuntimeFuncId::Apply, "dala_apply", 4, 1),
        ];

        let max_idx = names.len();
        self.funcs.resize(max_idx, None);

        for (id, name, num_args, num_rets) in names.iter() {
            let idx = *id as usize;
            let sig = Self::make_runtime_sig(*num_args, *num_rets);
            let func_ref = module
                .declare_function(name, cranelift_module::Linkage::Import, &sig)
                .expect("failed to declare runtime function");
            self.funcs[idx] = Some(func_ref);
        }
    }

    pub fn get_alloc_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::Alloc as usize].unwrap()
    }
    pub fn get_should_yield_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::ShouldYield as usize].unwrap()
    }
    pub fn get_reductions_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::ConsumeReductions as usize].unwrap()
    }
    pub fn get_bif_dispatch_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::BifDispatch as usize].unwrap()
    }
    pub fn get_throw_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::Throw as usize].unwrap()
    }
    pub fn get_send_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::Send as usize].unwrap()
    }
    pub fn get_recv_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::Receive as usize].unwrap()
    }
    pub fn get_load_literal_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::LoadLiteral as usize].unwrap()
    }
    pub fn get_make_fun_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::MakeFun as usize].unwrap()
    }
    pub fn get_binary_new_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::BinaryNew as usize].unwrap()
    }
    pub fn get_binary_size_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::BinarySize as usize].unwrap()
    }
    pub fn get_binary_extract_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::BinaryExtract as usize].unwrap()
    }
    pub fn get_list_cons_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::ListCons as usize].unwrap()
    }
    pub fn get_list_head_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::ListHead as usize].unwrap()
    }
    pub fn get_list_tail_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::ListTail as usize].unwrap()
    }
    pub fn get_map_get_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::MapGet as usize].unwrap()
    }
    pub fn get_map_put_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::MapPut as usize].unwrap()
    }
    pub fn get_tuple_element_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::TupleElement as usize].unwrap()
    }
    pub fn get_raise_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::Raise as usize].unwrap()
    }
    pub fn get_apply_fn(&self) -> FuncRef {
        self.funcs[RuntimeFuncId::Apply as usize].unwrap()
    }
}

impl Default for RuntimeGlue {
    fn default() -> Self {
        Self::new()
    }
}
