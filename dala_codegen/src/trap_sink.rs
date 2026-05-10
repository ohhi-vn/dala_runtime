//! Trap sink - collects trap/catch site information during code generation.

use cranelift::ir;

/// A trap site recorded during code generation.
#[derive(Debug, Clone)]
pub struct TrapSite {
    pub ebb: ir::Ebb,
    pub offset: u32,
    pub trap_code: ir::TrapCode,
    pub beam_offset: u32,
}

/// Collects trap sites during code generation.
#[derive(Default)]
pub struct TrapSink {
    traps: Vec<TrapSite>,
}

impl TrapSink {
    pub fn new() -> Self {
        Self { traps: Vec::new() }
    }

    pub fn trap(&mut self, ebb: ir::Ebb, trap_code: ir::TrapCode, beam_offset: u32) {
        self.traps.push(TrapSite {
            ebb,
            offset: 0,
            trap_code,
            beam_offset,
        });
    }

    pub fn traps(&self) -> &[TrapSite] {
        &self.traps
    }

    pub fn clear(&mut self) {
        self.traps.clear();
    }

    pub fn len(&self) -> usize {
        self.traps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.traps.is_empty()
    }
}

impl std::ops::Deref for TrapSink {
    type Target = [TrapSite];

    fn deref(&self) -> &Self::Target {
        &self.traps
    }
}
