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
    device_order: Vec<String>,
    active_device: Option<String>,
    selected_outputs: Vec<QualifiedPort>,
    selected_inputs: Vec<QualifiedPort>,
    flash_timers: HashMap<String, Arc<AtomicBool>>,
}

fn power_to_percent(level: u8) -> u8 {
    ((level.min(8) as u16 * 100) / 8) as u8
}

impl PortManager {
    pub fn new() -> Self {
        PortManager {
            devices: HashMap::new(),
            device_order: Vec::new(),
            active_device: None,
            selected_outputs: Vec::new(),
            selected_inputs: Vec::new(),
            flash_timers: HashMap::new(),
        }
    }

    pub fn add_device(&mut self, name: &str, adapter: Box<dyn HardwareAdapter>) {
        let mut port_states = HashMap::new();
        for port in adapter.output_ports() {
            port_states.insert(
                port.clone(),
                OutputPortState {
                    direction: PortDirection::Even,
                    power: 4,
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
        self.selected_outputs.retain(|p| p.device_name != name);
        self.selected_inputs.retain(|p| p.device_name != name);
        if self.active_device.as_deref() == Some(name) {
            self.active_device = self
                .device_order
                .iter()
                .find(|device_name| self.devices.contains_key(device_name.as_str()))
                .cloned();
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
        self.selected_outputs.clear();
        self.selected_inputs.clear();
    }

    pub fn set_active_device(&mut self, name: &str) -> Result<(), String> {
        if !self.devices.contains_key(name) {
            return Err(format!("No device named \"{}\"", name));
        }
        self.active_device = Some(name.to_string());
        Ok(())
    }

    pub fn get_active_device_name(&self) -> Option<&str> {
        self.active_device.as_deref()
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

    pub fn get_selected_output_display_ports(&self) -> Vec<String> {
        self.format_selected_ports(&self.selected_outputs)
    }

    pub fn get_selected_input_display_ports(&self) -> Vec<String> {
        self.format_selected_ports(&self.selected_inputs)
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

    fn format_selected_ports(&self, ports: &[QualifiedPort]) -> Vec<String> {
        let active = self.active_device.as_deref();
        ports
            .iter()
            .map(|qp| {
                if active == Some(qp.device_name.as_str()) {
                    qp.port.clone()
                } else {
                    format!("{}.{}", qp.device_name, qp.port)
                }
            })
            .collect()
    }

    fn get_state(&mut self, qp: &QualifiedPort) -> &mut OutputPortState {
        let entry = self.devices.get_mut(&qp.device_name).unwrap();
        entry
            .port_states
            .entry(qp.port.clone())
            .or_insert(OutputPortState {
                direction: PortDirection::Even,
                power: 4,
                is_running: false,
            })
    }

    fn sync_port(&mut self, qp: &QualifiedPort) -> Result<(), String> {
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
                power: 4,
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
                power_to_percent(desired.power),
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
            let entry = self.devices.get(&qp.device_name).unwrap();
            let state = entry
                .port_states
                .get(&qp.port)
                .cloned()
                .unwrap_or(OutputPortState {
                    direction: PortDirection::Even,
                    power: 4,
                    is_running: false,
                });
            groups.entry(qp.device_name.clone()).or_default().push((
                qp.port.clone(),
                state.direction,
                power_to_percent(state.power),
            ));
        }
        groups.into_iter().collect()
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

    // ── TC Logo port commands ─────────────────────

    pub fn talk_to(&mut self, ports: &[String]) -> Result<(), String> {
        let mut resolved = Vec::new();
        for p in ports {
            resolved.push(self.resolve_port(p)?);
        }
        self.selected_outputs = resolved;
        // Ensure port states exist
        for qp in &self.selected_outputs {
            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            entry
                .port_states
                .entry(qp.port.clone())
                .or_insert(OutputPortState {
                    direction: PortDirection::Even,
                    power: 4,
                    is_running: false,
                });
        }
        Ok(())
    }

    pub fn listen_to(&mut self, ports: &[String]) -> Result<(), String> {
        let mut resolved = Vec::new();
        for p in ports {
            resolved.push(self.resolve_port(p)?);
        }
        self.selected_inputs = resolved;
        Ok(())
    }

    pub fn set_even(&mut self) {
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.get_state(qp).direction = PortDirection::Even;
            let _ = self.sync_port(qp);
        }
    }

    pub fn set_odd(&mut self) {
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.get_state(qp).direction = PortDirection::Odd;
            let _ = self.sync_port(qp);
        }
    }

    pub fn reverse_direction(&mut self) {
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            let state = self.get_state(qp);
            state.direction = state.direction.toggle();
            let _ = self.sync_port(qp);
        }
    }

    pub fn set_power(&mut self, level: u8) {
        let clamped = level.min(8);
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            let state = self.get_state(qp);
            state.power = clamped;
            if clamped == 0 {
                state.is_running = false;
            }
            let _ = self.sync_port(qp);
        }
    }

    pub fn on(&mut self) -> Result<(), String> {
        // Validate all first
        for qp in &self.selected_outputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.cancel_flash(qp);
            self.get_state(qp).is_running = true;
        }
        // Batch start per device
        self.batch_start(&ports)?;
        Ok(())
    }

    pub fn off(&mut self) -> Result<(), String> {
        for qp in &self.selected_outputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.cancel_flash(qp);
            self.get_state(qp).is_running = false;
        }
        // Batch stop per device
        self.batch_stop(&ports)?;
        Ok(())
    }

    pub fn on_for(&mut self, tenths: u32) -> Result<(), String> {
        // Validate all first
        for qp in &self.selected_outputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.cancel_flash(qp);
        }

        // Group by device and call batch method
        let groups = self.group_by_device(&ports);
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
            entry.adapter.run_ports_for_time(&commands, tenths)?;
        }

        for qp in &ports {
            self.get_state(qp).is_running = false;
        }
        Ok(())
    }

    pub fn rotate(&mut self, degrees: i32) -> Result<(), String> {
        for qp in &self.selected_outputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.cancel_flash(qp);
        }

        let groups = self.group_by_device(&ports);
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
            entry.adapter.rotate_ports_by_degrees(&commands, degrees)?;
        }
        Ok(())
    }

    pub fn rotate_to(&mut self, position: i32) -> Result<(), String> {
        for qp in &self.selected_outputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.cancel_flash(qp);
        }

        let groups = self.group_by_device(&ports);
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
            entry
                .adapter
                .rotate_ports_to_position(&commands, position)?;
        }
        Ok(())
    }

    pub fn reset_zero(&mut self) -> Result<(), String> {
        for qp in &self.selected_outputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            entry.adapter.reset_port_zero(&qp.port)?;
        }
        Ok(())
    }

    pub fn rotate_to_home(&mut self) -> Result<(), String> {
        for qp in &self.selected_outputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.cancel_flash(qp);
        }

        let groups = self.group_by_device(&ports);
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
            entry.adapter.rotate_ports_to_home(&commands)?;
        }
        Ok(())
    }

    pub fn flash(
        &mut self,
        on_tenths: u32,
        off_tenths: u32,
        pm: Arc<Mutex<PortManager>>,
    ) -> Result<(), String> {
        for qp in &self.selected_outputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            entry.adapter.validate_output_port(&qp.port)?;
        }
        let ports: Vec<QualifiedPort> = self.selected_outputs.clone();
        for qp in &ports {
            self.cancel_flash(qp);
            let key = format!("{}:{}", qp.device_name, qp.port);
            let state = self.get_state(qp).clone();

            // Start the port immediately
            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            entry
                .adapter
                .start_port(&qp.port, state.direction, power_to_percent(state.power))?;

            // Spawn cycling thread
            let cancelled = Arc::new(AtomicBool::new(false));
            self.flash_timers.insert(key.clone(), cancelled.clone());

            let pm = pm.clone();
            let device_name = qp.device_name.clone();
            let port = qp.port.clone();
            let on_ms = on_tenths as u64 * 100;
            let off_ms = off_tenths as u64 * 100;

            std::thread::spawn(move || {
                let mut is_on = true;
                loop {
                    let delay = if is_on { on_ms } else { off_ms };
                    std::thread::sleep(std::time::Duration::from_millis(delay));

                    if cancelled.load(Ordering::Relaxed) {
                        return;
                    }

                    let Ok(mut pm) = pm.lock() else {
                        return;
                    };
                    let Some(entry) = pm.devices.get_mut(&device_name) else {
                        return;
                    };
                    if !entry.adapter.connected() {
                        return;
                    }

                    if is_on {
                        let _ = entry.adapter.stop_port(&port);
                    } else {
                        let state =
                            entry
                                .port_states
                                .get(&port)
                                .cloned()
                                .unwrap_or(OutputPortState {
                                    direction: PortDirection::Even,
                                    power: 4,
                                    is_running: false,
                                });
                        let _ = entry.adapter.start_port(
                            &port,
                            state.direction,
                            power_to_percent(state.power),
                        );
                    }
                    is_on = !is_on;
                }
            });
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
                state.direction = PortDirection::Even;
                state.power = 8;
                entry.last_sent.remove(port);
                let _ = entry.adapter.stop_port(port);
            }
        }
    }

    // ── Sensor commands ───────────────────────────

    pub fn read_sensor(&mut self, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        if self.selected_inputs.is_empty() {
            return Err("No sensor port selected (use listento)".to_string());
        }

        // Validate all first
        for qp in &self.selected_inputs.clone() {
            let entry = self
                .devices
                .get(&qp.device_name)
                .ok_or_else(|| format!("No device named \"{}\"", qp.device_name))?;
            if !entry.adapter.connected() {
                return Err(format!("Device \"{}\" is not connected", qp.device_name));
            }
            entry.adapter.validate_sensor_port(&qp.port, mode)?;
        }

        // Single port: return value directly
        if self.selected_inputs.len() == 1 {
            let qp = self.selected_inputs[0].clone();
            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            return entry.adapter.read_sensor(&qp.port, mode);
        }

        // Multiple ports: return list
        let ports = self.selected_inputs.clone();
        let mut results = Vec::new();
        for qp in &ports {
            let entry = self.devices.get_mut(&qp.device_name).unwrap();
            let val = entry.adapter.read_sensor(&qp.port, mode)?;
            results.push(val.unwrap_or(LogoValue::Word("false".to_string())));
        }
        Ok(Some(LogoValue::List(results)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAdapter {
        ports: Vec<String>,
        connected: bool,
        start_calls: Vec<(String, PortDirection, u8)>,
        stop_calls: Vec<String>,
    }

    impl MockAdapter {
        fn new(ports: &[&str]) -> Self {
            MockAdapter {
                ports: ports.iter().map(|s| s.to_string()).collect(),
                connected: true,
                start_calls: Vec::new(),
                stop_calls: Vec::new(),
            }
        }
    }

    impl HardwareAdapter for MockAdapter {
        fn display_name(&self) -> &str {
            "Mock"
        }
        fn output_ports(&self) -> &[String] {
            &self.ports
        }
        fn input_ports(&self) -> &[String] {
            &[]
        }
        fn connected(&self) -> bool {
            self.connected
        }
        fn connect(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn disconnect(&mut self) {
            self.connected = false;
        }
        fn validate_output_port(&self, _port: &str) -> Result<(), String> {
            Ok(())
        }
        fn validate_sensor_port(&self, _port: &str, _mode: Option<&str>) -> Result<(), String> {
            Ok(())
        }
        fn start_port(&mut self, port: &str, dir: PortDirection, power: u8) -> Result<(), String> {
            self.start_calls.push((port.to_string(), dir, power));
            Ok(())
        }
        fn stop_port(&mut self, port: &str) -> Result<(), String> {
            self.stop_calls.push(port.to_string());
            Ok(())
        }
        fn run_port_for_time(
            &mut self,
            _port: &str,
            _dir: PortDirection,
            _power: u8,
            _tenths: u32,
        ) -> Result<(), String> {
            Ok(())
        }
        fn rotate_port_by_degrees(
            &mut self,
            _port: &str,
            _dir: PortDirection,
            _power: u8,
            _degrees: i32,
        ) -> Result<(), String> {
            Ok(())
        }
        fn rotate_port_to_position(
            &mut self,
            _port: &str,
            _dir: PortDirection,
            _power: u8,
            _pos: i32,
        ) -> Result<(), String> {
            Ok(())
        }
        fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
            Ok(())
        }
        fn rotate_to_home(
            &mut self,
            _port: &str,
            _dir: PortDirection,
            _power: u8,
        ) -> Result<(), String> {
            Ok(())
        }
        fn read_sensor(
            &mut self,
            _port: &str,
            _mode: Option<&str>,
        ) -> Result<Option<LogoValue>, String> {
            Ok(None)
        }
    }

    #[test]
    fn test_first_device_becomes_active() {
        let mut pm = PortManager::new();
        pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
        assert_eq!(pm.get_active_device_name(), Some("bot"));
    }

    #[test]
    fn test_second_device_not_active() {
        let mut pm = PortManager::new();
        pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
        pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])));
        assert_eq!(pm.get_active_device_name(), Some("bot1"));
    }

    #[test]
    fn test_use_switches_active() {
        let mut pm = PortManager::new();
        pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
        pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])));
        pm.set_active_device("bot2").unwrap();
        assert_eq!(pm.get_active_device_name(), Some("bot2"));
    }

    #[test]
    fn test_remove_device_fallback() {
        let mut pm = PortManager::new();
        pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
        pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])));
        pm.remove_device("bot1");
        assert!(pm.get_active_device_name().is_some());
    }

    #[test]
    fn test_talk_to() {
        let mut pm = PortManager::new();
        pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
        pm.talk_to(&["a".to_string()]).unwrap();
    }

    #[test]
    fn test_talk_to_qualified() {
        let mut pm = PortManager::new();
        pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
        pm.talk_to(&["bot.a".to_string()]).unwrap();
    }

    #[test]
    fn test_on_off() {
        let mut pm = PortManager::new();
        pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
        pm.talk_to(&["a".to_string(), "b".to_string()]).unwrap();
        pm.on().unwrap();
        pm.off().unwrap();
    }

    #[test]
    fn test_set_power() {
        let mut pm = PortManager::new();
        pm.add_device("bot", Box::new(MockAdapter::new(&["a"])));
        pm.talk_to(&["a".to_string()]).unwrap();
        pm.set_power(8);
        pm.on().unwrap();
    }

    #[test]
    fn test_all_off() {
        let mut pm = PortManager::new();
        pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
        pm.talk_to(&["a".to_string(), "b".to_string()]).unwrap();
        pm.on().unwrap();
        pm.all_off();
    }

    #[test]
    fn test_read_sensor_no_port() {
        let mut pm = PortManager::new();
        pm.add_device("bot", Box::new(MockAdapter::new(&["a"])));
        assert!(pm.read_sensor(None).is_err());
    }

    #[test]
    fn test_remove_all() {
        let mut pm = PortManager::new();
        pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
        pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])));
        pm.remove_all();
        assert!(pm.get_active_device_name().is_none());
        assert!(pm.get_connected_device_names().is_empty());
    }
}
