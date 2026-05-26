//! Compilation pipeline - orchestrates the full AOT compilation process.

use std::collections::HashMap;
use std::path::PathBuf;

use tracing::{debug, info, warn};

use dala_beam_loader::{BeamFunction, BeamInstruction, BeamModule, BeamOperand, BeamRegister};
use dala_codegen::{CodeGenerator, CodegenConfig, CompilationMode, CompiledFunction};
use dala_ir::opt;
use dala_ir::{IRFunction, IRInstKind, IRModule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Jit,
    Aot,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    None,
    Less,
    Default,
    Aggressive,
}

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub target: String,
    pub mode: Mode,
    pub opt_level: OptLevel,
}

#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    pub functions_compiled: usize,
    pub total_code_size: usize,
    pub opt_passes_run: usize,
    pub ir_instructions_before: usize,
    pub ir_instructions_after: usize,
}

#[derive(Debug, Clone, Default)]
pub struct OptStats {
    pub passes_run: usize,
    pub iterations: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("BEAM loading error: {0}")]
    BeamLoadError(String),
    #[error("IR translation error: {0}")]
    IrTranslationError(String),
    #[error("Codegen error: {0}")]
    CodegenError(String),
    #[error("I/O error: {0}")]
    IoError(String),
}

pub struct CompilationResult {
    pub functions: Vec<CompiledFunction>,
    pub functions_compiled: usize,
    pub total_code_size: usize,
}

pub struct Pipeline {
    config: PipelineConfig,
    ir_module: Option<IRModule>,
    compiled_functions: Vec<CompiledFunction>,
}

impl Pipeline {
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            ir_module: None,
            compiled_functions: Vec::new(),
        }
    }

    pub fn run(&mut self) -> Result<PipelineStats, PipelineError> {
        let mut stats = PipelineStats::default();
        info!("Stage 1: Loading BEAM bytecode");
        let beam_module = self.load_beam()?;
        info!("Stage 2: Translating to Dala IR");
        let mut ir_module = translate_beam_to_ir(&beam_module)
            .map_err(|e| PipelineError::IrTranslationError(e.to_string()))?;
        stats.ir_instructions_before = count_ir_instructions(&ir_module);
        info!("Stage 3: Running optimization passes");
        let opt_stats = optimize_module(&mut ir_module);
        stats.opt_passes_run = opt_stats.passes_run;
        stats.ir_instructions_after = count_ir_instructions(&ir_module);
        info!(
            "Optimization: {} -> {} instructions ({} passes, {} iters)",
            stats.ir_instructions_before,
            stats.ir_instructions_after,
            opt_stats.passes_run,
            opt_stats.iterations
        );
        self.ir_module = Some(ir_module.clone());
        info!("Stage 4: Generating native code");
        let compiled = compile_module(&ir_module, &self.config)
            .map_err(|e| PipelineError::CodegenError(e.to_string()))?;
        stats.functions_compiled = compiled.functions_compiled;
        stats.total_code_size = compiled.total_code_size;
        self.compiled_functions = compiled.functions;
        info!("Stage 5: Emitting output");
        self.emit_output(&stats)?;
        Ok(stats)
    }

    fn load_beam(&self) -> Result<BeamModule, PipelineError> {
        let path = self
            .config
            .input
            .to_str()
            .ok_or_else(|| PipelineError::IoError("Invalid path".to_string()))?;
        dala_beam_loader::load_beam_file(path)
            .map_err(|e| PipelineError::BeamLoadError(e.to_string()))
    }

    fn emit_output(&self, stats: &PipelineStats) -> Result<(), PipelineError> {
        let mut output = String::new();
        output.push_str("; Dala AOT Compilation Output\n");
        output.push_str(&format!("; Module: {}\n", self.config.input.display()));
        output.push_str(&format!("; Target: {}\n", self.config.target));
        output.push_str(&format!(
            "; Functions compiled: {}\n",
            stats.functions_compiled
        ));
        output.push_str(&format!(
            "; Total code size: {} bytes\n",
            stats.total_code_size
        ));
        output.push_str(&format!(
            "; IR: {} -> {} instructions\n\n",
            stats.ir_instructions_before, stats.ir_instructions_after
        ));
        for (i, func) in self.compiled_functions.iter().enumerate() {
            output.push_str(&format!(
                "func_{}: ptr={:p}, size={}, frame={}, spills={}\n",
                i, func.code_ptr, func.code_size, func.frame_size, func.spill_count
            ));
        }
        std::fs::write(&self.config.output, output)
            .map_err(|e| PipelineError::IoError(format!("Failed to write: {}", e)))?;
        info!("Output written to {}", self.config.output.display());
        Ok(())
    }
}

pub fn translate_beam_to_ir(beam_module: &BeamModule) -> Result<IRModule, String> {
    let module_name = hash_str(&beam_module.name);
    let mut ir_module = IRModule::new(module_name);
    let atom_table = build_atom_table(beam_module);
    for ((func_name, arity), beam_func) in &beam_module.functions {
        debug!("Translating function {}/{}", func_name, arity);
        let name_atom = atom_table.get(func_name).copied().unwrap_or(0);
        let ir_func = translate_beam_function(beam_func, module_name, name_atom, &atom_table);
        ir_module.function_bodies.push(ir_func);
    }
    for (name, arity, _label) in &beam_module.exports {
        let name_atom = atom_table.get(name).copied().unwrap_or(0);
        ir_module.add_export(name_atom, *arity);
    }
    Ok(ir_module)
}

pub fn optimize_module(module: &mut IRModule) -> OptStats {
    let mut stats = OptStats::default();
    let mut changed = true;
    let mut iteration = 0;
    let max_iter = match module.function_bodies.len() {
        0 => 0,
        n if n > 100 => 5,
        _ => 10,
    };
    while changed && iteration < max_iter {
        changed = false;
        iteration += 1;
        for func in &mut module.function_bodies {
            let before = count_function_instructions(func);
            opt::optimize(func);
            if count_function_instructions(func) != before {
                stats.passes_run += 1;
                changed = true;
            }
        }
    }
    stats.iterations = iteration;
    stats
}

pub fn compile_module(
    module: &IRModule,
    config: &PipelineConfig,
) -> Result<CompilationResult, String> {
    let cg_mode = match config.mode {
        Mode::Jit => CompilationMode::Jit,
        Mode::Aot | Mode::Mixed => CompilationMode::Aot,
    };
    let cg_config = CodegenConfig {
        mode: cg_mode,
        target: config.target.clone(),
        opt_level: match config.opt_level {
            OptLevel::None => "none",
            OptLevel::Less => "less",
            OptLevel::Default => "speed",
            OptLevel::Aggressive => "speed_and_size",
        },
        debug_assertions: matches!(config.opt_level, OptLevel::None),
        verbose: false,
    };
    let mut codegen = CodeGenerator::new(cg_config)
        .map_err(|e| format!("Failed to create code generator: {}", e))?;
    let mut functions = Vec::new();
    let mut total_size = 0usize;
    for func in &module.function_bodies {
        match codegen.compile_function(func) {
            Ok(cf) => {
                total_size += cf.code_size;
                functions.push(cf);
            }
            Err(e) => warn!("Failed to compile {}: {}", func.full_name(), e),
        }
    }
    Ok(CompilationResult {
        functions_compiled: functions.len(),
        total_code_size: total_size,
        functions,
    })
}

pub(crate) fn hash_str(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

pub(crate) fn build_atom_table(module: &BeamModule) -> HashMap<String, u64> {
    module
        .atoms
        .iter()
        .enumerate()
        .map(|(i, a)| (a.clone(), i as u64 + 1))
        .collect()
}

pub(crate) fn count_ir_instructions(module: &IRModule) -> usize {
    module
        .function_bodies
        .iter()
        .map(|f| count_function_instructions(f))
        .sum()
}

pub(crate) fn count_function_instructions(func: &IRFunction) -> usize {
    func.blocks.iter().map(|b| b.instructions.len()).sum()
}

pub(crate) fn beam_reg_to_ir(reg: &BeamRegister) -> dala_ir::instruction::Reg {
    match reg {
        BeamRegister::X(n) => dala_ir::instruction::Reg::X(*n),
        BeamRegister::Y(n) => dala_ir::instruction::Reg::Y(*n),
        BeamRegister::F(n) => dala_ir::instruction::Reg::F(*n),
    }
}

pub(crate) fn translate_beam_function(
    beam_func: &BeamFunction,
    module: u64,
    name_atom: u64,
    atom_table: &HashMap<String, u64>,
) -> IRFunction {
    let mut func = IRFunction::new(module, name_atom, beam_func.arity);
    let entry = func.entry_block;
    let block = func.get_block_mut(entry);
    for inst in &beam_func.code {
        translate_beam_instruction(block, inst, atom_table);
    }
    if !block.is_terminated() {
        block.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Ret {
            value: dala_ir::value::IRValueId(0),
        }));
    }
    func
}

#[inline]
fn push_nop(block: &mut dala_ir::function::BasicBlock) {
    block.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Nop));
}

#[inline]
fn push_ret(block: &mut dala_ir::function::BasicBlock) {
    block.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Ret {
        value: dala_ir::value::IRValueId(0),
    }));
}

pub(crate) fn translate_beam_instruction(
    block: &mut dala_ir::function::BasicBlock,
    inst: &BeamInstruction,
    atom_table: &HashMap<String, u64>,
) {
    use dala_ir::instruction::{IRInst, IRInstKind, Reg};
    use dala_ir::value::IRValueId;

    // Helper: convert a BeamOperand to an IRValueId
    let operand = |i: usize| -> IRValueId {
        match inst.operands.get(i) {
            Some(BeamOperand::Register(BeamRegister::X(n))) => IRValueId(*n as usize),
            Some(BeamOperand::Register(BeamRegister::Y(n))) => IRValueId(256 + *n as usize),
            Some(BeamOperand::Register(BeamRegister::F(n))) => IRValueId(1280 + *n as usize),
            Some(BeamOperand::Integer(n)) => IRValueId(2000 + i),
            Some(BeamOperand::AtomIndex(a)) => IRValueId(3000 + *a as usize),
            Some(BeamOperand::Label(l)) => IRValueId(4000 + *l as usize),
            Some(BeamOperand::Float(_)) => IRValueId(5000 + i),
            None => IRValueId(0),
        }
    };

    // Helper: get register from operand
    let reg = |i: usize| -> Reg {
        match inst.operands.get(i) {
            Some(BeamOperand::Register(r)) => beam_reg_to_ir(r),
            _ => Reg::X(0),
        }
    };

    // Helper: get integer from operand
    let int_op = |i: usize| -> i64 {
        match inst.operands.get(i) {
            Some(BeamOperand::Integer(n)) => *n,
            _ => 0,
        }
    };

    // Helper: get atom index from operand
    let atom_op = |i: usize| -> u32 {
        match inst.operands.get(i) {
            Some(BeamOperand::AtomIndex(a)) => *a,
            _ => 0,
        }
    };

    // Helper: get label from operand
    let label_op = |i: usize| -> u32 {
        match inst.operands.get(i) {
            Some(BeamOperand::Label(l)) => *l,
            _ => 0,
        }
    };

    // Helper: collect all register operands as IRValueIds
    let reg_operands =
        |start: usize, end: usize| -> Vec<IRValueId> { (start..end).map(|i| operand(i)).collect() };

    match inst.opcode {
        // 0: move - move value between registers
        0 => {
            block.push_inst(IRInst::new(IRInstKind::Move {
                src: reg(0),
                dst: reg(1),
            }));
        }
        // 1: label - no-op in IR (labels are block boundaries)
        1 => {
            push_nop(block);
        }
        // 2: return
        2 => {
            push_ret(block);
        }
        // 3-11: type tests
        3 => {
            block.push_inst(IRInst::new(IRInstKind::IsSmallInt));
        }
        4 => {
            block.push_inst(IRInst::new(IRInstKind::IsAtom));
        }
        5 => {
            block.push_inst(IRInst::new(IRInstKind::IsTuple));
        }
        6 => {
            block.push_inst(IRInst::new(IRInstKind::IsList));
        }
        7 => {
            block.push_inst(IRInst::new(IRInstKind::IsFloat));
        }
        8 => {
            block.push_inst(IRInst::new(IRInstKind::IsNil));
        }
        9 => {
            block.push_inst(IRInst::new(IRInstKind::IsBinary));
        }
        10 => {
            block.push_inst(IRInst::new(IRInstKind::IsFun));
        }
        11 => {
            block.push_inst(IRInst::new(IRInstKind::IsPid));
        }

        // 12-16: arithmetic
        12 => {
            block.push_inst(IRInst::new(IRInstKind::Add));
        }
        13 => {
            block.push_inst(IRInst::new(IRInstKind::Sub));
        }
        14 => {
            block.push_inst(IRInst::new(IRInstKind::Mul));
        }
        15 => {
            block.push_inst(IRInst::new(IRInstKind::Div));
        }
        16 => {
            block.push_inst(IRInst::new(IRInstKind::Rem));
        }

        // 17-19: comparisons
        17 => {
            block.push_inst(IRInst::new(IRInstKind::Eq));
        }
        18 => {
            block.push_inst(IRInst::new(IRInstKind::Lt));
        }
        19 => {
            block.push_inst(IRInst::new(IRInstKind::Gt));
        }

        // 20: allocate heap space
        20 => {
            block.push_inst(IRInst::new(IRInstKind::Alloc {
                words: int_op(0) as u32,
            }));
        }
        // 21: deallocate (stack trim) - no-op in our IR
        21 => {
            push_nop(block);
        }
        // 22: GC safepoint
        22 => {
            block.push_inst(IRInst::new(IRInstKind::GcSafe));
        }

        // 23-25: BIF calls (apply, apply_last, bif)
        23 | 24 | 25 => {
            let args: Vec<IRValueId> = (2..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::CallBif {
                module: operand(0),
                function: operand(1),
                args,
            }));
        }

        // 26: call
        26 => {
            let args: Vec<IRValueId> = (1..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::Call {
                func: operand(0),
                args,
            }));
        }
        // 27: tail call
        27 => {
            let args: Vec<IRValueId> = (1..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::TailCall {
                func: operand(0),
                args,
            }));
        }
        // 28: unconditional branch
        28 => {
            block.push_inst(IRInst::new(IRInstKind::Br {
                target: dala_ir::instruction::Label(label_op(0)),
            }));
        }

        // 29-34: more comparisons
        29 => {
            block.push_inst(IRInst::new(IRInstKind::Eq));
        }
        30 => {
            block.push_inst(IRInst::new(IRInstKind::Ne));
        }
        31 => {
            block.push_inst(IRInst::new(IRInstKind::Lt));
        }
        32 => {
            block.push_inst(IRInst::new(IRInstKind::Ge));
        }
        33 => {
            block.push_inst(IRInst::new(IRInstKind::Gt));
        }
        34 => {
            block.push_inst(IRInst::new(IRInstKind::Le));
        }

        // 35-36: is_function2, is_integer_fetch (type tests with register)
        35 => {
            block.push_inst(IRInst::new(IRInstKind::IsFun));
        }
        36 => {
            block.push_inst(IRInst::new(IRInstKind::IsSmallInt));
        }

        // 37: tuple_element (tuple_get)
        37 => {
            block.push_inst(IRInst::new(IRInstKind::TupleGet {
                tuple: operand(1),
                index: int_op(2) as u32,
            }));
        }
        38 => {
            block.push_inst(IRInst::new(IRInstKind::TupleSet {
                tuple: operand(0),
                index: int_op(2) as u32,
                value: operand(1),
            }));
        }
        39 => {
            // call_fun with arity check - treat as call
            let args: Vec<IRValueId> = (1..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::Call {
                func: operand(0),
                args,
            }));
        }
        40 => {
            block.push_inst(IRInst::new(IRInstKind::Throw { reason: operand(0) }));
        }
        41 => {
            block.push_inst(IRInst::new(IRInstKind::Send {
                dest: operand(0),
                msg: operand(1),
            }));
        }
        42 => {
            // loop_rec - receive loop message, simplified to recv
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: 0 }));
        }
        43 => {
            block.push_inst(IRInst::new(IRInstKind::ConsumeReductions { count: 1 }));
        }
        44 => {
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: 0 }));
        }
        45 => {
            // loop_rec_end - no-op in our IR
            push_nop(block);
        }
        46 => {
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: u32::MAX }));
        }
        47 => {
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: 0 }));
        }
        48 => {
            block.push_inst(IRInst::new(IRInstKind::IsTrue));
        }
        49 => {
            // wait_timeout - simplified to recv with timeout
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: u32::MAX }));
        }
        50 => {
            // is_boolean type test
            let val = operand(0);
            block.push_inst(IRInst::new(IRInstKind::IsTrue));
            // Also need IsFalse check - simplified
            let _ = val;
        }
        51 => {
            // is_function2 - check function with specific arity
            block.push_inst(IRInst::new(IRInstKind::IsFun));
        }
        52 => {
            block.push_inst(IRInst::new(IRInstKind::IsMap));
        }
        53 => {
            block.push_inst(IRInst::new(IRInstKind::IsList));
        }
        54 => {
            block.push_inst(IRInst::new(IRInstKind::IsBinary));
        }
        55..=59 => {
            // Various type tests: is_number, is_atom, is_tuple, etc.
            block.push_inst(IRInst::new(IRInstKind::IsSmallInt));
        }
        60 => {
            // is_reference - always false for now
            block.push_inst(IRInst::new(IRInstKind::IsPid));
        }
        61 => {
            // is_port type test
            block.push_inst(IRInst::new(IRInstKind::IsPid));
        }
        62 => {
            // is_nil type test
            block.push_inst(IRInst::new(IRInstKind::IsNil));
        }
        63 => {
            // is_binary type test
            block.push_inst(IRInst::new(IRInstKind::IsBinary));
        }
        64 => {
            // is_bitstring type test
            block.push_inst(IRInst::new(IRInstKind::IsBinary));
        }
        65 => {
            // fmove - float register move
            block.push_inst(IRInst::new(IRInstKind::Move {
                src: reg(0),
                dst: reg(1),
            }));
        }
        66 => {
            // fnegate - float negation
            block.push_inst(IRInst::new(IRInstKind::Neg));
        }
        67 => {
            block.push_inst(IRInst::new(IRInstKind::Add));
        }
        68 => {
            block.push_inst(IRInst::new(IRInstKind::Sub));
        }
        69 => {
            block.push_inst(IRInst::new(IRInstKind::Mul));
        }
        70 => {
            block.push_inst(IRInst::new(IRInstKind::Div));
        }
        71 => {
            block.push_inst(IRInst::new(IRInstKind::Neg));
        }
        72 => {
            block.push_inst(IRInst::new(IRInstKind::MakeFun {
                module: operand(0),
                function: operand(1),
                arity: int_op(2) as u32,
                fvs: (3..inst.operands.len()).map(|i| operand(i)).collect(),
            }));
        }
        73 => {
            block.push_inst(IRInst::new(IRInstKind::Catch {
                handler: dala_ir::instruction::Label(label_op(0)),
            }));
        }
        74 => {
            // catch_y - catch with Y register
            block.push_inst(IRInst::new(IRInstKind::Catch {
                handler: dala_ir::instruction::Label(label_op(0)),
            }));
        }
        75 => {
            block.push_inst(IRInst::new(IRInstKind::CatchPop));
        }
        76 => {
            block.push_inst(IRInst::new(IRInstKind::Throw { reason: operand(0) }));
        }
        77 => {
            // raise - rethrow exception
            block.push_inst(IRInst::new(IRInstKind::Throw { reason: operand(0) }));
        }
        78 => {
            // bif0 - BIF with 0 args
            block.push_inst(IRInst::new(IRInstKind::CallBif {
                module: operand(0),
                function: operand(1),
                args: vec![],
            }));
        }
        79 | 80 | 81 | 127 => {
            // bif1, bif2, bif3, call_bif - BIF calls with args
            let args: Vec<IRValueId> = (2..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::CallBif {
                module: operand(0),
                function: operand(1),
                args,
            }));
        }
        82 => {
            // gc_bif1 - GC-aware BIF with 1 arg
            block.push_inst(IRInst::new(IRInstKind::GcSafe));
            let args: Vec<IRValueId> = (3..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::CallBif {
                module: operand(0),
                function: operand(1),
                args,
            }));
        }
        83 | 84 => {
            // gc_bif2, gc_bif3 - GC-aware BIFs
            block.push_inst(IRInst::new(IRInstKind::GcSafe));
            let args: Vec<IRValueId> = (3..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::CallBif {
                module: operand(0),
                function: operand(1),
                args,
            }));
        }
        85 | 86 => {
            // bif13, bif14 - type test BIFs
            block.push_inst(IRInst::new(IRInstKind::IsSmallInt));
        }
        87 => {
            // truncate - integer truncation, no-op for small ints
            push_nop(block);
        }
        88 => {
            block.push_inst(IRInst::new(IRInstKind::IsFun));
        }
        89..=98 => {
            // Various arithmetic BIFs: *,+, -, div, rem, band, bor, bxor, bsl, bsr
            block.push_inst(IRInst::new(IRInstKind::Add));
        }
        99 => {
            // fsub - float subtraction
            block.push_inst(IRInst::new(IRInstKind::Sub));
        }
        100 => {
            // fmul - float multiplication
            block.push_inst(IRInst::new(IRInstKind::Mul));
        }
        101 => {
            // fdiv - float division
            block.push_inst(IRInst::new(IRInstKind::Div));
        }
        102 => {
            // fnegate - float negation
            block.push_inst(IRInst::new(IRInstKind::Neg));
        }
        103 => {
            // fadd - float addition
            block.push_inst(IRInst::new(IRInstKind::Add));
        }
        104 => {
            // allocate_heap_zero - allocate with zeroing
            block.push_inst(IRInst::new(IRInstKind::Alloc {
                words: int_op(0) as u32,
            }));
        }
        105 => {
            block.push_inst(IRInst::new(IRInstKind::Add));
        }
        106..=110 => {
            // More BIFs: abs, negate, floor, ceil, round
            block.push_inst(IRInst::new(IRInstKind::Neg));
        }
        111 | 112 => {
            // select_val, select_tuple_arity - switch dispatch
            block.push_inst(IRInst::new(IRInstKind::Switch {
                value: operand(0),
                default: dala_ir::instruction::Label(label_op(1)),
                targets: vec![],
            }));
        }
        113 => {
            // set_tuple_element
            block.push_inst(IRInst::new(IRInstKind::TupleSet {
                tuple: operand(0),
                index: int_op(1) as u32,
                value: operand(2),
            }));
        }
        114 => {
            // call_fun_last - tail call with dealloc
            let args: Vec<IRValueId> = (1..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::TailCall {
                func: operand(0),
                args,
            }));
        }
        115 => {
            block.push_inst(IRInst::new(IRInstKind::BinaryNew { data: operand(0) }));
        }
        116..=123 => {
            // Binary operations: binary_part, binary_split, etc.
            block.push_inst(IRInst::new(IRInstKind::BinaryExtract {
                binary: operand(0),
                offset: operand(1),
                size: operand(2),
                flags: 0,
            }));
        }
        124 => {
            // is_bitstr - check if bitstring
            block.push_inst(IRInst::new(IRInstKind::IsBinary));
        }
        125 | 126 => {
            // bs_init, bs_init_writable - binary init
            block.push_inst(IRInst::new(IRInstKind::BinaryNew { data: operand(0) }));
        }
        128 => {
            // bs_append - binary append
            block.push_inst(IRInst::new(IRInstKind::BinaryNew { data: operand(0) }));
        }
        129 => {
            block.push_inst(IRInst::new(IRInstKind::IsMap));
        }
        130 => {
            // bs_match_string - binary match
            block.push_inst(IRInst::new(IRInstKind::BinaryExtract {
                binary: operand(0),
                offset: IRValueId(0),
                size: operand(1),
                flags: 0,
            }));
        }
        131 | 132 => {
            // bs_get_integer, bs_get_float - binary read
            block.push_inst(IRInst::new(IRInstKind::BinaryExtract {
                binary: operand(0),
                offset: operand(1),
                size: operand(2),
                flags: 0,
            }));
        }
        133 => {
            block.push_inst(IRInst::new(IRInstKind::TupleGet {
                tuple: operand(0),
                index: 0,
            }));
        }
        134 => {
            block.push_inst(IRInst::new(IRInstKind::MakeFun {
                module: operand(0),
                function: operand(1),
                arity: int_op(2) as u32,
                fvs: (3..inst.operands.len()).map(|i| operand(i)).collect(),
            }));
        }
        135 => {
            block.push_inst(IRInst::new(IRInstKind::Throw { reason: operand(0) }));
        }
        136 => {
            // bs_skip_bits - skip bits in binary
            push_nop(block);
        }
        137..=139 => {
            // bs_test_tail, bs_test_unit, bs_start_match
            block.push_inst(IRInst::new(IRInstKind::IsBinary));
        }
        140 => {
            // bs_get_position - get binary position
            block.push_inst(IRInst::new(IRInstKind::BinarySize { binary: operand(0) }));
        }
        141 => {
            block.push_inst(IRInst::new(IRInstKind::TupleGet {
                tuple: operand(0),
                index: 0,
            }));
        }
        142..=143 => {
            // bs_set_position, bs_get_binary
            block.push_inst(IRInst::new(IRInstKind::BinaryExtract {
                binary: operand(0),
                offset: IRValueId(0),
                size: operand(1),
                flags: 0,
            }));
        }
        144 => {
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: 0 }));
        }
        145 | 146 => {
            // has_map_fields, get_map_elements - map operations
            block.push_inst(IRInst::new(IRInstKind::IsMap));
        }
        147 | 151 | 153 | 156 | 158 => {
            // is_eq_exact, is_ne_exact, etc. - exact comparisons
            block.push_inst(IRInst::new(IRInstKind::Eq));
        }
        148 | 152 | 157 => {
            // is_ne_exact, is_not_equal, etc. - exact inequality
            block.push_inst(IRInst::new(IRInstKind::Ne));
        }
        149 | 154 => {
            // is_lt_exact - less than exact
            block.push_inst(IRInst::new(IRInstKind::Lt));
        }
        150 | 155 => {
            // is_ge_exact - greater or equal exact
            block.push_inst(IRInst::new(IRInstKind::Ge));
        }
        159 => {
            block.push_inst(IRInst::new(IRInstKind::Add));
        }
        160 => {
            block.push_inst(IRInst::new(IRInstKind::Sub));
        }
        161 => {
            block.push_inst(IRInst::new(IRInstKind::Mul));
        }
        162..=165 => {
            // More BIFs: map_get, map_size, map_is_key, map_keys
            block.push_inst(IRInst::new(IRInstKind::IsMap));
        }
        166 => {
            block.push_inst(IRInst::new(IRInstKind::TupleGet {
                tuple: operand(0),
                index: 0,
            }));
        }
        167 | 168 => {
            // put_tuple, build_stacktrace
            block.push_inst(IRInst::new(IRInstKind::TupleSet {
                tuple: operand(0),
                index: 0,
                value: operand(1),
            }));
        }
        169 | 170 => {
            // call_ext, call_ext_only - external calls
            let args: Vec<IRValueId> = (1..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::Call {
                func: operand(0),
                args,
            }));
        }
        171 | 172 | 173 => {
            // call_ext_last, apply, apply_last
            let args: Vec<IRValueId> = (1..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::Call {
                func: operand(0),
                args,
            }));
        }
        174 | 175 | 176 | 177 | 178 => {
            // call_only, call_last, call_ext_only, call_ext_last, apply_last
            let args: Vec<IRValueId> = (1..inst.operands.len()).map(|i| operand(i)).collect();
            block.push_inst(IRInst::new(IRInstKind::TailCall {
                func: operand(0),
                args,
            }));
        }
        179 => {
            // move with X register
            block.push_inst(IRInst::new(IRInstKind::Move {
                src: reg(0),
                dst: reg(1),
            }));
        }
        182 | 183 => {
            // try, try_case - exception handling
            block.push_inst(IRInst::new(IRInstKind::Catch {
                handler: dala_ir::instruction::Label(label_op(0)),
            }));
        }
        _ => {
            push_nop(block);
        }
    }
}
