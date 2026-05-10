//! Compiler driver - orchestrates the compilation pipeline.
//!
//! This module provides the high-level `Compiler` struct that ties together
//! the BEAM loader, IR builder, optimizer, and code generator.

use std::path::Path;

use dala_beam_loader::{BeamModule, load_beam_bytes, load_beam_file};
use dala_ir::opt;
use dala_ir::{IRBuilder, IRFunction, IRModule};
use dala_runtime::code::ModuleCode;
use dala_runtime::{CodePtr, CodeRegistry, FunctionEntry};

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

    /// Compile a BEAM module from a file path.
    pub fn compile_file<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<Vec<crate::CompiledFunction>, String> {
        let beam_module = load_beam_file(path.as_ref())
            .map_err(|e| format!("Failed to load BEAM file: {:?}", e))?;
        self.compile_beam_module(&beam_module)
    }

    /// Compile a BEAM module from bytes.
    pub fn compile_bytes(&mut self, data: &[u8]) -> Result<Vec<crate::CompiledFunction>, String> {
        let beam_module =
            load_beam_bytes(data).map_err(|e| format!("Failed to load BEAM bytes: {:?}", e))?;
        self.compile_beam_module(&beam_module)
    }

    /// Compile a loaded BEAM module.
    pub fn compile_beam_module(
        &mut self,
        beam_module: &BeamModule,
    ) -> Result<Vec<crate::CompiledFunction>, String> {
        // Translate BEAM bytecode to IR
        let mut ir_module = self.translate_to_ir(beam_module)?;

        // Run optimization passes on each function
        for func in &mut ir_module.function_bodies {
            opt::optimize(func);
        }

        // Compile each function
        let mut compiled_functions = Vec::new();
        for func in &ir_module.function_bodies {
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

    /// Translate a BEAM module to IR.
    fn translate_to_ir(&mut self, beam_module: &BeamModule) -> Result<IRModule, String> {
        let module_name = 0u64; // TODO: use actual atom index
        let mut ir_module = IRModule::new(module_name);

        // Register exports
        for (name, arity, _label) in &beam_module.exports {
            ir_module.add_export(*name, *arity);
        }

        // Translate each function
        for ((name, arity), beam_func) in &beam_module.functions {
            let func_id = ir_module.add_function(*name, *arity);
            let func = ir_module.get_function_body_mut(func_id);

            // Set source info
            func.file = 0;
            func.line = 0;

            // Build the IR from BEAM instructions
            self.translate_function(func, beam_func)?;
        }

        Ok(ir_module)
    }

    /// Translate a single BEAM function to IR.
    fn translate_function(
        &self,
        func: &mut IRFunction,
        beam_func: &dala_beam_loader::BeamFunction,
    ) -> Result<(), String> {
        let mut builder = IRBuilder::new(func.module, func.name, func.arity);

        // For now, emit a simple entry block that returns nil
        // Full translation would map each BEAM instruction to IR
        builder.emit_ret(builder.const_nil());

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
        let mut mc = self
            .code_registry
            .modules
            .write()
            .unwrap()
            .remove(&module)
            .unwrap_or_else(|| ModuleCode::new(module));
        mc.add_function(entry);
        self.code_registry
            .modules
            .write()
            .unwrap()
            .insert(module, mc);
    }
}
