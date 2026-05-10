//! Dala AOT Compiler - Command-line interface.
//!
//! This is the main entry point for the `dala_aot` tool.
//! It provides commands for:
//! - Compiling BEAM modules to native code
//! - Inspecting .beam files
//! - Running compiled code

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cli;

use cli::Cli;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        cli::Commands::Compile {
            input,
            output,
            target,
            mode,
            optimize,
        } => {
            println!("Compiling {} -> {}", input.display(), output.display());
            println!(
                "Target: {}, Mode: {:?}, Optimize: {}",
                target, mode, optimize
            );
            // Compilation logic would go here
            Ok(())
        }
        cli::Commands::Inspect { input } => {
            println!("Inspecting {}", input.display());
            // Inspection logic would go here
            Ok(())
        }
        cli::Commands::Run { input, args, mode } => {
            println!("Running {} in {:?} mode", input.display(), mode);
            println!("Args: {:?}", args);
            // Execution logic would go here
            Ok(())
        }
        cli::Commands::Disasm { input } => {
            println!("Disassembling {}", input.display());
            // Disassembly logic would go here
            Ok(())
        }
    }
}
