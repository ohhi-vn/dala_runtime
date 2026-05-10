//! Intrinsics — special runtime functions callable from compiled code.
//!
//! Intrinsics are functions that the compiler knows about and can
//! generate inline code for, rather than making a regular function call.
//! This is critical for performance in the BEAM runtime.

use cranelift::prelude::*;

use dala_ir::{IRInstKind, Reg};
use dala_runtime::{Process, Term};

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
    /// Get the signature for this intrinsic.
    pub fn signature(&self) -> Signature {
        let mut sig = Signature::new(CallConv::SystemV);

        // First parameter is always the process pointer
        sig.params.push(AbiParam::new(types::I64));

        match self {
            Intrinsic::GetProcess
            | Intrinsic::GetReductions
            | Intrinsic::GetHeapPtr
            | Intrinsic::GetStackPtr => {
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::SetReductions
            | Intrinsic::SetHeapPtr
            | Intrinsic::SetStackPtr
            | Intrinsic::GcBarrier => {
                // No return value
            }
            Intrinsic::ShouldYield => {
                sig.returns.push(AbiParam::new(types::I1));
            }
            Intrinsic::IsSmallInt
            | Intrinsic::IsAtom
            | Intrinsic::IsTuple
            | Intrinsic::IsList
            | Intrinsic::IsFloat
            | Intrinsic::IsMap
            | Intrinsic::IsBinary
            | Intrinsic::IsFun
            | Intrinsic::IsPid
            | Intrinsic::IsPort => {
                // Takes a term as second arg
                sig.params.push(AbiParam::new(types::I64));
                sig.returns.push(AbiParam::new(types::I1));
            }
            Intrinsic::TupleElement => {
                // Takes tuple pointer and index
                sig.params.push(AbiParam::new(types::I64)); // tuple
                sig.params.push(AbiParam::new(types::I64)); // index
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::MapGet | Intrinsic::MapPut => {
                // Takes map and key
                sig.params.push(AbiParam::new(types::I64)); // map
                sig.params.push(AbiParam::new(types::I64)); // key
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::BinaryNew => {
                sig.params.push(AbiParam::new(types::I64)); // data
                sig.params.push(AbiParam::new(types::I64)); // size
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::BinaryMatch => {
                sig.params.push(AbiParam::new(types::I64)); // binary
                sig.params.push(AbiParam::new(types::I64)); // offset
                sig.params.push(AbiParam::new(types::I64)); // size
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::ListCons => {
                sig.params.push(AbiParam::new(types::I64)); // head
                sig.params.push(AbiParam::new(types::I64)); // tail
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::ListHead | Intrinsic::ListTail => {
                sig.params.push(AbiParam::new(types::I64)); // list
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::Raise | Intrinsic::Error | Intrinsic::Throw => {
                sig.params.push(AbiParam::new(types::I64)); // reason
                                                            // No return (diverges)
            }
            Intrinsic::Apply => {
                sig.params.push(AbiParam::new(types::I64)); // func
                sig.params.push(AbiParam::new(types::I64)); // args
                sig.params.push(AbiParam::new(types::I64)); // nargs
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::Send => {
                sig.params.push(AbiParam::new(types::I64)); // dest
                sig.params.push(AbiParam::new(types::I64)); // msg
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::Receive => {
                sig.params.push(AbiParam::new(types::I64)); // timeout
                sig.returns.push(AbiParam::new(types::I64));
            }
            Intrinsic::Unreachable => {
                // No return
            }
        }

        sig
    }

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
pub fn emit_intrinsic(
    builder: &mut FunctionBuilder,
    intrinsic: Intrinsic,
    args: &[Value],
) -> Option<Value> {
    match intrinsic {
        Intrinsic::GetProcess => {
            // Process pointer is passed as the first argument
            Some(args[0])
        }
        Intrinsic::GetReductions => {
            // Load reductions field from process struct
            let proc_ptr = args[0];
            let offset = 8 * 3; // Offset to reductions field (after pid, heap_ptr, heap_top)
            let addr = builder.ins().iadd_imm(proc_ptr, offset);
            Some(builder.ins().load(types::I32, MemFlags::trusted(), addr, 0))
        }
        Intrinsic::SetReductions => {
            let proc_ptr = args[0];
            let value = args[1];
            let offset = 8 * 3;
            let addr = builder.ins().iadd_imm(proc_ptr, offset);
            builder.ins().store(MemFlags::trusted(), value, addr, 0);
            None
        }
        Intrinsic::ShouldYield => {
            let proc_ptr = args[0];
            let offset = 8 * 3;
            let addr = builder.ins().iadd_imm(proc_ptr, offset);
            let reds = builder.ins().load(types::I32, MemFlags::trusted(), addr, 0);
            let zero = builder.ins().iconst(types::I32, 0);
            Some(builder.ins().icmp(IntCC::SignedLE, reds, zero))
        }
        Intrinsic::GetHeapPtr => {
            let proc_ptr = args[0];
            let offset = 8 * 1; // heap_ptr field
            let addr = builder.ins().iadd_imm(proc_ptr, offset);
            Some(builder.ins().load(types::I64, MemFlags::trusted(), addr, 0))
        }
        Intrinsic::SetHeapPtr => {
            let proc_ptr = args[0];
            let value = args[1];
            let offset = 8 * 1;
            let addr = builder.ins().iadd_imm(proc_ptr, offset);
            builder.ins().store(MemFlags::trusted(), value, addr, 0);
            None
        }
        Intrinsic::GetStackPtr => {
            let proc_ptr = args[0];
            let offset = 8 * 2; // stack_ptr field
            let addr = builder.ins().iadd_imm(proc_ptr, offset);
            Some(builder.ins().load(types::I64, MemFlags::trusted(), addr, 0))
        }
        Intrinsic::SetStackPtr => {
            let proc_ptr = args[0];
            let value = args[1];
            let offset = 8 * 2;
            let addr = builder.ins().iadd_imm(proc_ptr, offset);
            builder.ins().store(MemFlags::trusted(), value, addr, 0);
            None
        }
        Intrinsic::GcBarrier => {
            // GC barrier is a compiler hint - no code needed in most cases
            // In a real implementation, this would ensure all writes are visible
            None
        }
        Intrinsic::IsSmallInt => {
            let term = args[1];
            // Check if (term & 0xF) == 0xF (IMMED1 tag)
            // and ((term >> 28) & 0xF) == 0x0 (SMALL subtag)
            let tag_mask = builder.ins().iconst(types::I64, 0xF);
            let tag = builder.ins().band(term, tag_mask);
            let is_immed1 =
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 0xF));
            let subtag_mask = builder.ins().iconst(types::I64, 0xF << 28);
            let subtag = builder.ins().band(term, subtag_mask);
            let is_small =
                builder
                    .ins()
                    .icmp(IntCC::Equal, subtag, builder.ins().iconst(types::I64, 0));
            Some(builder.ins().band(is_immed1, is_small))
        }
        Intrinsic::IsAtom => {
            let term = args[1];
            // Check if (term & 0xF000000F) == 0xF0000003
            // IMMED1_IMMED2 | IMMED2_ATOM
            let atom_tag = builder
                .ins()
                .iconst(types::I64, (0x0F << 28) | (0x03 << 25) | 0x0F);
            let masked = builder.ins().band_imm(term, ((1 << 30) - 1) | 0xF);
            Some(builder.ins().icmp(IntCC::Equal, masked, atom_tag))
        }
        Intrinsic::IsTuple => {
            let term = args[1];
            // Primary tag == 0b10 (HEADER)
            let tag = builder.ins().band_imm(term, 3);
            Some(
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 2)),
            )
        }
        Intrinsic::IsList => {
            let term = args[1];
            // Primary tag == 0b01 (LIST)
            let tag = builder.ins().band_imm(term, 3);
            Some(
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 1)),
            )
        }
        Intrinsic::IsFloat => {
            let term = args[1];
            // Primary tag == 0b10 (HEADER) and header tag == HEADER_FLOAT
            let tag = builder.ins().band_imm(term, 3);
            let is_header =
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 2));
            // Further check header tag... simplified
            Some(is_header)
        }
        Intrinsic::IsMap => {
            let term = args[1];
            let tag = builder.ins().band_imm(term, 3);
            let is_header =
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 2));
            Some(is_header)
        }
        Intrinsic::IsBinary => {
            let term = args[1];
            let tag = builder.ins().band_imm(term, 3);
            let is_header =
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 2));
            Some(is_header)
        }
        Intrinsic::IsFun => {
            let term = args[1];
            let tag = builder.ins().band_imm(term, 3);
            let is_header =
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 2));
            Some(is_header)
        }
        Intrinsic::IsPid => {
            let term = args[1];
            // Check if (term & 0xF000000F) == 0xF0000001
            let tag = builder.ins().band_imm(term, 0xF);
            let is_immed1 =
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 0xF));
            let subtag = builder.ins().band_imm(term, 0xF << 28);
            let is_pid = builder.ins().icmp(
                IntCC::Equal,
                subtag,
                builder.ins().iconst(types::I64, 0x1 << 28),
            );
            Some(builder.ins().band(is_immed1, is_pid))
        }
        Intrinsic::IsPort => {
            let term = args[1];
            let tag = builder.ins().band_imm(term, 0xF);
            let is_immed1 =
                builder
                    .ins()
                    .icmp(IntCC::Equal, tag, builder.ins().iconst(types::I64, 0xF));
            let subtag = builder.ins().band_imm(term, 0xF << 28);
            let is_port = builder.ins().icmp(
                IntCC::Equal,
                subtag,
                builder.ins().iconst(types::I64, 0x2 << 28),
            );
            Some(builder.ins().band(is_immed1, is_port))
        }
        Intrinsic::TupleElement => {
            let tuple_ptr = args[1];
            let index = args[2];
            // Tuple layout: [header | elem0 | elem1 | ...]
            // Offset = (1 + index) * 8
            let one = builder.ins().iconst(types::I64, 1);
            let eight = builder.ins().iconst(types::I64, 8);
            let idx_plus_one = builder.ins().iadd(index, one);
            let byte_offset = builder.ins().imul(idx_plus_one, eight);
            let addr = builder.ins().iadd(tuple_ptr, byte_offset);
            Some(builder.ins().load(types::I64, MemFlags::trusted(), addr, 0))
        }
        Intrinsic::MapGet => {
            // Simplified - would need actual map lookup
            let map_ptr = args[1];
            let _key = args[2];
            // Return a placeholder
            Some(builder.ins().iconst(types::I64, 0))
        }
        Intrinsic::MapPut => {
            // Simplified - would need actual map update
            let _map_ptr = args[1];
            let _key = args[2];
            let _value = args[3];
            Some(builder.ins().iconst(types::I64, 0))
        }
        Intrinsic::BinaryNew => {
            let _proc = args[0];
            let _data = args[1];
            let _size = args[2];
            // Simplified
            Some(builder.ins().iconst(types::I64, 0))
        }
        Intrinsic::BinaryMatch => {
            let _binary = args[1];
            let _offset = args[2];
            let _size = args[3];
            // Simplified
            Some(builder.ins().iconst(types::I64, 0))
        }
        Intrinsic::ListCons => {
            let _head = args[1];
            let _tail = args[2];
            // Simplified - would allocate cons cell on heap
            Some(builder.ins().iconst(types::I64, 0))
        }
        Intrinsic::ListHead => {
            let list_ptr = args[1];
            // Load head from cons cell (first word)
            Some(
                builder
                    .ins()
                    .load(types::I64, MemFlags::trusted(), list_ptr, 0),
            )
        }
        Intrinsic::ListTail => {
            let list_ptr = args[1];
            // Load tail from cons cell (second word)
            let eight = builder.ins().iconst(types::I64, 8);
            let addr = builder.ins().iadd(list_ptr, eight);
            Some(builder.ins().load(types::I64, MemFlags::trusted(), addr, 0))
        }
        Intrinsic::Raise => {
            let _proc = args[0];
            let _reason = args[1];
            // Call runtime exception handler
            // In real implementation: call runtime function
            // For now, trap
            builder.ins().trap(TrapCode::User0);
            None
        }
        Intrinsic::Error => {
            let _proc = args[0];
            let _reason = args[1];
            builder.ins().trap(TrapCode::User0);
            None
        }
        Intrinsic::Throw => {
            let _proc = args[0];
            let _reason = args[1];
            builder.ins().trap(TrapCode::User0);
            None
        }
        Intrinsic::Apply => {
            let _proc = args[0];
            let _func = args[1];
            let _args = args[2];
            let _nargs = args[3];
            // Simplified
            Some(builder.ins().iconst(types::I64, 0))
        }
        Intrinsic::Send => {
            let _proc = args[0];
            let _dest = args[1];
            let _msg = args[2];
            // Simplified
            Some(builder.ins().iconst(types::I64, 0))
        }
        Intrinsic::Receive => {
            let _proc = args[0];
            let _timeout = args[1];
            // Simplified
            Some(builder.ins().iconst(types::I64, 0))
        }
        Intrinsic::Unreachable => {
            builder.ins().trap(TrapCode::UnreachableCode);
            None
        }
    }
}
