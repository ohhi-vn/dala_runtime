//! Scheduler - manages process execution across multiple OS threads.
//!
//! The BEAM scheduler uses a reduction-counting preemptive scheduling model.
//! Each process runs for a configurable number of reductions before being
//! preempted and potentially moved to a different run queue.
//!
//! This implementation supports SMP (Symmetric Multi-Processing) with
//! one scheduler thread per CPU core.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use parking_lot::Mutex as ParkingMutex;

use crate::process::Process;
use crate::RuntimeConfig;

/// Message to the scheduler
pub enum SchedulerMessage {
    /// Spawn a new process
    Spawn {
        pid: u64,
        module: u64,
        function: u64,
        arity: u32,
        args: Vec<crate::term::Term>,
    },
    /// Send a message to a process
    Message { pid: u64, msg: crate::term::Term },
    /// Kill a process
    Kill(u64),
    /// System halt
    Halt,
}

/// Global scheduler state shared across all scheduler threads.
struct GlobalState {
    config: RuntimeConfig,
    /// Run queues - one per scheduler, work-stealing enabled
    run_queues: Vec<ParkingMutex<Vec<usize>>>,
    /// All known processes
    processes: dashmap::DashMap<u64, Arc<Mutex<Option<Process>>>>,
    /// Next PID to assign
    next_pid: AtomicU64,
    /// Whether the runtime is shutting down
    shutting_down: AtomicBool,
    /// Number of active schedulers
    active_schedulers: AtomicUsize,
}

/// A scheduler thread that picks processes and runs them.
pub struct Scheduler {
    id: usize,
    global: Arc<GlobalState>,
}

/// Thread-local scheduler reference
#[cfg(feature = "smp")]
thread_local! {
    static CURRENT_SCHEDULER: std::cell::Cell<usize> = std::cell::Cell::new(0);
}

impl GlobalState {
    fn new(config: RuntimeConfig) -> Self {
        let scheduler_count = config.scheduler_count.max(1);
        let mut run_queues = Vec::with_capacity(scheduler_count);
        for _ in 0..scheduler_count {
            run_queues.push(ParkingMutex::new(Vec::new()));
        }

        Self {
            config,
            run_queues,
            processes: dashmap::DashMap::new(),
            next_pid: AtomicU64::new(1), // PID 0 is reserved
            shutting_down: AtomicBool::new(false),
            active_schedulers: AtomicUsize::new(0),
        }
    }

    fn next_pid(&self) -> u64 {
        self.next_pid.fetch_add(1, Ordering::SeqCst)
    }

    fn schedule_process(&self, pid: usize, scheduler_id: usize) {
        let idx = scheduler_id % self.run_queues.len();
        self.run_queues[idx].lock().push(pid);
    }
}

impl Scheduler {
    /// Initialize the global scheduler with the given configuration.
    pub fn global_init(config: RuntimeConfig) -> Result<(), crate::RuntimeError> {
        let global = Arc::new(GlobalState::new(config));
        let scheduler_count = global.config.scheduler_count;

        for i in 0..scheduler_count {
            let scheduler = Scheduler {
                id: i,
                global: global.clone(),
            };
            global.active_schedulers.fetch_add(1, Ordering::SeqCst);

            let _handle = thread::Builder::new()
                .name(format!("dala-scheduler-{}", i))
                .spawn(move || {
                    scheduler.run();
                })
                .expect("failed to spawn scheduler thread");
        }

        Ok(())
    }

    /// The main scheduler loop.
    fn run(self) {
        #[cfg(feature = "smp")]
        CURRENT_SCHEDULER.with(|c| c.set(self.id));

        log::info!("Scheduler {} started", self.id);

        loop {
            // Check for shutdown
            {
                let global = &self.global;
                if global.shutting_down.load(Ordering::SeqCst) {
                    break;
                }
            }

            // Try to get a process from our local run queue
            let pid = {
                let queue = self.global.run_queues[self.id].lock();
                queue.pop()
            };

            match pid {
                Some(pid) => {
                    self.run_process(pid);
                }
                None => {
                    // Try work stealing from other schedulers
                    let stolen = self.steal_work();
                    if let Some(pid) = stolen {
                        self.run_process(pid);
                    } else {
                        // No work available, sleep briefly
                        thread::sleep(Duration::from_micros(100));
                    }
                }
            }
        }

        log::info!("Scheduler {} shutting down", self.id);
    }

    /// Try to steal work from other scheduler queues.
    fn steal_work(&self) -> Option<usize> {
        let count = self.global.run_queues.len();
        for offset in 1..count {
            let victim = (self.id + offset) % count;
            let queue = self.global.run_queues[victim].lock();
            if let Some(pid) = queue.pop() {
                return Some(pid);
            }
        }
        None
    }

    /// Run a single process until it yields or completes.
    fn run_process(&self, pid: usize) {
        let process_guard = match self.global.processes.get(&(pid as u64)) {
            Some(guard) => guard,
            None => return,
        };

        let mut process = match process_guard.lock() {
            Some(Some(proc)) => proc,
            _ => return,
        };

        process.status = crate::process::ProcessStatus::Running;
        process.reset_reductions();

        // Here we would call the compiled or interpreted function
        // For now, this is the integration point with the codegen/dispatch layer
        log::trace!("Running process {} on scheduler {}", pid, self.id);

        // Simulate execution - in the real implementation, this calls
        // the compiled function which checks reductions periodically
        process.status = crate::process::ProcessStatus::Runnable;
    }

    /// Spawn a new process.
    pub fn spawn(
        &self,
        module: u64,
        function: u64,
        arity: u32,
        args: Vec<crate::term::Term>,
    ) -> u64 {
        let pid = self.global.next_pid();

        // Build the process
        let builder =
            crate::process::ProcessBuilder::new(pid as u64).initial_call(module, function, arity);

        if let Ok(mut process) = builder.build() {
            // Set initial arguments in X registers
            for (i, arg) in args.into_iter().enumerate().take(256) {
                process.registers.x[i] = arg;
            }
            process.status = crate::process::ProcessStatus::Runnable;

            // Store the process
            self.global
                .processes
                .insert(pid, Arc::new(Mutex::new(Some(process))));

            // Schedule it
            self.global.schedule_process(pid as usize, self.id);
        }

        pid
    }

    /// Send a message to a process.
    pub fn send_message(&self, pid: u64, msg: crate::term::Term) {
        if let Some(process_guard) = self.global.processes.get(&pid) {
            if let Ok(Some(mut process)) = process_guard.try_lock() {
                process.send(msg);

                // If process is waiting, make it runnable
                if process.status == crate::process::ProcessStatus::Waiting {
                    process.status = crate::process::ProcessStatus::Runnable;
                    self.global.schedule_process(pid as usize, self.id);
                }
            }
        }
    }
}

impl Drop for GlobalState {
    fn drop(&mut self) {
        self.shutting_down.store(true, Ordering::SeqCst);
    }
}
