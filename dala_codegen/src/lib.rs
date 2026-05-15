//! Dala Codegen - Native code generation using Cranelift.
//!
//! This crate translates Dala IR into native machine code using the
//! Cranelift code generator. It supports both JIT (for desktop/Android)
//! and AOT (for iOS/restricted environments) compilation modes.
//!
//! Architecture:
//!   Dala IR -> Cranelift IR -> Machine code

pub mod compiler;
pub mod intrinsics;
pub mod runtime_glue;
pub mod stack_map;
pub mod trap_sink;

// Re-exports
pub use compiler::Compiler;
pub use intrinsics::Intrinsic;
pub use runtime_glue::RuntimeGlue;
pub use stack_map::StackMapRegistry;
pub use trap_sink::TrapSink;

/// Compilation target (JIT or AOT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationMode {
    /// JIT compilation for immediate execution
    Jit,
    /// AOT compilation for ahead-of-time deployment
    Aot,
}

/// Code generator configuration.
#[derive(Debug, Clone)]
pub struct CodegenConfig {
    /// Compilation mode
    pub mode: CompilationMode,
    /// Target architecture
    pub target: String,
    /// Optimization level
    pub opt_level: &'static str,
    /// Enable debug assertions
    pub debug_assertions: bool,
    /// Enable verbose logging
    pub verbose: bool,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            mode: CompilationMode::Jit,
            target: "x86_64".to_string(),
            opt_level: "speed",
            debug_assertions: false,
            verbose: false,
        }
    }
}

/// A compiled function ready for execution.
#[repr(C)]
pub struct CompiledFunction {
    /// The native code pointer
    pub code_ptr: *const u8,
    /// The size of the compiled code
    pub code_size: usize,
    /// Stack map for GC
    pub stack_map: Option<Vec<u8>>,
    /// Frame size
    pub frame_size: usize,
    /// Number of spills
    pub spill_count: usize,
}

impl CompiledFunction {
    /// Get the function as a callable pointer.
    pub fn as_fn(&self) -> Option<unsafe extern "C" fn()> {
        if self.code_ptr.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute(self.code_ptr) })
        }
    }
}

/// A code generator that translates Dala IR to native code.
pub struct CodeGenerator {
    /// Configuration
    #[allow(dead_code)]
    config: CodegenConfig,
}

impl CodeGenerator {
    /// Create a new code generator.
    pub fn new(config: CodegenConfig) -> Result<Self, CodegenError> {
        Ok(Self { config })
    }

    /// Compile an IR function to native code.
    ///
    /// NOTE: This is a stub implementation. Full Cranelift codegen
    /// requires significant API updates for the current Cranelift version.
    pub fn compile_function(
        &mut self,
        ir_func: &dala_ir::IRFunction,
    ) -> Result<CompiledFunction, CodegenError> {
        log::info!(
            "Codegen stub: compiling function {} ({} instructions)",
            ir_func.full_name(),
            ir_func
                .blocks
                .iter()
                .map(|b| b.instructions.len())
                .sum::<usize>()
        );

        // Return a null pointer as placeholder
        // Full implementation would use Cranelift to generate native code
        Ok(CompiledFunction {
            code_ptr: std::ptr::null(),
            code_size: 0,
            stack_map: None,
            frame_size: 0,
            spill_count: 0,
        })
    }
}

/// Code generation errors.
#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    #[error("target error: {0}")]
    TargetError(String),
    #[error("compilation error: {0}")]
    CompilationError(String),
    #[error("unsupported feature: {0}")]
    Unsupported(String),
    #[error("link error: {0}")]
    LinkError(String),
}
