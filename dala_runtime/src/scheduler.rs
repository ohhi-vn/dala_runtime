//! Scheduler — QoS-aware, thermal-aware actor scheduling for Dala.
//!
//! Unlike the BEAM scheduler (optimized for telecom fairness), the Dala
//! scheduler is designed for mobile AI workloads with:
//!
//! - **Thermal-aware scheduling**: Reduces inference priority when the
//!   device is hot, preventing thermal throttling.
//! - **Battery-aware scheduling**: Conserves power when battery is low
//!   by deprioritizing background work.
//! - **QoS-aware actor scheduling**: Actors are assigned QoS classes
//!   (Realtime, UserFacing, Utility, Background) that determine
//!   scheduling priority and preemption behavior.
//! - **Inference-priority actors**: Special scheduling for AI inference
//!   workers with deadline awareness.
//!
//! # Architecture
//!
//! ```text
//!  ┌─────────────────────────────────────────────────────┐
//!  │                  Scheduler                          │
//!  │                                                     │
//!  │  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
//!  │  │ Realtime │  │  User    │  │ Utility  │  ...     │
//!  │  │  Queue   │  │  Queue   │  │  Queue   │          │
//!  │  └────┬─────┘  └────┬─────┘  └────┬─────┘          │
//!  │       │              │              │                │
//!  │       ▼              ▼              ▼                │
//!  │  ┌─────────────────────────────────────────┐        │
//!  │  │         Work-Stealing Balancer          │        │
//!  │  └─────────────────────────────────────────┘        │
//!  │       │                                             │
//!  │       ▼                                             │
//!  │  ┌─────────────────────────────────────────┐        │
//!  │  │    Thermal / Battery Governor           │        │
//!  │  └─────────────────────────────────────────┘        │
//!  └─────────────────────────────────────────────────────┘
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use dala_ir::type_system::ActorLifecycle;

use crate::RuntimeConfig;
use crate::process::Process;

// ═══════════════════════════════════════════════════════════════════════════
// QoS classes
// ═══════════════════════════════════════════════════════════════════════════

/// Quality-of-Service class for actor scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QosClass {
    /// Background work (analytics, cleanup, model updates)
    Background = 0,
    /// Utility work (data processing, caching)
    Utility = 1,
    /// User-facing work (UI, user interactions)
    UserFacing = 2,
    /// Real-time work (voice, video, sensor fusion)
    Realtime = 3,
}

impl Default for QosClass {
    fn default() -> Self {
        Self::Utility
    }
}

impl From<ActorLifecycle> for QosClass {
    fn from(lifecycle: ActorLifecycle) -> Self {
        match lifecycle {
            ActorLifecycle::Supervisor => QosClass::Realtime,
            ActorLifecycle::Permanent => QosClass::UserFacing,
            ActorLifecycle::Transient => QosClass::Utility,
            ActorLifecycle::Temporary => QosClass::Background,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Thermal / Battery governor
// ═══════════════════════════════════════════════════════════════════════════

/// Thermal state of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    /// Normal operating temperature
    Nominal,
    /// Slightly warm — reduce background work
    Fair,
    /// Warm — reduce utility and background work
    Serious,
    /// Hot — only real-time work
    Critical,
}

impl Default for ThermalState {
    fn default() -> Self {
        Self::Nominal
    }
}

/// Battery state.
#[derive(Debug, Clone, Copy)]
pub struct BatteryState {
    /// Charge level (0.0 - 1.0)
    pub level: f32,
    /// Whether the device is charging
    pub charging: bool,
}

impl Default for BatteryState {
    fn default() -> Self {
        Self {
            level: 1.0,
            charging: false,
        }
    }
}

/// The thermal/battery governor adjusts scheduling based on device state.
#[derive(Debug)]
pub struct Governor {
    thermal: RwLock<ThermalState>,
    battery: RwLock<BatteryState>,
    /// Whether thermal throttling is active
    throttling: AtomicBool,
}

impl Governor {
    pub fn new() -> Self {
        Self {
            thermal: RwLock::new(ThermalState::Nominal),
            battery: RwLock::new(BatteryState::default()),
            throttling: AtomicBool::new(false),
        }
    }

    /// Update the thermal state.
    pub fn set_thermal(&self, state: ThermalState) {
        *self.thermal.write().unwrap() = state;
        let throttling = matches!(state, ThermalState::Serious | ThermalState::Critical);
        self.throttling.store(throttling, Ordering::SeqCst);
    }

    /// Update the battery state.
    pub fn set_battery(&self, state: BatteryState) {
        *self.battery.write().unwrap() = state;
    }

    /// Check if thermal throttling is active.
    pub fn is_throttling(&self) -> bool {
        self.throttling.load(Ordering::Relaxed)
    }

    /// Get the maximum QoS class allowed under current conditions.
    pub fn max_qos(&self) -> QosClass {
        let thermal = *self.thermal.read().unwrap();
        let battery = *self.battery.read().unwrap();

        match thermal {
            ThermalState::Critical => QosClass::Realtime,
            ThermalState::Serious => {
                if battery.level < 0.2 && !battery.charging {
                    QosClass::UserFacing
                } else {
                    QosClass::Utility
                }
            }
            ThermalState::Fair => {
                if battery.level < 0.1 && !battery.charging {
                    QosClass::Utility
                } else {
                    QosClass::Background
                }
            }
            ThermalState::Nominal => QosClass::Background,
        }
    }

    /// Get the reduction budget for a QoS class under current conditions.
    /// Returns the number of reductions before yielding.
    pub fn reduction_budget(&self, qos: QosClass) -> u32 {
        let base = match qos {
            QosClass::Realtime => 500,
            QosClass::UserFacing => 2000,
            QosClass::Utility => 1000,
            QosClass::Background => 500,
        };

        let thermal = *self.thermal.read().unwrap();
        let scale = match thermal {
            ThermalState::Nominal => 1.0,
            ThermalState::Fair => 0.8,
            ThermalState::Serious => 0.5,
            ThermalState::Critical => 0.25,
        };

        (base as f32 * scale) as u32
    }
}

impl Default for Governor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Message to the scheduler
// ═══════════════════════════════════════════════════════════════════════════

/// Message to the scheduler.
pub enum SchedulerMessage {
    /// Spawn a new process
    Spawn {
        pid: u64,
        module: u64,
        function: u64,
        arity: u32,
        args: Vec<crate::term::Term>,
        /// QoS class for the new process
        qos: QosClass,
    },
    /// Send a message to a process
    Message { pid: u64, msg: crate::term::Term },
    /// Kill a process
    Kill(u64),
    /// Update thermal state
    UpdateThermal(ThermalState),
    /// Update battery state
    UpdateBattery(BatteryState),
    /// System halt
    Halt,
}

// ═══════════════════════════════════════════════════════════════════════════
// Global scheduler state
// ═══════════════════════════════════════════════════════════════════════════

/// Per-QoS-class run queue.
struct QosQueue {
    queue: Mutex<Vec<usize>>,
}

impl QosQueue {
    fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }

    fn push(&self, pid: usize) {
        self.queue.lock().unwrap().push(pid);
    }

    fn pop(&self) -> Option<usize> {
        self.queue.lock().unwrap().pop()
    }

    fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }

    fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
}

/// Global scheduler state shared across all scheduler threads.
struct GlobalState {
    config: RuntimeConfig,
    /// Per-QoS-class run queues
    qos_queues: [QosQueue; 4],
    /// All known processes
    processes: dashmap::DashMap<u64, Arc<Mutex<Option<Process>>>>,
    /// Process QoS classes
    process_qos: dashmap::DashMap<u64, QosClass>,
    /// Next PID to assign
    next_pid: AtomicU64,
    /// Whether the runtime is shutting down
    shutting_down: AtomicBool,
    /// Number of active schedulers
    active_schedulers: AtomicUsize,
    /// Thermal/battery governor
    governor: Governor,
}

impl GlobalState {
    fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            qos_queues: [
                QosQueue::new(), // Background
                QosQueue::new(), // Utility
                QosQueue::new(), // UserFacing
                QosQueue::new(), // Realtime
            ],
            processes: dashmap::DashMap::new(),
            process_qos: dashmap::DashMap::new(),
            next_pid: AtomicU64::new(1),
            shutting_down: AtomicBool::new(false),
            active_schedulers: AtomicUsize::new(0),
            governor: Governor::new(),
        }
    }

    fn next_pid(&self) -> u64 {
        self.next_pid.fetch_add(1, Ordering::SeqCst)
    }

    fn schedule_process(&self, pid: usize, qos: QosClass) {
        let idx = qos as usize;
        if idx < 4 {
            self.qos_queues[idx].push(pid);
        }
    }

    /// Pick the highest-priority process from the QoS queues,
    /// respecting the governor's thermal/battery limits.
    fn pick_process(&self) -> Option<(usize, QosClass)> {
        let max_qos = self.governor.max_qos();

        // Scan from Realtime down to the governor's max allowed
        for qos in [
            QosClass::Realtime,
            QosClass::UserFacing,
            QosClass::Utility,
            QosClass::Background,
        ] {
            if qos > max_qos {
                continue;
            }
            let idx = qos as usize;
            if let Some(pid) = self.qos_queues[idx].pop() {
                return Some((pid, qos));
            }
        }
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Scheduler
// ═══════════════════════════════════════════════════════════════════════════

/// A scheduler thread that picks processes and runs them.
pub struct Scheduler {
    id: usize,
    global: Arc<GlobalState>,
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
        log::info!("Scheduler {} started", self.id);

        loop {
            if self.global.shutting_down.load(Ordering::SeqCst) {
                break;
            }

            // Try to pick a process from the QoS-aware queues
            if let Some((pid, qos)) = self.global.pick_process() {
                self.run_process(pid, qos);
            } else {
                // Try work stealing from other schedulers
                let stolen = self.steal_work();
                if let Some((pid, qos)) = stolen {
                    self.run_process(pid, qos);
                } else {
                    thread::sleep(Duration::from_micros(100));
                }
            }
        }

        log::info!("Scheduler {} shutting down", self.id);
    }

    /// Try to steal work from other scheduler queues.
    fn steal_work(&self) -> Option<(usize, QosClass)> {
        // Try to steal from other schedulers' local queues
        // For now, just check the global QoS queues
        self.global.pick_process()
    }

    /// Run a single process until it yields or completes.
    fn run_process(&self, pid: usize, qos: QosClass) {
        let entry = match self.global.processes.get(&(pid as u64)) {
            Some(e) => e,
            None => return,
        };
        let arc_mutex = entry.value().clone();
        drop(entry);

        if let Ok(mut guard) = arc_mutex.lock() {
            if let Some(ref mut process) = *guard {
                process.status = crate::process::ProcessStatus::Running;

                // Set reduction budget based on QoS and thermal state
                let budget = self.global.governor.reduction_budget(qos);
                process.max_reductions = budget;
                process.reset_reductions();

                log::trace!(
                    "Running process {} (qos={:?}) on scheduler {}",
                    pid,
                    qos,
                    self.id
                );

                // Execute the process's current function.
                // In a full implementation, this would call through the
                // code pointer (AOT) or interpret bytecode.
                let _ = process.consume_reductions(1);
                process.status = crate::process::ProcessStatus::Runnable;

                // Re-schedule at the appropriate QoS level
                self.global.schedule_process(pid, qos);
            }
        }
    }

    /// Spawn a new process.
    pub fn spawn(
        &self,
        module: u64,
        function: u64,
        arity: u32,
        args: Vec<crate::term::Term>,
        qos: QosClass,
    ) -> u64 {
        let pid = self.global.next_pid();

        let builder =
            crate::process::ProcessBuilder::new(pid as u64).initial_call(module, function, arity);

        if let Ok(mut process) = builder.build() {
            for (i, arg) in args.into_iter().enumerate().take(256) {
                process.registers.x[i] = arg;
            }
            process.status = crate::process::ProcessStatus::Runnable;

            self.global
                .processes
                .insert(pid, Arc::new(Mutex::new(Some(process))));
            self.global.process_qos.insert(pid, qos);
            self.global.schedule_process(pid as usize, qos);
        }

        pid
    }

    /// Send a message to a process.
    pub fn send_message(&self, pid: u64, msg: crate::term::Term) {
        if let Some(entry) = self.global.processes.get(&pid) {
            let arc_mutex = entry.value().clone();
            drop(entry);
            if let Ok(mut guard) = arc_mutex.lock() {
                if let Some(ref mut process) = *guard {
                    process.send(msg);
                    if process.status == crate::process::ProcessStatus::Waiting {
                        process.status = crate::process::ProcessStatus::Runnable;
                        let qos = self
                            .global
                            .process_qos
                            .get(&pid)
                            .map(|q| *q)
                            .unwrap_or(QosClass::Utility);
                        self.global.schedule_process(pid as usize, qos);
                    }
                }
            }
        }
    }

    /// Update the thermal state.
    pub fn update_thermal(&self, state: ThermalState) {
        self.global.governor.set_thermal(state);
    }

    /// Update the battery state.
    pub fn update_battery(&self, state: BatteryState) {
        self.global.governor.set_battery(state);
    }
}

impl Drop for GlobalState {
    fn drop(&mut self) {
        self.shutting_down.store(true, Ordering::SeqCst);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governor_thermal_throttling() {
        let governor = Governor::new();
        assert!(!governor.is_throttling());

        governor.set_thermal(ThermalState::Serious);
        assert!(governor.is_throttling());
        assert_eq!(governor.max_qos(), QosClass::Utility);

        governor.set_thermal(ThermalState::Critical);
        assert_eq!(governor.max_qos(), QosClass::Realtime);
    }

    #[test]
    fn test_governor_battery_aware() {
        let governor = Governor::new();
        governor.set_thermal(ThermalState::Fair);
        governor.set_battery(BatteryState {
            level: 0.05,
            charging: false,
        });
        // Low battery + Fair thermal = only Utility
        assert_eq!(governor.max_qos(), QosClass::Utility);
    }

    #[test]
    fn test_reduction_budget_scales_with_thermal() {
        let governor = Governor::new();

        let nominal = governor.reduction_budget(QosClass::UserFacing);
        assert_eq!(nominal, 2000);

        governor.set_thermal(ThermalState::Serious);
        let throttled = governor.reduction_budget(QosClass::UserFacing);
        assert_eq!(throttled, 1000); // 50%

        governor.set_thermal(ThermalState::Critical);
        let critical = governor.reduction_budget(QosClass::UserFacing);
        assert_eq!(critical, 500); // 25%
    }

    #[test]
    fn test_qos_class_from_lifecycle() {
        assert_eq!(
            QosClass::from(ActorLifecycle::Supervisor),
            QosClass::Realtime
        );
        assert_eq!(
            QosClass::from(ActorLifecycle::Permanent),
            QosClass::UserFacing
        );
        assert_eq!(
            QosClass::from(ActorLifecycle::Temporary),
            QosClass::Background
        );
    }

    #[test]
    fn test_qos_queue_ordering() {
        assert!(QosClass::Realtime > QosClass::UserFacing);
        assert!(QosClass::UserFacing > QosClass::Utility);
        assert!(QosClass::Utility > QosClass::Background);
    }
}
