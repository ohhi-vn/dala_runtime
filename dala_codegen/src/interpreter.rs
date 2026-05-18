//! Interpreter - Direct execution of Dala IR without native codegen.
//!
//! Tree-walking interpreter for Dala IR, enabling end-to-end BEAM
//! bytecode execution without the Cranelift backend.

use std::collections::HashMap;

use crate::CompiledFunction;
use dala_ir::instruction::{IRInstKind, Reg};
use dala_ir::{IRFunction, IRValueId};
use dala_runtime::bif::{self, BifFn};
use dala_runtime::exception::Exception;
use dala_runtime::process::Process;
use dala_runtime::term::Term;

use crate::CodegenError;

/// Interpreter execution context.
pub struct Interpreter {
    bif_table: HashMap<(u32, u32, u32), BifFn>,
}

impl Interpreter {
    /// Create a new interpreter with all BIFs registered.
    pub fn new() -> Self {
        let mut bif_table = HashMap::new();
        let bifs = bif::register_all_bifs();
        for desc in &bifs {
            bif_table.insert(
                (desc.module, desc.function, desc.arity),
                desc.implementation,
            );
        }
        Self { bif_table }
    }

    /// "Compile" an IR function — validates and returns a CompiledFunction.
    pub fn compile_function(&self, ir_func: &IRFunction) -> Result<CompiledFunction, CodegenError> {
        if ir_func.blocks.is_empty() {
            return Err(CodegenError::CompilationError(format!(
                "Function {} has no basic blocks",
                ir_func.full_name()
            )));
        }
        if ir_func.entry_block.0 >= ir_func.blocks.len() {
            return Err(CodegenError::CompilationError(format!(
                "Function {} has invalid entry block {}",
                ir_func.full_name(),
                ir_func.entry_block.0
            )));
        }
        Ok(CompiledFunction {
            code_ptr: std::ptr::null(),
            code_size: 0,
            stack_map: None,
            frame_size: 0,
            spill_count: 0,
            name: ir_func.full_name(),
            arity: ir_func.arity,
        })
    }

    /// Execute a compiled IR function in the given process with the given arguments.
    pub fn execute(
        &self,
        ir_func: &IRFunction,
        process: &mut Process,
        args: &[Term],
    ) -> Result<Term, Exception> {
        let mut env: HashMap<IRValueId, Term> = HashMap::new();
        for (i, arg) in args.iter().enumerate() {
            if i < ir_func.arity as usize {
                env.insert(IRValueId(i), *arg);
                if i < 256 {
                    process.registers.x[i] = *arg;
                }
            }
        }
        let mut current_block = ir_func.entry_block;
        let mut pc: usize = 0;
        loop {
            let block = &ir_func.blocks[current_block.0];
            if pc >= block.instructions.len() {
                return Ok(Term::nil());
            }
            let inst = &block.instructions[pc];
            match &inst.kind {
                IRInstKind::Nop => {
                    pc += 1;
                }
                IRInstKind::Ret { value } => {
                    return if inst.operands.is_empty() && *value == IRValueId(0) {
                        Ok(Term::nil())
                    } else {
                        Ok(self.resolve_value(*value, &env, process))
                    };
                }
                IRInstKind::Move { src, dst } => {
                    let val = self.read_reg(src, &env, process);
                    self.write_reg(dst, val, &mut env, process);
                    pc += 1;
                }
                IRInstKind::GetReg { reg } => {
                    let val = self.read_reg(reg, &env, process);
                    if let Some(result) = inst.result {
                        env.insert(result, val);
                    }
                    pc += 1;
                }
                IRInstKind::SetReg { reg, value } => {
                    let val = self.resolve_value(*value, &env, process);
                    self.write_reg(reg, val, &mut env, process);
                    pc += 1;
                }
                IRInstKind::ConstSmallInt { value } => {
                    if let Some(result) = inst.result {
                        env.insert(result, Term::small(*value));
                    }
                    pc += 1;
                }
                IRInstKind::ConstAtom { index } => {
                    if let Some(result) = inst.result {
                        env.insert(result, Term::atom(*index as u32));
                    }
                    pc += 1;
                }
                IRInstKind::ConstNil => {
                    if let Some(result) = inst.result {
                        env.insert(result, Term::nil());
                    }
                    pc += 1;
                }
                IRInstKind::ConstTrue => {
                    if let Some(result) = inst.result {
                        env.insert(result, Term::true_());
                    }
                    pc += 1;
                }
                IRInstKind::ConstFalse => {
                    if let Some(result) = inst.result {
                        env.insert(result, Term::false_());
                    }
                    pc += 1;
                }
                IRInstKind::Add => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    let r = self.bif_add(a, b);
                    if let Some(res) = inst.result {
                        env.insert(res, r);
                    }
                    pc += 1;
                }
                IRInstKind::Sub => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    let r = self.bif_sub(a, b);
                    if let Some(res) = inst.result {
                        env.insert(res, r);
                    }
                    pc += 1;
                }
                IRInstKind::Mul => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    let r = self.bif_mul(a, b);
                    if let Some(res) = inst.result {
                        env.insert(res, r);
                    }
                    pc += 1;
                }
                IRInstKind::Div => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    let r = self.bif_div(a, b);
                    if let Some(res) = inst.result {
                        env.insert(res, r);
                    }
                    pc += 1;
                }
                IRInstKind::Rem => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    let r = self.bif_rem(a, b);
                    if let Some(res) = inst.result {
                        env.insert(res, r);
                    }
                    pc += 1;
                }
                IRInstKind::Neg => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let r = self.bif_neg(a);
                    if let Some(res) = inst.result {
                        env.insert(res, r);
                    }
                    pc += 1;
                }
                IRInstKind::Eq => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a == b));
                    }
                    pc += 1;
                }
                IRInstKind::Ne => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a != b));
                    }
                    pc += 1;
                }
                IRInstKind::Lt => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.to_raw() < b.to_raw()));
                    }
                    pc += 1;
                }
                IRInstKind::Gt => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.to_raw() > b.to_raw()));
                    }
                    pc += 1;
                }
                IRInstKind::Ge => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.to_raw() >= b.to_raw()));
                    }
                    pc += 1;
                }
                IRInstKind::Le => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    let b = self.resolve_op(&inst.operands, 1, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.to_raw() <= b.to_raw()));
                    }
                    pc += 1;
                }
                IRInstKind::IsSmallInt => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_small()));
                    }
                    pc += 1;
                }
                IRInstKind::IsAtom => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_atom()));
                    }
                    pc += 1;
                }
                IRInstKind::IsTuple => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_tuple()));
                    }
                    pc += 1;
                }
                IRInstKind::IsList => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_list()));
                    }
                    pc += 1;
                }
                IRInstKind::IsFloat => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_float()));
                    }
                    pc += 1;
                }
                IRInstKind::IsNil => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_nil()));
                    }
                    pc += 1;
                }
                IRInstKind::IsBinary => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_binary()));
                    }
                    pc += 1;
                }
                IRInstKind::IsFun => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_fun()));
                    }
                    pc += 1;
                }
                IRInstKind::IsPid => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_pid()));
                    }
                    pc += 1;
                }
                IRInstKind::IsMap => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_map()));
                    }
                    pc += 1;
                }
                IRInstKind::IsTrue => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_true()));
                    }
                    pc += 1;
                }
                IRInstKind::IsFalse => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_false()));
                    }
                    pc += 1;
                }
                IRInstKind::IsStableTuple => {
                    let a = self.resolve_op(&inst.operands, 0, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(a.is_tuple()));
                    }
                    pc += 1;
                }
                IRInstKind::IsMessage => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(false));
                    }
                    pc += 1;
                }
                IRInstKind::IsActor => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(false));
                    }
                    pc += 1;
                }
                IRInstKind::IsTensor => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(false));
                    }
                    pc += 1;
                }
                IRInstKind::IsCapability => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::bool(false));
                    }
                    pc += 1;
                }
                IRInstKind::TupleGet { tuple, index } => {
                    let tv = self.resolve_value(*tuple, &env, process);
                    let result = if tv.is_tuple() {
                        let header = tv.header();
                        let arity = Term::header_arity(header);
                        if (*index as usize) < arity {
                            tv.tuple_get(*index as usize)
                        } else {
                            Term::nil()
                        }
                    } else {
                        Term::nil()
                    };
                    if let Some(res) = inst.result {
                        env.insert(res, result);
                    }
                    pc += 1;
                }
                IRInstKind::TupleSet { value, .. } => {
                    let val = self.resolve_value(*value, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, val);
                    }
                    pc += 1;
                }
                IRInstKind::Alloc { words } => {
                    let ptr = process.alloc_words(*words as usize);
                    if let Some(res) = inst.result {
                        env.insert(res, Term::from_raw(ptr as u64));
                    }
                    pc += 1;
                }
                IRInstKind::GcSafe => {
                    pc += 1;
                }
                IRInstKind::Br { target } => {
                    current_block = self.find_block(ir_func, target.0);
                    pc = 0;
                }
                IRInstKind::BrIf {
                    cond,
                    true_target,
                    false_target,
                } => {
                    let cv = self.resolve_value(*cond, &env, process);
                    let is_true =
                        cv.is_true() || (cv.to_raw() != 0 && !cv.is_nil() && !cv.is_false());
                    if is_true {
                        current_block = self.find_block(ir_func, true_target.0);
                    } else {
                        current_block = self.find_block(ir_func, false_target.0);
                    }
                    pc = 0;
                }
                IRInstKind::Switch {
                    value,
                    default,
                    targets,
                } => {
                    let val = self.resolve_value(*value, &env, process);
                    let raw = val.to_raw();
                    let mut found = false;
                    for (tv, label) in targets {
                        if raw == *tv as u64 {
                            current_block = self.find_block(ir_func, label.0);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        current_block = self.find_block(ir_func, default.0);
                    }
                    pc = 0;
                }
                IRInstKind::CallBif {
                    module,
                    function,
                    args,
                } => {
                    let mod_val = self.resolve_value(*module, &env, process);
                    let fn_val = self.resolve_value(*function, &env, process);
                    let call_args: Vec<Term> = args
                        .iter()
                        .map(|a| self.resolve_value(*a, &env, process))
                        .collect();
                    let mod_atom = mod_val.get_atom_index().unwrap_or(0);
                    let fn_atom = fn_val.get_atom_index().unwrap_or(0);
                    let arity = call_args.len() as u32;
                    let result =
                        if let Some(&bif_fn) = self.bif_table.get(&(mod_atom, fn_atom, arity)) {
                            unsafe { bif_fn(process, &call_args).unwrap_or(Term::nil()) }
                        } else {
                            Term::nil()
                        };
                    if let Some(res) = inst.result {
                        env.insert(res, result);
                    }
                    pc += 1;
                }
                IRInstKind::Throw { reason } => {
                    let val = self.resolve_value(*reason, &env, process);
                    return Err(Exception::throw(val));
                }
                IRInstKind::Catch { .. } => {
                    pc += 1;
                }
                IRInstKind::CatchPop => {
                    pc += 1;
                }
                IRInstKind::Send { dest: _, msg } => {
                    let m = self.resolve_value(*msg, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, m);
                    }
                    pc += 1;
                }
                IRInstKind::Recv { .. } => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::nil());
                    }
                    pc += 1;
                }
                IRInstKind::ConsumeReductions { count } => {
                    process.consume_reductions(*count);
                    pc += 1;
                }
                IRInstKind::MakeFun { .. } => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::nil());
                    }
                    pc += 1;
                }
                IRInstKind::BinaryNew { .. } => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::nil());
                    }
                    pc += 1;
                }
                IRInstKind::BinarySize { .. } => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::small(0));
                    }
                    pc += 1;
                }
                IRInstKind::BinaryExtract { .. } => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::nil());
                    }
                    pc += 1;
                }
                IRInstKind::LoadLiteral { index } => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::small(*index as i64));
                    }
                    pc += 1;
                }
                IRInstKind::Load { .. } => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::nil());
                    }
                    pc += 1;
                }
                IRInstKind::Store { value, .. } => {
                    let v = self.resolve_value(*value, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, v);
                    }
                    pc += 1;
                }
                IRInstKind::Push { value } => {
                    let v = self.resolve_value(*value, &env, process);
                    process.push(v);
                    pc += 1;
                }
                IRInstKind::Pop => {
                    let val = process.pop();
                    if let Some(res) = inst.result {
                        env.insert(res, val);
                    }
                    pc += 1;
                }
                IRInstKind::GetStackPtr => {
                    if let Some(res) = inst.result {
                        env.insert(res, Term::nil());
                    }
                    pc += 1;
                }
                IRInstKind::SetStackPtr { .. } => {
                    pc += 1;
                }
                IRInstKind::Narrow { value, .. } => {
                    let v = self.resolve_value(*value, &env, process);
                    if let Some(res) = inst.result {
                        env.insert(res, v);
                    }
                    pc += 1;
                }
                _ => {
                    log::warn!("Interpreter: unhandled instruction {:?}", inst.kind);
                    pc += 1;
                }
            }
        }
    }

    // ===== Helper methods =====

    fn resolve_value(
        &self,
        id: IRValueId,
        env: &HashMap<IRValueId, Term>,
        process: &Process,
    ) -> Term {
        if id == IRValueId(0) && !env.contains_key(&id) {
            return Term::nil();
        }
        if let Some(&val) = env.get(&id) {
            return val;
        }
        if (id.0 as usize) < 256 {
            return process.registers.x[id.0 as usize];
        }
        Term::nil()
    }

    fn resolve_op(
        &self,
        operands: &[IRValueId],
        index: usize,
        env: &HashMap<IRValueId, Term>,
        process: &Process,
    ) -> Term {
        if let Some(id) = operands.get(index) {
            self.resolve_value(*id, env, process)
        } else {
            Term::nil()
        }
    }

    fn read_reg(&self, reg: &Reg, _env: &HashMap<IRValueId, Term>, process: &Process) -> Term {
        match reg {
            Reg::X(n) => process.registers.x[*n as usize],
            Reg::Y(n) => process.registers.y[*n as usize],
            Reg::F(n) => Term::from_raw(process.registers.f[*n as usize].to_bits()),
        }
    }

    fn write_reg(
        &self,
        reg: &Reg,
        val: Term,
        env: &mut HashMap<IRValueId, Term>,
        process: &mut Process,
    ) {
        match reg {
            Reg::X(n) => {
                process.registers.x[*n as usize] = val;
                env.insert(IRValueId(*n as usize), val);
            }
            Reg::Y(n) => process.registers.y[*n as usize] = val,
            Reg::F(n) => process.registers.f[*n as usize] = f64::from_bits(val.to_raw()),
        }
    }

    fn find_block(&self, func: &IRFunction, label: u32) -> dala_ir::BlockId {
        for (i, block) in func.blocks.iter().enumerate() {
            if block.label.0 == label {
                return dala_ir::BlockId(i);
            }
        }
        func.entry_block
    }

    fn bif_add(&self, a: Term, b: Term) -> Term {
        if let (Some(av), Some(bv)) = (a.get_small(), b.get_small()) {
            Term::small(av + bv)
        } else {
            Term::nil()
        }
    }
    fn bif_sub(&self, a: Term, b: Term) -> Term {
        if let (Some(av), Some(bv)) = (a.get_small(), b.get_small()) {
            Term::small(av - bv)
        } else {
            Term::nil()
        }
    }
    fn bif_mul(&self, a: Term, b: Term) -> Term {
        if let (Some(av), Some(bv)) = (a.get_small(), b.get_small()) {
            Term::small(av * bv)
        } else {
            Term::nil()
        }
    }
    fn bif_div(&self, a: Term, b: Term) -> Term {
        if let (Some(av), Some(bv)) = (a.get_small(), b.get_small()) {
            if bv == 0 {
                Term::nil()
            } else {
                Term::small(av / bv)
            }
        } else {
            Term::nil()
        }
    }
    fn bif_rem(&self, a: Term, b: Term) -> Term {
        if let (Some(av), Some(bv)) = (a.get_small(), b.get_small()) {
            if bv == 0 {
                Term::nil()
            } else {
                Term::small(av % bv)
            }
        } else {
            Term::nil()
        }
    }
    fn bif_neg(&self, a: Term) -> Term {
        if let Some(av) = a.get_small() {
            Term::small(-av)
        } else {
            Term::nil()
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}
