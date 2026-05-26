//! Edge case tests for dala_aot pipeline.

use std::collections::HashMap;

use dala_beam_loader::{BeamFunction, BeamInstruction, BeamModule, BeamOperand, BeamRegister};
use dala_ir::instruction::{IRInstKind, Reg};

use crate::pipeline::*;

// ═══════════════════════════════════════════════════════════════════════════
// CLI types (re-exported from cli module for test access)
// ═══════════════════════════════════════════════════════════════════════════

// Note: CLI types are defined in cli.rs and used in main.rs.
// We test the pipeline-level types (Mode, OptLevel) which are
// the ones used by the pipeline itself.

// ═══════════════════════════════════════════════════════════════════════════
// Mode - PartialEq
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_mode_jit_eq() {
    assert_eq!(Mode::Jit, Mode::Jit);
}

#[test]
fn test_mode_aot_eq() {
    assert_eq!(Mode::Aot, Mode::Aot);
}

#[test]
fn test_mode_mixed_eq() {
    assert_eq!(Mode::Mixed, Mode::Mixed);
}

#[test]
fn test_mode_jit_ne_aot() {
    assert_ne!(Mode::Jit, Mode::Aot);
}

#[test]
fn test_mode_jit_ne_mixed() {
    assert_ne!(Mode::Jit, Mode::Mixed);
}

#[test]
fn test_mode_aot_ne_mixed() {
    assert_ne!(Mode::Aot, Mode::Mixed);
}

#[test]
fn test_mode_copy() {
    let m = Mode::Aot;
    let m2 = m;
    assert_eq!(m, m2);
}

#[test]
fn test_mode_debug() {
    let m = Mode::Jit;
    let s = format!("{:?}", m);
    assert!(s.contains("Jit"));
}

// ═══════════════════════════════════════════════════════════════════════════
// OptLevel - PartialEq
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_opt_level_none_eq() {
    assert_eq!(OptLevel::None, OptLevel::None);
}

#[test]
fn test_opt_level_less_eq() {
    assert_eq!(OptLevel::Less, OptLevel::Less);
}

#[test]
fn test_opt_level_default_eq() {
    assert_eq!(OptLevel::Default, OptLevel::Default);
}

#[test]
fn test_opt_level_aggressive_eq() {
    assert_eq!(OptLevel::Aggressive, OptLevel::Aggressive);
}

#[test]
fn test_opt_level_none_ne_less() {
    assert_ne!(OptLevel::None, OptLevel::Less);
}

#[test]
fn test_opt_level_none_ne_default() {
    assert_ne!(OptLevel::None, OptLevel::Default);
}

#[test]
fn test_opt_level_none_ne_aggressive() {
    assert_ne!(OptLevel::None, OptLevel::Aggressive);
}

#[test]
fn test_opt_level_less_ne_default() {
    assert_ne!(OptLevel::Less, OptLevel::Default);
}

#[test]
fn test_opt_level_less_ne_aggressive() {
    assert_ne!(OptLevel::Less, OptLevel::Aggressive);
}

#[test]
fn test_opt_level_default_ne_aggressive() {
    assert_ne!(OptLevel::Default, OptLevel::Aggressive);
}

#[test]
fn test_opt_level_copy() {
    let o = OptLevel::Default;
    let o2 = o;
    assert_eq!(o, o2);
}

#[test]
fn test_opt_level_debug() {
    let o = OptLevel::Aggressive;
    let s = format!("{:?}", o);
    assert!(s.contains("Aggressive"));
}

// ═══════════════════════════════════════════════════════════════════════════
// PipelineConfig
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pipeline_config_creation_jit_none() {
    let config = PipelineConfig {
        input: "/tmp/test.beam".into(),
        output: "/tmp/out".into(),
        target: "x86_64".to_string(),
        mode: Mode::Jit,
        opt_level: OptLevel::None,
    };
    assert_eq!(config.target, "x86_64");
    assert_eq!(config.mode, Mode::Jit);
    assert_eq!(config.opt_level, OptLevel::None);
}

#[test]
fn test_pipeline_config_creation_aot_aggressive() {
    let config = PipelineConfig {
        input: "/tmp/test.beam".into(),
        output: "/tmp/out".into(),
        target: "aarch64".to_string(),
        mode: Mode::Aot,
        opt_level: OptLevel::Aggressive,
    };
    assert_eq!(config.target, "aarch64");
    assert_eq!(config.mode, Mode::Aot);
    assert_eq!(config.opt_level, OptLevel::Aggressive);
}

#[test]
fn test_pipeline_config_creation_mixed_default() {
    let config = PipelineConfig {
        input: "/tmp/in.beam".into(),
        output: "/tmp/out.bin".into(),
        target: "host".to_string(),
        mode: Mode::Mixed,
        opt_level: OptLevel::Default,
    };
    assert_eq!(config.mode, Mode::Mixed);
    assert_eq!(config.opt_level, OptLevel::Default);
}

#[test]
fn test_pipeline_config_creation_all_opt_levels() {
    let levels = vec![
        OptLevel::None,
        OptLevel::Less,
        OptLevel::Default,
        OptLevel::Aggressive,
    ];
    for level in levels {
        let config = PipelineConfig {
            input: "/tmp/t.beam".into(),
            output: "/tmp/o".into(),
            target: "x86_64".to_string(),
            mode: Mode::Aot,
            opt_level: level,
        };
        assert_eq!(config.opt_level, level);
    }
}

#[test]
fn test_pipeline_config_clone() {
    let config = PipelineConfig {
        input: "/a".into(),
        output: "/b".into(),
        target: "x86_64".to_string(),
        mode: Mode::Jit,
        opt_level: OptLevel::Less,
    };
    let cloned = config.clone();
    assert_eq!(config.target, cloned.target);
    assert_eq!(config.mode, cloned.mode);
    assert_eq!(config.opt_level, cloned.opt_level);
}

// ═══════════════════════════════════════════════════════════════════════════
// PipelineStats - default values
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pipeline_stats_default() {
    let stats = PipelineStats::default();
    assert_eq!(stats.functions_compiled, 0);
    assert_eq!(stats.total_code_size, 0);
    assert_eq!(stats.opt_passes_run, 0);
    assert_eq!(stats.ir_instructions_before, 0);
    assert_eq!(stats.ir_instructions_after, 0);
}

#[test]
fn test_pipeline_stats_clone() {
    let stats = PipelineStats {
        functions_compiled: 10,
        total_code_size: 2048,
        opt_passes_run: 5,
        ir_instructions_before: 100,
        ir_instructions_after: 80,
    };
    let cloned = stats.clone();
    assert_eq!(stats.functions_compiled, cloned.functions_compiled);
    assert_eq!(stats.total_code_size, cloned.total_code_size);
    assert_eq!(stats.opt_passes_run, cloned.opt_passes_run);
    assert_eq!(stats.ir_instructions_before, cloned.ir_instructions_before);
    assert_eq!(stats.ir_instructions_after, cloned.ir_instructions_after);
}

// ═══════════════════════════════════════════════════════════════════════════
// OptStats - default values
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_opt_stats_default() {
    let stats = OptStats::default();
    assert_eq!(stats.passes_run, 0);
    assert_eq!(stats.iterations, 0);
}

#[test]
fn test_opt_stats_clone() {
    let stats = OptStats {
        passes_run: 3,
        iterations: 7,
    };
    let cloned = stats.clone();
    assert_eq!(stats.passes_run, cloned.passes_run);
    assert_eq!(stats.iterations, cloned.iterations);
}

// ═══════════════════════════════════════════════════════════════════════════
// PipelineError - Display formatting
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pipeline_error_beam_load_error_display() {
    let err = PipelineError::BeamLoadError("bad magic".to_string());
    assert_eq!(format!("{}", err), "BEAM loading error: bad magic");
}

#[test]
fn test_pipeline_error_ir_translation_error_display() {
    let err = PipelineError::IrTranslationError("unknown opcode".to_string());
    assert_eq!(format!("{}", err), "IR translation error: unknown opcode");
}

#[test]
fn test_pipeline_error_codegen_error_display() {
    let err = PipelineError::CodegenError("register allocation failed".to_string());
    assert_eq!(
        format!("{}", err),
        "Codegen error: register allocation failed"
    );
}

#[test]
fn test_pipeline_error_io_error_display() {
    let err = PipelineError::IoError("permission denied".to_string());
    assert_eq!(format!("{}", err), "I/O error: permission denied");
}

#[test]
fn test_pipeline_error_debug() {
    let err = PipelineError::BeamLoadError("test".to_string());
    let debug = format!("{:?}", err);
    assert!(debug.contains("BeamLoadError"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Pipeline::new
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pipeline_new() {
    let config = PipelineConfig {
        input: "/tmp/test.beam".into(),
        output: "/tmp/out".into(),
        target: "x86_64".to_string(),
        mode: Mode::Aot,
        opt_level: OptLevel::Default,
    };
    let pipeline = Pipeline::new(config);
    // Pipeline is created; we can't inspect private fields directly,
    // but we can verify it doesn't panic.
    let _ = pipeline;
}

// ═══════════════════════════════════════════════════════════════════════════
// translate_beam_to_ir
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_translate_beam_to_ir_empty_module() {
    let beam_module = BeamModule::new("empty_mod".to_string());
    let ir_module = translate_beam_to_ir(&beam_module).unwrap();
    assert_eq!(ir_module.function_count(), 0);
    assert!(ir_module.exports.is_empty());
}

#[test]
fn test_translate_beam_to_ir_module_with_functions() {
    let mut beam_module = BeamModule::new("test_mod".to_string());
    beam_module.atoms = vec!["hello".to_string(), "world".to_string()];
    beam_module.functions.insert(
        ("hello".to_string(), 1),
        BeamFunction {
            name: "hello".to_string(),
            arity: 1,
            label: 0,
            code: vec![
                BeamInstruction {
                    opcode: 0, // move
                    operands: vec![
                        BeamOperand::Register(BeamRegister::X(0)),
                        BeamOperand::Register(BeamRegister::X(1)),
                    ],
                    line: None,
                },
                BeamInstruction {
                    opcode: 2, // return
                    operands: vec![],
                    line: None,
                },
            ],
        },
    );

    let ir_module = translate_beam_to_ir(&beam_module).unwrap();
    assert_eq!(ir_module.function_count(), 1);
}

#[test]
fn test_translate_beam_to_ir_module_with_exports() {
    let mut beam_module = BeamModule::new("export_mod".to_string());
    beam_module.atoms = vec!["main".to_string()];
    beam_module.exports = vec![("main".to_string(), 0, 1)];
    beam_module.functions.insert(
        ("main".to_string(), 0),
        BeamFunction {
            name: "main".to_string(),
            arity: 0,
            label: 1,
            code: vec![],
        },
    );

    let ir_module = translate_beam_to_ir(&beam_module).unwrap();
    assert_eq!(ir_module.exports.len(), 1);
    assert_eq!(ir_module.exports[0].1, 0); // arity
}

#[test]
fn test_translate_beam_to_ir_module_name_is_hashed() {
    let beam_module = BeamModule::new("my_module".to_string());
    let ir_module = translate_beam_to_ir(&beam_module).unwrap();
    // The module name should be a hash (non-zero for non-empty name)
    assert_ne!(ir_module.name, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// optimize_module
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_optimize_module_empty() {
    let mut ir_module = dala_ir::IRModule::new(0);
    let stats = optimize_module(&mut ir_module);
    // Empty module should have 0 iterations (max_iter = 0 for empty)
    assert_eq!(stats.iterations, 0);
    assert_eq!(stats.passes_run, 0);
}

#[test]
fn test_optimize_module_with_functions() {
    let mut beam_module = BeamModule::new("opt_test".to_string());
    beam_module.atoms = vec!["f".to_string()];
    beam_module.functions.insert(
        ("f".to_string(), 0),
        BeamFunction {
            name: "f".to_string(),
            arity: 0,
            label: 0,
            code: vec![
                BeamInstruction {
                    opcode: 0,
                    operands: vec![
                        BeamOperand::Register(BeamRegister::X(0)),
                        BeamOperand::Register(BeamRegister::X(1)),
                    ],
                    line: None,
                },
                BeamInstruction {
                    opcode: 2,
                    operands: vec![],
                    line: None,
                },
            ],
        },
    );

    let mut ir_module = translate_beam_to_ir(&beam_module).unwrap();
    let stats = optimize_module(&mut ir_module);
    // Should run at least one iteration
    assert!(stats.iterations >= 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// compile_module
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_compile_module_empty() {
    let ir_module = dala_ir::IRModule::new(0);
    let config = PipelineConfig {
        input: "/tmp/t.beam".into(),
        output: "/tmp/o".into(),
        target: "x86_64".to_string(),
        mode: Mode::Aot,
        opt_level: OptLevel::Default,
    };
    let result = compile_module(&ir_module, &config).unwrap();
    assert_eq!(result.functions_compiled, 0);
    assert_eq!(result.total_code_size, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// hash_str
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_hash_str_empty() {
    let h = hash_str("");
    // Empty string should produce a deterministic hash
    let h2 = hash_str("");
    assert_eq!(h, h2);
}

#[test]
fn test_hash_str_same_string_same_hash() {
    let h1 = hash_str("hello");
    let h2 = hash_str("hello");
    assert_eq!(h1, h2);
}

#[test]
fn test_hash_str_different_strings_likely_different() {
    let h1 = hash_str("hello");
    let h2 = hash_str("world");
    // Different strings should (almost certainly) produce different hashes
    assert_ne!(h1, h2);
}

#[test]
fn test_hash_str_deterministic() {
    // Same string should always produce the same hash
    for _ in 0..10 {
        assert_eq!(hash_str("test_atom"), hash_str("test_atom"));
    }
}

#[test]
fn test_hash_str_various_lengths() {
    // Hash various length strings without panicking
    let _ = hash_str("a");
    let _ = hash_str("ab");
    let _ = hash_str("abcdefghijklmnopqrstuvwxyz");
    let _ = hash_str(&"x".repeat(10000));
}

// ═══════════════════════════════════════════════════════════════════════════
// build_atom_table
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_build_atom_table_empty() {
    let module = BeamModule::new("test".to_string());
    let table = build_atom_table(&module);
    assert!(table.is_empty());
}

#[test]
fn test_build_atom_table_single() {
    let mut module = BeamModule::new("test".to_string());
    module.atoms = vec!["hello".to_string()];
    let table = build_atom_table(&module);
    assert_eq!(table.len(), 1);
    assert_eq!(table.get("hello"), Some(&1u64));
}

#[test]
fn test_build_atom_table_multiple() {
    let mut module = BeamModule::new("test".to_string());
    module.atoms = vec![
        "atom0".to_string(),
        "atom1".to_string(),
        "atom2".to_string(),
    ];
    let table = build_atom_table(&module);
    assert_eq!(table.len(), 3);
    assert_eq!(table.get("atom0"), Some(&1u64));
    assert_eq!(table.get("atom1"), Some(&2u64));
    assert_eq!(table.get("atom2"), Some(&3u64));
}

#[test]
fn test_build_atom_table_indices_start_at_one() {
    let mut module = BeamModule::new("test".to_string());
    module.atoms = vec!["first".to_string()];
    let table = build_atom_table(&module);
    // Indices should start at 1 (not 0)
    assert_eq!(table.get("first"), Some(&1u64));
    assert_ne!(table.get("first"), Some(&0u64));
}

// ═══════════════════════════════════════════════════════════════════════════
// count_ir_instructions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_count_ir_instructions_empty_module() {
    let module = dala_ir::IRModule::new(0);
    assert_eq!(count_ir_instructions(&module), 0);
}

#[test]
fn test_count_ir_instructions_module_with_functions() {
    let mut beam_module = BeamModule::new("count_test".to_string());
    beam_module.atoms = vec!["f".to_string()];
    beam_module.functions.insert(
        ("f".to_string(), 0),
        BeamFunction {
            name: "f".to_string(),
            arity: 0,
            label: 0,
            code: vec![
                BeamInstruction {
                    opcode: 0,
                    operands: vec![
                        BeamOperand::Register(BeamRegister::X(0)),
                        BeamOperand::Register(BeamRegister::X(1)),
                    ],
                    line: None,
                },
                BeamInstruction {
                    opcode: 2,
                    operands: vec![],
                    line: None,
                },
            ],
        },
    );

    let ir_module = translate_beam_to_ir(&beam_module).unwrap();
    let count = count_ir_instructions(&ir_module);
    // Should have at least some instructions (move + ret + any auto-inserted)
    assert!(count > 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// count_function_instructions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_count_function_instructions_empty() {
    let func = dala_ir::IRFunction::new(0, 0, 0);
    assert_eq!(count_function_instructions(&func), 0);
}

#[test]
fn test_count_function_instructions_with_blocks() {
    let mut func = dala_ir::IRFunction::new(0, 0, 0);
    let block_id = func.entry_block;
    let block = func.get_block_mut(block_id);

    // Push some instructions
    block.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Nop));
    block.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Nop));
    block.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Ret {
        value: dala_ir::value::IRValueId(0),
    }));

    assert_eq!(count_function_instructions(&func), 3);
}

#[test]
fn test_count_function_instructions_multiple_blocks() {
    let mut func = dala_ir::IRFunction::new(0, 0, 0);

    // Entry block with 2 instructions
    let entry = func.get_block_mut(func.entry_block);
    entry.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Nop));
    entry.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Ret {
        value: dala_ir::value::IRValueId(0),
    }));

    // Second block with 3 instructions
    let block2 = func.create_block();
    let b2 = func.get_block_mut(block2);
    b2.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Nop));
    b2.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Nop));
    b2.push_inst(dala_ir::instruction::IRInst::new(IRInstKind::Ret {
        value: dala_ir::value::IRValueId(0),
    }));

    assert_eq!(count_function_instructions(&func), 5);
}

// ═══════════════════════════════════════════════════════════════════════════
// beam_reg_to_ir
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_beam_reg_to_ir_x() {
    let beam_reg = BeamRegister::X(5);
    let ir_reg = beam_reg_to_ir(&beam_reg);
    assert_eq!(ir_reg, Reg::X(5));
}

#[test]
fn test_beam_reg_to_ir_y() {
    let beam_reg = BeamRegister::Y(10);
    let ir_reg = beam_reg_to_ir(&beam_reg);
    assert_eq!(ir_reg, Reg::Y(10));
}

#[test]
fn test_beam_reg_to_ir_f() {
    let beam_reg = BeamRegister::F(3);
    let ir_reg = beam_reg_to_ir(&beam_reg);
    assert_eq!(ir_reg, Reg::F(3));
}

#[test]
fn test_beam_reg_to_ir_zero() {
    assert_eq!(beam_reg_to_ir(&BeamRegister::X(0)), Reg::X(0));
    assert_eq!(beam_reg_to_ir(&BeamRegister::Y(0)), Reg::Y(0));
    assert_eq!(beam_reg_to_ir(&BeamRegister::F(0)), Reg::F(0));
}

#[test]
fn test_beam_reg_to_ir_max() {
    let max = u32::MAX;
    assert_eq!(beam_reg_to_ir(&BeamRegister::X(max)), Reg::X(max));
    assert_eq!(beam_reg_to_ir(&BeamRegister::Y(max)), Reg::Y(max));
    assert_eq!(beam_reg_to_ir(&BeamRegister::F(max)), Reg::F(max));
}

// ═══════════════════════════════════════════════════════════════════════════
// translate_beam_function
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_translate_beam_function_empty_code() {
    let beam_func = BeamFunction {
        name: "empty".to_string(),
        arity: 0,
        label: 0,
        code: vec![],
    };
    let atom_table = HashMap::new();
    let ir_func = translate_beam_function(&beam_func, 42, 1, &atom_table);
    assert_eq!(ir_func.module, 42);
    assert_eq!(ir_func.name, 1);
    assert_eq!(ir_func.arity, 0);
    // Empty code should still have a return instruction auto-inserted
    let block = ir_func.get_block(ir_func.entry_block);
    assert!(!block.instructions.is_empty());
    assert!(block.is_terminated());
}

#[test]
fn test_translate_beam_function_with_move() {
    let beam_func = BeamFunction {
        name: "move_test".to_string(),
        arity: 2,
        label: 0,
        code: vec![
            BeamInstruction {
                opcode: 0, // move
                operands: vec![
                    BeamOperand::Register(BeamRegister::X(0)),
                    BeamOperand::Register(BeamRegister::X(1)),
                ],
                line: None,
            },
            BeamInstruction {
                opcode: 2, // return
                operands: vec![],
                line: None,
            },
        ],
    };
    let atom_table = HashMap::new();
    let ir_func = translate_beam_function(&beam_func, 0, 0, &atom_table);
    let block = ir_func.get_block(ir_func.entry_block);
    // Should have move + return
    assert!(block.instructions.len() >= 2);
}

#[test]
fn test_translate_beam_function_with_label() {
    let beam_func = BeamFunction {
        name: "label_test".to_string(),
        arity: 0,
        label: 0,
        code: vec![
            BeamInstruction {
                opcode: 1, // label → nop
                operands: vec![BeamOperand::Label(1)],
                line: None,
            },
            BeamInstruction {
                opcode: 2, // return
                operands: vec![],
                line: None,
            },
        ],
    };
    let atom_table = HashMap::new();
    let ir_func = translate_beam_function(&beam_func, 0, 0, &atom_table);
    let block = ir_func.get_block(ir_func.entry_block);
    // label becomes nop, plus return
    assert!(block.instructions.len() >= 2);
}

#[test]
fn test_translate_beam_function_auto_ret() {
    // Function without explicit return should get auto-inserted ret
    let beam_func = BeamFunction {
        name: "no_ret".to_string(),
        arity: 0,
        label: 0,
        code: vec![BeamInstruction {
            opcode: 1, // label (nop)
            operands: vec![BeamOperand::Label(0)],
            line: None,
        }],
    };
    let atom_table = HashMap::new();
    let ir_func = translate_beam_function(&beam_func, 0, 0, &atom_table);
    let block = ir_func.get_block(ir_func.entry_block);
    assert!(block.is_terminated());
}

// ═══════════════════════════════════════════════════════════════════════════
// translate_beam_instruction - specific opcodes
// ═══════════════════════════════════════════════════════════════════════════

fn make_block() -> dala_ir::function::BasicBlock {
    dala_ir::function::BasicBlock::new(dala_ir::instruction::Label(0))
}

fn make_atom_table() -> HashMap<String, u64> {
    HashMap::new()
}

#[test]
fn test_translate_opcode_0_move() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 0,
        operands: vec![
            BeamOperand::Register(BeamRegister::X(0)),
            BeamOperand::Register(BeamRegister::X(1)),
        ],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert_eq!(block.instructions.len(), 1);
    assert!(matches!(
        &block.instructions[0].kind,
        IRInstKind::Move { .. }
    ));
}

#[test]
fn test_translate_opcode_1_label() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 1,
        operands: vec![BeamOperand::Label(5)],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert_eq!(block.instructions.len(), 1);
    assert!(matches!(&block.instructions[0].kind, IRInstKind::Nop));
}

#[test]
fn test_translate_opcode_2_return() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 2,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert_eq!(block.instructions.len(), 1);
    assert!(matches!(
        &block.instructions[0].kind,
        IRInstKind::Ret { .. }
    ));
}

#[test]
fn test_translate_opcode_3_is_small_int() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 3,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(
        &block.instructions[0].kind,
        IRInstKind::IsSmallInt
    ));
}

#[test]
fn test_translate_opcode_4_is_atom() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 4,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(&block.instructions[0].kind, IRInstKind::IsAtom));
}

#[test]
fn test_translate_opcode_5_is_tuple() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 5,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(&block.instructions[0].kind, IRInstKind::IsTuple));
}

#[test]
fn test_translate_opcode_6_is_list() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 6,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(&block.instructions[0].kind, IRInstKind::IsList));
}

#[test]
fn test_translate_opcode_7_is_float() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 7,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(&block.instructions[0].kind, IRInstKind::IsFloat));
}

#[test]
fn test_translate_opcode_8_is_nil() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 8,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(&block.instructions[0].kind, IRInstKind::IsNil));
}

#[test]
fn test_translate_opcode_179_move_x() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 179,
        operands: vec![
            BeamOperand::Register(BeamRegister::X(0)),
            BeamOperand::Register(BeamRegister::X(1)),
        ],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(
        &block.instructions[0].kind,
        IRInstKind::Move { .. }
    ));
}

#[test]
fn test_translate_opcode_182_try() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 182,
        operands: vec![BeamOperand::Label(1)],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(
        &block.instructions[0].kind,
        IRInstKind::Catch { .. }
    ));
}

#[test]
fn test_translate_opcode_183_try_case() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 183,
        operands: vec![BeamOperand::Label(1)],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(
        &block.instructions[0].kind,
        IRInstKind::Catch { .. }
    ));
}

#[test]
fn test_translate_unknown_opcode() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 999,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    // Unknown opcodes should produce a Nop
    assert!(matches!(&block.instructions[0].kind, IRInstKind::Nop));
}

#[test]
fn test_translate_opcode_100_fmul() {
    // opcode 100 = fmul (float multiplication), not unknown
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 100,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(&block.instructions[0].kind, IRInstKind::Mul));
}

#[test]
fn test_translate_opcode_u32_max() {
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: u32::MAX,
        operands: vec![],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert!(matches!(&block.instructions[0].kind, IRInstKind::Nop));
}

#[test]
fn test_translate_multiple_instructions_sequential() {
    let mut block = make_block();
    let atom_table = make_atom_table();

    // Push several instructions
    for opcode in [0, 1, 2] {
        let inst = BeamInstruction {
            opcode,
            operands: if opcode == 0 {
                vec![
                    BeamOperand::Register(BeamRegister::X(0)),
                    BeamOperand::Register(BeamRegister::X(1)),
                ]
            } else if opcode == 1 {
                vec![BeamOperand::Label(0)]
            } else {
                vec![]
            },
            line: None,
        };
        translate_beam_instruction(&mut block, &inst, &atom_table);
    }

    assert_eq!(block.instructions.len(), 3);
    assert!(matches!(
        &block.instructions[0].kind,
        IRInstKind::Move { .. }
    ));
    assert!(matches!(&block.instructions[1].kind, IRInstKind::Nop));
    assert!(matches!(
        &block.instructions[2].kind,
        IRInstKind::Ret { .. }
    ));
}

#[test]
fn test_translate_opcode_with_no_operands() {
    // Many opcodes don't need operands; verify they don't panic
    for opcode in [3, 4, 5, 6, 7, 8] {
        let mut block = make_block();
        let inst = BeamInstruction {
            opcode,
            operands: vec![],
            line: None,
        };
        translate_beam_instruction(&mut block, &inst, &make_atom_table());
        assert_eq!(block.instructions.len(), 1);
    }
}

#[test]
fn test_translate_opcode_with_extra_operands_ignored() {
    // Extra operands beyond what the opcode uses should be harmless
    let mut block = make_block();
    let inst = BeamInstruction {
        opcode: 2, // return
        operands: vec![
            BeamOperand::Register(BeamRegister::X(0)),
            BeamOperand::Integer(42),
            BeamOperand::Float(3.14),
        ],
        line: None,
    };
    translate_beam_instruction(&mut block, &inst, &make_atom_table());
    assert_eq!(block.instructions.len(), 1);
    assert!(matches!(
        &block.instructions[0].kind,
        IRInstKind::Ret { .. }
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// CompilationResult
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_compilation_result_creation() {
    let result = CompilationResult {
        functions: vec![],
        functions_compiled: 0,
        total_code_size: 0,
    };
    assert_eq!(result.functions_compiled, 0);
    assert_eq!(result.total_code_size, 0);
    assert!(result.functions.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// Integration: full pipeline with empty module
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_full_pipeline_empty_module() {
    let beam_module = BeamModule::new("empty".to_string());

    // Translate
    let mut ir_module = translate_beam_to_ir(&beam_module).unwrap();
    assert_eq!(ir_module.function_count(), 0);

    // Optimize
    let opt_stats = optimize_module(&mut ir_module);
    assert_eq!(opt_stats.iterations, 0);

    // Compile
    let config = PipelineConfig {
        input: "/tmp/t.beam".into(),
        output: "/tmp/o".into(),
        target: "x86_64".to_string(),
        mode: Mode::Aot,
        opt_level: OptLevel::Default,
    };
    let result = compile_module(&ir_module, &config).unwrap();
    assert_eq!(result.functions_compiled, 0);
    assert_eq!(result.total_code_size, 0);
}

#[test]
fn test_full_pipeline_with_function() {
    let mut beam_module = BeamModule::new("pipeline_test".to_string());
    beam_module.atoms = vec!["run".to_string()];
    beam_module.functions.insert(
        ("run".to_string(), 0),
        BeamFunction {
            name: "run".to_string(),
            arity: 0,
            label: 0,
            code: vec![
                BeamInstruction {
                    opcode: 0,
                    operands: vec![
                        BeamOperand::Register(BeamRegister::X(0)),
                        BeamOperand::Register(BeamRegister::X(1)),
                    ],
                    line: None,
                },
                BeamInstruction {
                    opcode: 2,
                    operands: vec![],
                    line: None,
                },
            ],
        },
    );

    // Translate
    let mut ir_module = translate_beam_to_ir(&beam_module).unwrap();
    assert_eq!(ir_module.function_count(), 1);

    // Count before optimization
    let _before = count_ir_instructions(&ir_module);

    // Optimize
    let _opt_stats = optimize_module(&mut ir_module);

    // Count after optimization
    let _after = count_ir_instructions(&ir_module);

    // Compile
    let config = PipelineConfig {
        input: "/tmp/t.beam".into(),
        output: "/tmp/o".into(),
        target: "x86_64".to_string(),
        mode: Mode::Aot,
        opt_level: OptLevel::Default,
    };
    let result = compile_module(&ir_module, &config).unwrap();
    assert!(result.functions_compiled <= 1);
}
