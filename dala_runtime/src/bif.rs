//! BIFs (Built-In Functions) - the core BEAM built-in function implementations.
//!
//! BIFs are functions implemented in C (or Rust, in our case) that are
//! callable from Erlang/Elixir code. They include things like:
//! - `erlang:+/2`, `erlang:-/2` (arithmetic)
//! - `erlang:is_integer/1`, `erlang:is_atom/1` (type tests)
//! - `erlang:send/2`, `erlang:self/0` (process operations)
//! - `erlang:spawn/3` (process creation)

use crate::exception::Exception;
use crate::process::Process;
use crate::term::Term;

/// Result type for BIF execution.
pub type BifResult = Result<Term, Exception>;

/// A BIF function signature.
pub type BifFn = unsafe fn(&mut Process, &[Term]) -> BifResult;

/// BIF descriptor.
pub struct BifDescriptor {
    pub module: u32,
    pub function: u32,
    pub arity: u32,
    pub implementation: BifFn,
}

/// Macro for defining a BIF implementation.
#[macro_export]
macro_rules! bif {
    ($module:expr, $func:expr, $arity:expr, $fn:ident) => {
        BifDescriptor {
            module: $module,
            function: $func,
            arity: $arity,
            implementation: $fn,
        }
    };
}

// ===== Helper Functions =====

/// Helper: create a badarg exception.
fn badarg() -> Exception {
    Exception::error(Term::atom(crate::atom::atom("badarg")))
}

/// Helper: create a badarith exception.
fn badarith() -> Exception {
    Exception::error(Term::atom(crate::atom::atom("badarith")))
}

/// Internal macro for arity checking.
macro_rules! check_arity {
    ($args:expr, $expected:expr) => {
        if $args.len() != $expected {
            return Err(badarg());
        }
    };
}

// ===== Arithmetic BIFs =====

/// `erlang:+/2` - integer addition.
pub unsafe fn add_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    let a = args[0].get_small().ok_or_else(|| badarith())?;
    let b = args[1].get_small().ok_or_else(|| badarith())?;
    Ok(Term::small(a + b))
}

/// `erlang:-/2` - integer subtraction.
pub unsafe fn sub_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    let a = args[0].get_small().ok_or_else(|| badarith())?;
    let b = args[1].get_small().ok_or_else(|| badarith())?;
    Ok(Term::small(a - b))
}

/// `erlang:*/2` - integer multiplication.
pub unsafe fn mul_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    let a = args[0].get_small().ok_or_else(|| badarith())?;
    let b = args[1].get_small().ok_or_else(|| badarith())?;
    Ok(Term::small(a * b))
}

/// `erlang:div/2` - integer division.
pub unsafe fn div_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    let a = args[0].get_small().ok_or_else(|| badarith())?;
    let b = args[1].get_small().ok_or_else(|| badarith())?;
    if b == 0 {
        return Err(badarith());
    }
    Ok(Term::small(a / b))
}

/// `erlang:rem/2` - integer remainder.
pub unsafe fn rem_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    let a = args[0].get_small().ok_or_else(|| badarith())?;
    let b = args[1].get_small().ok_or_else(|| badarith())?;
    if b == 0 {
        return Err(badarith());
    }
    Ok(Term::small(a % b))
}

/// `erlang:-/1` - integer negation.
pub unsafe fn neg_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    let a = args[0].get_small().ok_or_else(|| badarith())?;
    Ok(Term::small(-a))
}

// ===== Type Test BIFs =====

/// `erlang:is_integer/1`
pub unsafe fn is_integer_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_small()))
}

/// `erlang:is_atom/1`
pub unsafe fn is_atom_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_atom()))
}

/// `erlang:is_binary/1`
pub unsafe fn is_binary_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_binary()))
}

/// `erlang:is_boolean/1`
pub unsafe fn is_boolean_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_true() || args[0].is_false()))
}

/// `erlang:is_tuple/1`
pub unsafe fn is_tuple_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_tuple()))
}

/// `erlang:is_list/1`
pub unsafe fn is_list_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_list()))
}

/// `erlang:is_pid/1`
pub unsafe fn is_pid_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_pid()))
}

/// `erlang:is_port/1`
pub unsafe fn is_port_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_port()))
}

/// `erlang:is_function/1`
pub unsafe fn is_function_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(
        args[0].is_boxed() && args[0].header_tag() == crate::term::tags::HEADER_FUN,
    ))
}

/// `erlang:is_map/1`
pub unsafe fn is_map_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_map()))
}

/// `erlang:is_number/1`
pub unsafe fn is_number_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(
        args[0].is_small() || args[0].is_float() || args[0].is_big(),
    ))
}

/// `erlang:is_float/1`
pub unsafe fn is_float_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::bool(args[0].is_float()))
}

// ===== Comparison BIFs =====

/// `erlang:==/2` - term equality.
pub unsafe fn eq_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    Ok(Term::bool(args[0] == args[1]))
}

/// `erlang:/=/2` - term inequality.
pub unsafe fn neq_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    Ok(Term::bool(args[0] != args[1]))
}

/// `erlang:exact_eq/2` - exact term equality (bitwise).
pub unsafe fn exact_eq_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    Ok(Term::bool(args[0].to_raw() == args[1].to_raw()))
}

/// `erlang:exact_ne/2` - exact term inequality (bitwise).
pub unsafe fn exact_ne_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    Ok(Term::bool(args[0].to_raw() != args[1].to_raw()))
}

// ===== Process BIFs =====

/// `erlang:self/0` - returns the current process's PID.
pub unsafe fn self_0(proc: &mut Process, _args: &[Term]) -> BifResult {
    check_arity!(_args, 0);
    Ok(proc.pid_term())
}

/// `erlang:spawn/3` - spawn a new process.
pub unsafe fn spawn_3(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 3);
    let _module = args[0].get_atom_index().ok_or_else(|| badarg())?;
    let _function = args[1].get_atom_index().ok_or_else(|| badarg())?;
    if !args[2].is_list() && !args[2].is_nil() {
        return Err(badarg());
    }
    // TODO: actual spawn via scheduler; return new PID
    Ok(Term::small(0))
}

/// `erlang:send/2` - send a message to a process.
pub unsafe fn send_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    if !args[0].is_pid() && !args[0].is_atom() {
        return Err(badarg());
    }
    // TODO: actual send via scheduler
    Ok(args[1]) // Returns the message
}

// ===== Error BIFs =====

/// `erlang:error/1` - raise an error.
pub unsafe fn error_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Err(Exception::error(args[0]))
}

/// `erlang:error/2` - raise an error with arguments.
pub unsafe fn error_2(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 2);
    // args[0] = reason, args[1] = arguments (for stacktrace)
    let _ = args[1]; // TODO: build stacktrace from args[1]
    Err(Exception::error(args[0]))
}

/// `erlang:throw/1` - throw a term.
pub unsafe fn throw_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Err(Exception::throw(args[0]))
}

/// `erlang:exit/1` - exit the process.
pub unsafe fn exit_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Err(Exception::exit(args[0]))
}

/// `erlang:fault/1` - raise a fault.
pub unsafe fn fault_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Err(Exception::error(args[0]))
}

// ===== Utility BIFs =====

/// `erlang:tuple_size/1`
pub unsafe fn tuple_size_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    if !args[0].is_tuple() {
        return Err(badarg());
    }
    let header = args[0].header();
    let arity = Term::header_arity(header);
    Ok(Term::small(arity as i64))
}

/// `erlang:size/1` - generic size for tuples and binaries.
pub unsafe fn size_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    if args[0].is_tuple() {
        let header = args[0].header();
        let arity = Term::header_arity(header);
        Ok(Term::small(arity as i64))
    } else if args[0].is_binary() {
        // Simplified - would need actual binary size
        Ok(Term::small(0))
    } else {
        Err(badarg())
    }
}

/// `erlang:length/1` - length of a list.
pub unsafe fn length_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    if !args[0].is_list() && !args[0].is_nil() {
        return Err(badarg());
    }
    let mut len: i64 = 0;
    let mut current = args[0];
    loop {
        if current.is_nil() {
            break;
        }
        if !current.is_list() {
            return Err(badarg());
        }
        len += 1;
        // Follow the tail of the cons cell
        let ptr = current.get_list_ptr();
        current = unsafe { *ptr.add(1) };
    }
    Ok(Term::small(len))
}

/// `erlang:hd/1` - head of a list.
pub unsafe fn hd_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    if !args[0].is_list() {
        return Err(badarg());
    }
    let ptr = args[0].get_list_ptr();
    Ok(unsafe { *ptr })
}

/// `erlang:tl/1` - tail of a list.
pub unsafe fn tl_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    if !args[0].is_list() {
        return Err(badarg());
    }
    let ptr = args[0].get_list_ptr();
    Ok(unsafe { *ptr.add(1) }) // cons: [head | tail] at consecutive words
}

/// `erlang:node/0`
pub unsafe fn node_0(_proc: &mut Process, _args: &[Term]) -> BifResult {
    check_arity!(_args, 0);
    Ok(Term::atom(0)) // 'nonode@nohost' placeholder
}

/// `erlang:nodes/0`
pub unsafe fn nodes_0(_proc: &mut Process, _args: &[Term]) -> BifResult {
    check_arity!(_args, 0);
    Ok(Term::nil())
}

// ===== Conversion BIFs =====

/// `erlang:integer_to_list/1`
pub unsafe fn integer_to_list_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    let _val = args[0].get_small().ok_or_else(|| badarg())?;
    // TODO: allocate a list of digits on the process heap
    Ok(Term::nil()) // Placeholder
}

/// `erlang:list_to_integer/1`
pub unsafe fn list_to_integer_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    // Simplified
    Ok(Term::small(0))
}

/// `erlang:atom_to_list/1`
pub unsafe fn atom_to_list_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    if !args[0].is_atom() {
        return Err(badarg());
    }
    Ok(Term::nil()) // Placeholder
}

/// `erlang:list_to_atom/1`
pub unsafe fn list_to_atom_1(_proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    Ok(Term::atom(0)) // Placeholder
}

// ===== Float BIFs =====

/// `erlang:float/1` - convert integer to float.
pub unsafe fn float_1(proc: &mut Process, args: &[Term]) -> BifResult {
    check_arity!(args, 1);
    let val = if args[0].is_small() {
        args[0].get_small().unwrap() as f64
    } else if args[0].is_float() {
        args[0].get_float().unwrap()
    } else {
        return Err(badarith());
    };

    // Allocate float on heap: header (HEADER_FLOAT | arity 0x0003) + f64 value
    let header = crate::term::tags::HEADER_FLOAT | 0x0003;
    let heap_ptr = proc.alloc_words(3); // header + 2 words for f64 alignment
    unsafe {
        // heap_ptr was already advanced by alloc_words, so write at the position
        // 3 words before the current heap_ptr
        let base_ptr = heap_ptr.sub(3);
        *base_ptr = Term::from_raw(header);
        let float_ptr = base_ptr.add(1) as *mut f64;
        *float_ptr = val;
    }
    // The boxed pointer with FLOAT header tag
    let result_ptr = unsafe { heap_ptr.sub(3) };
    Ok(Term::from_raw(
        result_ptr as u64 | crate::term::tags::PRIMARY_TAG_BOXED,
    ))
}

/// Register all BIFs into the runtime.
pub fn register_all_bifs() -> Vec<BifDescriptor> {
    // Pre-compute atom indices
    let erlang = crate::atom::atom("erlang");
    let plus = crate::atom::atom("+");
    let minus = crate::atom::atom("-");
    let multiply = crate::atom::atom("*");
    let div = crate::atom::atom("div");
    let rem = crate::atom::atom("rem");
    let is_integer = crate::atom::atom("is_integer");
    let is_atom = crate::atom::atom("is_atom");
    let is_binary = crate::atom::atom("is_binary");
    let is_boolean = crate::atom::atom("is_boolean");
    let is_tuple = crate::atom::atom("is_tuple");
    let is_list = crate::atom::atom("is_list");
    let is_pid = crate::atom::atom("is_pid");
    let is_port = crate::atom::atom("is_port");
    let is_function = crate::atom::atom("is_function");
    let is_map = crate::atom::atom("is_map");
    let is_number = crate::atom::atom("is_number");
    let is_float = crate::atom::atom("is_float");
    let eq = crate::atom::atom("==");
    let neq = crate::atom::atom("/=");
    let exact_eq = crate::atom::atom("=:=");
    let exact_ne = crate::atom::atom("=/=");
    let self_ = crate::atom::atom("self");
    let spawn = crate::atom::atom("spawn");
    let send = crate::atom::atom("send");
    let error = crate::atom::atom("error");
    let throw = crate::atom::atom("throw");
    let exit = crate::atom::atom("exit");
    let fault = crate::atom::atom("fault");
    let tuple_size = crate::atom::atom("tuple_size");
    let size = crate::atom::atom("size");
    let length = crate::atom::atom("length");
    let hd = crate::atom::atom("hd");
    let tl = crate::atom::atom("tl");
    let node = crate::atom::atom("node");
    let nodes = crate::atom::atom("nodes");
    let integer_to_list = crate::atom::atom("integer_to_list");
    let list_to_integer = crate::atom::atom("list_to_integer");
    let atom_to_list = crate::atom::atom("atom_to_list");
    let list_to_atom = crate::atom::atom("list_to_atom");
    let float = crate::atom::atom("float");

    vec![
        bif!(erlang, plus, 2, add_2),
        bif!(erlang, minus, 2, sub_2),
        bif!(erlang, multiply, 2, mul_2),
        bif!(erlang, div, 2, div_2),
        bif!(erlang, rem, 2, rem_2),
        bif!(erlang, minus, 1, neg_1),
        bif!(erlang, is_integer, 1, is_integer_1),
        bif!(erlang, is_atom, 1, is_atom_1),
        bif!(erlang, is_binary, 1, is_binary_1),
        bif!(erlang, is_boolean, 1, is_boolean_1),
        bif!(erlang, is_tuple, 1, is_tuple_1),
        bif!(erlang, is_list, 1, is_list_1),
        bif!(erlang, is_pid, 1, is_pid_1),
        bif!(erlang, is_port, 1, is_port_1),
        bif!(erlang, is_function, 1, is_function_1),
        bif!(erlang, is_map, 1, is_map_1),
        bif!(erlang, is_number, 1, is_number_1),
        bif!(erlang, is_float, 1, is_float_1),
        bif!(erlang, eq, 2, eq_2),
        bif!(erlang, neq, 2, neq_2),
        bif!(erlang, exact_eq, 2, exact_eq_2),
        bif!(erlang, exact_ne, 2, exact_ne_2),
        bif!(erlang, self_, 0, self_0),
        bif!(erlang, spawn, 3, spawn_3),
        bif!(erlang, send, 2, send_2),
        bif!(erlang, error, 1, error_1),
        bif!(erlang, error, 2, error_2),
        bif!(erlang, throw, 1, throw_1),
        bif!(erlang, exit, 1, exit_1),
        bif!(erlang, fault, 1, fault_1),
        bif!(erlang, tuple_size, 1, tuple_size_1),
        bif!(erlang, size, 1, size_1),
        bif!(erlang, length, 1, length_1),
        bif!(erlang, hd, 1, hd_1),
        bif!(erlang, tl, 1, tl_1),
        bif!(erlang, node, 0, node_0),
        bif!(erlang, nodes, 0, nodes_0),
        bif!(erlang, integer_to_list, 1, integer_to_list_1),
        bif!(erlang, list_to_integer, 1, list_to_integer_1),
        bif!(erlang, atom_to_list, 1, atom_to_list_1),
        bif!(erlang, list_to_atom, 1, list_to_atom_1),
        bif!(erlang, float, 1, float_1),
    ]
}

/// Look up a BIF by module, function, and arity.
pub fn lookup_bif(module: u32, function: u32, arity: u32) -> Option<BifFn> {
    let bifs = register_all_bifs();
    for bif in &bifs {
        if bif.module == module && bif.function == function && bif.arity == arity {
            return Some(bif.implementation);
        }
    }
    None
}
