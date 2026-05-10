//! Stack and heap layout computation for the BEAM runtime.
//!
//! This module computes the memory layout for compiled functions,
//! including:
//! - Stack frame sizes and offsets
//! - Register assignments
//! - Spill slot allocation
//! - Stack map generation for GC

use crate::instruction::Reg;
use crate::type_system::{IRType, TypeId};

/// Stack frame layout for a compiled function.
#[derive(Debug, Clone)]
pub struct FrameLayout {
    /// Total frame size in words
    pub frame_size: u32,
    /// Number of spill slots
    pub spill_slots: u32,
    /// Register assignments (register -> stack offset)
    pub register_slots: Vec<(Reg, u32)>,
    /// Spill slot offsets
    pub spill_offsets: Vec<u32>,
    /// Saved Y register count
    pub saved_y_count: u32,
    /// Whether this frame needs GC safepoints
    pub needs_gc: bool,
}

/// A stack slot - either a register or a stack location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// A register
    Reg(Reg),
    /// A stack slot (offset from frame base)
    Stack(u32),
    /// A spill slot
    Spill(u32),
}

/// Layout calculator for function frames.
pub struct FrameLayoutCalculator {
    /// The function being laid out
    pub func_name: String,
    /// Register pressure for each physical register
    register_pressure: Vec<u32>,
    /// Spill slot counter
    next_spill: u32,
    /// Stack-allocated values
    stack_slots: Vec<Slot>,
}

impl FrameLayoutCalculator {
    /// Create a new layout calculator.
    pub fn new(func_name: String) -> Self {
        Self {
            func_name,
            register_pressure: vec![0; 256],
            next_spill: 0,
            stack_slots: Vec::new(),
        }
    }

    /// Compute the layout for a function.
    pub fn compute_layout(
        &mut self,
        num_x_params: usize,
        num_y_slots: usize,
        needs_gc: bool,
    ) -> FrameLayout {
        // Allocate X register parameters
        for i in 0..num_x_params {
            self.register_pressure[i] += 1;
        }

        // Allocate Y stack slots
        for i in 0..num_y_slots {
            self.stack_slots.push(Slot::Stack(i as u32));
        }

        // Calculate frame size
        let frame_size = num_y_slots as u32 + self.next_spill;

        // Build register slot mappings
        let register_slots: Vec<(Reg, u32)> = (0..num_x_params)
            .map(|i| (Reg::X(i as u32), i as u32))
            .collect();

        FrameLayout {
            frame_size,
            spill_slots: self.next_spill,
            register_slots,
            spill_offsets: (0..self.next_spill).collect(),
            saved_y_count: num_y_slots as u32,
            needs_gc,
        }
    }

    /// Allocate a spill slot.
    pub fn allocate_spill(&mut self) -> u32 {
        let slot = self.next_spill;
        self.next_spill += 1;
        slot
    }

    /// Get the slot for a value, allocating if necessary.
    pub fn get_or_alloc_slot(&mut self, value_type: &IRType) -> Slot {
        // Try to use a register first
        if let IRType::SmallInt | IRType::NonNegInt | IRType::Float | IRType::Atom = value_type {
            // Find least-pressure register
            if let Some(reg_idx) = self.find_lowest_pressure_reg() {
                self.register_pressure[reg_idx] += 1;
                return Slot::Reg(Reg::X(reg_idx as u32));
            }
        }

        // Fall back to stack
        let offset = self.stack_slots.len() as u32;
        self.stack_slots.push(Slot::Stack(offset));
        Slot::Stack(offset)
    }

    /// Find the register with the lowest pressure.
    fn find_lowest_pressure_reg(&self) -> Option<usize> {
        // Skip X0-X7 (param/result registers in BEAM calling convention)
        (8..256).min_by_key(|&i| self.register_pressure[i])
    }
}

/// Compute the GC stack map for a function's frame layout.
pub fn compute_stack_map(layout: &FrameLayout, live_at_safepoint: &[Slot]) -> Vec<(u32, bool)> {
    let mut map = Vec::new();

    for slot in live_at_safepoint {
        match slot {
            Slot::Reg(reg) => {
                let offset = match reg {
                    Reg::X(i) => *i,
                    Reg::Y(i) => 256 + i, // Y registers after X in the frame
                    Reg::F(i) => 512 + i, // F registers after Y
                };
                map.push((offset, true)); // true = pointer
            }
            Slot::Stack(offset) => {
                map.push((*offset, true));
            }
            Slot::Spill(offset) => {
                map.push((*offset, true));
            }
        }
    }

    map
}

/// The calling convention for BEAM functions.
///
/// BEAM uses a custom calling convention:
/// - X0-X7: First 8 arguments / return value
/// - X8-X255: General purpose
/// - Y0-Y*: Stack frame slots (callee-saved)
/// - F0-F255: Floating point registers
/// - CP: Continuation pointer (return address)
/// - HEAP: Heap pointer (caller-saved)
/// - STACK: Stack pointer (callee-saved)
pub struct BeamCallingConvention;

impl BeamCallingConvention {
    /// Number of argument registers
    pub const ARG_REGS: usize = 8;

    /// Get the register for argument i.
    pub fn arg_reg(i: usize) -> Reg {
        Reg::X(i as u32)
    }

    /// Get the return value register.
    pub fn ret_reg() -> Reg {
        Reg::X(0)
    }

    /// Get the stack frame alignment requirement (in words).
    pub fn stack_alignment() -> usize {
        16 // Must be 16-word aligned for SIMD
    }
}
