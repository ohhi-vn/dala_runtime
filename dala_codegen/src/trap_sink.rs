//! Trap sink - collects trap/catch site information during code generation.
//!
//! NOTE: Simplified stub. Full implementation would use Cranelift's trap API.

/// A trap site recorded during code generation.
#[derive(Debug, Clone)]
pub struct TrapSite {
    pub offset: u32,
    pub trap_code: u32,
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

    pub fn trap(&mut self, _ebb: (), trap_code: u32, beam_offset: u32) {
        self.traps.push(TrapSite {
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
