//! IR Optimizations - transforms to improve native code quality.
//!
//! This module implements standard compiler optimizations on the SSA IR:
//! - Dead code elimination
//! - Constant folding & propagation
//! - Common subexpression elimination (CSE)
//! - Branch optimization
//! - Inlining (simple function inlining)
//!
//! Each optimization pass is designed to be composable and idempotent.

pub mod const_prop;
pub mod cse;
pub mod dce;
pub mod pattern_match;
pub mod simplify_cfg;
pub mod tail_call;
pub mod validation;

use crate::function::IRFunction;

/// Run all optimization passes on a function.
pub fn optimize(func: &mut IRFunction) {
    // Run passes iteratively until convergence
    let mut changed = true;
    let mut iteration = 0;
    let max_iterations = 10;

    while changed && iteration < max_iterations {
        changed = false;
        iteration += 1;

        // Dead code elimination
        if dce::eliminate_dead_code(func) {
            changed = true;
        }

        // Constant propagation
        if const_prop::propagate_constants(func) {
            changed = true;
        }

        // Constant folding
        if const_prop::fold_constants(func) {
            changed = true;
        }

        // Common subexpression elimination
        if cse::eliminate_common_subexprs(func) {
            changed = true;
        }

        // CFG simplification
        if simplify_cfg::simplify(func) {
            changed = true;
        }

        // Tail call analysis
        if tail_call::analyze(func) {
            changed = true;
        }

        // Pattern matching optimization
        if pattern_match::optimize(func) {
            changed = true;
        }
    }

    log::debug!("Optimization converged after {} iterations", iteration);
}

/// Run a single optimization pass (for debugging/analysis).
pub fn run_pass(func: &mut IRFunction, pass_name: &str) -> bool {
    match pass_name {
        "dce" => dce::eliminate_dead_code(func),
        "const-prop" => const_prop::propagate_constants(func),
        "fold" => const_prop::fold_constants(func),
        "cse" => cse::eliminate_common_subexprs(func),
        "simplify-cfg" => simplify_cfg::simplify(func),
        "tail-call" => tail_call::analyze(func),
        "pattern-match" => pattern_match::optimize(func),
        _ => {
            log::warn!("Unknown optimization pass: {}", pass_name);
            false
        }
    }
}
