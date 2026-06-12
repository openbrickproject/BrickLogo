use std::sync::Arc;
use std::time::{Duration, Instant};
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::shared_brick_interface::SharedBrickInterface;

const OUTPUT_PORTS: &[&str] = &[
    "red1", "blue1", "red2", "blue2", "red3", "blue3", "red4", "blue4",
];
const MAX_POWER: u8 = 7;

// How long to hold the shared device lock per burst-wait slice. Interface A
// traffic on the same BrickInterface gets the lock between slices, so this
// bounds how long a sensor poll can stall behind a PF send.
const PF_WAIT_SLICE_MS: u64 = 20;
// Overall cap on waiting for the previous burst (one burst is ~0.6 s max).
const PF_WAIT_TOTAL_MS: u64 = 2000;

fn parse_port(port: &str) -> Option<(u8, bool)> {
    let (is_blue, num_str) = if let Some(n) = port.strip_prefix("blue") {
        (true, n)
    } else if let Some(n) = port.strip_prefix("red") {
        (false, n)
    } else {
        return None;
    };
    let ch: u8 = num_str.parse().ok()?;
    if ch < 1 || ch > 4 {
        return None;
    }
    Some((ch - 1, is_blue))
}

fn port_index(channel: u8, is_blue: bool) -> usize {
    channel as usize * 2 + if is_blue { 1 } else { 0 }
}

fn to_step(direction: PortDirection, power: u8) -> u8 {
    if power == 0 {
        return 0;
    }
    let p = power.min(7);
    match direction {
        PortDirection::Even => p,
        PortDirection::Odd => 16 - p,
    }
}

pub struct PowerFunctionsAdapter {
    shared: Option<Arc<SharedBrickInterface>>,
    display_name: String,
    output_ports: Vec<String>,
    steps: [u8; 8],
}

impl PowerFunctionsAdapter {
    pub fn new_with_shared(shared: Arc<SharedBrickInterface>) -> Self {
        PowerFunctionsAdapter {
            shared: Some(shared),
            display_name: "LEGO Power Functions (BrickInterface)".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            steps: [0u8; 8],
        }
    }

    fn device_lock(&self) -> Result<std::sync::MutexGuard<'_, rust_brickinterface::BrickInterface>, String> {
        self.shared
            .as_ref()
            .ok_or_else(|| "Not connected".to_string())?
            .device
            .lock()
            .map_err(|_| "Device mutex poisoned".to_string())
    }

    /// Issue one PF send, waiting out any previous burst in short lock
    /// slices. Sequential semantics are unchanged (this call still blocks
    /// until the previous burst finishes and this one is accepted), but the
    /// shared device lock is released between slices so concurrent
    /// Interface A commands on the same BrickInterface keep flowing.
    fn send_pf(
        &self,
        send: impl Fn(&mut rust_brickinterface::BrickInterface) -> Result<(), String>,
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_millis(PF_WAIT_TOTAL_MS);
        loop {
            let mut dev = self.device_lock()?;
            if dev.pf_wait_idle(PF_WAIT_SLICE_MS)? {
                // Transmitter idle — pf_send's own wait is a no-op now, so
                // the lock is only held for the accept round-trip (~ms).
                return send(&mut dev);
            }
            drop(dev); // let Interface A (and other) callers in
            if Instant::now() >= deadline {
                return Err("Timed out waiting for IR transmission to complete".to_string());
            }
            // std's Mutex is unfair; without a pause we tend to re-acquire
            // immediately and starve the threads we just unblocked.
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn send_single_pwm(&self, channel: u8, output_b: bool, step: u8) -> Result<(), String> {
        self.send_pf(|dev| dev.pf_send_single_pwm(channel, output_b, step))
    }

    fn send_combo_pwm(&self, channel: u8, step_a: u8, step_b: u8) -> Result<(), String> {
        self.send_pf(|dev| dev.pf_send_combo_pwm(channel, step_a, step_b))
    }

    /// Apply new steps to a set of outputs. When both outputs of a channel
    /// are targeted, they get the hybrid sequence: one Combo PWM (both
    /// outputs change in the same IR message — motors start/stop together),
    /// then a latching Single PWM per *running* output, because combo state
    /// stops on the receiver's lost-IR timeout while single state survives
    /// going out of range. Stopped outputs need no latch — the timeout's
    /// effect (float) is the state they're already in — so a pair-off is a
    /// single combo burst. A lost latch degrades to staggered starts; lone
    /// outputs get a plain single. Channels are served in first-appearance
    /// order — authors who care about cross-channel ordering can order
    /// their port lists.
    fn apply_steps(&mut self, targets: &[(u8, bool, u8)]) -> Result<(), String> {
        let mut order: Vec<u8> = Vec::new();
        let mut wanted: [[Option<u8>; 2]; 4] = [[None; 2]; 4];
        for &(ch, is_blue, step) in targets {
            if !order.contains(&ch) {
                order.push(ch);
            }
            wanted[ch as usize][is_blue as usize] = Some(step);
            self.steps[port_index(ch, is_blue)] = step;
        }
        for ch in order {
            match wanted[ch as usize] {
                [Some(step_a), Some(step_b)] => {
                    self.send_combo_pwm(ch, step_a, step_b)?;
                    if step_a != 0 {
                        self.send_single_pwm(ch, false, step_a)?;
                    }
                    if step_b != 0 {
                        self.send_single_pwm(ch, true, step_b)?;
                    }
                }
                [Some(step_a), None] => self.send_single_pwm(ch, false, step_a)?,
                [None, Some(step_b)] => self.send_single_pwm(ch, true, step_b)?,
                [None, None] => {}
            }
        }
        Ok(())
    }
}

impl HardwareAdapter for PowerFunctionsAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { self.shared.is_some() }
    fn max_power(&self) -> u8 { MAX_POWER }

    fn connect(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(ref shared) = self.shared {
            if let Ok(mut dev) = shared.device.lock() {
                let _ = dev.ir_abort_all();
            }
            shared.release_powerfunctions();
        }
        self.shared = None;
        self.steps = [0u8; 8];
    }

    fn parallel_safe(&self) -> bool { false }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if parse_port(port).is_some() {
            Ok(())
        } else {
            Err(format!("Unknown output port \"{}\"", port))
        }
    }

    fn validate_sensor_port(&self, port: &str, _mode: Option<&str>) -> Result<(), String> {
        Err(format!("Power Functions has no sensor ports (got \"{}\")", port))
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let (ch, is_blue) = parse_port(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        let step = to_step(direction, power);
        self.apply_steps(&[(ch, is_blue, step)])
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let (ch, is_blue) = parse_port(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        self.apply_steps(&[(ch, is_blue, 0)])
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("Power Functions does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("Power Functions does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("Power Functions does not support position reset".to_string())
    }

    fn rotate_to_abs(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("Power Functions does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        Err(format!("Power Functions has no sensor ports (got \"{}\")", port))
    }

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        let mut targets = Vec::with_capacity(commands.len());
        for cmd in commands {
            let (ch, is_blue) =
                parse_port(cmd.port).ok_or_else(|| format!("Unknown port \"{}\"", cmd.port))?;
            targets.push((ch, is_blue, to_step(cmd.direction, cmd.power)));
        }
        self.apply_steps(&targets)
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        let mut targets = Vec::with_capacity(ports.len());
        for port in ports {
            let (ch, is_blue) =
                parse_port(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
            targets.push((ch, is_blue, 0));
        }
        self.apply_steps(&targets)
    }
}

#[cfg(test)]
#[path = "../tests/powerfunctions_adapter.rs"]
mod tests;
