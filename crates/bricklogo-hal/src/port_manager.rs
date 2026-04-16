use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use bricklogo_lang::value::LogoValue;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone)]
pub struct OutputPortState {
    pub direction: PortDirection,
    pub power: u8, // 0-8
    pub is_running: bool,
}

#[derive(Debug, Clone)]
struct SentState {
    direction: PortDirection,
    power: u8,
    is_running: bool,
}

#[derive(Debug, Clone)]
struct QualifiedPort {
    device_name: String,
    port: String,
}

struct DeviceEntry {
    adapter: Box<dyn HardwareAdapter>,
    port_states: HashMap<String, OutputPortState>,
    last_sent: HashMap<String, SentState>,
}

pub struct PortManager {
    devices: HashMap<String, DeviceEntry>,
    /// Insertion order, for UI display.
    device_order: Vec<String>,
    /// Most-recently-used stack (end = most recent). Every `add_device` and
    /// `set_active_device` call pushes the name to the end; `remove_device`
    /// falls back to the new top.
    mru: Vec<String>,
    active_device: Option<String>,
    flash_timers: HashMap<String, Arc<AtomicBool>>,
}

impl PortManager {
    pub fn new() -> Self {
        PortManager {
            devices: HashMap::new(),
            device_order: Vec::new(),
            mru: Vec::new(),
            active_device: None,
            flash_timers: HashMap::new(),
        }
    }

    fn touch_mru(&mut self, name: &str) {
        self.mru.retain(|n| n != name);
        self.mru.push(name.to_string());
    }

    pub fn add_device(&mut self, name: &str, adapter: Box<dyn HardwareAdapter>) {
        // Default power on a fresh port is half the device's native maximum,
        // so `on` without a prior `setpower` runs at ~50% on every hub.
        let default_power = adapter.max_power() / 2;
        let mut port_states = HashMap::new();
        for port in adapter.output_ports() {
            port_states.insert(
                port.clone(),
                OutputPortState {
                    direction: PortDirection::Even,
                    power: default_power,
                    is_running: false,
                },
            );
        }
        self.devices.insert(
            name.to_string(),
            DeviceEntry {
                adapter,
                port_states,
                last_sent: HashMap::new(),
            },
        );
        self.device_order.push(name.to_string());
        self.touch_mru(name);
        if self.devices.len() == 1 {
            self.active_device = Some(name.to_string());
        }
    }

    pub fn remove_device(&mut self, name: &str) {
        // Cancel flash timers for this device
        let keys_to_remove: Vec<String> = self
            .flash_timers
            .keys()
            .filter(|k| k.starts_with(&format!("{}:", name)))
            .cloned()
            .collect();
        for key in keys_to_remove {
            if let Some(flag) = self.flash_timers.remove(&key) {
                flag.store(true, Ordering::Relaxed);
            }
        }

        if let Some(mut entry) = self.devices.remove(name) {
            if entry.adapter.connected() {
                entry.adapter.disconnect();
            }
        }
        self.device_order.retain(|device_name| device_name != name);
        self.mru.retain(|n| n != name);
        if self.active_device.as_deref() == Some(name) {
            // Fall back to the most-recently-used remaining device.
            self.active_device = self.mru.last().cloned();
        }
    }

    pub fn remove_all(&mut self) {
        for flag in self.flash_timers.values() {
            flag.store(true, Ordering::Relaxed);
        }
        self.flash_timers.clear();

        let names: Vec<String> = self.devices.keys().cloned().collect();
        for name in names {
            if let Some(mut entry) = self.devices.remove(&name) {
                if entry.adapter.connected() {
                    entry.adapter.disconnect();
                }
            }
        }
        self.active_device = None;
        self.device_order.clear();
        self.mru.clear();
    }

    pub fn set_active_device(&mut self, name: &str) -> Result<(), String> {
        if !self.devices.contains_key(name) {
            return Err(format!("No device named \"{}\"", name));
        }
        self.touch_mru(name);
        self.active_device = Some(name.to_string());
        Ok(())
    }

    pub fn get_active_device_name(&self) -> Option<&str> {
        self.active_device.as_deref()
    }

    /// Return the names of devices whose adapter no longer reports
    /// `connected()` — used by the health watchdog to reconcile state.
    pub fn dead_device_names(&self) -> Vec<String> {
        self.devices
            .iter()
            .filter(|(_, entry)| !entry.adapter.connected())
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn get_connected_device_names(&self) -> Vec<String> {
        self.device_order
            .iter()
            .filter(|name| {
                self.devices
                    .get(name.as_str())
                    .is_some_and(|entry| entry.adapter.connected())
            })
            .cloned()
            .collect()
    }

    pub fn get_active_device_name_owned(&self) -> Option<String> {
        self.active_device.clone()
    }

    pub fn format_port_names(&self, ports: &[String]) -> Vec<String> {
        let active = self.active_device.as_deref();
        ports.iter().map(|p| {
            if let Some(dot_idx) = p.find('.') {
                let device = &p[..dot_idx];
                let port = &p[dot_idx + 1..];
                if active == Some(device) {
                    port.to_string()
                } else {
                    p.clone()
                }
            } else {
                p.clone()
            }
        }).collect()
    }

    fn resolve_port(&self, port_str: &str) -> Result<QualifiedPort, String> {
        if let Some(dot_idx) = port_str.find('.') {
            let device_name = &port_str[..dot_idx];
            let port = &port_str[dot_idx + 1..];
            if !self.devices.contains_key(device_name) {
                return Err(format!("No device named \"{}\"", device_name));
            }
            Ok(QualifiedPort {
                device_name: device_name.to_string(),
                port: port.to_string(),
            })
        } else {
            let device = self
                .active_device
                .as_ref()
                .ok_or_else(|| "No device connected".to_string())?;
            Ok(QualifiedPort {
                device_name: device.clone(),
                port: port_str.to_string(),
            })
        }
    }

    /// Default initial power for a fresh port — 50% of the device's native max,
    /// so `on` without a prior `setpower` runs at half speed regardless of hub.
    fn default_power_for(&self, device_name: &str) -> u8 {
        self.devices
            .get(device_name)
            .map(|e| e.adapter.max_power() / 2)
            .unwrap_or(4)
    }

    fn get_state(&mut self, qp: &QualifiedPort) -> &mut OutputPortState {
        let default_power = self.default_power_for(&qp.device_name);
        let entry = self.devices.get_mut(&qp.device_name).unwrap();
        entry
            .port_states
            .entry(qp.port.clone())
            .or_insert(OutputPortState {
                direction: PortDirection::Even,
                power: default_power,
                is_running: false,
            })
    }

    fn sync_port(&mut self, qp: &QualifiedPort) -> Result<(), String> {
        let default_power = self.default_power_for(&qp.device_name);
        let entry = self
            .devices
            .get_mut(&qp.device_name)
            .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
        let state = entry
            .port_states
            .get(&qp.port)
            .cloned()
            .unwrap_or(OutputPortState {
                direction: PortDirection::Even,
                power: default_power,
                is_running: false,
            });

        let desired = SentState {
            direction: state.direction,
            power: state.power,
            is_running: state.is_running,
        };

        if let Some(last) = entry.last_sent.get(&qp.port) {
            if last.is_running == desired.is_running
                && last.direction == desired.direction
                && last.power == desired.power
            {
                return Ok(());
            }
        }

        if desired.is_running {
            entry.adapter.start_port(
                &qp.port,
                desired.direction,
                desired.power,
            )?;
        } else if entry
            .last_sent
            .get(&qp.port)
            .map(|l| l.is_running)
            .unwrap_or(false)
        {
            entry.adapter.stop_port(&qp.port)?;
        }

        entry.last_sent.insert(qp.port.clone(), desired);
        Ok(())
    }

    /// Group ports by device name, collecting their current state.
    fn group_by_device(
        &self,
        ports: &[QualifiedPort],
    ) -> Vec<(String, Vec<(String, PortDirection, u8)>)> {
        let mut groups: HashMap<String, Vec<(String, PortDirection, u8)>> = HashMap::new();
        for qp in ports {
            let default_power = self.default_power_for(&qp.device_name);
            let entry = self.devices.get(&qp.device_name).unwrap();
            let state = entry
                .port_states
                .get(&qp.port)
                .cloned()
                .unwrap_or(OutputPortState {
                    direction: PortDirection::Even,
                    power: default_power,
                    is_running: false,
                });
            groups.entry(qp.device_name.clone()).or_default().push((
                qp.port.clone(),
                state.direction,
                state.power,
            ));
        }
        groups.into_iter().collect()
    }

    /// Run a batch operation in parallel across devices.
    ///
    /// Each device gets its own scoped thread so blocking operations
    /// (`run_ports_for_time`, `rotate_ports_by_degrees`, etc.) on different
    /// devices fire simultaneously instead of serially. Waits for all
    /// threads before returning; if any returned an error, returns the
    /// first one.
    fn run_parallel_by_device<F>(
        &mut self,
        groups: Vec<(String, Vec<(String, PortDirection, u8)>)>,
        op: F,
    ) -> Result<(), String>
    where
        F: Fn(&mut dyn HardwareAdapter, &[PortCommand]) -> Result<(), String> + Sync,
    {
        let group_map: HashMap<String, Vec<(String, PortDirection, u8)>> =
            groups.into_iter().collect();

        // Collect &mut refs to each adapter in the batch, keyed by name.
        let mut work: Vec<(
            &mut Box<dyn HardwareAdapter>,
            &Vec<(String, PortDirection, u8)>,
        )> = Vec::new();
        for (name, entry) in self.devices.iter_mut() {
            if let Some(port_cmds) = group_map.get(name) {
                work.push((&mut entry.adapter, port_cmds));
            }
        }

        let results: Vec<Result<(), String>> = std::thread::scope(|s| {
            let handles: Vec<_> = work
                .into_iter()
                .map(|(adapter, port_cmds)| {
                    let op = &op;
                    s.spawn(move || {
                        let commands: Vec<PortCommand> = port_cmds
                            .iter()
                            .map(|(port, dir, power)| PortCommand {
                                port: port.as_str(),
                                direction: *dir,
                                power: *power,
                            })
                            .collect();
                        op(adapter.as_mut(), &commands)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err("adapter thread panicked".to_string()))
                })
                .collect()
        });

        results.into_iter().find(|r| r.is_err()).unwrap_or(Ok(()))
    }

    /// Batch start ports, grouped by device.
    fn batch_start(&mut self, ports: &[QualifiedPort]) -> Result<(), String> {
        let groups = self.group_by_device(ports);
        for (device_name, port_cmds) in groups {
            let entry = self.devices.get_mut(&device_name).unwrap();
            let commands: Vec<PortCommand> = port_cmds
                .iter()
                .map(|(port, dir, power)| PortCommand {
                    port: port.as_str(),
                    direction: *dir,
                    power: *power,
                })
                .collect();
            entry.adapter.start_ports(&commands)?;
        }
        Ok(())
    }

    /// Batch stop ports, grouped by device.
    fn batch_stop(&mut self, ports: &[QualifiedPort]) -> Result<(), String> {
        let groups = self.group_by_device(ports);
        for (device_name, port_cmds) in groups {
            let entry = self.devices.get_mut(&device_name).unwrap();
            let port_refs: Vec<&str> = port_cmds.iter().map(|(p, _, _)| p.as_str()).collect();
            entry.adapter.stop_ports(&port_refs)?;
        }
        Ok(())
    }

    fn cancel_flash(&mut self, qp: &QualifiedPort) {
        let key = format!("{}:{}", qp.device_name, qp.port);
        if let Some(flag) = self.flash_timers.remove(&key) {
            flag.store(true, Ordering::Relaxed);
            if let Some(entry) = self.devices.get_mut(&qp.device_name) {
                if entry.adapter.connected() {
                    let _ = entry.adapter.stop_port(&qp.port);
                }
            }
        }
    }

    // ── Port resolution helper ────────────────────

    fn resolve_ports(&self, port_strs: &[String]) -> Result<Vec<QualifiedPort>, String> {
        let mut resolved = Vec::new();
        for p in port_strs {
            resolved.push(self.resolve_port(p)?);
        }
        Ok(resolved)
    }

    /// Ensure port states exist for the given ports. Called after talkto.
    pub fn ensure_port_states(&mut self, port_strs: &[String]) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let default_power = self.default_power_for(&qp.device_name);
            let entry = self.devices.get_mut(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.port_states.entry(qp.port.clone()).or_insert(OutputPortState {
                direction: PortDirection::Even,
                power: default_power,
                is_running: false,
            });
        }
        Ok(())
    }

    // ── TC Logo port commands ─────────────────────

    pub fn set_even(&mut self, port_strs: &[String]) {
        if let Ok(ports) = self.resolve_ports(port_strs) {
            for qp in &ports {
                self.get_state(qp).direction = PortDirection::Even;
                let _ = self.sync_port(qp);
            }
        }
    }

    pub fn set_odd(&mut self, port_strs: &[String]) {
        if let Ok(ports) = self.resolve_ports(port_strs) {
            for qp in &ports {
                self.get_state(qp).direction = PortDirection::Odd;
                let _ = self.sync_port(qp);
            }
        }
    }

    pub fn reverse_direction(&mut self, port_strs: &[String]) {
        if let Ok(ports) = self.resolve_ports(port_strs) {
            for qp in &ports {
                let state = self.get_state(qp);
                state.direction = state.direction.toggle();
                let _ = self.sync_port(qp);
            }
        }
    }

    /// Set power on the resolved ports. The level must be in `0..=device.max_power()`
    /// for *every* device touched by the port selection — if any device's max
    /// is below the requested level, the call errors and no state changes.
    pub fn set_power(&mut self, port_strs: &[String], level: u8) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;

        // Validate against every selected device's max before mutating anything.
        let mut seen_devices: Vec<&str> = Vec::new();
        for qp in &ports {
            if seen_devices.contains(&qp.device_name.as_str()) {
                continue;
            }
            seen_devices.push(qp.device_name.as_str());
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            let max = entry.adapter.max_power();
            if level > max {
                return Err(format!(
                    "{} (\"{}\") supports power 0-{}, got {}",
                    entry.adapter.display_name(),
                    qp.device_name,
                    max,
                    level
                ));
            }
        }

        for qp in &ports {
            let state = self.get_state(qp);
            state.power = level;
            if level == 0 { state.is_running = false; }
            let _ = self.sync_port(qp);
        }
        Ok(())
    }

    pub fn on(&mut self, port_strs: &[String]) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        for qp in &ports {
            self.cancel_flash(qp);
            self.get_state(qp).is_running = true;
        }
        self.batch_start(&ports)?;
        Ok(())
    }

    pub fn off(&mut self, port_strs: &[String]) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        for qp in &ports {
            self.cancel_flash(qp);
            self.get_state(qp).is_running = false;
        }
        self.batch_stop(&ports)?;
        Ok(())
    }

    pub fn on_for(&mut self, port_strs: &[String], tenths: u32) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        for qp in &ports { self.cancel_flash(qp); }

        let groups = self.group_by_device(&ports);
        let result = self.run_parallel_by_device(groups, |adapter, commands| {
            adapter.run_ports_for_time(commands, tenths)
        });

        for qp in &ports { self.get_state(qp).is_running = false; }
        result
    }

    pub fn rotate(&mut self, port_strs: &[String], degrees: i32) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        for qp in &ports { self.cancel_flash(qp); }

        let groups = self.group_by_device(&ports);
        let result = self.run_parallel_by_device(groups, |adapter, commands| {
            adapter.rotate_ports_by_degrees(commands, degrees)
        });

        for qp in &ports { self.get_state(qp).is_running = false; }
        result
    }

    pub fn rotate_to(&mut self, port_strs: &[String], position: i32) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        for qp in &ports { self.cancel_flash(qp); }

        let groups = self.group_by_device(&ports);
        let result = self.run_parallel_by_device(groups, |adapter, commands| {
            adapter.rotate_ports_to_position(commands, position)
        });

        for qp in &ports { self.get_state(qp).is_running = false; }
        result
    }

    pub fn reset_zero(&mut self, port_strs: &[String]) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        for qp in &ports {
            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            entry.adapter.reset_port_zero(&qp.port)?;
        }
        Ok(())
    }

    pub fn rotate_to_home(&mut self, port_strs: &[String]) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        for qp in &ports { self.cancel_flash(qp); }

        let groups = self.group_by_device(&ports);
        let result = self.run_parallel_by_device(groups, |adapter, commands| {
            adapter.rotate_ports_to_home(commands)
        });

        for qp in &ports { self.get_state(qp).is_running = false; }
        result
    }

    pub fn flash(
        &mut self,
        port_strs: &[String],
        on_tenths: u32,
        off_tenths: u32,
        pm: Arc<Mutex<PortManager>>,
    ) -> Result<(), String> {
        let ports = self.resolve_ports(port_strs)?;
        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        for qp in &ports {
            self.cancel_flash(qp);
            let key = format!("{}:{}", qp.device_name, qp.port);
            let state = self.get_state(qp).clone();

            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            entry.adapter.start_port(&qp.port, state.direction, state.power)?;

            let cancelled = Arc::new(AtomicBool::new(false));
            self.flash_timers.insert(key.clone(), cancelled.clone());

            let pm = pm.clone();
            let cancelled_for_task = cancelled.clone();
            let device_name = qp.device_name.clone();
            let port = qp.port.clone();
            let on_ms = on_tenths as u64 * 100;
            let off_ms = off_tenths as u64 * 100;
            let mut is_on = true;
            let mut next_toggle =
                std::time::Instant::now() + std::time::Duration::from_millis(on_ms);

            // Periodic task — the scheduler thread (60 Hz) invokes this
            // every tick. We check whether it's time to toggle, and if so
            // issue the stop/start command. Returning `false` retires the
            // task; `true` keeps it scheduled.
            crate::scheduler::register_task(Box::new(move || -> bool {
                if cancelled_for_task.load(Ordering::Relaxed) { return false; }
                if std::time::Instant::now() < next_toggle { return true; }

                let Ok(mut pm) = pm.lock() else { return false; };
                let Some(entry) = pm.devices.get_mut(&device_name) else { return false; };
                if !entry.adapter.connected() { return false; }

                if is_on {
                    let _ = entry.adapter.stop_port(&port);
                    is_on = false;
                    next_toggle =
                        std::time::Instant::now() + std::time::Duration::from_millis(off_ms);
                } else {
                    // Re-read the port's current direction/power every cycle
                    // so a `setpower` issued mid-flash takes effect on the
                    // next on-phase.
                    let default_power = entry.adapter.max_power() / 2;
                    let state = entry.port_states.get(&port).cloned()
                        .unwrap_or(OutputPortState {
                            direction: PortDirection::Even,
                            power: default_power,
                            is_running: false,
                        });
                    let _ = entry.adapter.start_port(&port, state.direction, state.power);
                    is_on = true;
                    next_toggle =
                        std::time::Instant::now() + std::time::Duration::from_millis(on_ms);
                }
                true
            }));
        }
        Ok(())
    }

    pub fn all_off(&mut self) {
        for flag in self.flash_timers.values() {
            flag.store(true, Ordering::Relaxed);
        }
        self.flash_timers.clear();

        let names: Vec<String> = self.devices.keys().cloned().collect();
        for name in &names {
            let entry = self.devices.get_mut(name).unwrap();
            if !entry.adapter.connected() {
                continue;
            }
            let ports: Vec<String> = entry.port_states.keys().cloned().collect();
            for port in &ports {
                let state = entry.port_states.get_mut(port).unwrap();
                state.is_running = false;
                entry.last_sent.remove(port);
                let _ = entry.adapter.stop_port(port);
            }
        }
    }

    // ── Sensor commands ───────────────────────────

    pub fn read_sensor(&mut self, port_strs: &[String], mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        if port_strs.is_empty() {
            return Err("No sensor port selected (use listento)".to_string());
        }

        let ports = self.resolve_ports(port_strs)?;

        for qp in &ports {
            let entry = self.devices.get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            if !entry.adapter.connected() {
                return Err(format!("Device \"{}\" is not connected", qp.device_name));
            }
            entry.adapter.validate_sensor_port(&qp.port, mode)?;
        }

        if ports.len() == 1 {
            let qp = &ports[0];
            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            return entry.adapter.read_sensor(&qp.port, mode);
        }

        let mut results = Vec::new();
        for qp in &ports {
            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            let val = entry.adapter.read_sensor(&qp.port, mode)?;
            results.push(val.unwrap_or(LogoValue::Word("false".to_string())));
        }
        Ok(Some(LogoValue::List(results)))
    }

    // ── Firmware upload ─────────────────────────

    /// Prepare a device for firmware upload. Disconnects its driver slot
    /// and returns transport config (serial path or None for USB).
    pub fn prepare_firmware_upload(&mut self, name: &str) -> Result<Option<String>, String> {
        let entry = self.devices.get_mut(name)
            .ok_or_else(|| format!("No device named \"{}\"", name))?;
        entry.adapter.prepare_firmware_upload()
    }

    /// Reconnect a device after firmware upload.
    pub fn reconnect_after_firmware(&mut self, name: &str) -> Result<(), String> {
        let entry = self.devices.get_mut(name)
            .ok_or_else(|| format!("No device named \"{}\"", name))?;
        entry.adapter.reconnect_after_firmware()
    }
}

#[cfg(test)]
#[path = "tests/port_manager.rs"]
mod tests;
