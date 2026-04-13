use crate::bridge::register_hardware_primitives;
use bricklogo_hal::health;
use bricklogo_hal::port_manager::PortManager;
use bricklogo_lang::evaluator::Evaluator;
use bricklogo_lang::primitives::register_core_primitives;
use std::sync::{Arc, Mutex};

/// Construct an Evaluator + PortManager pair wired up with all primitives.
///
/// Shared by the TUI (`App::new`) and the script runner. The callbacks
/// receive every `print` / `show` / `type` and every system message
/// respectively, and are responsible for routing them somewhere visible.
///
/// Also starts the BLE health watchdog so unexpectedly-dropped peripherals
/// (e.g. flaky bluez-async on a Pi) get reaped from the port manager and
/// surfaced to the user instead of silently lingering.
pub fn build_evaluator(
    output_fn: Arc<dyn Fn(&str) + Send + Sync>,
    system_fn: Arc<dyn Fn(&str) + Send + Sync>,
) -> (Evaluator, Arc<Mutex<PortManager>>) {
    let mut evaluator = Evaluator::new(output_fn);
    register_core_primitives(&mut evaluator);
    evaluator.set_system_fn(system_fn.clone());
    let port_manager = Arc::new(Mutex::new(PortManager::new()));
    register_hardware_primitives(&mut evaluator, port_manager.clone(), system_fn.clone());
    health::start(port_manager.clone(), system_fn);
    (evaluator, port_manager)
}
