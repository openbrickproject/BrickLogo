use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::Instant;
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::scheduler::{self, DeviceSlot};
use rust_cybermaster::constants::*;
use rust_cybermaster::protocol;

const OUTPUT_PORTS: &[&str] = &["a", "b", "c"];
const INPUT_PORTS: &[&str] = &["1", "2", "3"];

fn to_direction(direction: PortDirection) -> u8 {
    match direction {
        PortDirection::Even => DIR_FORWARD,
        PortDirection::Odd => DIR_REVERSE,
    }
}

/// CyberMaster native power range is 0..7 (3-bit PWM), same as the RCX.
const MAX_POWER: u8 = 7;

fn power_to_cm(power: u8) -> u8 {
    power.min(MAX_POWER)
}

// ── Transport ───────────────────────────────────

/// Transport for the CyberMaster RF tower (serial). A trait so tests can
/// inject a mock.
pub trait CmTransport: Send {
    fn send(&mut self, msg: &[u8]) -> Result<(), String>;
    fn request(&mut self, msg: &[u8]) -> Result<Vec<u8>, String>;
}

struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
}

impl CmTransport for SerialTransport {
    fn send(&mut self, msg: &[u8]) -> Result<(), String> {
        self.port.write_all(msg).map_err(|e| format!("Write failed: {}", e))?;
        self.port.flush().map_err(|e| format!("Flush failed: {}", e))?;
        Ok(())
    }

    fn request(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.port.write_all(msg).map_err(|e| format!("Write failed: {}", e))?;
        self.port.flush().map_err(|e| format!("Flush failed: {}", e))?;
        let deadline = Instant::now() + std::time::Duration::from_millis(COMMAND_TIMEOUT_MS);
        let mut buf = [0u8; 256];
        let mut response = Vec::new();
        let sent_len = msg.len();

        while Instant::now() < deadline {
            match self.port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    // The serial tower echoes what we send — skip past the echo.
                    if response.len() > sent_len {
                        if let Some(payload) = protocol::parse_reply(&response[sent_len..]) {
                            return Ok(payload);
                        }
                    }
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(format!("Read failed: {}", e)),
            }
        }
        Err("CyberMaster reply timed out".to_string())
    }
}

// ── Driver slot ─────────────────────────────────

type ReplyTx = Option<mpsc::Sender<Result<(), String>>>;

enum CmCommand {
    SetDirection { mask: u8, direction: u8, reply_tx: ReplyTx },
    SetPower { mask: u8, power: u8, reply_tx: ReplyTx },
    MotorOn { mask: u8, reply_tx: ReplyTx },
    MotorOff { mask: u8, reply_tx: ReplyTx },
    SetSensorMode { sensor: u8, mode: u8 },
    ReadSensor { sensor: u8, source: u8, reply_tx: mpsc::Sender<Result<i16, String>> },
}

struct CmSlot {
    transport: Box<dyn CmTransport>,
    rx: mpsc::Receiver<CmCommand>,
    alive: bool,
    // Toggle-bit state: alternated on every logical command so the brick never
    // mistakes a new command for a retransmission of the previous one.
    toggle: bool,
}

impl CmSlot {
    fn next_toggle(&mut self) -> bool {
        let t = self.toggle;
        self.toggle = !self.toggle;
        t
    }

    fn send_with_retry(&mut self, msg: &[u8]) -> Result<(), String> {
        let mut last_err = String::new();
        for _ in 0..COMMAND_RETRIES {
            match self.transport.request(msg) {
                Ok(_) => return Ok(()),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    fn reply(tx: ReplyTx, result: Result<(), String>) {
        if let Some(tx) = tx {
            let _ = tx.send(result);
        }
    }
}

impl DeviceSlot for CmSlot {
    fn tick(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                CmCommand::SetDirection { mask, direction, reply_tx } => {
                    let msg = protocol::cmd_set_direction(mask, direction, self.next_toggle());
                    let r = self.send_with_retry(&msg);
                    Self::reply(reply_tx, r);
                }
                CmCommand::SetPower { mask, power, reply_tx } => {
                    let msg = protocol::cmd_set_power(mask, power, self.next_toggle());
                    let r = self.send_with_retry(&msg);
                    Self::reply(reply_tx, r);
                }
                CmCommand::MotorOn { mask, reply_tx } => {
                    let msg = protocol::cmd_set_motor_state(mask, MOTOR_ON, self.next_toggle());
                    let r = self.send_with_retry(&msg);
                    Self::reply(reply_tx, r);
                }
                CmCommand::MotorOff { mask, reply_tx } => {
                    let msg = protocol::cmd_set_motor_state(mask, MOTOR_OFF, self.next_toggle());
                    let r = self.send_with_retry(&msg);
                    Self::reply(reply_tx, r);
                }
                CmCommand::SetSensorMode { sensor, mode } => {
                    let msg = protocol::cmd_set_sensor_mode(sensor, mode, self.next_toggle());
                    let _ = self.send_with_retry(&msg);
                }
                CmCommand::ReadSensor { sensor, source, reply_tx } => {
                    let msg = protocol::cmd_get_value(source, sensor, self.next_toggle());
                    let result = self.transport.request(&msg).and_then(|payload| {
                        protocol::reply_value(&payload)
                            .ok_or_else(|| "Invalid sensor reply".to_string())
                    });
                    let _ = reply_tx.send(result);
                }
            }
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

// ── Adapter ─────────────────────────────────────

pub struct CyberMasterAdapter {
    tx: Option<mpsc::Sender<CmCommand>>,
    slot_id: Option<usize>,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
    // Caches the sensor mode last set per input port, so a repeated read
    // doesn't re-send the mode-set command.
    sensor_modes: HashMap<usize, u8>,
    serial_path: String,
}

impl CyberMasterAdapter {
    pub fn new(serial_path: &str) -> Self {
        CyberMasterAdapter {
            tx: None,
            slot_id: None,
            display_name: "LEGO Technic CyberMaster".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            sensor_modes: HashMap::new(),
            serial_path: serial_path.to_string(),
        }
    }

    fn send_cmd(&self, cmd: CmCommand) -> Result<(), String> {
        self.tx.as_ref().ok_or("Not connected")?
            .send(cmd).map_err(|_| "Send failed".to_string())
    }

    fn send_cmd_wait(&self, cmd_fn: impl FnOnce(ReplyTx) -> CmCommand) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.send_cmd(cmd_fn(Some(tx)))?;
        rx.recv_timeout(std::time::Duration::from_millis(2000))
            .map_err(|_| "Command timed out".to_string())?
    }
}

impl HardwareAdapter for CyberMasterAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &self.input_ports }
    fn connected(&self) -> bool { self.tx.is_some() }

    fn connect(&mut self) -> Result<(), String> {
        let port = rust_cybermaster::serial::open_port(&self.serial_path)?;
        let mut transport: Box<dyn CmTransport> = Box::new(SerialTransport { port });

        // The CyberMaster requires an unlock before it accepts commands. The RF
        // brick must be powered and in range; the ack is best-effort here, the
        // same way the RCX adapter doesn't gate connect on a ping.
        let _ = transport.request(&protocol::cmd_unlock());

        let (tx, rx) = mpsc::channel();
        let slot = CmSlot {
            transport,
            rx,
            alive: true,
            toggle: false,
        };

        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.slot_id = Some(slot_id);
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(id) = self.slot_id.take() {
            scheduler::deregister_slot(id);
        }
        self.tx = None;
    }

    fn max_power(&self) -> u8 { MAX_POWER }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if motor_mask(port).is_some() { Ok(()) }
        else { Err(format!("Unknown output port \"{}\"", port)) }
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        match mode {
            // Rotation reads the motor-embedded tachometers, which live on the
            // two internal motor ports (a, b) — not the passive sensor inputs.
            Some("rotation") => {
                if tacho_index(port).is_some() {
                    Ok(())
                } else {
                    Err(format!(
                        "Port \"{}\" has no tachometer — rotation is available on motor ports a and b",
                        port
                    ))
                }
            }
            // Touch / raw read the passive analog sensor inputs (1-3).
            Some("touch") | Some("raw") | None => {
                if sensor_index(port).is_some() {
                    Ok(())
                } else {
                    Err(format!("Unknown sensor port \"{}\"", port))
                }
            }
            Some(m) => Err(format!("Unsupported sensor mode \"{}\" for CyberMaster", m)),
        }
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let mask = motor_mask(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        self.send_cmd_wait(|reply_tx| CmCommand::SetDirection { mask, direction: to_direction(direction), reply_tx })?;
        self.send_cmd_wait(|reply_tx| CmCommand::SetPower { mask, power: power_to_cm(power), reply_tx })?;
        self.send_cmd_wait(|reply_tx| CmCommand::MotorOn { mask, reply_tx })
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let mask = motor_mask(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        self.send_cmd_wait(|reply_tx| CmCommand::SetPower { mask, power: 0, reply_tx })?;
        self.send_cmd_wait(|reply_tx| CmCommand::MotorOff { mask, reply_tx })
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("CyberMaster does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("CyberMaster does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("CyberMaster does not support position reset".to_string())
    }

    fn rotate_to_abs(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("CyberMaster does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        // Rotation reads a motor's embedded tachometer: a signed cumulative
        // count (direction in the sign) polled directly from the tacho source —
        // no sensor-mode configuration, unlike the passive inputs.
        if mode == Some("rotation") {
            let idx = tacho_index(port).ok_or_else(|| {
                format!(
                    "Port \"{}\" has no tachometer — rotation is available on motor ports a and b",
                    port
                )
            })?;
            let (reply_tx, reply_rx) = mpsc::channel();
            self.send_cmd(CmCommand::ReadSensor { sensor: idx, source: SOURCE_TACHO_COUNT, reply_tx })?;
            let result = reply_rx.recv_timeout(std::time::Duration::from_secs(2))
                .map_err(|_| "Tachometer read timed out".to_string())??;
            return Ok(Some(LogoValue::Number(result as f64)));
        }

        let idx = sensor_index(port)
            .ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;

        // The CyberMaster reads its passive inputs as raw analog values; the
        // host thresholds for touch. Set raw mode once per port (cached).
        if self.sensor_modes.get(&(idx as usize)) != Some(&SENSOR_MODE_RAW) {
            self.send_cmd(CmCommand::SetSensorMode { sensor: idx, mode: SENSOR_MODE_RAW })?;
            self.sensor_modes.insert(idx as usize, SENSOR_MODE_RAW);
            // Give the brick a moment to sample the input.
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_cmd(CmCommand::ReadSensor { sensor: idx, source: SOURCE_INPUT_VALUE, reply_tx })?;
        let result = reply_rx.recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|_| "Sensor read timed out".to_string())??;

        match mode {
            Some("raw") => Ok(Some(LogoValue::Number(result as f64))),
            // "touch" (and the default) — low value = pressed, high = released.
            _ => {
                let pressed = result < TOUCH_PRESSED_BELOW;
                Ok(Some(LogoValue::Word(if pressed { "true" } else { "false" }.to_string())))
            }
        }
    }

    // ── Batch overrides (combine masks into single frames) ───

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        let mut combined_mask: u8 = 0;
        for cmd in commands {
            let mask = motor_mask(cmd.port)
                .ok_or_else(|| format!("Unknown port \"{}\"", cmd.port))?;
            combined_mask |= mask;
            let dir = to_direction(cmd.direction);
            self.send_cmd_wait(|reply_tx| CmCommand::SetDirection { mask, direction: dir, reply_tx })?;
        }

        let powers: Vec<u8> = commands.iter().map(|c| power_to_cm(c.power)).collect();
        if powers.windows(2).all(|w| w[0] == w[1]) {
            let p = powers[0];
            let m = combined_mask;
            self.send_cmd_wait(|reply_tx| CmCommand::SetPower { mask: m, power: p, reply_tx })?;
        } else {
            for cmd in commands {
                let mask = motor_mask(cmd.port).unwrap();
                let p = power_to_cm(cmd.power);
                self.send_cmd_wait(|reply_tx| CmCommand::SetPower { mask, power: p, reply_tx })?;
            }
        }

        let m = combined_mask;
        self.send_cmd_wait(|reply_tx| CmCommand::MotorOn { mask: m, reply_tx })
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        let mut combined_mask: u8 = 0;
        for port in ports {
            let mask = motor_mask(port)
                .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
            combined_mask |= mask;
        }
        let m = combined_mask;
        self.send_cmd_wait(|reply_tx| CmCommand::SetPower { mask: m, power: 0, reply_tx })?;
        let m = combined_mask;
        self.send_cmd_wait(|reply_tx| CmCommand::MotorOff { mask: m, reply_tx })
    }

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        self.start_ports(commands)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        let ports: Vec<&str> = commands.iter().map(|c| c.port).collect();
        self.stop_ports(&ports)
    }
}

#[cfg(test)]
#[path = "../tests/cybermaster_adapter.rs"]
mod tests;
