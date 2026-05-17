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

fn hash_str(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn build_atom_table(module: &BeamModule) -> HashMap<String, u64> {
    module
        .atoms
        .iter()
        .enumerate()
        .map(|(i, a)| (a.clone(), i as u64 + 1))
        .collect()
}

fn count_ir_instructions(module: &IRModule) -> usize {
    module
        .function_bodies
        .iter()
        .map(|f| count_function_instructions(f))
        .sum()
}

fn count_function_instructions(func: &IRFunction) -> usize {
    func.blocks.iter().map(|b| b.instructions.len()).sum()
}

fn beam_reg_to_ir(reg: &BeamRegister) -> dala_ir::instruction::Reg {
    match reg {
        BeamRegister::X(n) => dala_ir::instruction::Reg::X(*n),
        BeamRegister::Y(n) => dala_ir::instruction::Reg::Y(*n),
        BeamRegister::F(n) => dala_ir::instruction::Reg::F(*n),
    }
}

fn translate_beam_function(
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

fn translate_beam_instruction(
    block: &mut dala_ir::function::BasicBlock,
    inst: &BeamInstruction,
    _atom_table: &HashMap<String, u64>,
) {
    use dala_ir::instruction::{IRInst, IRInstKind, Reg};
    use dala_ir::value::IRValueId;

    macro_rules! nop {
        () => {
            block.push_inst(IRInst::new(IRInstKind::Nop))
        };
    }
    macro_rules! ret {
        () => {
            block.push_inst(IRInst::new(IRInstKind::Ret {
                value: IRValueId(0),
            }))
        };
    }

    match inst.opcode {
        0 => {
            // move
            if let (Some(BeamOperand::Register(src)), Some(BeamOperand::Register(dst))) =
                (inst.operands.first(), inst.operands.get(1))
            {
                block.push_inst(IRInst::new(IRInstKind::Move {
                    src: beam_reg_to_ir(src),
                    dst: beam_reg_to_ir(dst),
                }));
            }
        }
        1 => {
            nop!();
        } // call
        2 => {
            ret!();
        } // return
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
        17 => {
            block.push_inst(IRInst::new(IRInstKind::Eq));
        }
        18 => {
            block.push_inst(IRInst::new(IRInstKind::Lt));
        }
        19 => {
            block.push_inst(IRInst::new(IRInstKind::Gt));
        }
        20 => {
            // allocate
            let words = match inst.operands.first() {
                Some(BeamOperand::Integer(n)) => *n as u32,
                _ => 1,
            };
            block.push_inst(IRInst::new(IRInstKind::Alloc { words }));
        }
        21 => {
            nop!();
        } // deallocate => nop (GC)
        22 => {
            block.push_inst(IRInst::new(IRInstKind::GcSafe));
        }
        23 | 24 | 25 => {
            block.push_inst(IRInst::new(IRInstKind::CallBif {
                module: IRValueId(0),
                function: IRValueId(0),
                args: vec![],
            }));
        }
        26 => {
            block.push_inst(IRInst::new(IRInstKind::Call {
                func: IRValueId(0),
                args: vec![],
            }));
        }
        27 => {
            block.push_inst(IRInst::new(IRInstKind::TailCall {
                func: IRValueId(0),
                args: vec![],
            }));
        }
        28 => {
            block.push_inst(IRInst::new(IRInstKind::Br {
                target: dala_ir::instruction::Label(0),
            }));
        }
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
        35 | 36 => {
            nop!();
        } // put_tuple / put
        37 => {
            // get_tuple_element
            let idx = match inst.operands.get(2) {
                Some(BeamOperand::Integer(n)) => *n as u32,
                _ => 0,
            };
            block.push_inst(IRInst::new(IRInstKind::TupleGet {
                tuple: IRValueId(0),
                index: idx,
            }));
        }
        38 => {
            block.push_inst(IRInst::new(IRInstKind::TupleSet {
                tuple: IRValueId(0),
                index: 0,
                value: IRValueId(0),
            }));
        }
        39 => {
            nop!();
        } // put_list
        40 => {
            block.push_inst(IRInst::new(IRInstKind::Throw {
                reason: IRValueId(0),
            }));
        }
        41 => {
            block.push_inst(IRInst::new(IRInstKind::Send {
                dest: IRValueId(0),
                msg: IRValueId(0),
            }));
        }
        42 => {
            nop!();
        } // remove_message
        43 => {
            block.push_inst(IRInst::new(IRInstKind::ConsumeReductions { count: 1 }));
        }
        44 => {
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: 0 }));
        }
        45 => {
            nop!();
        } // loop_rec_end
        46 => {
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: u32::MAX }));
        } // wait
        47 => {
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: 0 }));
        } // wait_timeout
        48 => {
            block.push_inst(IRInst::new(IRInstKind::IsTrue));
        } // is_boolean
        49 | 50 | 51 => {
            nop!();
        } // is_number / is_port / is_reference
        52 => {
            block.push_inst(IRInst::new(IRInstKind::IsMap));
        }
        53 => {
            block.push_inst(IRInst::new(IRInstKind::IsList));
        } // is_nonempty_list
        54 => {
            block.push_inst(IRInst::new(IRInstKind::IsBinary));
        } // is_bitstring
        55..=62 => {
            nop!();
        } // bs_* opcodes
        63 | 64 => {
            nop!();
        } // fclearerror / fcheckerror
        65 => {
            block.push_inst(IRInst::new(IRInstKind::Move {
                src: Reg::F(0),
                dst: Reg::F(0),
            }));
        }
        66 => {
            nop!();
        } // fconv
        67 => {
            block.push_inst(IRInst::new(IRInstKind::Add));
        } // fadd
        68 => {
            block.push_inst(IRInst::new(IRInstKind::Sub));
        } // fsub
        69 => {
            block.push_inst(IRInst::new(IRInstKind::Mul));
        } // fmul
        70 => {
            block.push_inst(IRInst::new(IRInstKind::Div));
        } // fdiv
        71 => {
            block.push_inst(IRInst::new(IRInstKind::Neg));
        } // fnegate
        72 => {
            block.push_inst(IRInst::new(IRInstKind::MakeFun {
                module: IRValueId(0),
                function: IRValueId(0),
                arity: 0,
                fvs: vec![],
            }));
        }
        73 => {
            block.push_inst(IRInst::new(IRInstKind::Catch {
                handler: dala_ir::instruction::Label(0),
            }));
        }
        74 => {
            nop!();
        } // try_end
        75 => {
            block.push_inst(IRInst::new(IRInstKind::CatchPop));
        } // try_case
        76 => {
            block.push_inst(IRInst::new(IRInstKind::Throw {
                reason: IRValueId(0),
            }));
        }
        77 | 78 => {
            nop!();
        } // apply / apply_last
        79 | 80 | 81 | 127 => {
            block.push_inst(IRInst::new(IRInstKind::CallBif {
                module: IRValueId(0),
                function: IRValueId(0),
                args: vec![],
            }));
        }
        82 => {
            nop!();
        } // trim
        83 | 84 => {
            nop!();
        } // get_hd / get_tl
        85 | 86 => {
            nop!();
        } // put_map_assoc/exact
        87 => {
            nop!();
        } // get_map_element
        88 => {
            block.push_inst(IRInst::new(IRInstKind::IsFun));
        } // is_function2
        89..=98 => {
            nop!();
        } // bs_start_match3 through bs_restore2
        99 | 100 => {
            nop!();
        } // catch / catch_end
        101 => {
            block.push_inst(IRInst::new(IRInstKind::Call {
                func: IRValueId(0),
                args: vec![],
            }));
        }
        102 => {
            nop!();
        } // is_seq_trace
        103 | 104 => {
            nop!();
        } // bs_init2 / bs_bits_to_bytes
        105 => {
            block.push_inst(IRInst::new(IRInstKind::Add));
        } // bs_add
        106..=110 => {
            nop!();
        } // utf8/16/32 bs ops
        111 | 112 => {
            block.push_inst(IRInst::new(IRInstKind::Switch {
                value: IRValueId(0),
                default: dala_ir::instruction::Label(0),
                targets: vec![],
            }));
        }
        113 => {
            nop!();
        } // line
        114 => {
            block.push_inst(IRInst::new(IRInstKind::TailCall {
                func: IRValueId(0),
                args: vec![],
            }));
        }
        115 => {
            block.push_inst(IRInst::new(IRInstKind::BinaryNew { data: IRValueId(0) }));
        }
        116..=123 => {
            nop!();
        } // bs_utf8/16/32 get/skip + bs_init_writable
        124 => {
            nop!();
        } // on_load
        125 | 126 => {
            nop!();
        } // recv_mark / recv_set
        128 => {
            nop!();
        } // put_literal
        129 => {
            block.push_inst(IRInst::new(IRInstKind::IsMap));
        } // is_map_key
        130 => {
            nop!();
        } // get_map_values
        131 | 132 => {
            nop!();
        } // get_sd / set_sd
        133 | 180..=200 => {
            block.push_inst(IRInst::new(IRInstKind::TupleGet {
                tuple: IRValueId(0),
                index: 0,
            }));
        }
        134 => {
            block.push_inst(IRInst::new(IRInstKind::MakeFun {
                module: IRValueId(0),
                function: IRValueId(0),
                arity: 0,
                fvs: vec![],
            }));
        }
        135 => {
            block.push_inst(IRInst::new(IRInstKind::Throw {
                reason: IRValueId(0),
            }));
        }
        136 => {
            nop!();
        } // i_recv_set
        137..=139 => {
            block.push_inst(IRInst::new(IRInstKind::Switch {
                value: IRValueId(0),
                default: dala_ir::instruction::Label(0),
                targets: vec![],
            }));
        }
        140 => {
            nop!();
        } // i_get_map_element
        141 => {
            block.push_inst(IRInst::new(IRInstKind::TupleGet {
                tuple: IRValueId(0),
                index: 0,
            }));
        }
        142..=143 => {
            nop!();
        } // i_put_tuple / i_fetch
        144 => {
            block.push_inst(IRInst::new(IRInstKind::Recv { timeout: 0 }));
        } // i_loop_rec
        145 | 146 => {
            nop!();
        } // i_wait_timeout / i_wait_error
        147 | 151 | 153 | 156 | 158 => {
            block.push_inst(IRInst::new(IRInstKind::Eq));
        } // i_is_eq_*
        148 | 152 | 157 => {
            block.push_inst(IRInst::new(IRInstKind::Ne));
        } // i_is_ne_*
        149 | 154 => {
            block.push_inst(IRInst::new(IRInstKind::Lt));
        } // i_is_lt_*
        150 | 155 => {
            block.push_inst(IRInst::new(IRInstKind::Ge));
        } // i_is_ge_*
        159 => {
            block.push_inst(IRInst::new(IRInstKind::Add));
        } // i_increment
        160 => {
            block.push_inst(IRInst::new(IRInstKind::Sub));
        } // i_decrement
        161 => {
            block.push_inst(IRInst::new(IRInstKind::Mul));
        } // i_times
        162..=165 => {
            nop!();
        } // i_maybe_match_*
        166 => {
            block.push_inst(IRInst::new(IRInstKind::TupleGet {
                tuple: IRValueId(0),
                index: 0,
            }));
        }
        167 | 168 => {
            nop!();
        } // i_apply / i_apply_last
        169 | 170 => {
            block.push_inst(IRInst::new(IRInstKind::Call {
                func: IRValueId(0),
                args: vec![],
            }));
        }
        171 | 172 | 173 => {
            block.push_inst(IRInst::new(IRInstKind::Call {
                func: IRValueId(0),
                args: vec![],
            }));
        }
        174 | 175 | 176 | 177 | 178 => {
            block.push_inst(IRInst::new(IRInstKind::TailCall {
                func: IRValueId(0),
                args: vec![],
            }));
        }
        179 => {
            block.push_inst(IRInst::new(IRInstKind::Move {
                src: Reg::X(0),
                dst: Reg::X(0),
            }));
        }
        182 | 183 => {
            nop!();
        } // i_put_tuple2 / i_put_tuple3
        _ => {
            nop!();
        }
    }
}
