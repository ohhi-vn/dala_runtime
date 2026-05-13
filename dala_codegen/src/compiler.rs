//! Compiler driver - orchestrates the compilation pipeline.
//!
//! This module provides the high-level `Compiler` struct that ties together
//! the IR builder, optimizer, and code generator.

use dala_ir::opt;
use dala_ir::{IRBuilder, IRFunction, IRModule};
use dala_runtime::code::{CodePtr, CodeRegistry, FunctionEntry, ModuleCode};

use crate::CodeGenerator;

/// The compiler driver.
pub struct Compiler {
    /// Code generator
    codegen: CodeGenerator,
    /// Code registry for runtime code management
    code_registry: CodeRegistry,
}

impl Compiler {
    /// Create a new compiler with the given configuration.
    pub fn new(config: crate::CodegenConfig) -> Result<Self, crate::CodegenError> {
        Ok(Self {
            codegen: CodeGenerator::new(config)?,
            code_registry: CodeRegistry::new(),
        })
    }

    /// Get a reference to the code generator.
    pub fn codegen(&self) -> &CodeGenerator {
        &self.codegen
    }

    /// Get a mutable reference to the code generator.
    pub fn codegen_mut(&mut self) -> &mut CodeGenerator {
        &mut self.codegen
    }

    /// Get the code registry.
    pub fn code_registry(&self) -> &CodeRegistry {
        &self.code_registry
    }

    /// Compile a loaded BEAM module (stub - full implementation would load .beam files).
    pub fn compile_beam_module(
        &mut self,
        ir_module: &IRModule,
    ) -> Result<Vec<crate::CompiledFunction>, String> {
        // Run optimization passes on each function
        let mut optimized_module = ir_module.clone();
        for func in &mut optimized_module.function_bodies {
            opt::optimize(func);
        }

        // Compile each function
        let mut compiled_functions = Vec::new();
        for func in &optimized_module.function_bodies {
            match self.codegen.compile_function(func) {
                Ok(cf) => {
                    compiled_functions.push(cf);
                }
                Err(e) => {
                    log::warn!("Failed to compile function {}: {:?}", func.full_name(), e);
                }
            }
        }

        Ok(compiled_functions)
    }

    /// Translate a single BEAM function to IR (stub).
    pub fn translate_function(&self, func: &mut IRFunction) -> Result<(), String> {
        let mut builder = IRBuilder::new(func.module, func.name, func.arity);

        // For now, emit a simple entry block that returns nil
        let nil_val = builder.const_nil();
        builder.emit_ret(nil_val);

        // Replace the function's blocks with the built IR
        func.blocks = builder.function.blocks;
        func.entry_block = builder.function.entry_block;

        Ok(())
    }

    /// Register compiled code in the code registry.
    pub fn register_code(
        &mut self,
        module: u64,
        name: u64,
        arity: u32,
        code_ptr: CodePtr,
        is_aot: bool,
    ) {
        let entry = FunctionEntry {
            code: code_ptr,
            is_aot,
            module,
            function: name,
            arity,
            file: 0,
            line: 0,
        };
        let mut mc = ModuleCode::new(module);
        mc.add_function(entry);
        self.code_registry.register_module(module, mc);
    }
}
