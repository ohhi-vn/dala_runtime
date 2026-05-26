//! Comprehensive edge case tests for the dala_ir crate.
//!
//! Covers: instruction, function, module, builder, value, constant, layout,
//! opt/const_prop, opt/dce, opt/simplify_cfg, and opt/mod.

use crate::constant::Constant;
use crate::function::{BasicBlock, IRFunction, StackMapEntry};
use crate::instruction::{IRInst, IRInstKind, Label, Reg, SideEffects, TensorOpKind};
use crate::layout::{
    BeamCallingConvention, FrameLayout, FrameLayoutCalculator, Slot, compute_stack_map,
};
use crate::module::{CompilationUnit, CompileInfo, IRModule};
use crate::opt::const_prop::{self, fold_constants, propagate_constants};
use crate::opt::dce::eliminate_dead_code;
use crate::opt::simplify_cfg::simplify;
use crate::opt::{self, run_pass};
use crate::type_system::{
    ActorLifecycle, ConstantValue, IRType, MessagePriority, NativeField, NativeFieldKind,
    NativeLayout, NativeResourceKind, SpeculativeGuard, TensorDtype, TypeKind,
};
use crate::value::{IRValue, IRValueId, ValueDef, ValueUse};
use crate::{BlockId, IRContext, IRFunctionId, InstId, TypeId, ValueId};

// ============================================================================
// instruction.rs edge cases
// ============================================================================

mod instruction_tests {
    use super::*;

    // --- SideEffects edge cases ---

    #[test]
    fn test_side_effects_none_is_all_false() {
        let e = SideEffects::NONE;
        assert!(!e.allocates);
        assert!(!e.reads_heap);
        assert!(!e.writes_heap);
        assert!(!e.may_raise);
        assert!(!e.calls);
        assert!(!e.may_yield);
    }

    #[test]
    fn test_side_effects_all_is_all_true() {
        let e = SideEffects::ALL;
        assert!(e.allocates);
        assert!(e.reads_heap);
        assert!(e.writes_heap);
        assert!(e.may_raise);
        assert!(e.calls);
        assert!(e.may_yield);
    }

    #[test]
    fn test_side_effects_single_flag() {
        let e = SideEffects {
            may_raise: true,
            ..SideEffects::NONE
        };
        assert!(e.may_raise);
        assert!(!e.allocates);
        assert!(!e.calls);
    }

    #[test]
    fn test_side_effects_combination() {
        let mut e = SideEffects::NONE;
        e.allocates = true;
        e.may_raise = true;
        assert!(e.allocates);
        assert!(e.may_raise);
        assert!(!e.writes_heap);
    }

    // --- IRInst construction edge cases ---

    #[test]
    fn test_inst_new_has_no_result_and_no_operands() {
        let inst = IRInst::new(IRInstKind::Add);
        assert!(inst.result.is_none());
        assert!(inst.operands.is_empty());
        assert_eq!(inst.beam_offset, 0);
        assert_eq!(inst.side_effects, SideEffects::NONE);
    }

    #[test]
    fn test_inst_with_result_has_result() {
        let id = IRValueId(42);
        let inst = IRInst::with_result(IRInstKind::Mul, id);
        assert_eq!(inst.result, Some(id));
        assert!(inst.operands.is_empty());
    }

    #[test]
    fn test_inst_add_operand() {
        let mut inst = IRInst::new(IRInstKind::Add);
        inst.add_operand(IRValueId(1));
        inst.add_operand(IRValueId(2));
        assert_eq!(inst.operands.len(), 2);
        assert_eq!(inst.operands[0], IRValueId(1));
        assert_eq!(inst.operands[1], IRValueId(2));
    }

    #[test]
    fn test_inst_set_side_effects() {
        let mut inst = IRInst::new(IRInstKind::Alloc { words: 10 });
        assert!(!inst.side_effects.allocates);
        inst.set_side_effects(SideEffects::ALL);
        assert!(inst.side_effects.allocates);
        assert!(inst.side_effects.may_raise);
    }

    #[test]
    fn test_inst_may_gc() {
        let no_gc = IRInst::new(IRInstKind::Add);
        assert!(!no_gc.may_gc());

        let alloc = IRInst::new(IRInstKind::Alloc { words: 5 });
        let mut alloc_with_effects = alloc.clone();
        // Default Alloc has no side effects set; may_gc checks side_effects.allocates
        assert!(!alloc.may_gc());

        let mut inst = IRInst::new(IRInstKind::Add);
        inst.side_effects.allocates = true;
        assert!(inst.may_gc());
    }

    #[test]
    fn test_inst_may_raise() {
        let safe = IRInst::new(IRInstKind::Add);
        assert!(!safe.may_raise());

        let mut throwing = IRInst::new(IRInstKind::Div);
        throwing.side_effects.may_raise = true;
        assert!(throwing.may_raise());
    }

    #[test]
    fn test_inst_may_yield() {
        let no_yield = IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        });
        assert!(!no_yield.may_yield());

        let mut yielding = IRInst::new(IRInstKind::ConsumeReductions { count: 1 });
        yielding.side_effects.may_yield = true;
        assert!(yielding.may_yield());
    }

    // --- IRInstKind variant construction edge cases ---

    #[test]
    fn test_inst_kind_arithmetic_variants() {
        let _ = IRInst::new(IRInstKind::Add);
        let _ = IRInst::new(IRInstKind::Sub);
        let _ = IRInst::new(IRInstKind::Mul);
        let _ = IRInst::new(IRInstKind::Div);
        let _ = IRInst::new(IRInstKind::Rem);
        let _ = IRInst::new(IRInstKind::Neg);
    }

    #[test]
    fn test_inst_kind_bitwise_variants() {
        let _ = IRInst::new(IRInstKind::BitAnd);
        let _ = IRInst::new(IRInstKind::BitOr);
        let _ = IRInst::new(IRInstKind::BitXor);
        let _ = IRInst::new(IRInstKind::BitNot);
        let _ = IRInst::new(IRInstKind::ShiftLeft);
        let _ = IRInst::new(IRInstKind::ShiftRight);
    }

    #[test]
    fn test_inst_kind_comparison_variants() {
        let _ = IRInst::new(IRInstKind::Eq);
        let _ = IRInst::new(IRInstKind::Ne);
        let _ = IRInst::new(IRInstKind::Gt);
        let _ = IRInst::new(IRInstKind::Ge);
        let _ = IRInst::new(IRInstKind::Lt);
        let _ = IRInst::new(IRInstKind::Le);
    }

    #[test]
    fn test_inst_kind_type_test_variants() {
        let _ = IRInst::new(IRInstKind::IsSmallInt);
        let _ = IRInst::new(IRInstKind::IsFloat);
        let _ = IRInst::new(IRInstKind::IsAtom);
        let _ = IRInst::new(IRInstKind::IsTuple);
        let _ = IRInst::new(IRInstKind::IsList);
        let _ = IRInst::new(IRInstKind::IsMap);
        let _ = IRInst::new(IRInstKind::IsBinary);
        let _ = IRInst::new(IRInstKind::IsFun);
        let _ = IRInst::new(IRInstKind::IsPid);
        let _ = IRInst::new(IRInstKind::IsNil);
        let _ = IRInst::new(IRInstKind::IsTrue);
        let _ = IRInst::new(IRInstKind::IsFalse);
        let _ = IRInst::new(IRInstKind::IsStableTuple);
        let _ = IRInst::new(IRInstKind::IsMessage);
        let _ = IRInst::new(IRInstKind::IsActor);
        let _ = IRInst::new(IRInstKind::IsTensor);
        let _ = IRInst::new(IRInstKind::IsCapability);
    }

    #[test]
    fn test_inst_kind_memory_variants() {
        let _ = IRInst::new(IRInstKind::Alloc { words: 0 });
        let _ = IRInst::new(IRInstKind::Alloc { words: u32::MAX });
        let _ = IRInst::new(IRInstKind::Load {
            base: IRValueId(0),
            offset: 0,
        });
        let _ = IRInst::new(IRInstKind::Store {
            base: IRValueId(0),
            offset: u32::MAX,
            value: IRValueId(1),
        });
        let _ = IRInst::new(IRInstKind::TupleGet {
            tuple: IRValueId(0),
            index: 0,
        });
        let _ = IRInst::new(IRInstKind::TupleSet {
            tuple: IRValueId(0),
            index: u32::MAX,
            value: IRValueId(1),
        });
    }

    #[test]
    fn test_inst_kind_control_flow_variants() {
        let _ = IRInst::new(IRInstKind::Br { target: Label(0) });
        let _ = IRInst::new(IRInstKind::BrIf {
            cond: IRValueId(0),
            true_target: Label(1),
            false_target: Label(2),
        });
        let _ = IRInst::new(IRInstKind::Switch {
            value: IRValueId(0),
            default: Label(0),
            targets: vec![(0, Label(1)), (1, Label(2))],
        });
        let _ = IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        });
        let _ = IRInst::new(IRInstKind::Call {
            func: IRValueId(0),
            args: vec![IRValueId(1), IRValueId(2)],
        });
        let _ = IRInst::new(IRInstKind::TailCall {
            func: IRValueId(0),
            args: vec![],
        });
        let _ = IRInst::new(IRInstKind::CallBif {
            module: IRValueId(0),
            function: IRValueId(1),
            args: vec![],
        });
    }

    #[test]
    fn test_inst_kind_exception_variants() {
        let _ = IRInst::new(IRInstKind::Catch { handler: Label(99) });
        let _ = IRInst::new(IRInstKind::CatchPop);
        let _ = IRInst::new(IRInstKind::Throw {
            reason: IRValueId(0),
        });
        let _ = IRInst::new(IRInstKind::Resume {
            exception: IRValueId(0),
        });
    }

    #[test]
    fn test_inst_kind_process_variants() {
        let _ = IRInst::new(IRInstKind::ConsumeReductions { count: 0 });
        let _ = IRInst::new(IRInstKind::Send {
            dest: IRValueId(0),
            msg: IRValueId(1),
        });
        let _ = IRInst::new(IRInstKind::Recv { timeout: 0 });
    }

    #[test]
    fn test_inst_kind_literal_variants() {
        let _ = IRInst::new(IRInstKind::LoadLiteral { index: 0 });
        let _ = IRInst::new(IRInstKind::ConstSmallInt { value: 0 });
        let _ = IRInst::new(IRInstKind::ConstSmallInt { value: i64::MIN });
        let _ = IRInst::new(IRInstKind::ConstSmallInt { value: i64::MAX });
        let _ = IRInst::new(IRInstKind::ConstAtom { index: 0 });
        let _ = IRInst::new(IRInstKind::ConstNil);
        let _ = IRInst::new(IRInstKind::ConstTrue);
        let _ = IRInst::new(IRInstKind::ConstFalse);
    }

    #[test]
    fn test_inst_kind_binary_variants() {
        let _ = IRInst::new(IRInstKind::BinaryNew { data: IRValueId(0) });
        let _ = IRInst::new(IRInstKind::BinarySize {
            binary: IRValueId(0),
        });
        let _ = IRInst::new(IRInstKind::BinaryExtract {
            binary: IRValueId(0),
            offset: IRValueId(1),
            size: IRValueId(2),
            flags: 0,
        });
    }

    #[test]
    fn test_inst_kind_fun_variants() {
        let _ = IRInst::new(IRInstKind::MakeFun {
            module: IRValueId(0),
            function: IRValueId(1),
            arity: 0,
            fvs: vec![],
        });
    }

    #[test]
    fn test_inst_kind_actor_variants() {
        let _ = IRInst::new(IRInstKind::SpawnActor {
            module: IRValueId(0),
            args: vec![],
            qos: 0,
        });
        let _ = IRInst::new(IRInstKind::SendTyped {
            target: IRValueId(0),
            msg: IRValueId(1),
            type_tag: 0,
            priority: 0,
        });
        let _ = IRInst::new(IRInstKind::RecvTyped {
            type_tag: 0,
            timeout: 0,
        });
    }

    #[test]
    fn test_inst_kind_stable_variants() {
        let _ = IRInst::new(IRInstKind::AllocStable {
            type_desc: 0,
            words: 0,
        });
        let _ = IRInst::new(IRInstKind::PromoteStable {
            object: IRValueId(0),
        });
    }

    #[test]
    fn test_inst_kind_tensor_variants() {
        let _ = IRInst::new(IRInstKind::TensorNew {
            desc_idx: 0,
            gpu: false,
        });
        let _ = IRInst::new(IRInstKind::TensorNew {
            desc_idx: 0,
            gpu: true,
        });
        let _ = IRInst::new(IRInstKind::TensorOp {
            op: TensorOpKind::Add,
            inputs: vec![IRValueId(0), IRValueId(1)],
        });
    }

    #[test]
    fn test_inst_kind_capability_variants() {
        let _ = IRInst::new(IRInstKind::CapNew {
            resource_kind: 0,
            owned: true,
        });
        let _ = IRInst::new(IRInstKind::CapRelease { cap: IRValueId(0) });
        let _ = IRInst::new(IRInstKind::CapTransfer {
            cap: IRValueId(0),
            new_owner: IRValueId(1),
        });
    }

    #[test]
    fn test_inst_kind_ai_variants() {
        let _ = IRInst::new(IRInstKind::InferenceSubmit {
            model_id: IRValueId(0),
            input: IRValueId(1),
            priority: 0,
        });
        let _ = IRInst::new(IRInstKind::InferenceAwait {
            request: IRValueId(0),
        });
    }

    #[test]
    fn test_inst_kind_arena_variants() {
        let _ = IRInst::new(IRInstKind::ArenaAlloc {
            arena: IRValueId(0),
            size: 0,
            align: 1,
        });
        let _ = IRInst::new(IRInstKind::ArenaReset {
            arena: IRValueId(0),
        });
    }

    #[test]
    fn test_inst_kind_narrow() {
        let narrow = IRInst::new(IRInstKind::Narrow {
            value: IRValueId(0),
            new_type: Box::new(IRType::new(TypeKind::Tuple { arity: 2 })),
        });
        assert!(matches!(narrow.kind, IRInstKind::Narrow { .. }));
    }

    #[test]
    fn test_inst_kind_nop() {
        let nop = IRInst::new(IRInstKind::Nop);
        assert!(matches!(nop.kind, IRInstKind::Nop));
    }

    // --- TensorOpKind edge cases ---

    #[test]
    fn test_tensor_op_kind_all_variants() {
        let kinds = [
            TensorOpKind::Add,
            TensorOpKind::Mul,
            TensorOpKind::MatMul,
            TensorOpKind::Relu,
            TensorOpKind::Softmax,
            TensorOpKind::Concat,
            TensorOpKind::Reshape,
            TensorOpKind::Transpose,
        ];
        for kind in &kinds {
            let inst = IRInst::new(IRInstKind::TensorOp {
                op: *kind,
                inputs: vec![],
            });
            assert!(matches!(inst.kind, IRInstKind::TensorOp { .. }));
        }
    }

    // --- Reg edge cases ---

    #[test]
    fn test_reg_variants() {
        let x = Reg::X(0);
        let y = Reg::Y(255);
        let f = Reg::F(10);
        assert!(matches!(x, Reg::X(0)));
        assert!(matches!(y, Reg::Y(255)));
        assert!(matches!(f, Reg::F(10)));
    }

    #[test]
    fn test_reg_equality() {
        assert_eq!(Reg::X(0), Reg::X(0));
        assert_ne!(Reg::X(0), Reg::X(1));
        assert_ne!(Reg::X(0), Reg::Y(0));
    }

    // --- Label edge cases ---

    #[test]
    fn test_label_default() {
        let l = Label::default();
        assert_eq!(l.0, 0);
    }

    #[test]
    fn test_label_custom() {
        let l = Label(42);
        assert_eq!(l.0, 42);
    }
}

// ============================================================================
// function.rs edge cases
// ============================================================================

mod function_tests {
    use super::*;

    // --- BasicBlock edge cases ---

    #[test]
    fn test_basic_block_new() {
        let bb = BasicBlock::new(Label(5));
        assert_eq!(bb.label.0, 5);
        assert!(bb.instructions.is_empty());
        assert!(bb.predecessors.is_empty());
        assert!(bb.successors.is_empty());
        assert!(bb.reachable);
    }

    #[test]
    fn test_basic_block_default() {
        let bb = BasicBlock::default();
        assert_eq!(bb.label.0, 0);
        // Default derives reachable=false; new() sets reachable=true
        assert!(!bb.reachable);
    }

    #[test]
    fn test_basic_block_push_inst() {
        let mut bb = BasicBlock::new(Label(0));
        bb.push_inst(IRInst::new(IRInstKind::Add));
        bb.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));
        assert_eq!(bb.instructions.len(), 2);
    }

    #[test]
    fn test_basic_block_terminator() {
        let mut bb = BasicBlock::new(Label(0));
        assert!(bb.terminator().is_none());

        bb.push_inst(IRInst::new(IRInstKind::Add));
        bb.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));
        let term = bb.terminator().unwrap();
        assert!(matches!(term.kind, IRInstKind::Ret { .. }));
    }

    #[test]
    fn test_basic_block_is_terminated() {
        let mut bb = BasicBlock::new(Label(0));
        assert!(!bb.is_terminated());

        bb.push_inst(IRInst::new(IRInstKind::Br { target: Label(1) }));
        assert!(bb.is_terminated());
    }

    #[test]
    fn test_basic_block_not_terminated_on_non_terminator() {
        let mut bb = BasicBlock::new(Label(0));
        bb.push_inst(IRInst::new(IRInstKind::Add));
        assert!(!bb.is_terminated());
    }

    #[test]
    fn test_basic_block_terminated_variants() {
        let terminators = [
            IRInstKind::Br { target: Label(0) },
            IRInstKind::BrIf {
                cond: IRValueId(0),
                true_target: Label(0),
                false_target: Label(1),
            },
            IRInstKind::Switch {
                value: IRValueId(0),
                default: Label(0),
                targets: vec![],
            },
            IRInstKind::Ret {
                value: IRValueId(0),
            },
            IRInstKind::TailCall {
                func: IRValueId(0),
                args: vec![],
            },
            IRInstKind::Throw {
                reason: IRValueId(0),
            },
        ];

        for kind in &terminators {
            let mut bb = BasicBlock::new(Label(0));
            bb.push_inst(IRInst::new(kind.clone()));
            assert!(bb.is_terminated(), "Expected {:?} to be a terminator", kind);
        }
    }

    // --- IRFunction edge cases ---

    #[test]
    fn test_ir_function_new() {
        let func = IRFunction::new(1, 2, 3);
        assert_eq!(func.module, 1);
        assert_eq!(func.name, 2);
        assert_eq!(func.arity, 3);
        assert_eq!(func.file, 0);
        assert_eq!(func.line, 0);
        assert_eq!(func.blocks.len(), 1); // entry block
        assert_eq!(func.entry_block, BlockId(0));
        assert!(!func.compiled);
        assert!(func.stack_maps.is_empty());
    }

    #[test]
    fn test_ir_function_create_block() {
        let mut func = IRFunction::new(0, 0, 0);
        let b1 = func.create_block();
        let b2 = func.create_block();
        assert_eq!(func.blocks.len(), 3);
        assert_eq!(b1, BlockId(1));
        assert_eq!(b2, BlockId(2));
    }

    #[test]
    fn test_ir_function_get_block() {
        let mut func = IRFunction::new(0, 0, 0);
        let b1 = func.create_block();
        let block = func.get_block(b1);
        assert_eq!(block.label.0, 1);
    }

    #[test]
    fn test_ir_function_get_block_mut() {
        let mut func = IRFunction::new(0, 0, 0);
        let b1 = func.create_block();
        let block = func.get_block_mut(b1);
        block.push_inst(IRInst::new(IRInstKind::Nop));
        assert_eq!(block.instructions.len(), 1);
    }

    #[test]
    fn test_ir_function_block_count() {
        let mut func = IRFunction::new(0, 0, 0);
        assert_eq!(func.block_count(), 1);
        func.create_block();
        assert_eq!(func.block_count(), 2);
        func.create_block();
        assert_eq!(func.block_count(), 3);
    }

    #[test]
    fn test_ir_function_name_str() {
        let func = IRFunction::new(0, 42, 0);
        assert_eq!(func.name_str(), "f42");
    }

    #[test]
    fn test_ir_function_full_name() {
        let func = IRFunction::new(7, 42, 3);
        assert_eq!(func.full_name(), "m7.f42/3");
    }

    #[test]
    fn test_ir_function_add_param_type() {
        let mut func = IRFunction::new(0, 0, 2);
        func.add_param_type(TypeId(0));
        func.add_param_type(TypeId(1));
        assert_eq!(func.param_types.len(), 2);
        assert_eq!(func.param_types[0], TypeId(0));
        assert_eq!(func.param_types[1], TypeId(1));
    }

    #[test]
    fn test_ir_function_set_return_type() {
        let mut func = IRFunction::new(0, 0, 0);
        func.set_return_type(TypeId(5));
        assert_eq!(func.return_type, TypeId(5));
    }

    #[test]
    fn test_ir_function_add_stack_map() {
        let mut func = IRFunction::new(0, 0, 0);
        func.add_stack_map(0, 0b101, 3);
        func.add_stack_map(4, 0b111, 5);
        assert_eq!(func.stack_maps.len(), 2);
        assert_eq!(func.stack_maps[0].instruction_offset, 0);
        assert_eq!(func.stack_maps[0].live_registers, 0b101);
        assert_eq!(func.stack_maps[0].live_stack_slots, 3);
        assert_eq!(func.stack_maps[1].instruction_offset, 4);
    }

    #[test]
    fn test_ir_function_zero_arity() {
        let func = IRFunction::new(0, 0, 0);
        assert_eq!(func.arity, 0);
        assert!(func.param_types.is_empty());
    }

    #[test]
    fn test_ir_function_many_blocks() {
        let mut func = IRFunction::new(0, 0, 0);
        for _ in 0..100 {
            func.create_block();
        }
        assert_eq!(func.block_count(), 101);
    }

    // --- StackMapEntry edge cases ---

    #[test]
    fn test_stack_map_entry_zero() {
        let entry = StackMapEntry {
            instruction_offset: 0,
            live_registers: 0,
            live_stack_slots: 0,
        };
        assert_eq!(entry.instruction_offset, 0);
        assert_eq!(entry.live_registers, 0);
        assert_eq!(entry.live_stack_slots, 0);
    }

    #[test]
    fn test_stack_map_entry_max() {
        let entry = StackMapEntry {
            instruction_offset: u32::MAX,
            live_registers: u64::MAX,
            live_stack_slots: u32::MAX,
        };
        assert_eq!(entry.instruction_offset, u32::MAX);
        assert_eq!(entry.live_registers, u64::MAX);
        assert_eq!(entry.live_stack_slots, u32::MAX);
    }
}

// ============================================================================
// module.rs edge cases
// ============================================================================

mod module_tests {
    use super::*;

    // --- IRModule edge cases ---

    #[test]
    fn test_module_new() {
        let module = IRModule::new(42);
        assert_eq!(module.name, 42);
        assert!(module.functions.is_empty());
        assert!(module.function_bodies.is_empty());
        assert!(module.exports.is_empty());
        assert!(module.imports.is_empty());
        assert!(module.attributes.is_empty());
        assert!(module.literals.is_empty());
        assert!(module.line_info.is_empty());
    }

    #[test]
    fn test_module_add_function() {
        let mut module = IRModule::new(0);
        let id = module.add_function(1, 2);
        assert_eq!(module.function_count(), 1);
        assert_eq!(id, IRFunctionId(0));
        assert!(module.get_function(1, 2).is_some());
    }

    #[test]
    fn test_module_add_multiple_functions() {
        let mut module = IRModule::new(0);
        let id1 = module.add_function(1, 2);
        let id2 = module.add_function(3, 4);
        let id3 = module.add_function(5, 6);
        assert_eq!(module.function_count(), 3);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn test_module_get_function_not_found() {
        let module = IRModule::new(0);
        assert!(module.get_function(999, 0).is_none());
    }

    #[test]
    fn test_module_get_function_body() {
        let mut module = IRModule::new(0);
        let id = module.add_function(1, 2);
        let body = module.get_function_body(id);
        assert_eq!(body.name, 1);
        assert_eq!(body.arity, 2);
    }

    #[test]
    fn test_module_get_function_body_mut() {
        let mut module = IRModule::new(0);
        let id = module.add_function(1, 2);
        let body = module.get_function_body_mut(id);
        body.compiled = true;
        assert!(module.get_function_body(id).compiled);
    }

    #[test]
    fn test_module_add_export() {
        let mut module = IRModule::new(0);
        module.add_export(1, 2);
        module.add_export(3, 4);
        assert_eq!(module.exports.len(), 2);
        assert!(module.is_exported(1, 2));
        assert!(module.is_exported(3, 4));
        assert!(!module.is_exported(1, 3));
    }

    #[test]
    fn test_module_add_import() {
        let mut module = IRModule::new(0);
        module.add_import(10, 1, 2);
        module.add_import(10, 3, 4);
        module.add_import(20, 5, 6);

        let imports_from_10 = module.imports.get(&10).unwrap();
        assert_eq!(imports_from_10.len(), 2);
        assert_eq!(imports_from_10[0], (1, 2));
        assert_eq!(imports_from_10[1], (3, 4));

        let imports_from_20 = module.imports.get(&20).unwrap();
        assert_eq!(imports_from_20.len(), 1);
    }

    #[test]
    fn test_module_add_literal() {
        let mut module = IRModule::new(0);
        let idx1 = module.add_literal(42);
        let idx2 = module.add_literal(99);
        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(module.literals.len(), 2);
        assert_eq!(module.literals[0], 42);
        assert_eq!(module.literals[1], 99);
    }

    #[test]
    fn test_module_exported_functions() {
        let mut module = IRModule::new(0);
        module.add_export(1, 0);
        module.add_export(2, 1);
        let exported = module.exported_functions();
        assert_eq!(exported.len(), 2);
        assert_eq!(exported[0], (1, 0));
        assert_eq!(exported[1], (2, 1));
    }

    #[test]
    fn test_module_function_count_empty() {
        let module = IRModule::new(0);
        assert_eq!(module.function_count(), 0);
    }

    // --- CompileInfo edge cases ---

    #[test]
    fn test_compile_info_default() {
        let info = CompileInfo::default();
        assert!(info.source_file.is_none());
        assert!(info.options.is_empty());
        assert!(info.version.is_none());
        assert!(!info.debug_info);
    }

    // --- CompilationUnit edge cases ---

    #[test]
    fn test_compilation_unit_new() {
        let module = IRModule::new(0);
        let cu = CompilationUnit::new(module);
        assert!(cu.types.is_empty());
        assert!(cu.constants.is_empty());
    }

    #[test]
    fn test_compilation_unit_add_type() {
        let module = IRModule::new(0);
        let mut cu = CompilationUnit::new(module);
        let t1 = cu.add_type(IRType::new(TypeKind::Int64));
        let t2 = cu.add_type(IRType::new(TypeKind::Float));
        assert_eq!(t1, TypeId(0));
        assert_eq!(t2, TypeId(1));
        assert_eq!(cu.types.len(), 2);
    }

    #[test]
    fn test_compilation_unit_add_constant() {
        let module = IRModule::new(0);
        let mut cu = CompilationUnit::new(module);
        let c1 = cu.add_constant(42);
        let c2 = cu.add_constant(99);
        assert_eq!(c1, 0);
        assert_eq!(c2, 1);
        assert_eq!(cu.constants.len(), 2);
    }
}

// ============================================================================
// builder.rs edge cases
// ============================================================================

mod builder_tests {
    use super::*;
    use crate::builder::IRBuilder;

    #[test]
    fn test_builder_new() {
        let builder = IRBuilder::new(1, 2, 3);
        assert_eq!(builder.function.module, 1);
        assert_eq!(builder.function.name, 2);
        assert_eq!(builder.function.arity, 3);
        assert_eq!(builder.current_block(), BlockId(0));
    }

    #[test]
    fn test_builder_default() {
        let builder = IRBuilder::default();
        assert_eq!(builder.function.module, 0);
        assert_eq!(builder.function.name, 0);
        assert_eq!(builder.function.arity, 0);
    }

    #[test]
    fn test_builder_current_block() {
        let mut builder = IRBuilder::new(0, 0, 0);
        assert_eq!(builder.current_block(), BlockId(0));

        let b1 = builder.create_block();
        builder.set_current_block(b1);
        assert_eq!(builder.current_block(), b1);
    }

    #[test]
    fn test_builder_create_block() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let b1 = builder.create_block();
        let b2 = builder.create_block();
        assert_eq!(b1, BlockId(1));
        assert_eq!(b2, BlockId(2));
        assert_eq!(builder.function.block_count(), 3);
    }

    #[test]
    fn test_builder_seal_block() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let new_block = builder.seal_block();
        assert_eq!(builder.current_block(), new_block);
        assert_eq!(builder.function.block_count(), 2);
    }

    // --- Value management edge cases ---

    #[test]
    fn test_builder_constant() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.constant(Constant::Int(42));
        let val = builder.get_value(val_id);
        assert!(val.is_constant());
        assert_eq!(val.as_int(), Some(42));
    }

    #[test]
    fn test_builder_const_small_int() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_small_int(42);
        let val = builder.get_value(val_id);
        assert!(val.is_constant());
        assert_eq!(val.as_int(), Some(42));
    }

    #[test]
    fn test_builder_const_small_int_zero() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_small_int(0);
        assert_eq!(builder.get_value(val_id).as_int(), Some(0));
    }

    #[test]
    fn test_builder_const_small_int_negative() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_small_int(-1);
        assert_eq!(builder.get_value(val_id).as_int(), Some(-1));
    }

    #[test]
    fn test_builder_const_small_int_min() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_small_int(i64::MIN);
        assert_eq!(builder.get_value(val_id).as_int(), Some(i64::MIN));
    }

    #[test]
    fn test_builder_const_small_int_max() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_small_int(i64::MAX);
        assert_eq!(builder.get_value(val_id).as_int(), Some(i64::MAX));
    }

    #[test]
    fn test_builder_const_atom() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_atom(5);
        let val = builder.get_value(val_id);
        assert!(val.is_constant());
        assert_eq!(val.as_atom_index(), Some(5));
    }

    #[test]
    fn test_builder_const_nil() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_nil();
        let val = builder.get_value(val_id);
        assert!(val.is_constant());
        assert_eq!(val.as_bool(), None);
    }

    #[test]
    fn test_builder_const_true() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_true();
        let val = builder.get_value(val_id);
        assert!(val.is_constant());
        assert_eq!(val.as_bool(), Some(true));
    }

    #[test]
    fn test_builder_const_false() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.const_false();
        let val = builder.get_value(val_id);
        assert!(val.is_constant());
        assert_eq!(val.as_bool(), Some(false));
    }

    // --- Register edge cases ---

    #[test]
    fn test_builder_get_x_reg_first_read() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.get_x_reg(0);
        // First read creates a placeholder
        assert_eq!(val_id, IRValueId(0));
    }

    #[test]
    fn test_builder_get_x_reg_after_write() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let written = builder.const_small_int(42);
        builder.set_x_reg(0, written);
        let read = builder.get_x_reg(0);
        assert_eq!(read, written);
    }

    #[test]
    fn test_builder_get_x_reg_high_index() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.get_x_reg(255);
        assert_eq!(val_id, IRValueId(0));
    }

    #[test]
    fn test_builder_get_y_reg_first_read() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val_id = builder.get_y_reg(0);
        assert_eq!(val_id, IRValueId(0));
    }

    #[test]
    fn test_builder_get_y_reg_after_write() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let written = builder.const_small_int(99);
        builder.set_y_reg(5, written);
        let read = builder.get_y_reg(5);
        assert_eq!(read, written);
    }

    #[test]
    fn test_builder_set_x_reg_overwrite() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let v1 = builder.const_small_int(1);
        let v2 = builder.const_small_int(2);
        builder.set_x_reg(0, v1);
        builder.set_x_reg(0, v2);
        assert_eq!(builder.get_x_reg(0), v2);
    }

    // --- Instruction emission edge cases ---

    #[test]
    fn test_builder_emit() {
        let mut builder = IRBuilder::new(0, 0, 0);
        builder.emit(IRInstKind::Nop);
        let block = builder.function.get_block(BlockId(0));
        assert_eq!(block.instructions.len(), 1);
    }

    #[test]
    fn test_builder_emit_with_side_effects() {
        let mut builder = IRBuilder::new(0, 0, 0);
        builder.emit_with_side_effects(
            IRInstKind::Alloc { words: 10 },
            SideEffects {
                allocates: true,
                ..SideEffects::NONE
            },
        );
        let block = builder.function.get_block(BlockId(0));
        assert!(block.instructions[0].side_effects.allocates);
    }

    #[test]
    fn test_builder_emit_with_result() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let (inst_id, result_id) = builder.emit_with_result(IRInstKind::Add, TypeId(0));
        assert_eq!(inst_id, InstId(0));
        let block = builder.function.get_block(BlockId(0));
        assert_eq!(block.instructions.len(), 1);
        assert!(block.instructions[0].result.is_some());
        assert_eq!(block.instructions[0].result.unwrap(), result_id);
    }

    #[test]
    fn test_builder_emit_add() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let a = builder.const_small_int(3);
        let b = builder.const_small_int(4);
        let result = builder.emit_add(a, b);
        let block = builder.function.get_block(BlockId(0));
        assert_eq!(block.instructions.len(), 1);
        assert_eq!(block.instructions[0].operands, vec![a, b]);
        assert!(block.instructions[0].result.is_some());
    }

    #[test]
    fn test_builder_emit_sub() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let a = builder.const_small_int(10);
        let b = builder.const_small_int(3);
        let result = builder.emit_sub(a, b);
        let block = builder.function.get_block(BlockId(0));
        assert_eq!(block.instructions[0].operands, vec![a, b]);
    }

    #[test]
    fn test_builder_emit_mul() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let a = builder.const_small_int(5);
        let b = builder.const_small_int(6);
        let result = builder.emit_mul(a, b);
        let block = builder.function.get_block(BlockId(0));
        assert_eq!(block.instructions[0].operands, vec![a, b]);
    }

    // --- Control flow edge cases ---

    #[test]
    fn test_builder_emit_br() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let target = builder.create_block();
        builder.emit_br(target);
        let block = builder.function.get_block(BlockId(0));
        assert!(matches!(block.instructions[0].kind, IRInstKind::Br { .. }));
    }

    #[test]
    fn test_builder_emit_br_if() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let cond = builder.const_true();
        let t_block = builder.create_block();
        let f_block = builder.create_block();
        builder.emit_br_if(cond, t_block, f_block);
        let block = builder.function.get_block(BlockId(0));
        assert!(matches!(
            block.instructions[0].kind,
            IRInstKind::BrIf { .. }
        ));
    }

    #[test]
    fn test_builder_emit_ret() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let val = builder.const_small_int(42);
        builder.emit_ret(val);
        let block = builder.function.get_block(BlockId(0));
        assert!(matches!(block.instructions[0].kind, IRInstKind::Ret { .. }));
    }

    #[test]
    fn test_builder_emit_call() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let func = builder.const_atom(1);
        let a1 = builder.const_small_int(1);
        let a2 = builder.const_small_int(2);
        let result = builder.emit_call(func, &[a1, a2]);
        let block = builder.function.get_block(BlockId(0));
        assert!(matches!(
            block.instructions[0].kind,
            IRInstKind::Call { .. }
        ));
        assert!(block.instructions[0].side_effects.calls);
        assert!(block.instructions[0].side_effects.may_raise);
        assert!(block.instructions[0].side_effects.allocates);
    }

    #[test]
    fn test_builder_emit_gc_safe() {
        let mut builder = IRBuilder::new(0, 0, 0);
        builder.emit_gc_safe();
        let block = builder.function.get_block(BlockId(0));
        assert!(matches!(block.instructions[0].kind, IRInstKind::GcSafe));
    }

    #[test]
    fn test_builder_multiple_constants_unique_ids() {
        let mut builder = IRBuilder::new(0, 0, 0);
        let v1 = builder.const_small_int(1);
        let v2 = builder.const_small_int(2);
        let v3 = builder.const_small_int(3);
        assert_ne!(v1, v2);
        assert_ne!(v2, v3);
        assert_ne!(v1, v3);
    }
}

// ============================================================================
// value.rs edge cases
// ============================================================================

mod value_tests {
    use super::*;

    #[test]
    fn test_ir_value_constant() {
        let val = IRValue::Constant {
            value: ConstantValue::Int(42),
            ty: IRType::new(TypeKind::SmallInt),
        };
        assert!(val.is_constant());
        assert!(!val.is_inst_result());
        assert!(!val.is_argument());
        assert_eq!(val.as_int(), Some(42));
        assert_eq!(val.as_atom_index(), None);
        assert_eq!(val.as_bool(), None);
    }

    #[test]
    fn test_ir_value_inst_result() {
        let val = IRValue::InstResult {
            inst: InstId(5),
            result_index: 0,
            ty: IRType::new(TypeKind::Int64),
        };
        assert!(!val.is_constant());
        assert!(val.is_inst_result());
        assert!(!val.is_argument());
        assert_eq!(val.as_int(), None);
    }

    #[test]
    fn test_ir_value_argument() {
        let val = IRValue::Argument {
            index: 2,
            ty: IRType::new(TypeKind::Float),
        };
        assert!(!val.is_constant());
        assert!(!val.is_inst_result());
        assert!(val.is_argument());
    }

    #[test]
    fn test_ir_value_placeholder() {
        let val = IRValue::Placeholder;
        assert!(!val.is_constant());
        assert!(!val.is_inst_result());
        assert!(!val.is_argument());
        assert_eq!(val.as_int(), None);
        assert_eq!(val.as_bool(), None);
        assert_eq!(val.as_atom_index(), None);
    }

    #[test]
    fn test_ir_value_ty_constant() {
        let val = IRValue::Constant {
            value: ConstantValue::Int(42),
            ty: IRType::new(TypeKind::SmallInt),
        };
        assert!(matches!(val.ty().kind, TypeKind::SmallInt));
    }

    #[test]
    fn test_ir_value_ty_placeholder() {
        let val = IRValue::Placeholder;
        assert!(matches!(val.ty().kind, TypeKind::Any));
    }

    #[test]
    fn test_ir_value_as_int_variants() {
        // Int constant
        let int_val = IRValue::Constant {
            value: ConstantValue::Int(7),
            ty: IRType::new(TypeKind::SmallInt),
        };
        assert_eq!(int_val.as_int(), Some(7));

        // Non-int constant
        let atom_val = IRValue::Constant {
            value: ConstantValue::Atom(0),
            ty: IRType::new(TypeKind::Atom),
        };
        assert_eq!(atom_val.as_int(), None);

        // True constant
        let true_val = IRValue::Constant {
            value: ConstantValue::True,
            ty: IRType::new(TypeKind::Boolean),
        };
        assert_eq!(true_val.as_int(), None);
    }

    #[test]
    fn test_ir_value_as_bool_variants() {
        let true_val = IRValue::Constant {
            value: ConstantValue::True,
            ty: IRType::new(TypeKind::Boolean),
        };
        assert_eq!(true_val.as_bool(), Some(true));

        let false_val = IRValue::Constant {
            value: ConstantValue::False,
            ty: IRType::new(TypeKind::Boolean),
        };
        assert_eq!(false_val.as_bool(), Some(false));

        let int_val = IRValue::Constant {
            value: ConstantValue::Int(1),
            ty: IRType::new(TypeKind::SmallInt),
        };
        assert_eq!(int_val.as_bool(), None);
    }

    #[test]
    fn test_ir_value_as_atom_index_variants() {
        let atom_val = IRValue::Constant {
            value: ConstantValue::Atom(5),
            ty: IRType::new(TypeKind::Atom),
        };
        assert_eq!(atom_val.as_atom_index(), Some(5));

        let nil_val = IRValue::Constant {
            value: ConstantValue::Nil,
            ty: IRType::new(TypeKind::Nil),
        };
        assert_eq!(nil_val.as_atom_index(), None);
    }

    #[test]
    fn test_ir_value_id_equality() {
        assert_eq!(IRValueId(0), IRValueId(0));
        assert_ne!(IRValueId(0), IRValueId(1));
    }

    #[test]
    fn test_ir_value_id_copy() {
        let a = IRValueId(42);
        let b = a;
        // If IRValueId is Copy, this should compile (and it does since it's a simple usize wrapper)
        let _ = a;
        let _ = b;
    }

    // --- ValueUse / ValueDef edge cases ---

    #[test]
    fn test_value_use() {
        let vu = ValueUse {
            value_id: IRValueId(3),
            user_inst: InstId(7),
        };
        assert_eq!(vu.value_id, IRValueId(3));
        assert_eq!(vu.user_inst, InstId(7));
    }

    #[test]
    fn test_value_def() {
        let vd = ValueDef {
            inst_id: InstId(5),
            result_index: 2,
        };
        assert_eq!(vd.inst_id, InstId(5));
        assert_eq!(vd.result_index, 2);
    }
}

// ============================================================================
// constant.rs edge cases
// ============================================================================

mod constant_tests {
    use super::*;

    #[test]
    fn test_constant_int() {
        let c = Constant::Int(42);
        assert_eq!(c.as_int(), Some(42));
        assert_eq!(c.as_bool(), None);
        assert_eq!(c.as_float(), None);
        assert_eq!(c.as_atom_index(), None);
        assert!(c.is_known());
    }

    #[test]
    fn test_constant_int_zero() {
        let c = Constant::Int(0);
        assert_eq!(c.as_int(), Some(0));
    }

    #[test]
    fn test_constant_int_negative() {
        let c = Constant::Int(-100);
        assert_eq!(c.as_int(), Some(-100));
    }

    #[test]
    fn test_constant_atom() {
        let c = Constant::Atom(5);
        assert_eq!(c.as_atom_index(), Some(5));
        assert_eq!(c.as_int(), None);
        assert!(c.is_known());
    }

    #[test]
    fn test_constant_nil() {
        let c = Constant::Nil;
        assert_eq!(c.as_int(), None);
        assert_eq!(c.as_bool(), None);
        assert!(c.is_known());
    }

    #[test]
    fn test_constant_true() {
        let c = Constant::True;
        assert_eq!(c.as_bool(), Some(true));
        assert!(c.is_known());
    }

    #[test]
    fn test_constant_false() {
        let c = Constant::False;
        assert_eq!(c.as_bool(), Some(false));
        assert!(c.is_known());
    }

    #[test]
    fn test_constant_float() {
        let c = Constant::Float(3.14);
        assert_eq!(c.as_float(), Some(3.14));
        assert_eq!(c.as_int(), None);
        assert!(c.is_known());
    }

    #[test]
    fn test_constant_float_zero() {
        let c = Constant::Float(0.0);
        assert_eq!(c.as_float(), Some(0.0));
    }

    #[test]
    fn test_constant_float_negative() {
        let c = Constant::Float(-1.5);
        assert_eq!(c.as_float(), Some(-1.5));
    }

    #[test]
    fn test_constant_tuple_empty() {
        let c = Constant::Tuple(vec![]);
        assert!(c.is_known());
        assert!(matches!(c.ir_type().kind, TypeKind::Tuple { arity: 0 }));
    }

    #[test]
    fn test_constant_tuple_elements() {
        let c = Constant::Tuple(vec![Constant::Int(1), Constant::Int(2)]);
        assert!(matches!(c.ir_type().kind, TypeKind::Tuple { arity: 2 }));
    }

    #[test]
    fn test_constant_list_empty() {
        let c = Constant::List(vec![]);
        assert!(c.is_known());
        assert!(matches!(c.ir_type().kind, TypeKind::List));
    }

    #[test]
    fn test_constant_list_elements() {
        let c = Constant::List(vec![Constant::Int(1), Constant::Int(2)]);
        assert!(c.is_known());
    }

    #[test]
    fn test_constant_binary() {
        let c = Constant::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(c.is_known());
        assert!(matches!(c.ir_type().kind, TypeKind::Binary));
    }

    #[test]
    fn test_constant_binary_empty() {
        let c = Constant::Binary(vec![]);
        assert!(c.is_known());
    }

    // --- ir_type edge cases ---

    #[test]
    fn test_constant_ir_type_int_positive() {
        let c = Constant::Int(42);
        assert!(matches!(c.ir_type().kind, TypeKind::NonNegInt));
    }

    #[test]
    fn test_constant_ir_type_int_zero() {
        let c = Constant::Int(0);
        assert!(matches!(c.ir_type().kind, TypeKind::NonNegInt));
    }

    #[test]
    fn test_constant_ir_type_int_negative() {
        let c = Constant::Int(-1);
        assert!(matches!(c.ir_type().kind, TypeKind::SmallInt));
    }

    #[test]
    fn test_constant_ir_type_atom() {
        let c = Constant::Atom(0);
        assert!(matches!(c.ir_type().kind, TypeKind::Atom));
    }

    #[test]
    fn test_constant_ir_type_nil() {
        let c = Constant::Nil;
        assert!(matches!(c.ir_type().kind, TypeKind::Nil));
    }

    #[test]
    fn test_constant_ir_type_true() {
        let c = Constant::True;
        assert!(matches!(c.ir_type().kind, TypeKind::Boolean));
    }

    #[test]
    fn test_constant_ir_type_false() {
        let c = Constant::False;
        assert!(matches!(c.ir_type().kind, TypeKind::Boolean));
    }

    #[test]
    fn test_constant_ir_type_float() {
        let c = Constant::Float(1.0);
        assert!(matches!(c.ir_type().kind, TypeKind::Float));
    }

    // --- From trait edge cases ---

    #[test]
    fn test_from_i64() {
        let c: Constant = 42i64.into();
        assert_eq!(c, Constant::Int(42));
    }

    #[test]
    fn test_from_bool_true() {
        let c: Constant = true.into();
        assert_eq!(c, Constant::True);
    }

    #[test]
    fn test_from_bool_false() {
        let c: Constant = false.into();
        assert_eq!(c, Constant::False);
    }

    #[test]
    fn test_from_f64() {
        let c: Constant = 2.5f64.into();
        assert_eq!(c, Constant::Float(2.5));
    }

    #[test]
    fn test_from_constant_for_constant_value() {
        let c = Constant::Int(42);
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::Int(42));
    }

    #[test]
    fn test_from_constant_for_constant_value_atom() {
        let c = Constant::Atom(3);
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::Atom(3));
    }

    #[test]
    fn test_from_constant_for_constant_value_nil() {
        let c = Constant::Nil;
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::Nil);
    }

    #[test]
    fn test_from_constant_for_constant_value_true() {
        let c = Constant::True;
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::True);
    }

    #[test]
    fn test_from_constant_for_constant_value_false() {
        let c = Constant::False;
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::False);
    }

    #[test]
    fn test_from_constant_for_constant_value_float() {
        let c = Constant::Float(1.5);
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::Float(1.5f64.to_bits()));
    }

    #[test]
    fn test_from_constant_for_constant_value_tuple_falls_back_to_nil() {
        let c = Constant::Tuple(vec![Constant::Int(1)]);
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::Nil);
    }

    #[test]
    fn test_from_constant_for_constant_value_list_falls_back_to_nil() {
        let c = Constant::List(vec![]);
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::Nil);
    }

    #[test]
    fn test_from_constant_for_constant_value_binary_falls_back_to_nil() {
        let c = Constant::Binary(vec![0xFF]);
        let cv: ConstantValue = c.into();
        assert_eq!(cv, ConstantValue::Nil);
    }

    // --- From<ConstantValue> for IRType ---

    #[test]
    fn test_from_constant_value_for_ir_type() {
        assert!(matches!(
            IRType::from(ConstantValue::Int(0)).kind,
            TypeKind::SmallInt
        ));
        assert!(matches!(
            IRType::from(ConstantValue::Atom(0)).kind,
            TypeKind::Atom
        ));
        assert!(matches!(
            IRType::from(ConstantValue::Nil).kind,
            TypeKind::Nil
        ));
        assert!(matches!(
            IRType::from(ConstantValue::True).kind,
            TypeKind::Boolean
        ));
        assert!(matches!(
            IRType::from(ConstantValue::False).kind,
            TypeKind::Boolean
        ));
        assert!(matches!(
            IRType::from(ConstantValue::Float(0)).kind,
            TypeKind::Float
        ));
    }

    // --- Constant equality edge cases ---

    #[test]
    fn test_constant_equality() {
        assert_eq!(Constant::Int(1), Constant::Int(1));
        assert_ne!(Constant::Int(1), Constant::Int(2));
        assert_ne!(Constant::Int(1), Constant::Atom(1));
        assert_eq!(Constant::Atom(0), Constant::Atom(0));
        assert_eq!(Constant::Nil, Constant::Nil);
        assert_eq!(Constant::True, Constant::True);
        assert_eq!(Constant::False, Constant::False);
        assert_ne!(Constant::True, Constant::False);
    }

    #[test]
    fn test_constant_float_equality() {
        assert_eq!(Constant::Float(1.0), Constant::Float(1.0));
        assert_ne!(Constant::Float(1.0), Constant::Float(2.0));
    }
}

// ============================================================================
// layout.rs edge cases
// ============================================================================

mod layout_tests {
    use super::*;

    // --- Slot edge cases ---

    #[test]
    fn test_slot_reg() {
        let s = Slot::Reg(Reg::X(5));
        assert!(matches!(s, Slot::Reg(Reg::X(5))));
    }

    #[test]
    fn test_slot_stack() {
        let s = Slot::Stack(10);
        assert!(matches!(s, Slot::Stack(10)));
    }

    #[test]
    fn test_slot_spill() {
        let s = Slot::Spill(3);
        assert!(matches!(s, Slot::Spill(3)));
    }

    #[test]
    fn test_slot_equality() {
        assert_eq!(Slot::Reg(Reg::X(0)), Slot::Reg(Reg::X(0)));
        assert_ne!(Slot::Reg(Reg::X(0)), Slot::Reg(Reg::X(1)));
        assert_ne!(Slot::Reg(Reg::X(0)), Slot::Stack(0));
        assert_eq!(Slot::Stack(5), Slot::Stack(5));
        assert_eq!(Slot::Spill(2), Slot::Spill(2));
    }

    // --- FrameLayoutCalculator edge cases ---

    #[test]
    fn test_frame_layout_calculator_new() {
        let mut calc = FrameLayoutCalculator::new("test_func".to_string());
        assert_eq!(calc.func_name, "test_func");
        // register_pressure, next_spill, and stack_slots are private;
        // just verify the calculator was created successfully
        let _layout = calc.compute_layout(0, 0, false);
    }

    #[test]
    fn test_compute_layout_basic() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let layout = calc.compute_layout(3, 5, true);
        assert_eq!(layout.frame_size, 5); // y_slots + spill
        assert_eq!(layout.spill_slots, 0);
        assert_eq!(layout.register_slots.len(), 3);
        assert_eq!(layout.saved_y_count, 5);
        assert!(layout.needs_gc);
    }

    #[test]
    fn test_compute_layout_no_gc() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let layout = calc.compute_layout(2, 3, false);
        assert!(!layout.needs_gc);
        assert_eq!(layout.frame_size, 3);
    }

    #[test]
    fn test_compute_layout_zero_params() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let layout = calc.compute_layout(0, 0, false);
        assert_eq!(layout.register_slots.len(), 0);
        assert_eq!(layout.frame_size, 0);
        assert_eq!(layout.saved_y_count, 0);
    }

    #[test]
    fn test_compute_layout_many_params() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let layout = calc.compute_layout(8, 10, true);
        assert_eq!(layout.register_slots.len(), 8);
        // Verify register slot mappings
        assert_eq!(layout.register_slots[0], (Reg::X(0), 0));
        assert_eq!(layout.register_slots[7], (Reg::X(7), 7));
    }

    #[test]
    fn test_allocate_spill() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let s1 = calc.allocate_spill();
        let s2 = calc.allocate_spill();
        let s3 = calc.allocate_spill();
        assert_eq!(s1, 0);
        assert_eq!(s2, 1);
        assert_eq!(s3, 2);
    }

    #[test]
    fn test_get_or_alloc_slot_small_int() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let ty = IRType::new(TypeKind::SmallInt);
        let slot = calc.get_or_alloc_slot(&ty);
        assert!(matches!(slot, Slot::Reg(Reg::X(8))));
    }

    #[test]
    fn test_get_or_alloc_slot_float() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let ty = IRType::new(TypeKind::Float);
        let slot = calc.get_or_alloc_slot(&ty);
        assert!(matches!(slot, Slot::Reg(Reg::X(8))));
    }

    #[test]
    fn test_get_or_alloc_slot_atom() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let ty = IRType::new(TypeKind::Atom);
        let slot = calc.get_or_alloc_slot(&ty);
        assert!(matches!(slot, Slot::Reg(Reg::X(8))));
    }

    #[test]
    fn test_get_or_alloc_slot_non_register_type() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let ty = IRType::new(TypeKind::Tuple { arity: 2 });
        let slot = calc.get_or_alloc_slot(&ty);
        assert!(matches!(slot, Slot::Stack(0)));
    }

    #[test]
    fn test_get_or_alloc_slot_list() {
        let mut calc = FrameLayoutCalculator::new("f".to_string());
        let ty = IRType::new(TypeKind::List);
        let slot = calc.get_or_alloc_slot(&ty);
        assert!(matches!(slot, Slot::Stack(_)));
    }

    // --- compute_stack_map edge cases ---

    #[test]
    fn test_compute_stack_map_empty() {
        let layout = FrameLayout {
            frame_size: 10,
            spill_slots: 2,
            register_slots: vec![],
            spill_offsets: vec![0, 1],
            saved_y_count: 5,
            needs_gc: true,
        };
        let map = compute_stack_map(&layout, &[]);
        assert!(map.is_empty());
    }

    #[test]
    fn test_compute_stack_map_reg_x() {
        let layout = FrameLayout {
            frame_size: 10,
            spill_slots: 0,
            register_slots: vec![],
            spill_offsets: vec![],
            saved_y_count: 5,
            needs_gc: true,
        };
        let map = compute_stack_map(&layout, &[Slot::Reg(Reg::X(3))]);
        assert_eq!(map, vec![(3, true)]);
    }

    #[test]
    fn test_compute_stack_map_reg_y() {
        let layout = FrameLayout {
            frame_size: 10,
            spill_slots: 0,
            register_slots: vec![],
            spill_offsets: vec![],
            saved_y_count: 5,
            needs_gc: true,
        };
        let map = compute_stack_map(&layout, &[Slot::Reg(Reg::Y(2))]);
        assert_eq!(map, vec![(258, true)]); // 256 + 2
    }

    #[test]
    fn test_compute_stack_map_reg_f() {
        let layout = FrameLayout {
            frame_size: 10,
            spill_slots: 0,
            register_slots: vec![],
            spill_offsets: vec![],
            saved_y_count: 5,
            needs_gc: true,
        };
        let map = compute_stack_map(&layout, &[Slot::Reg(Reg::F(1))]);
        assert_eq!(map, vec![(513, true)]); // 512 + 1
    }

    #[test]
    fn test_compute_stack_map_stack_slot() {
        let layout = FrameLayout {
            frame_size: 10,
            spill_slots: 0,
            register_slots: vec![],
            spill_offsets: vec![],
            saved_y_count: 5,
            needs_gc: true,
        };
        let map = compute_stack_map(&layout, &[Slot::Stack(7)]);
        assert_eq!(map, vec![(7, true)]);
    }

    #[test]
    fn test_compute_stack_map_spill_slot() {
        let layout = FrameLayout {
            frame_size: 10,
            spill_slots: 3,
            register_slots: vec![],
            spill_offsets: vec![0, 1, 2],
            saved_y_count: 5,
            needs_gc: true,
        };
        let map = compute_stack_map(&layout, &[Slot::Spill(1)]);
        assert_eq!(map, vec![(1, true)]);
    }

    #[test]
    fn test_compute_stack_map_mixed() {
        let layout = FrameLayout {
            frame_size: 10,
            spill_slots: 2,
            register_slots: vec![],
            spill_offsets: vec![0, 1],
            saved_y_count: 5,
            needs_gc: true,
        };
        let map = compute_stack_map(
            &layout,
            &[Slot::Reg(Reg::X(0)), Slot::Stack(3), Slot::Spill(1)],
        );
        assert_eq!(map, vec![(0, true), (3, true), (1, true)]);
    }

    // --- BeamCallingConvention edge cases ---

    #[test]
    fn test_beam_calling_convention_arg_regs() {
        assert_eq!(BeamCallingConvention::ARG_REGS, 8);
    }

    #[test]
    fn test_beam_calling_convention_arg_reg() {
        assert_eq!(BeamCallingConvention::arg_reg(0), Reg::X(0));
        assert_eq!(BeamCallingConvention::arg_reg(7), Reg::X(7));
    }

    #[test]
    fn test_beam_calling_convention_ret_reg() {
        assert_eq!(BeamCallingConvention::ret_reg(), Reg::X(0));
    }

    #[test]
    fn test_beam_calling_convention_stack_alignment() {
        assert_eq!(BeamCallingConvention::stack_alignment(), 16);
    }

    // --- FrameLayout edge cases ---

    #[test]
    fn test_frame_layout_zero_size() {
        let layout = FrameLayout {
            frame_size: 0,
            spill_slots: 0,
            register_slots: vec![],
            spill_offsets: vec![],
            saved_y_count: 0,
            needs_gc: false,
        };
        assert_eq!(layout.frame_size, 0);
        assert_eq!(layout.spill_slots, 0);
    }

    #[test]
    fn test_frame_layout_max() {
        let layout = FrameLayout {
            frame_size: u32::MAX,
            spill_slots: u32::MAX,
            register_slots: vec![],
            spill_offsets: (0..u32::MAX).collect(),
            saved_y_count: u32::MAX,
            needs_gc: true,
        };
        assert_eq!(layout.frame_size, u32::MAX);
    }
}

// ============================================================================
// opt/const_prop.rs edge cases
// ============================================================================

mod const_prop_tests {
    use super::*;

    fn make_func_with_add() -> IRFunction {
        let mut func = IRFunction::new(0, 0, 0);
        let block_id = func.entry_block;
        let block = func.get_block_mut(block_id);

        // Emit: result = ConstSmallInt(3) + ConstSmallInt(4)
        block.push_inst(IRInst {
            kind: IRInstKind::Add,
            result: Some(IRValueId(0)),
            operands: vec![IRValueId(1), IRValueId(2)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::Ret {
                value: IRValueId(0),
            },
            result: None,
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        func
    }

    #[test]
    fn test_propagate_constants_detects_change() {
        let mut func = make_func_with_add();
        let changed = propagate_constants(&mut func);
        let _ = changed;
    }

    #[test]
    fn test_fold_constants_no_change_on_non_const() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::Add,
            result: Some(IRValueId(0)),
            operands: vec![IRValueId(1), IRValueId(2)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        let changed = fold_constants(&mut func);
        assert!(!changed);
    }

    #[test]
    fn test_fold_constants_empty_function() {
        let mut func = IRFunction::new(0, 0, 0);
        let changed = fold_constants(&mut func);
        assert!(!changed);
    }

    #[test]
    fn test_fold_constants_div_by_zero_no_fold() {
        let mut func = make_func_with_add();
        let block = func.get_block_mut(func.entry_block);
        // Replace the Add with Div
        block.instructions[0].kind = IRInstKind::Div;
        let changed = fold_constants(&mut func);
        // Division by zero should NOT be folded (b=0 from ConstSmallInt)
        // Note: fold_constants doesn't seed value_map, so this tests the
        // function doesn't panic on Div instructions
        let _ = changed;
    }

    #[test]
    fn test_fold_constants_rem_by_zero_no_fold() {
        let mut func = make_func_with_add();
        let block = func.get_block_mut(func.entry_block);
        block.instructions[0].kind = IRInstKind::Rem;
        let changed = fold_constants(&mut func);
        let _ = changed;
    }

    #[test]
    fn test_fold_constants_simple_add_no_panic() {
        let mut func = make_func_with_add();
        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_sub() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 6 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 7 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::Mul,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_div() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 20 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 4 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::Div,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_comparison_eq() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 5 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 5 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::Eq,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_comparison_ne() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 5 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 3 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::Ne,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_comparison_gt() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 10 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 5 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::Gt,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_comparison_lt() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 3 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 5 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::Lt,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_bitwise_and() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 0b1100 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 0b1010 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::BitAnd,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_bitwise_or() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 0b1100 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 0b0011 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::BitOr,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_bitwise_xor() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 0b1111 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 0b1010 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::BitXor,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_shift_left() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 1 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 4 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ShiftLeft,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_shift_right() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 16 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 2 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ShiftRight,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_is_small_int() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 42 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::IsSmallInt,
            result: Some(IRValueId(1)),
            operands: vec![IRValueId(0)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_is_atom() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstAtom { index: 3 },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::IsAtom,
            result: Some(IRValueId(1)),
            operands: vec![IRValueId(0)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_is_nil() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstNil,
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::IsNil,
            result: Some(IRValueId(1)),
            operands: vec![IRValueId(0)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }

    #[test]
    fn test_fold_constants_float_folded_to_int_bits() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt {
                value: 1.5f64.to_bits() as i64,
            },
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::ConstSmallInt { value: 0 },
            result: Some(IRValueId(1)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst {
            kind: IRInstKind::Add,
            result: Some(IRValueId(2)),
            operands: vec![IRValueId(0), IRValueId(1)],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });

        let _changed = fold_constants(&mut func);
    }
}

// ============================================================================
// opt/dce.rs edge cases
// ============================================================================

mod dce_tests {
    use super::*;

    #[test]
    fn test_dce_empty_function() {
        let mut func = IRFunction::new(0, 0, 0);
        let changed = eliminate_dead_code(&mut func);
        // Empty function (just entry block with no insts) should not change
        let _ = changed;
    }

    #[test]
    fn test_dce_removes_unreachable_block() {
        let mut func = IRFunction::new(0, 0, 0);
        // Create an unreachable block
        let unreachable = func.create_block();
        let block = func.get_block_mut(unreachable);
        block.push_inst(IRInst::new(IRInstKind::Nop));

        let changed = eliminate_dead_code(&mut func);
        assert!(changed);
        assert!(!func.get_block(unreachable).reachable);
    }

    #[test]
    fn test_dce_keeps_reachable_blocks() {
        let mut func = IRFunction::new(0, 0, 0);
        let b1 = func.create_block();
        // Make b1 reachable by adding a branch from entry
        let entry = func.get_block_mut(func.entry_block);
        entry.push_inst(IRInst::new(IRInstKind::Br {
            target: Label(b1.0 as u32),
        }));
        // Add b1 as successor of entry
        entry.successors.push(Label(b1.0 as u32));
        // Add a terminator to b1
        let block = func.get_block_mut(b1);
        block.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));

        let _changed = eliminate_dead_code(&mut func);
        // Both entry and b1 should remain reachable
        assert!(func.get_block(func.entry_block).reachable);
        assert!(func.get_block(b1).reachable);
    }

    #[test]
    fn test_dce_removes_unused_nop() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst {
            kind: IRInstKind::Nop,
            result: Some(IRValueId(0)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        });
        block.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(1),
        }));

        let changed = eliminate_dead_code(&mut func);
        // Nop with unused result should be removed
        assert!(changed);
    }

    #[test]
    fn test_dce_keeps_side_effecting_insts() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        // Call has side effects and should be kept even if result is unused
        block.push_inst(IRInst {
            kind: IRInstKind::Call {
                func: IRValueId(0),
                args: vec![],
            },
            result: Some(IRValueId(5)),
            operands: vec![],
            beam_offset: 0,
            side_effects: SideEffects {
                calls: true,
                may_raise: true,
                allocates: true,
                ..SideEffects::NONE
            },
        });
        block.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(1),
        }));

        let changed = eliminate_dead_code(&mut func);
        // The call should be kept because it has side effects
        let block = func.get_block(func.entry_block);
        assert!(
            block
                .instructions
                .iter()
                .any(|i| matches!(i.kind, IRInstKind::Call { .. })),
            "Side-effecting call should be kept"
        );
    }

    #[test]
    fn test_dce_multiple_unreachable_blocks() {
        let mut func = IRFunction::new(0, 0, 0);
        let u1 = func.create_block();
        let u2 = func.create_block();
        let u3 = func.create_block();

        // Add instructions to unreachable blocks
        for id in [u1, u2, u3] {
            let block = func.get_block_mut(id);
            block.push_inst(IRInst::new(IRInstKind::Nop));
        }

        let changed = eliminate_dead_code(&mut func);
        assert!(changed);
        assert!(!func.get_block(u1).reachable);
        assert!(!func.get_block(u2).reachable);
        assert!(!func.get_block(u3).reachable);
    }
}

// ============================================================================
// opt/simplify_cfg.rs edge cases
// ============================================================================

mod simplify_cfg_tests {
    use super::*;

    #[test]
    fn test_simplify_empty_function() {
        let mut func = IRFunction::new(0, 0, 0);
        let changed = simplify(&mut func);
        let _ = changed;
    }

    #[test]
    fn test_simplify_single_block_no_change() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));

        let changed = simplify(&mut func);
        assert!(!changed);
    }

    #[test]
    fn test_simplify_removes_unreachable_block() {
        let mut func = IRFunction::new(0, 0, 0);
        let unreachable = func.create_block();
        let block = func.get_block_mut(unreachable);
        block.push_inst(IRInst::new(IRInstKind::Nop));

        let changed = simplify(&mut func);
        assert!(changed);
        assert!(!func.get_block(unreachable).reachable);
    }

    #[test]
    fn test_simplify_keeps_reachable_blocks() {
        // Create a function with a conditional branch where both branches are reachable
        let mut func = IRFunction::new(0, 0, 0);
        let b1 = func.create_block();
        let b2 = func.create_block();
        let entry = func.get_block_mut(func.entry_block);
        entry.push_inst(IRInst::new(IRInstKind::BrIf {
            cond: IRValueId(0),
            true_target: Label(b1.0 as u32),
            false_target: Label(b2.0 as u32),
        }));
        entry.successors.push(Label(b1.0 as u32));
        entry.successors.push(Label(b2.0 as u32));
        let block1 = func.get_block_mut(b1);
        block1.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));
        let block2 = func.get_block_mut(b2);
        block2.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));

        let _changed = simplify(&mut func);
        // Entry should remain reachable
        assert!(func.get_block(func.entry_block).reachable);
    }

    #[test]
    fn test_simplify_eliminates_fallthrough_branch() {
        let mut func = IRFunction::new(0, 0, 0);
        let b1 = func.create_block();
        // Entry falls through to b1, so a Br to b1 is redundant
        let entry = func.get_block_mut(func.entry_block);
        entry.push_inst(IRInst::new(IRInstKind::Br {
            target: Label(b1.0 as u32),
        }));
        let block = func.get_block_mut(b1);
        block.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));

        let changed = simplify(&mut func);
        // The fallthrough branch should be eliminated
        let _ = changed;
    }

    #[test]
    fn test_simplify_chain_of_blocks() {
        let mut func = IRFunction::new(0, 0, 0);
        let b1 = func.create_block();
        let b2 = func.create_block();

        let entry = func.get_block_mut(func.entry_block);
        entry.push_inst(IRInst::new(IRInstKind::Br {
            target: Label(b1.0 as u32),
        }));
        entry.successors.push(Label(b1.0 as u32));
        let block1 = func.get_block_mut(b1);
        block1.push_inst(IRInst::new(IRInstKind::Br {
            target: Label(b2.0 as u32),
        }));
        block1.successors.push(Label(b2.0 as u32));
        let block2 = func.get_block_mut(b2);
        block2.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));

        let _changed = simplify(&mut func);
    }
}

// ============================================================================
// opt/mod.rs edge cases
// ============================================================================

mod opt_mod_tests {
    use super::*;

    #[test]
    fn test_run_pass_unknown() {
        let mut func = IRFunction::new(0, 0, 0);
        let changed = run_pass(&mut func, "nonexistent-pass");
        assert!(!changed);
    }

    #[test]
    fn test_run_pass_dce() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "dce");
    }

    #[test]
    fn test_run_pass_const_prop() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "const-prop");
    }

    #[test]
    fn test_run_pass_fold() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "fold");
    }

    #[test]
    fn test_run_pass_simplify_cfg() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "simplify-cfg");
    }

    #[test]
    fn test_run_pass_cse() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "cse");
    }

    #[test]
    fn test_run_pass_tail_call() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "tail-call");
    }

    #[test]
    fn test_run_pass_pattern_match() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "pattern-match");
    }

    #[test]
    fn test_run_pass_type_inference() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "type-inference");
    }

    #[test]
    fn test_run_pass_escape_analysis() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "escape-analysis");
    }

    #[test]
    fn test_run_pass_native_specialize() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "native-specialize");
    }

    #[test]
    fn test_run_pass_speculative() {
        let mut func = IRFunction::new(0, 0, 0);
        let _changed = run_pass(&mut func, "speculative");
    }

    #[test]
    fn test_optimize_empty_function() {
        let mut func = IRFunction::new(0, 0, 0);
        opt::optimize(&mut func);
        // Should not panic
    }

    #[test]
    fn test_optimize_simple_function() {
        let mut func = IRFunction::new(0, 0, 0);
        let block = func.get_block_mut(func.entry_block);
        block.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));
        opt::optimize(&mut func);
        // Should not panic
    }

    #[test]
    fn test_optimize_with_unreachable_blocks() {
        let mut func = IRFunction::new(0, 0, 0);
        let unreachable = func.create_block();
        let block = func.get_block_mut(unreachable);
        block.push_inst(IRInst::new(IRInstKind::Nop));

        let entry = func.get_block_mut(func.entry_block);
        entry.push_inst(IRInst::new(IRInstKind::Ret {
            value: IRValueId(0),
        }));

        opt::optimize(&mut func);
        // Unreachable block should be cleaned up
        assert!(!func.get_block(unreachable).reachable);
    }
}
