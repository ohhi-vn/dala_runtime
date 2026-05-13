//! Stack map registry - manages stack maps for GC safepoints.

use std::collections::HashMap;

use dala_ir::{IRFunction, IRInstKind};

/// A registry of stack maps for compiled functions.
#[derive(Default)]
pub struct StackMapRegistry {
    maps: HashMap<u64, Vec<StackMapEntry>>,
}

/// A single stack map entry describing a live slot at a safepoint.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StackMapEntry {
    pub instruction_offset: u32,
    pub live_registers: u64,
    pub live_stack_count: u32,
}

impl StackMapRegistry {
    pub fn new() -> Self {
        Self {
            maps: HashMap::new(),
        }
    }

    pub fn register(&mut self, func_id: u64, maps: Vec<StackMapEntry>) {
        self.maps.insert(func_id, maps);
    }

    pub fn get(&self, func_id: u64) -> Option<&[StackMapEntry]> {
        self.maps.get(&func_id).map(|v| v.as_slice())
    }

    pub fn generate_maps(&mut self, func: &IRFunction) {
        let mut entries = Vec::new();
        for (block_idx, block) in func.blocks.iter().enumerate() {
            for (inst_idx, inst) in block.instructions.iter().enumerate() {
                if matches!(inst.kind, IRInstKind::GcSafe) {
                    entries.push(StackMapEntry {
                        instruction_offset: (block_idx as u32)
                            .saturating_mul(1000)
                            .saturating_add(inst_idx as u32),
                        live_registers: 0,
                        live_stack_count: 0,
                    });
                }
            }
        }
        let func_id = func as *const _ as u64;
        self.maps.insert(func_id, entries);
    }

    pub fn len(&self) -> usize {
        self.maps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}
