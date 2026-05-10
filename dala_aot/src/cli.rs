//! CLI argument definitions for dala_aot.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Dala AOT Compiler - Compile BEAM bytecode to native machine code.
#[derive(Parser)]
#[command(name = "dala_aot")]
#[command(about = "AOT compiler for BEAM (Erlang/OTP) bytecode", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile a BEAM file to native code
    Compile {
        /// Input .beam file path
        #[arg(short, long)]
        input: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Target architecture (x86_64, aarch64)
        #[arg(short, long, default_value = "x86_64")]
        target: String,

        /// Compilation mode (jit, aot)
        #[arg(short, long, default_value = "aot")]
        mode: CompilationMode,

        /// Optimization level (none, less, default, aggressive)
        #[arg(short = 'O', long, default_value = "default")]
        optimize: OptLevel,
    },

    /// Inspect a BEAM file (show structure, exports, etc.)
    Inspect {
        /// Input .beam file path
        #[arg(short, long)]
        input: PathBuf,
    },

    /// Run a BEAM module
    Run {
        /// Input .beam file path
        #[arg(short, long)]
        input: PathBuf,

        /// Arguments to pass to the module's main function
        #[arg(last = true)]
        args: Vec<String>,

        /// Execution mode
        #[arg(short, long, default_value = "mixed")]
        mode: ExecutionMode,
    },

    /// Disassemble BEAM bytecode
    Disasm {
        /// Input .beam file path
        #[arg(short, long)]
        input: PathBuf,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum CompilationMode {
    Jit,
    Aot,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum OptLevel {
    None,
    Less,
    Default,
    Aggressive,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ExecutionMode {
    Interpreted,
    Mixed,
    Native,
}
