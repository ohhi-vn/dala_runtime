//! Dala AOT Compiler - Command-line interface.
//!
//! This is the main entry point for the `dala_aot` tool.
//! It orchestrates the full compilation pipeline:
//!
//!   .beam file → BEAM loader → IR translation → Optimization → Codegen → Output

use std::fs;
use std::path::PathBuf;

use clap::Parser;
use tracing::{error, info};

mod cli;
mod pipeline;

use cli::{Cli, Commands, CompilationMode, OptLevel};
use pipeline::{Mode as PipelineMode, OptLevel as PipelineOptLevel, Pipeline, PipelineConfig};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("DALA_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Compile {
            input,
            output,
            target,
            mode,
            optimize,
        } => {
            let config = PipelineConfig {
                input: input.clone(),
                output: output.clone(),
                target: target.clone(),
                mode: match mode {
                    CompilationMode::Jit => PipelineMode::Jit,
                    CompilationMode::Aot => PipelineMode::Aot,
                },
                opt_level: match optimize {
                    OptLevel::None => PipelineOptLevel::None,
                    OptLevel::Less => PipelineOptLevel::Less,
                    OptLevel::Default => PipelineOptLevel::Default,
                    OptLevel::Aggressive => PipelineOptLevel::Aggressive,
                },
            };

            info!("Dala AOT Compiler");
            info!("  Input:  {}", input.display());
            info!("  Output: {}", output.display());
            info!("  Target: {}", target);
            info!("  Mode:   {:?}", mode);
            info!("  Opt:    {:?}", optimize);

            let mut pipeline = Pipeline::new(config);
            match pipeline.run() {
                Ok(stats) => {
                    info!("Compilation succeeded!");
                    info!("  Functions compiled: {}", stats.functions_compiled);
                    info!("  Total code size:    {} bytes", stats.total_code_size);
                    info!("  Optimization passes: {}", stats.opt_passes_run);
                    Ok(())
                }
                Err(e) => {
                    error!("Compilation failed: {}", e);
                    Err(anyhow::anyhow!(e))
                }
            }
        }
        Commands::Inspect { input } => {
            info!("Inspecting {}", input.display());
            let data = fs::read(&input)
                .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", input.display(), e))?;

            let beam_module = dala_beam_loader::load_beam_bytes(&data)
                .map_err(|e| anyhow::anyhow!("Failed to parse BEAM: {}", e))?;

            println!("Module: {}", beam_module.name);
            println!("Atoms:  {}", beam_module.atoms.len());
            println!("Functions: {}", beam_module.function_count());
            println!();
            println!("Exports:");
            for (name, arity, label) in &beam_module.exports {
                println!("  {}/{} @ label {}", name, arity, label);
            }
            println!();
            println!("Functions:");
            for ((name, arity), func) in &beam_module.functions {
                println!("  {}/{} ({} instructions)", name, arity, func.code.len());
            }

            Ok(())
        }
        Commands::Run { input, args, mode } => {
            info!("Running {} in {:?} mode", input.display(), mode);
            info!("Args: {:?}", args);

            let data = fs::read(&input)
                .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", input.display(), e))?;

            let beam_module = dala_beam_loader::load_beam_bytes(&data)
                .map_err(|e| anyhow::anyhow!("Failed to parse BEAM: {}", e))?;

            // Translate to IR
            let ir_module = pipeline::translate_beam_to_ir(&beam_module)
                .map_err(|e| anyhow::anyhow!("IR translation failed: {}", e))?;

            // Optimize
            let mut optimized = ir_module.clone();
            let opt_stats = pipeline::optimize_module(&mut optimized);

            info!(
                "Optimization: {} passes run, {} iterations",
                opt_stats.passes_run, opt_stats.iterations
            );

            // Compile
            let compiled = pipeline::compile_module(
                &optimized,
                &PipelineConfig {
                    input: input.clone(),
                    output: PathBuf::from("/dev/null"),
                    target: "host".to_string(),
                    mode: PipelineMode::Mixed,
                    opt_level: PipelineOptLevel::Default,
                },
            )
            .map_err(|e| anyhow::anyhow!("Compilation failed: {}", e))?;

            info!("Compiled {} functions", compiled.functions_compiled);
            info!("Note: Full execution requires a running Dala runtime.");
            info!("      Use `dala_aot compile` to produce a native binary.");

            Ok(())
        }
        Commands::Disasm { input } => {
            info!("Disassembling {}", input.display());
            let data = fs::read(&input)
                .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", input.display(), e))?;

            let beam_module = dala_beam_loader::load_beam_bytes(&data)
                .map_err(|e| anyhow::anyhow!("Failed to parse BEAM: {}", e))?;

            println!("Module: {}", beam_module.name);
            println!("═══════════════════════════════════════════════════════════");

            for ((name, arity), func) in &beam_module.functions {
                println!("\n  {}/{} ({} instructions)", name, arity, func.code.len());
                println!("  ─────────────────────────────────────────────────────");
                for (i, inst) in func.code.iter().enumerate() {
                    println!(
                        "    {:4}: opcode={} operands={:?}",
                        i, inst.opcode, inst.operands
                    );
                }
            }

            Ok(())
        }
    }
}
