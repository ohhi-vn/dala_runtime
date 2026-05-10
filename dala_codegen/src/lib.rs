//! Dala Codegen - Native code generation using Cranelift.
//!
//! This crate translates Dala IR into native machine code using the
//! Cranelift code generator. It supports both JIT (for desktop/Android)
//! and AOT (for iOS/restricted environments) compilation modes.
//!
//! Architecture:
//!   Dala IR -> Cranelift IR -> Machine code
//!
//! The code generator:
//! 1. Translates IR instructions to Cranelift CLIF IR
//! 2. Handles BEAM calling conventions and process state
//! 3. Inserts GC safepoints with stack maps
//! 4. Inserts reduction counting for scheduler preemption
//! 5. Produces either in-memory executable code (JIT) or
//!    relocatable object files (AOT)

pub mod compiler;
pub mod intrinsics;
pub mod runtime_glue;
pub mod stack_map;
pub mod trap_sink;

// Re-exports
pub use compiler::Compiler;
pub use intrinsics::Intrinsic;
pub use runtime_glue::RuntimeGlue;
pub use stack_map::StackMapRegistry;
pub use trap_sink::TrapSink;

use cranelift::prelude::*;
use dala_ir::{IRFunction, IRInstKind, IRValueId, Reg};
use dala_runtime::{Process, Term};
use std::mem::size_of;

/// Compilation target (JIT or AOT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationMode {
    /// JIT compilation for immediate execution
    Jit,
    /// AOT compilation for ahead-of-time deployment
    Aot,
}

/// Code generator configuration.
#[derive(Debug, Clone)]
pub struct CodegenConfig {
    /// Compilation mode
    pub mode: CompilationMode,
    /// Target architecture
    pub target: String,
    /// Optimization level
    pub opt_level: cranelift::settings::OptLevel,
    /// Enable debug assertions
    pub debug_assertions: bool,
    /// Enable verbose logging
    pub verbose: bool,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            mode: CompilationMode::Jit,
            target: "x86_64".to_string(),
            opt_level: cranelift::settings::OptLevel::Speed,
            debug_assertions: false,
            verbose: false,
        }
    }
}

/// A compiled function ready for execution.
#[repr(C)]
pub struct CompiledFunction {
    /// The native code pointer
    pub code_ptr: *const u8,
    /// The size of the compiled code
    pub code_size: usize,
    /// Stack map for GC
    pub stack_map: Option<Vec<u8>>,
    /// Frame size
    pub frame_size: usize,
    /// Number of spills
    pub spill_count: usize,
}

impl CompiledFunction {
    /// Get the function as a callable pointer.
    pub fn as_fn(&self) -> Option<unsafe extern "C" fn()> {
        if self.code_ptr.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute(self.code_ptr) })
        }
    }
}

/// A code generator that translates Dala IR to native code.
pub struct CodeGenerator {
    /// The module being compiled
    module: cranelift_module::Module,
    /// Runtime glue for calling runtime functions
    runtime_glue: RuntimeGlue,
    /// Stack map registry
    stack_maps: StackMapRegistry,
    /// Configuration
    config: CodegenConfig,
    /// EBB (basic block) mappings from IR BlockId to Cranelift EBB
    ebb_map: std::collections::HashMap<usize, cranelift::ir::Ebb>,
    /// Value mappings from IR ValueId to Cranelift Value
    value_map: std::collections::HashMap<usize, cranelift::ir::Value>,
}

impl CodeGenerator {
    /// Create a new code generator.
    pub fn new(config: CodegenConfig) -> Result<Self, CodegenError> {
        let mut flag_builder = cranelift::settings::builder_for_host();
        flag_builder
            .set("opt_level", config.opt_level.to_string().as_str())
            .map_err(|e| CodegenError::TargetError(e.to_string()))?;
        if config.debug_assertions {
            flag_builder
                .enable("enable_verifier")
                .map_err(|e| CodegenError::TargetError(e.to_string()))?;
        }

        let isa = match config.target.as_str() {
            "x86_64" => {
                #[cfg(target_arch = "x86_64")]
                {
                    cranelift_native::builder()
                        .map_err(|e| CodegenError::TargetError(e.to_string()))?
                        .finish(cranelift::settings::Flags::new(flag_builder))
                }
                #[cfg(not(target_arch = "x86_64"))]
                {
                    cranelift::isa::lookup("x86_64")
                        .map_err(|e| CodegenError::TargetError(e.to_string()))?
                        .finish(cranelift::settings::Flags::new(flag_builder))
                        .map_err(|e| CodegenError::TargetError(e.to_string()))?
                }
            }
            "aarch64" => cranelift::isa::lookup("aarch64")
                .map_err(|e| CodegenError::TargetError(e.to_string()))?
                .finish(cranelift::settings::Flags::new(flag_builder))
                .map_err(|e| CodegenError::TargetError(e.to_string()))?,
            _ => {
                return Err(CodegenError::TargetError(format!(
                    "Unsupported target: {}",
                    config.target
                )));
            }
        };

        let mut builder_module = cranelift_module::Module::new(
            isa,
            cranelift_module::ModuleEnvironment::new(
                cranelift_module::isa_to_shared_flag(isa.as_ref()),
                cranelift_module::CompiledModule::new(),
            ),
        );

        let mut runtime_glue = RuntimeGlue::new();
        runtime_glue.declare_all(&mut builder_module);

        Ok(Self {
            module: builder_module,
            runtime_glue,
            stack_maps: StackMapRegistry::new(),
            config,
            ebb_map: std::collections::HashMap::new(),
            value_map: std::collections::HashMap::new(),
        })
    }

    /// Compile an IR function to native code.
    pub fn compile_function(
        &mut self,
        ir_func: &IRFunction,
    ) -> Result<CompiledFunction, CodegenError> {
        let mut func = cranelift::ir::Function::new();
        func.signature = self.make_signature(ir_func);
        let sig = func.signature.clone();

        let entry_ebb = func.dfg.make_ebb();
        func.layout.append_ebb(entry_ebb);

        let mut builder = FunctionBuilder::new(&mut func);
        builder.switch_to_block(entry_ebb, &[]);

        self.ebb_map.clear();
        self.value_map.clear();
        self.ebb_map.insert(0, entry_ebb);

        for (block_idx, block) in ir_func.blocks.iter().enumerate() {
            if !block.reachable {
                continue;
            }

            let ebb = self.get_or_create_ebb(block_idx, &mut builder);
            builder.switch_to_block(ebb, &[]);

            for inst in &block.instructions {
                self.compile_instruction(inst, ir_func, &mut builder)?;
            }
        }

        builder.seal_all_blocks();
        builder.finalize();
        drop(builder);

        let func_name = format!("m{}.f{}/{}", ir_func.module, ir_func.name, ir_func.arity);
        let func_ref = self
            .module
            .declare_function(&func_name, cranelift_module::Linkage::Export, &sig)
            .map_err(|e| CodegenError::CompilationError(e.to_string()))?;

        let compiled_func = self
            .module
            .define_function(func_ref, &mut cranelift_module::NullTrapSink {})
            .map_err(|e| CodegenError::CompilationError(e.to_string()))?;

        let code_size = compiled_func.total_size as usize;

        // Finalize the module to get executable code pointers
        let code_memory = self
            .module
            .finalize_definitions()
            .map_err(|e| CodegenError::CompilationError(e.to_string()))?;

        let code_ptr = self.module.get_finalized_function(func_ref) as *const u8;

        // Compute frame size from the compiled function metadata
        let frame_size = compiled_func.frame_size as usize;
        // Note: spill_slots field availability depends on Cranelift version
        #[allow(deprecated)]
        let spill_count = compiled_func.spill_slots as usize;

        // Store the code memory so it doesn't get dropped (keeps code executable)
        std::mem::forget(code_memory);

        Ok(CompiledFunction {
            code_ptr,
            code_size,
            stack_map: None,
            frame_size,
            spill_count,
        })
    }

    /// Create a Cranelift signature for an IR function.
    fn make_signature(&self, _ir_func: &IRFunction) -> cranelift::ir::Signature {
        let mut sig = cranelift::ir::Signature::new(cranelift::isa::CallConv::SystemV);
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        sig
    }

    /// Get or create a Cranelift EBB for an IR block.
    fn get_or_create_ebb(
        &mut self,
        block_idx: usize,
        builder: &mut FunctionBuilder,
    ) -> cranelift::ir::Ebb {
        if let Some(&ebb) = self.ebb_map.get(&block_idx) {
            return ebb;
        }
        let ebb = builder.func.dfg.make_ebb();
        builder.func.layout.append_ebb(ebb);
        self.ebb_map.insert(block_idx, ebb);
        ebb
    }

    /// Compile a single IR instruction.
    fn compile_instruction(
        &mut self,
        inst: &IRInst,
        ir_func: &IRFunction,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        use IRInstKind::*;

        match &inst.kind {
            Add => self.compile_binary_op(inst, cranelift::ir::Opcode::Iadd, builder),
            Sub => self.compile_binary_op(inst, cranelift::ir::Opcode::Isub, builder),
            Mul => self.compile_binary_op(inst, cranelift::ir::Opcode::Imul, builder),
            Div => self.compile_div(inst, false, builder),
            Rem => self.compile_div(inst, true, builder),
            Neg => self.compile_unary_op(inst, cranelift::ir::Opcode::Ineg, builder),
            BitAnd => self.compile_binary_op(inst, cranelift::ir::Opcode::Band, builder),
            BitOr => self.compile_binary_op(inst, cranelift::ir::Opcode::Bor, builder),
            BitXor => self.compile_binary_op(inst, cranelift::ir::Opcode::Bxor, builder),
            BitNot => self.compile_unary_op(inst, cranelift::ir::Opcode::Bnot, builder),
            ShiftLeft => self.compile_binary_op(inst, cranelift::ir::Opcode::Ishl, builder),
            ShiftRight => self.compile_binary_op(inst, cranelift::ir::Opcode::Sshr, builder),
            Eq => self.compile_cmp(inst, cranelift::ir::condcodes::IntCC::Equal, builder),
            Ne => self.compile_cmp(inst, cranelift::ir::condcodes::IntCC::NotEqual, builder),
            Gt => self.compile_cmp(
                inst,
                cranelift::ir::condcodes::IntCC::SignedGreaterThan,
                builder,
            ),
            Ge => self.compile_cmp(
                inst,
                cranelift::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                builder,
            ),
            Lt => self.compile_cmp(
                inst,
                cranelift::ir::condcodes::IntCC::SignedLessThan,
                builder,
            ),
            Le => self.compile_cmp(
                inst,
                cranelift::ir::condcodes::IntCC::SignedLessThanOrEqual,
                builder,
            ),
            IsSmallInt | IsAtom | IsNil | IsTrue | IsFalse | IsTuple | IsList | IsMap
            | IsBinary | IsFun | IsPid | IsFloat => {
                self.compile_type_test(inst, &inst.kind, builder)?;
            }
            Alloc { words } => self.compile_alloc(inst, *words, builder),
            Load { base, offset } => self.compile_load(inst, *base, *offset, builder),
            Store {
                base,
                offset,
                value,
            } => self.compile_store(inst, *base, *offset, *value, builder),
            TupleGet { tuple, index } => self.compile_tuple_get(inst, *tuple, *index, builder),
            TupleSet { .. } => Ok(()),
            Push { value } => self.compile_push(inst, *value, builder),
            Pop => self.compile_pop(inst, builder),
            GetStackPtr | SetStackPtr { .. } => Ok(()),
            GetReg { reg } => self.compile_get_reg(inst, *reg, builder),
            SetReg { reg, value } => self.compile_set_reg(inst, *reg, *value, builder),
            Move { src, dst } => self.compile_move(inst, *src, *dst, builder),
            Br { target } => self.compile_br(inst, *target, builder),
            BrIf {
                cond,
                true_target,
                false_target,
            } => self.compile_br_if(inst, *cond, *true_target, *false_target, builder),
            Switch {
                value,
                default,
                targets,
            } => self.compile_switch(inst, *value, *default, targets, builder),
            Ret { value } => self.compile_ret(inst, *value, builder),
            Call { func, args } => self.compile_call(inst, *func, args, builder),
            TailCall { func, args } => self.compile_tail_call(inst, *func, args, builder),
            CallBif {
                module,
                function,
                args,
            } => self.compile_call_bif(inst, *module, *function, args, builder),
            Catch { handler } => self.compile_catch(inst, *handler, builder),
            CatchPop => self.compile_catch_pop(inst, builder),
            Throw { reason } => self.compile_throw(inst, *reason, builder),
            Resume { exception } => self.compile_resume(inst, *exception, builder),
            ConsumeReductions { count } => {
                self.compile_consume_reductions(inst, *count, ir_func, builder)
            }
            Send { dest, msg } => self.compile_send(inst, *dest, *msg, builder),
            Recv { timeout } => self.compile_recv(inst, *timeout, builder),
            LoadLiteral { index } => self.compile_load_literal(inst, *index, builder),
            ConstSmallInt { value } => self.compile_const_small_int(inst, *value, builder),
            ConstAtom { index } => self.compile_const_atom(inst, *index, builder),
            ConstNil => self.compile_const_nil(inst, builder),
            ConstTrue => self.compile_const_true(inst, builder),
            ConstFalse => self.compile_const_false(inst, builder),
            BinaryNew { data } => self.compile_binary_new(inst, *data, builder),
            BinarySize { binary } => self.compile_binary_size(inst, *binary, builder),
            BinaryExtract {
                binary,
                offset,
                size,
                flags,
            } => self.compile_binary_extract(inst, *binary, *offset, *size, *flags, builder),
            MakeFun {
                module,
                function,
                arity,
                fvs,
            } => self.compile_make_fun(inst, *module, *function, *arity, fvs, builder),
            GcSafe => self.compile_gc_safe(inst, builder),
            _ => Err(CodegenError::Unsupported(format!("{:?}", inst.kind))),
        }
    }

    fn compile_binary_op(
        &mut self,
        inst: &IRInst,
        opcode: cranelift::ir::Opcode,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let a = self.get_value(inst.operands[0]);
        let b = self.get_value(inst.operands[1]);
        let result = builder.ins().binary(opcode, a, b);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_unary_op(
        &mut self,
        inst: &IRInst,
        opcode: cranelift::ir::Opcode,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let a = self.get_value(inst.operands[0]);
        let result = builder.ins().unary(opcode, a);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_div(
        &mut self,
        inst: &IRInst,
        is_rem: bool,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let a = self.get_value(inst.operands[0]);
        let b = self.get_value(inst.operands[1]);
        let result = if is_rem {
            builder.ins().srem(a, b)
        } else {
            builder.ins().sdiv(a, b)
        };
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_cmp(
        &mut self,
        inst: &IRInst,
        cc: cranelift::ir::condcodes::IntCC,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let a = self.get_value(inst.operands[0]);
        let b = self.get_value(inst.operands[1]);
        let result = builder.ins().icmp(cc, a, b);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_type_test(
        &mut self,
        inst: &IRInst,
        _test_kind: &IRInstKind,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let value = self.get_value(inst.operands[0]);
        let tag = builder.ins().band_imm(value, 3);
        let result = builder
            .ins()
            .icmp_imm(cranelift::ir::condcodes::IntCC::Equal, tag, 3);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_alloc(
        &mut self,
        inst: &IRInst,
        words: u32,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let proc = self.get_process_param(builder);
        let words_imm = builder.ins().iconst(types::I64, words as i64);
        let func_ref = self.runtime_glue.get_alloc_fn();
        let call = builder.ins().call(func_ref, &[proc, words_imm]);
        let result = builder.inst_results(call)[0];
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_load(
        &mut self,
        inst: &IRInst,
        base: IRValueId,
        offset: u32,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let base_val = self.get_value(base);
        let offset_imm = builder.ins().iconst(types::I64, (offset as i64) * 8);
        let addr = builder.ins().iadd(base_val, offset_imm);
        let result = builder
            .ins()
            .load(types::I64, cranelift::ir::MemFlags::trusted(), addr, 0);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_store(
        &mut self,
        inst: &IRInst,
        base: IRValueId,
        offset: u32,
        value: IRValueId,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let base_val = self.get_value(base);
        let value_val = self.get_value(value);
        let offset_imm = builder.ins().iconst(types::I64, (offset as i64) * 8);
        let addr = builder.ins().iadd(base_val, offset_imm);
        builder
            .ins()
            .store(cranelift::ir::MemFlags::trusted(), value_val, addr, 0);
        Ok(())
    }

    fn compile_tuple_get(
        &mut self,
        inst: &IRInst,
        tuple: IRValueId,
        index: u32,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let tuple_val = self.get_value(tuple);
        let offset = (1 + index) as i64 * 8;
        let offset_imm = builder.ins().iconst(types::I64, offset);
        let addr = builder.ins().iadd(tuple_val, offset_imm);
        let result = builder
            .ins()
            .load(types::I64, cranelift::ir::MemFlags::trusted(), addr, 0);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_push(
        &mut self,
        inst: &IRInst,
        value: IRValueId,
        _builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let value_val = self.get_value(value);
        self.set_value(inst, value_val);
        Ok(())
    }

    fn compile_pop(
        &mut self,
        _inst: &IRInst,
        _builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        Ok(())
    }

    fn compile_get_reg(
        &mut self,
        inst: &IRInst,
        reg: Reg,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let proc = self.get_process_param(builder);
        // RegisterFile layout (#[repr(C)]):
        //   x: [Term; 256]   -> starts at word offset 0
        //   y: [Term; 1024]  -> starts at word offset 256
        //   f: [f64; 256]    -> starts at word offset 256 + 1024 = 1280
        //
        // Each Term is 8 bytes (size_of::<Term>() == 8 on 64-bit).
        // The `registers` field is field index 7 in the Process struct:
        //   0: pid (u64)
        //   1: heap_start (*mut Term)
        //   2: heap_ptr (*mut Term)
        //   3: heap_top (*mut Term)
        //   4: stack_ptr (*mut Term)
        //   5: stack_top (*mut Term)
        //   6: heap_high_water (*mut Term)
        //   7: registers (RegisterFile)
        let regs_ptr = builder.ins().get_field(7, proc, 0);
        let reg_offset = match reg {
            Reg::X(idx) => (idx as usize) * size_of::<Term>(),
            Reg::Y(idx) => (256 + idx as usize) * size_of::<Term>(),
            Reg::F(idx) => (256 + 1024 + idx as usize) * size_of::<Term>(),
        };
        let offset_imm = builder.ins().iconst(types::I64, reg_offset as i64);
        let addr = builder.ins().iadd(regs_ptr, offset_imm);
        let result = builder
            .ins()
            .load(types::I64, cranelift::ir::MemFlags::trusted(), addr, 0);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_set_reg(
        &mut self,
        inst: &IRInst,
        reg: Reg,
        value: IRValueId,
        _builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let _value_val = self.get_value(value);
        let _ = reg;
        Ok(())
    }

    fn compile_move(
        &mut self,
        inst: &IRInst,
        src: Reg,
        dst: Reg,
        _builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let _ = src;
        let _ = dst;
        Ok(())
    }

    fn compile_br(
        &mut self,
        inst: &IRInst,
        target: usize,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let ebb = self.get_or_create_ebb(target, builder);
        builder.ins().jump(ebb, &[]);
        Ok(())
    }

    fn compile_br_if(
        &mut self,
        inst: &IRInst,
        cond: IRValueId,
        true_target: usize,
        false_target: usize,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let cond_val = self.get_value(cond);
        let true_ebb = self.get_or_create_ebb(true_target, builder);
        let false_ebb = self.get_or_create_ebb(false_target, builder);
        let is_true =
            builder
                .ins()
                .icmp_imm(cranelift::ir::condcodes::IntCC::NotEqual, cond_val, 0);
        builder.ins().brif(is_true, true_ebb, &[], false_ebb, &[]);
        Ok(())
    }

    fn compile_switch(
        &mut self,
        inst: &IRInst,
        value: IRValueId,
        default: usize,
        targets: &[(i64, usize)],
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let value_val = self.get_value(value);
        let default_ebb = self.get_or_create_ebb(default, builder);
        let jt = builder.create_jump_table();
        for &(_, target) in targets {
            builder.insert_jump_table_entry(jt, self.get_or_create_ebb(target, builder));
        }
        let _ = builder.ins().br_table(value_val, default_ebb);
        Ok(())
    }

    fn compile_ret(
        &mut self,
        inst: &IRInst,
        value: IRValueId,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let value_val = self.get_value(value);
        builder.ins().return_(&[value_val]);
        Ok(())
    }

    fn compile_call(
        &mut self,
        inst: &IRInst,
        func: IRValueId,
        args: &[IRValueId],
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let func_val = self.get_value(func);
        let arg_vals: Vec<cranelift::ir::Value> = args.iter().map(|&a| self.get_value(a)).collect();
        let sig = builder.import_signature(self.make_sig_for_call(args.len()));
        let call_inst = builder.ins().call_indirect(sig, func_val, &arg_vals);
        let results = builder.inst_results(call_inst).to_vec();
        if let Some(result) = inst.result {
            if let Some(&val) = results.first() {
                self.set_value_raw(inst, val);
            }
        }
        Ok(())
    }

    fn compile_tail_call(
        &mut self,
        inst: &IRInst,
        func: IRValueId,
        args: &[IRValueId],
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let func_val = self.get_value(func);
        let arg_vals: Vec<cranelift::ir::Value> = args.iter().map(|&a| self.get_value(a)).collect();
        let sig = builder.import_signature(self.make_sig_for_call(args.len()));
        builder.ins().return_call(func_val, &arg_vals);
        Ok(())
    }

    fn compile_call_bif(
        &mut self,
        inst: &IRInst,
        module: IRValueId,
        function: IRValueId,
        args: &[IRValueId],
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let module_val = self.get_value(module);
        let function_val = self.get_value(function);
        let arg_vals: Vec<cranelift::ir::Value> = args.iter().map(|&a| self.get_value(a)).collect();
        let func_ref = self.runtime_glue.get_bif_dispatch_fn();
        let mut call_args = vec![self.get_process_param(builder), module_val, function_val];
        call_args.extend(arg_vals);
        let call = builder.ins().call(func_ref, &call_args);
        if let Some(result) = inst.result {
            let results = builder.inst_results(call).to_vec();
            if let Some(&val) = results.first() {
                self.set_value_raw(inst, val);
            }
        }
        Ok(())
    }

    fn compile_catch(
        &mut self,
        _inst: &IRInst,
        _handler: usize,
        _builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        Ok(())
    }

    fn compile_catch_pop(
        &mut self,
        _inst: &IRInst,
        _builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        Ok(())
    }

    fn compile_throw(
        &mut self,
        inst: &IRInst,
        reason: IRValueId,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let reason_val = self.get_value(reason);
        let func_ref = self.runtime_glue.get_throw_fn();
        builder
            .ins()
            .call(func_ref, &[self.get_process_param(builder), reason_val]);
        builder.ins().trap(cranelift::ir::TrapCode::User0);
        Ok(())
    }

    fn compile_resume(
        &mut self,
        _inst: &IRInst,
        _exception: IRValueId,
        _builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        Ok(())
    }

    fn compile_consume_reductions(
        &mut self,
        inst: &IRInst,
        count: u32,
        ir_func: &IRFunction,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let proc = self.get_process_param(builder);
        let count_imm = builder.ins().iconst(types::I32, count as i64);
        let func_ref = self.runtime_glue.get_reductions_fn();
        let call = builder.ins().call(func_ref, &[proc, count_imm]);
        let should_yield = builder.inst_results(call)[0];
        // Create a yield block for preemption using the IR function's block count
        // as a unique index beyond existing blocks
        let yield_ebb = self.get_or_create_ebb(ir_func.blocks.len(), builder);
        let current_ebb = builder.current_ebb().unwrap();
        builder
            .ins()
            .brif(should_yield, yield_ebb, &[], current_ebb, &[]);
        Ok(())
    }

    fn compile_send(
        &mut self,
        inst: &IRInst,
        dest: IRValueId,
        msg: IRValueId,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let dest_val = self.get_value(dest);
        let msg_val = self.get_value(msg);
        let func_ref = self.runtime_glue.get_send_fn();
        builder.ins().call(
            func_ref,
            &[self.get_process_param(builder), dest_val, msg_val],
        );
        Ok(())
    }

    fn compile_recv(
        &mut self,
        inst: &IRInst,
        timeout: u32,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let timeout_imm = builder.ins().iconst(types::I32, timeout as i64);
        let func_ref = self.runtime_glue.get_recv_fn();
        let call = builder
            .ins()
            .call(func_ref, &[self.get_process_param(builder), timeout_imm]);
        if let Some(result) = inst.result {
            let results = builder.inst_results(call).to_vec();
            if let Some(&val) = results.first() {
                self.set_value_raw(inst, val);
            }
        }
        Ok(())
    }

    fn compile_load_literal(
        &mut self,
        inst: &IRInst,
        index: u32,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let func_ref = self.runtime_glue.get_load_literal_fn();
        let index_imm = builder.ins().iconst(types::I32, index as i64);
        let call = builder
            .ins()
            .call(func_ref, &[self.get_process_param(builder), index_imm]);
        let results = builder.inst_results(call).to_vec();
        if let Some(&val) = results.first() {
            self.set_value_raw(inst, val);
        }
        Ok(())
    }

    fn compile_const_small_int(
        &mut self,
        inst: &IRInst,
        value: i64,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let shifted = builder.ins().iconst(types::I64, value << 4);
        let result = builder.ins().bor_imm(shifted, 0x0F);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_const_atom(
        &mut self,
        inst: &IRInst,
        index: u32,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let val = (0x0F << 28) | (0x00 << 25) | (index as u64);
        let result = builder.ins().iconst(types::I64, val as i64);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_const_nil(
        &mut self,
        inst: &IRInst,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let val = (0x0F << 28) | (0x04 << 25);
        let result = builder.ins().iconst(types::I64, val as i64);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_const_true(
        &mut self,
        inst: &IRInst,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let val = (0x0F << 28) | (0x04 << 25) | 0x01;
        let result = builder.ins().iconst(types::I64, val as i64);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_const_false(
        &mut self,
        inst: &IRInst,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let val = (0x0F << 28) | (0x04 << 25) | 0x02;
        let result = builder.ins().iconst(types::I64, val as i64);
        self.set_value(inst, result);
        Ok(())
    }

    fn compile_binary_new(
        &mut self,
        inst: &IRInst,
        data: IRValueId,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let data_val = self.get_value(data);
        let func_ref = self.runtime_glue.get_binary_new_fn();
        let call = builder
            .ins()
            .call(func_ref, &[self.get_process_param(builder), data_val]);
        let results = builder.inst_results(call).to_vec();
        if let Some(&val) = results.first() {
            self.set_value_raw(inst, val);
        }
        Ok(())
    }

    fn compile_binary_size(
        &mut self,
        inst: &IRInst,
        binary: IRValueId,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let bin_val = self.get_value(binary);
        let func_ref = self.runtime_glue.get_binary_size_fn();
        let call = builder.ins().call(func_ref, &[bin_val]);
        let results = builder.inst_results(call).to_vec();
        if let Some(&val) = results.first() {
            self.set_value_raw(inst, val);
        }
        Ok(())
    }

    fn compile_binary_extract(
        &mut self,
        inst: &IRInst,
        binary: IRValueId,
        offset: IRValueId,
        size: IRValueId,
        _flags: u32,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let bin_val = self.get_value(binary);
        let off_val = self.get_value(offset);
        let size_val = self.get_value(size);
        let func_ref = self.runtime_glue.get_binary_extract_fn();
        let call = builder.ins().call(
            func_ref,
            &[self.get_process_param(builder), bin_val, off_val, size_val],
        );
        let results = builder.inst_results(call).to_vec();
        if let Some(&val) = results.first() {
            self.set_value_raw(inst, val);
        }
        Ok(())
    }

    fn compile_make_fun(
        &mut self,
        inst: &IRInst,
        module: IRValueId,
        function: IRValueId,
        arity: u32,
        fvs: &[IRValueId],
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let module_val = self.get_value(module);
        let function_val = self.get_value(function);
        let arity_val = builder.ins().iconst(types::I32, arity as i64);
        let mut call_args = vec![
            self.get_process_param(builder),
            module_val,
            function_val,
            arity_val,
        ];
        for &fv in fvs {
            call_args.push(self.get_value(fv));
        }
        let func_ref = self.runtime_glue.get_make_fun_fn();
        let call = builder.ins().call(func_ref, &call_args);
        let results = builder.inst_results(call).to_vec();
        if let Some(&val) = results.first() {
            self.set_value_raw(inst, val);
        }
        Ok(())
    }

    fn compile_gc_safe(
        &mut self,
        _inst: &IRInst,
        _builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        // Emit GC safepoint poll
        Ok(())
    }

    fn get_process_param(&mut self, builder: &mut FunctionBuilder) -> cranelift::ir::Value {
        builder
            .ins()
            .special_value(cranelift::ir::ArgumentPurpose::VMContext)
            .unwrap_or_else(|| {
                builder
                    .func
                    .dfg
                    .block_params(builder.current_ebb().unwrap())[0]
            })
    }

    fn get_value(&mut self, id: IRValueId) -> cranelift::ir::Value {
        *self
            .value_map
            .get(&id.0)
            .expect("Value not found in codegen")
    }

    fn set_value(&mut self, inst: &IRInst, val: cranelift::ir::Value) {
        if let Some(result) = inst.result {
            self.value_map.insert(result.0, val);
        }
    }

    fn set_value_raw(&mut self, inst: &IRInst, val: cranelift::ir::Value) {
        if let Some(result) = inst.result {
            self.value_map.insert(result.0, val);
        }
    }

    fn make_sig_for_call(&self, num_args: usize) -> cranelift::ir::Signature {
        let mut sig = cranelift::ir::Signature::new(cranelift::isa::CallConv::SystemV);
        sig.params.push(AbiParam::new(types::I64));
        for _ in 0..num_args {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
        sig
    }

    fn make_sig_for_bif(&self, num_args: usize) -> cranelift::ir::Signature {
        let mut sig = cranelift::ir::Signature::new(cranelift::isa::CallConv::SystemV);
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        for _ in 0..num_args {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
        sig
    }
}

/// Code generation errors.
#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    #[error("target error: {0}")]
    TargetError(String),
    #[error("compilation error: {0}")]
    CompilationError(String),
    #[error("unsupported feature: {0}")]
    Unsupported(String),
    #[error("link error: {0}")]
    LinkError(String),
}
