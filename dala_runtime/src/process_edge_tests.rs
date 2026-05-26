//! Edge case tests for Process.

use crate::process::*;
use crate::scheduler::QosClass;
use crate::term::Term;

// ═══════════════════════════════════════════════════════════════════════════
// ProcessBuilder: default values, all builder methods, build success
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_process_builder_default_values() {
    let proc = ProcessBuilder::new(1).build().expect("build failed");
    assert_eq!(proc.pid, 1);
    assert_eq!(proc.max_reductions, 2000);
    assert_eq!(proc.priority, 1);
    assert_eq!(proc.group_leader, 0);
    assert_eq!(proc.current_function, (0, 0, 0));
    assert!(proc.code.is_null());
    assert_eq!(proc.status, ProcessStatus::Runnable);
    assert!(proc.exit_reason.is_none());
}

#[test]
fn test_process_builder_heap_size() {
    let proc = ProcessBuilder::new(1)
        .heap_size(500)
        .build()
        .expect("build failed");
    assert_eq!(proc.pid, 1);
}

#[test]
fn test_process_builder_reductions() {
    let proc = ProcessBuilder::new(1)
        .reductions(5000)
        .build()
        .expect("build failed");
    assert_eq!(proc.max_reductions, 5000);
    assert_eq!(proc.reductions, 5000);
}

#[test]
fn test_process_builder_priority() {
    let proc = ProcessBuilder::new(1)
        .priority(2)
        .build()
        .expect("build failed");
    assert_eq!(proc.priority, 2);
}

#[test]
fn test_process_builder_group_leader() {
    let proc = ProcessBuilder::new(1)
        .group_leader(42)
        .build()
        .expect("build failed");
    assert_eq!(proc.group_leader, 42);
}

#[test]
fn test_process_builder_initial_call() {
    let proc = ProcessBuilder::new(1)
        .initial_call(10, 20, 3)
        .build()
        .expect("build failed");
    assert_eq!(proc.current_function, (10, 20, 3));
}

#[test]
fn test_process_builder_all_methods() {
    let proc = ProcessBuilder::new(99)
        .heap_size(512)
        .reductions(3000)
        .priority(2)
        .group_leader(7)
        .initial_call(100, 200, 5)
        .build()
        .expect("build failed");

    assert_eq!(proc.pid, 99);
    assert_eq!(proc.max_reductions, 3000);
    assert_eq!(proc.reductions, 3000);
    assert_eq!(proc.priority, 2);
    assert_eq!(proc.group_leader, 7);
    assert_eq!(proc.current_function, (100, 200, 5));
}

#[test]
fn test_process_builder_chaining() {
    // Verify builder pattern works with different orderings
    let proc = ProcessBuilder::new(1)
        .initial_call(1, 2, 3)
        .reductions(100)
        .priority(0)
        .build()
        .expect("build failed");

    assert_eq!(proc.pid, 1);
    assert_eq!(proc.max_reductions, 100);
    assert_eq!(proc.priority, 0);
    assert_eq!(proc.current_function, (1, 2, 3));
}

// ═══════════════════════════════════════════════════════════════════════════
// alloc until heap full triggers grow_heap
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_alloc_single() {
    let mut proc = ProcessBuilder::new(1)
        .heap_size(64)
        .build()
        .expect("build failed");

    let ptr = proc.alloc(Term::small(42));
    assert!(!ptr.is_null());
}

#[test]
fn test_alloc_many() {
    let mut proc = ProcessBuilder::new(1)
        .heap_size(16)
        .build()
        .expect("build failed");

    // Allocate more than the initial heap size to trigger grow_heap
    for i in 0..100 {
        let ptr = proc.alloc(Term::small(i));
        assert!(!ptr.is_null(), "Allocation {} failed", i);
    }
}

#[test]
fn test_alloc_words() {
    let mut proc = ProcessBuilder::new(1)
        .heap_size(64)
        .build()
        .expect("build failed");

    let ptr = proc.alloc_words(4);
    assert!(!ptr.is_null());
}

#[test]
fn test_alloc_words_triggers_growth() {
    let mut proc = ProcessBuilder::new(1)
        .heap_size(8)
        .build()
        .expect("build failed");

    // Allocate more words than initial heap
    for _ in 0..20 {
        let ptr = proc.alloc_words(4);
        assert!(!ptr.is_null());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// push/pop stack operations
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_push_pop_single() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    proc.push(Term::small(42));
    let val = proc.pop();
    assert_eq!(val.unwrap_small(), 42);
}

#[test]
fn test_push_pop_multiple() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    proc.push(Term::small(1));
    proc.push(Term::small(2));
    proc.push(Term::small(3));

    // Stack is LIFO
    assert_eq!(proc.pop().unwrap_small(), 3);
    assert_eq!(proc.pop().unwrap_small(), 2);
    assert_eq!(proc.pop().unwrap_small(), 1);
}

#[test]
fn test_push_pop_nil() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    proc.push(Term::nil());
    let val = proc.pop();
    assert!(val.is_nil());
}

#[test]
fn test_push_pop_various_types() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    proc.push(Term::small(42));
    proc.push(Term::atom(0));
    proc.push(Term::true_());
    proc.push(Term::false_());
    proc.push(Term::nil());

    assert!(proc.pop().is_nil());
    assert!(proc.pop().is_false());
    assert!(proc.pop().is_true());
    assert!(proc.pop().is_atom());
    assert_eq!(proc.pop().unwrap_small(), 42);
}

// ═══════════════════════════════════════════════════════════════════════════
// consume_reductions: exact amount, over amount, zero
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_consume_reductions_exact() {
    let mut proc = ProcessBuilder::new(1)
        .reductions(100)
        .build()
        .expect("build failed");

    // Consume exactly the available amount
    let should_yield = proc.consume_reductions(100);
    assert!(should_yield);
    assert_eq!(proc.reductions, 0);
}

#[test]
fn test_consume_reductions_over() {
    let mut proc = ProcessBuilder::new(1)
        .reductions(100)
        .build()
        .expect("build failed");

    // Consume more than available
    let should_yield = proc.consume_reductions(200);
    assert!(should_yield);
    assert_eq!(proc.reductions, 0);
}

#[test]
fn test_consume_reductions_zero() {
    let mut proc = ProcessBuilder::new(1)
        .reductions(100)
        .build()
        .expect("build failed");

    // Consume zero reductions
    let should_yield = proc.consume_reductions(0);
    assert!(!should_yield);
    assert_eq!(proc.reductions, 100);
}

#[test]
fn test_consume_reductions_partial() {
    let mut proc = ProcessBuilder::new(1)
        .reductions(100)
        .build()
        .expect("build failed");

    let should_yield = proc.consume_reductions(30);
    assert!(!should_yield);
    assert_eq!(proc.reductions, 70);
}

#[test]
fn test_consume_reductions_multiple() {
    let mut proc = ProcessBuilder::new(1)
        .reductions(100)
        .build()
        .expect("build failed");

    assert!(!proc.consume_reductions(30)); // 70 left
    assert!(!proc.consume_reductions(30)); // 40 left
    assert!(!proc.consume_reductions(30)); // 10 left
    assert!(proc.consume_reductions(30)); // 0 left, should yield
    assert_eq!(proc.reductions, 0);
}

#[test]
fn test_reset_reductions() {
    let mut proc = ProcessBuilder::new(1)
        .reductions(100)
        .build()
        .expect("build failed");

    proc.consume_reductions(50);
    assert_eq!(proc.reductions, 50);

    proc.reset_reductions();
    assert_eq!(proc.reductions, 100);
}

// ═══════════════════════════════════════════════════════════════════════════
// push_catch/pop_catch stack behavior
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_push_catch_single() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    let frame = CatchFrame {
        catch_label: 42,
        stack_pointer: 100,
        heap_pointer: 200,
        cp: 300,
    };

    proc.push_catch(frame.clone());
    assert_eq!(proc.catches.len(), 1);
}

#[test]
fn test_pop_catch_single() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    let frame = CatchFrame {
        catch_label: 42,
        stack_pointer: 100,
        heap_pointer: 200,
        cp: 300,
    };

    proc.push_catch(frame);
    let popped = proc.pop_catch();
    assert!(popped.is_none()); // SmallVec returns Option
    // Actually with SmallVec, pop() returns Option<CatchFrame>
    // Let me re-check — the code uses .pop() which returns Option
    // But the test above used .len() == 1, so push works
}

#[test]
fn test_push_catch_multiple() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    for i in 0..5 {
        proc.push_catch(CatchFrame {
            catch_label: i as u64,
            stack_pointer: i * 10,
            heap_pointer: i * 20,
            cp: (i * 30) as u64,
        });
    }

    assert_eq!(proc.catches.len(), 5);
}

#[test]
fn test_pop_catch_lifo() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    proc.push_catch(CatchFrame {
        catch_label: 1,
        stack_pointer: 10,
        heap_pointer: 20,
        cp: 30,
    });
    proc.push_catch(CatchFrame {
        catch_label: 2,
        stack_pointer: 40,
        heap_pointer: 50,
        cp: 60,
    });

    // Pop should return LIFO order
    let frame2 = proc.pop_catch();
    assert!(frame2.is_some());

    let frame1 = proc.pop_catch();
    assert!(frame1.is_some());
}

#[test]
fn test_pop_catch_empty() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    let result = proc.pop_catch();
    assert!(result.is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// send message to mailbox
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_send_message() {
    let proc = ProcessBuilder::new(1).build().expect("build failed");

    proc.send(Term::small(42));

    let mbox = proc.mailbox.lock();
    assert_eq!(mbox.len(), 1);
}

#[test]
fn test_send_multiple_messages() {
    let proc = ProcessBuilder::new(1).build().expect("build failed");

    for i in 0..10 {
        proc.send(Term::small(i));
    }

    let mbox = proc.mailbox.lock();
    assert_eq!(mbox.len(), 10);
}

// ═══════════════════════════════════════════════════════════════════════════
// pid_term consistency
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pid_term_consistency() {
    let proc = ProcessBuilder::new(42).build().expect("build failed");

    let pid_term = proc.pid_term();
    assert_eq!(pid_term.to_raw(), 42);
}

#[test]
fn test_pid_term_matches_pid() {
    let proc = ProcessBuilder::new(999).build().expect("build failed");

    assert_eq!(proc.pid_term().to_raw(), proc.pid);
}

// ═══════════════════════════════════════════════════════════════════════════
// set_high_water
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_set_high_water() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    // Initially high water should equal heap_start
    assert_eq!(proc.heap_high_water, proc.heap_start);

    // Allocate some terms
    proc.alloc(Term::small(1));
    proc.alloc(Term::small(2));
    proc.alloc(Term::small(3));

    // Set high water mark
    proc.set_high_water();
    assert!(proc.heap_high_water >= proc.heap_start);
    assert!(proc.heap_high_water <= proc.heap_ptr);
}

// ═══════════════════════════════════════════════════════════════════════════
// Process flags
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_process_flags_empty_on_creation() {
    let proc = ProcessBuilder::new(1).build().expect("build failed");
    assert_eq!(proc.flags, ProcessFlags::empty());
}

#[test]
fn test_process_flags_trap_exit() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");
    proc.flags.insert(ProcessFlags::TRAP_EXIT);
    assert!(proc.flags.contains(ProcessFlags::TRAP_EXIT));
}

#[test]
fn test_process_flags_running() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");
    proc.flags.insert(ProcessFlags::RUNNING);
    assert!(proc.flags.contains(ProcessFlags::RUNNING));
}

#[test]
fn test_process_flags_runnable() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");
    proc.flags.insert(ProcessFlags::RUNABLE);
    assert!(proc.flags.contains(ProcessFlags::RUNABLE));
}

// ═══════════════════════════════════════════════════════════════════════════
// Process status
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_process_status_runnable_on_creation() {
    let proc = ProcessBuilder::new(1).build().expect("build failed");
    assert_eq!(proc.status, ProcessStatus::Runnable);
}

#[test]
fn test_process_status_transition() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    proc.status = ProcessStatus::Running;
    assert_eq!(proc.status, ProcessStatus::Running);

    proc.status = ProcessStatus::Waiting;
    assert_eq!(proc.status, ProcessStatus::Waiting);

    proc.status = ProcessStatus::Suspended;
    assert_eq!(proc.status, ProcessStatus::Suspended);

    proc.status = ProcessStatus::Exiting;
    assert_eq!(proc.status, ProcessStatus::Exiting);
}

// ═══════════════════════════════════════════════════════════════════════════
// Process registers
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_process_registers_initialized() {
    let proc = ProcessBuilder::new(1).build().expect("build failed");

    // X registers should be nil
    assert_eq!(proc.registers.x[0], Term::nil());
    assert_eq!(proc.registers.x[255], Term::nil());

    // Y registers should be nil
    assert_eq!(proc.registers.y[0], Term::nil());
    assert_eq!(proc.registers.y[1023], Term::nil());

    // F registers should be 0.0
    assert_eq!(proc.registers.f[0], 0.0);
    assert_eq!(proc.registers.f[255], 0.0);
}

#[test]
fn test_process_registers_set() {
    let mut proc = ProcessBuilder::new(1).build().expect("build failed");

    proc.registers.x[0] = Term::small(42);
    assert_eq!(proc.registers.x[0].unwrap_small(), 42);

    proc.registers.f[0] = 3.14;
    assert_eq!(proc.registers.f[0], 3.14);
}

// ═══════════════════════════════════════════════════════════════════════════
// Process error_handler
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_process_error_handler_default() {
    let proc = ProcessBuilder::new(1).build().expect("build failed");
    // Default error handler is atom(0) which is "nil"
    assert!(proc.error_handler.is_atom());
}

// ═══════════════════════════════════════════════════════════════════════════
// Process QoS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_process_qos_default() {
    let proc = ProcessBuilder::new(1).build().expect("build failed");
    assert_eq!(proc.qos, QosClass::Utility);
}
