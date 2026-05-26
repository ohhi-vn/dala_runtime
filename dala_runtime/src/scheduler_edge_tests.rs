//! Edge case tests for Scheduler.

use crate::RuntimeConfig;
use crate::scheduler::*;

// ═══════════════════════════════════════════════════════════════════════════
// Governor: thermal throttling at each level, battery awareness
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_governor_new() {
    let governor = Governor::new();
    assert!(!governor.is_throttling());
    assert_eq!(governor.max_qos(), QosClass::Background); // Nominal = Background max
}

#[test]
fn test_governor_thermal_nominal() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Nominal);

    assert!(!governor.is_throttling());
    assert_eq!(governor.max_qos(), QosClass::Background);
}

#[test]
fn test_governor_thermal_fair() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Fair);

    assert!(!governor.is_throttling());
    // Fair + good battery = Background
    assert_eq!(governor.max_qos(), QosClass::Background);
}

#[test]
fn test_governor_thermal_fair_low_battery() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Fair);
    governor.set_battery(BatteryState {
        level: 0.05,
        charging: false,
    });

    // Fair + very low battery = Utility
    assert_eq!(governor.max_qos(), QosClass::Utility);
}

#[test]
fn test_governor_thermal_serious() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Serious);

    assert!(governor.is_throttling());
    // Serious + good battery = Utility
    assert_eq!(governor.max_qos(), QosClass::Utility);
}

#[test]
fn test_governor_thermal_serious_low_battery() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Serious);
    governor.set_battery(BatteryState {
        level: 0.1,
        charging: false,
    });

    // Serious + low battery = UserFacing
    assert_eq!(governor.max_qos(), QosClass::UserFacing);
}

#[test]
fn test_governor_thermal_critical() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Critical);

    assert!(governor.is_throttling());
    assert_eq!(governor.max_qos(), QosClass::Realtime);
}

#[test]
fn test_governor_thermal_transitions() {
    let governor = Governor::new();

    // Nominal -> Fair -> Serious -> Critical -> Nominal
    governor.set_thermal(ThermalState::Nominal);
    assert!(!governor.is_throttling());

    governor.set_thermal(ThermalState::Fair);
    assert!(!governor.is_throttling());

    governor.set_thermal(ThermalState::Serious);
    assert!(governor.is_throttling());

    governor.set_thermal(ThermalState::Critical);
    assert!(governor.is_throttling());

    governor.set_thermal(ThermalState::Nominal);
    assert!(!governor.is_throttling());
}

#[test]
fn test_governor_battery_charging() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Fair);
    governor.set_battery(BatteryState {
        level: 0.05,
        charging: true,
    });

    // Fair + charging = Background (charging overrides low battery)
    assert_eq!(governor.max_qos(), QosClass::Background);
}

#[test]
fn test_governor_battery_full_charging() {
    let governor = Governor::new();
    governor.set_battery(BatteryState {
        level: 1.0,
        charging: true,
    });

    // Full battery + Nominal = Background
    assert_eq!(governor.max_qos(), QosClass::Background);
}

#[test]
fn test_governor_battery_empty() {
    let governor = Governor::new();
    governor.set_battery(BatteryState {
        level: 0.0,
        charging: false,
    });

    // Empty battery + Nominal = Background
    assert_eq!(governor.max_qos(), QosClass::Background);
}

#[test]
fn test_governor_default() {
    let governor: Governor = Default::default();
    assert!(!governor.is_throttling());
}

// ═══════════════════════════════════════════════════════════════════════════
// QosClass ordering and defaults
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_qos_class_default() {
    let qos: QosClass = Default::default();
    assert_eq!(qos, QosClass::Utility);
}

#[test]
fn test_qos_class_ordering() {
    assert!(QosClass::Realtime > QosClass::UserFacing);
    assert!(QosClass::UserFacing > QosClass::Utility);
    assert!(QosClass::Utility > QosClass::Background);
}

#[test]
fn test_qos_class_discriminants() {
    assert_eq!(QosClass::Background as usize, 0);
    assert_eq!(QosClass::Utility as usize, 1);
    assert_eq!(QosClass::UserFacing as usize, 2);
    assert_eq!(QosClass::Realtime as usize, 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// ThermalState transitions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_thermal_state_default() {
    let state: ThermalState = Default::default();
    assert_eq!(state, ThermalState::Nominal);
}

#[test]
fn test_thermal_state_all_variants() {
    let states = [
        ThermalState::Nominal,
        ThermalState::Fair,
        ThermalState::Serious,
        ThermalState::Critical,
    ];

    for state in &states {
        let _ = format!("{:?}", state);
    }
}

#[test]
fn test_thermal_state_equality() {
    assert_eq!(ThermalState::Nominal, ThermalState::Nominal);
    assert_ne!(ThermalState::Nominal, ThermalState::Critical);
}

// ═══════════════════════════════════════════════════════════════════════════
// BatteryState defaults
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_battery_state_default() {
    let battery: BatteryState = Default::default();
    assert_eq!(battery.level, 1.0);
    assert!(!battery.charging);
}

#[test]
fn test_battery_state_clone() {
    let battery = BatteryState {
        level: 0.5,
        charging: true,
    };
    let battery2 = battery.clone();
    assert_eq!(battery.level, battery2.level);
    assert_eq!(battery.charging, battery2.charging);
}

// ═══════════════════════════════════════════════════════════════════════════
// Reduction budget at each thermal level
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_reduction_budget_nominal() {
    let governor = Governor::new();

    assert_eq!(governor.reduction_budget(QosClass::Realtime), 500);
    assert_eq!(governor.reduction_budget(QosClass::UserFacing), 2000);
    assert_eq!(governor.reduction_budget(QosClass::Utility), 1000);
    assert_eq!(governor.reduction_budget(QosClass::Background), 500);
}

#[test]
fn test_reduction_budget_fair() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Fair);

    assert_eq!(governor.reduction_budget(QosClass::Realtime), 400); // 500 * 0.8
    assert_eq!(governor.reduction_budget(QosClass::UserFacing), 1600); // 2000 * 0.8
    assert_eq!(governor.reduction_budget(QosClass::Utility), 800); // 1000 * 0.8
    assert_eq!(governor.reduction_budget(QosClass::Background), 400); // 500 * 0.8
}

#[test]
fn test_reduction_budget_serious() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Serious);

    assert_eq!(governor.reduction_budget(QosClass::Realtime), 250); // 500 * 0.5
    assert_eq!(governor.reduction_budget(QosClass::UserFacing), 1000); // 2000 * 0.5
    assert_eq!(governor.reduction_budget(QosClass::Utility), 500); // 1000 * 0.5
    assert_eq!(governor.reduction_budget(QosClass::Background), 250); // 500 * 0.5
}

#[test]
fn test_reduction_budget_critical() {
    let governor = Governor::new();
    governor.set_thermal(ThermalState::Critical);

    assert_eq!(governor.reduction_budget(QosClass::Realtime), 125); // 500 * 0.25
    assert_eq!(governor.reduction_budget(QosClass::UserFacing), 500); // 2000 * 0.25
    assert_eq!(governor.reduction_budget(QosClass::Utility), 250); // 1000 * 0.25
    assert_eq!(governor.reduction_budget(QosClass::Background), 125); // 500 * 0.25
}

// ═══════════════════════════════════════════════════════════════════════════
// RuntimeConfig defaults
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_runtime_config_default() {
    let config = RuntimeConfig::default();
    assert!(config.scheduler_count > 0);
    assert_eq!(config.initial_heap_size, 233);
    assert_eq!(config.max_heap_size, 16_384);
    assert_eq!(config.reductions_per_yield, 2_000);
    assert!(!config.debug_gc);
    assert_eq!(config.execution_mode, crate::ExecutionMode::Mixed);
}
